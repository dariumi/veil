use anyhow::Result;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

use crate::transport::VeilConnection;

/// Attach an already-established VeilConnection to a SOCKS5 listener.
/// Used by Android VPN mode: the connection is created separately, then
/// we expose it as a local SOCKS5 server that tun2proxy routes into.
pub async fn run_server(
    bind_addr: &str,
    conn: std::sync::Arc<VeilConnection>,
) -> Result<()> {
    let socks5_addr: SocketAddr = bind_addr.parse()?;
    info!(socks5 = %socks5_addr, "SOCKS5 server started");
    run_socks5(socks5_addr, conn).await
}

/// Run local SOCKS5 proxy + HTTP CONNECT proxy
pub async fn run(server: &str, token: &str, profile: &str) -> Result<()> {
    let socks5_addr: SocketAddr = "127.0.0.1:1080".parse()?;
    let http_addr: SocketAddr = "127.0.0.1:8080".parse()?;

    let conn = VeilConnection::connect(server, token, profile).await?;
    let conn = std::sync::Arc::new(conn);

    info!(socks5 = %socks5_addr, http = %http_addr, "Local proxy started");
    println!("SOCKS5 proxy: {}", socks5_addr);
    println!("HTTP proxy:   {}", http_addr);
    println!("Press Ctrl+C to disconnect");

    tokio::select! {
        res = run_socks5(socks5_addr, conn.clone()) => res?,
        res = run_http_proxy(http_addr, conn) => res?,
        _ = tokio::signal::ctrl_c() => {
            println!("\nDisconnected.");
        }
    }

    Ok(())
}

async fn run_socks5(addr: SocketAddr, conn: std::sync::Arc<VeilConnection>) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, client_addr) = listener.accept().await?;
        let conn = conn.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_socks5(stream, client_addr, conn).await {
                debug!(client = %client_addr, err = %e, "SOCKS5 error");
            }
        });
    }
}

/// SOCKS5 handshake + relay
async fn handle_socks5(
    mut stream: TcpStream,
    client_addr: SocketAddr,
    conn: std::sync::Arc<VeilConnection>,
) -> Result<()> {
    let mut buf = [0u8; 512];

    // SOCKS5 greeting: VER(1) NMETHODS(1) METHODS(N)
    let n = stream.read(&mut buf).await?;
    if n < 3 || buf[0] != 5 {
        return Ok(());
    }

    // Reply: no auth required
    stream.write_all(&[5, 0]).await?;

    // Read request: VER CMD RSV ATYP DST_ADDR DST_PORT
    let n = stream.read(&mut buf).await?;
    if n < 7 || buf[0] != 5 || buf[1] != 1 {
        return Ok(());
    }

    let dest = parse_socks5_dest(&buf[..n])?;
    debug!(client = %client_addr, dest = %dest, "SOCKS5 connect");

    // Tell client: success
    stream.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;

    // Open relay stream through Veil connection
    conn.relay_tcp(dest, stream).await?;

    Ok(())
}

fn parse_socks5_dest(buf: &[u8]) -> Result<String> {
    // buf[3] = ATYP: 0x01=IPv4, 0x03=domain, 0x04=IPv6
    match buf[3] {
        0x01 => {
            // IPv4
            if buf.len() < 10 {
                anyhow::bail!("Short IPv4 request");
            }
            let ip = format!("{}.{}.{}.{}", buf[4], buf[5], buf[6], buf[7]);
            let port = u16::from_be_bytes([buf[8], buf[9]]);
            Ok(format!("{}:{}", ip, port))
        }
        0x03 => {
            // Domain
            let len = buf[4] as usize;
            if buf.len() < 5 + len + 2 {
                anyhow::bail!("Short domain request");
            }
            let domain = std::str::from_utf8(&buf[5..5 + len])?;
            let port = u16::from_be_bytes([buf[5 + len], buf[5 + len + 1]]);
            Ok(format!("{}:{}", domain, port))
        }
        _ => anyhow::bail!("Unsupported SOCKS5 address type"),
    }
}

async fn run_http_proxy(addr: SocketAddr, conn: std::sync::Arc<VeilConnection>) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, client_addr) = listener.accept().await?;
        let conn = conn.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_http_connect(stream, conn).await {
                debug!(client = %client_addr, err = %e, "HTTP CONNECT error");
            }
        });
    }
}

/// HTTP CONNECT proxy handler
async fn handle_http_connect(
    mut stream: TcpStream,
    conn: std::sync::Arc<VeilConnection>,
) -> Result<()> {
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = std::str::from_utf8(&buf[..n])?;

    // Parse CONNECT host:port HTTP/1.1
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.splitn(3, ' ').collect();
    if parts.len() < 2 || parts[0] != "CONNECT" {
        stream
            .write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n")
            .await?;
        return Ok(());
    }

    let dest = parts[1].to_string();
    stream
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;

    conn.relay_tcp(dest, stream).await?;
    Ok(())
}
