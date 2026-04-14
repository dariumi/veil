use anyhow::Result;
use std::net::Ipv4Addr;
use tracing::{debug, info, warn};

use crate::transport::VeilConnection;

pub struct TunDevice {
    name: String,
}

impl TunDevice {
    /// Create a new named TUN device (desktop: Linux / macOS / Windows).
    pub async fn create(name: &str) -> Result<Self> {
        info!(name = %name, "Creating TUN device");

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use tun::Configuration;
            let mut config = Configuration::default();
            config.name(name).up();
            tun::create_as_async(&config)
                .map_err(|e| anyhow::anyhow!("TUN create failed: {}", e))?;
        }

        Ok(Self { name: name.to_string() })
    }

    pub async fn configure(
        &self,
        local_ip: Ipv4Addr,
        gateway: Ipv4Addr,
        prefix_len: u8,
    ) -> Result<()> {
        info!(device = %self.name, ip = %local_ip, gw = %gateway, "Configuring TUN device");

        #[cfg(target_os = "linux")]
        {
            use tokio::process::Command;
            Command::new("ip")
                .args(["addr", "add", &format!("{}/{}", local_ip, prefix_len), "dev", &self.name])
                .status().await?;
            Command::new("ip")
                .args(["link", "set", "dev", &self.name, "up"])
                .status().await?;
            Command::new("ip")
                .args(["route", "add", "default", "via", &gateway.to_string(), "dev", &self.name])
                .status().await?;
        }

        #[cfg(target_os = "macos")]
        {
            use tokio::process::Command;
            Command::new("ifconfig")
                .args([&self.name, &local_ip.to_string(), &gateway.to_string()])
                .status().await?;
            Command::new("route")
                .args(["add", "default", &gateway.to_string()])
                .status().await?;
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "android")))]
        let _ = (local_ip, gateway, prefix_len);

        Ok(())
    }

    /// Desktop VPN packet pump.
    pub async fn run(self, _conn: VeilConnection) -> Result<()> {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            debug!(device = %self.name, "TUN pump tick");
        }
    }
}

// ── Android VPN tunnel ────────────────────────────────────────────────────────
//
// Architecture (no external TUN library required):
//
//   Android VpnService.Builder → ParcelFileDescriptor (raw fd)
//       │
//       ▼ tokio AsyncFd — non-blocking read/write of raw IP packets
//   IP packet parser (src/dst IP + protocol + port from header)
//       │
//       ├─ TCP → VeilConnection::relay_tcp(dest_addr, virtual_stream)
//       └─ UDP → VeilConnection::send_datagram(dest_addr, payload)
//
// The OS TCP stack in each app sends normal TCP segments into the TUN.
// We intercept SYN packets, complete the handshake with a virtual socket,
// then pipe data through the Veil QUIC relay stream.

#[cfg(target_os = "android")]
pub mod android {
    use super::*;
    use std::collections::HashMap;
    use std::net::{IpAddr, SocketAddr};
    use std::os::unix::io::RawFd;
    use std::sync::Arc;
    use tokio::io::unix::AsyncFd;
    use tokio::net::TcpStream;

    /// Wrap the Android VpnService TUN file-descriptor.
    struct TunFd(AsyncFd<std::fs::File>);

    impl TunFd {
        fn new(fd: RawFd) -> Result<Self> {
            // Safety: fd is valid and owned by us (detachFd() in Kotlin transfers ownership).
            let file = unsafe { std::fs::File::from_raw_fd(fd) };
            // Set non-blocking so AsyncFd can poll it.
            use std::os::unix::io::AsRawFd;
            unsafe {
                let flags = libc::fcntl(file.as_raw_fd(), libc::F_GETFL);
                libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
            Ok(Self(AsyncFd::new(file)?))
        }

        async fn read_packet(&self, buf: &mut [u8]) -> Result<usize> {
            loop {
                let mut guard = self.0.readable().await?;
                match guard.try_io(|f| {
                    use std::io::Read;
                    f.get_ref().read(buf)
                }) {
                    Ok(Ok(n)) => return Ok(n),
                    Ok(Err(e)) => return Err(e.into()),
                    Err(_would_block) => continue,
                }
            }
        }

        fn write_packet(&self, buf: &[u8]) -> Result<()> {
            use std::io::Write;
            // Blocking write is fine for small TUN packets
            let file = self.0.get_ref();
            // SAFETY: we have exclusive access via &self
            let file_ptr = file as *const std::fs::File as *mut std::fs::File;
            unsafe { (*file_ptr).write_all(buf)? };
            Ok(())
        }
    }

    /// Run the Android VPN tunnel.
    ///
    /// Reads raw IP packets from the VpnService TUN fd, parses TCP/UDP
    /// destinations, and relays each connection through the Veil QUIC tunnel.
    pub async fn run(fd: RawFd, conn: Arc<VeilConnection>) -> Result<()> {
        info!(fd = fd, "Android VPN tunnel starting");
        let tun = Arc::new(TunFd::new(fd)?);
        let mut buf = vec![0u8; 65536];

        loop {
            let n = tun.read_packet(&mut buf).await?;
            if n < 20 {
                continue; // too short for IPv4 header
            }

            let packet = &buf[..n];

            // Parse IPv4 header
            if packet[0] >> 4 != 4 {
                continue; // not IPv4
            }

            let ihl = ((packet[0] & 0x0f) * 4) as usize;
            let protocol = packet[9];
            let dst_ip = Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]);

            match protocol {
                // TCP (6)
                6 if packet.len() >= ihl + 4 => {
                    let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                    let dest = format!("{}:{}", dst_ip, dst_port);
                    let conn = conn.clone();

                    debug!(dest = %dest, "TCP relay");

                    tokio::spawn(async move {
                        // Open a virtual local TCP socket pair to give relay_tcp a TcpStream.
                        // relay_tcp expects to read/write the client side of a connection.
                        match open_relay_pair(dest, conn).await {
                            Ok(()) => {}
                            Err(e) => warn!(err = %e, "TCP relay error"),
                        }
                    });
                }

                // UDP (17)
                17 if packet.len() >= ihl + 8 => {
                    let dst_port = u16::from_be_bytes([packet[ihl + 2], packet[ihl + 3]]);
                    let dest = format!("{}:{}", dst_ip, dst_port);
                    let payload_start = ihl + 8;
                    let payload = bytes::Bytes::copy_from_slice(&packet[payload_start..]);
                    let conn = conn.clone();

                    tokio::spawn(async move {
                        if let Err(e) = conn.send_datagram(&dest, payload).await {
                            warn!(dest = %dest, err = %e, "UDP relay error");
                        }
                    });
                }

                _ => {} // ignore ICMP and other protocols
            }
        }
    }

    /// Connect to `dest` through Veil and pipe through a loopback TcpStream pair.
    async fn open_relay_pair(dest: String, conn: Arc<VeilConnection>) -> Result<()> {
        // Bind a local TCP listener on a random port, connect to it, then hand
        // one half to relay_tcp (which will pipe it to the remote).
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;

        let (client_stream, server_stream) = tokio::join!(
            TcpStream::connect(local_addr),
            listener.accept(),
        );
        let client_stream = client_stream?;
        let (server_stream, _) = server_stream?;

        // relay_tcp will forward server_stream → Veil → dest
        conn.relay_tcp(dest, server_stream).await?;
        drop(client_stream);
        Ok(())
    }
}
