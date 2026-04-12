use anyhow::Result;
use bytes::Bytes;
use quinn::{ClientConfig as QuinnClientConfig, Endpoint};
use rustls::ClientConfig as RustlsClientConfig;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

use veil_core::protocol::handshake::{AuthMethod, AuthRequest};
use veil_core::protocol::SessionToken;

/// Established connection to a Veil server (QUIC or TLS/TCP fallback)
pub struct VeilConnection {
    inner: ConnectionInner,
    session_id: String,
}

enum ConnectionInner {
    Quic(quinn::Connection),
    // TcpTls(tokio_rustls::client::TlsStream<TcpStream>),
}

impl VeilConnection {
    /// Connect to a Veil server, trying QUIC first then falling back to TCP/TLS
    pub async fn connect(server: &str, token: &str, profile: &str) -> Result<Self> {
        info!(server = %server, "Connecting to Veil server");

        // Try QUIC first
        match connect_quic(server, token, profile).await {
            Ok(conn) => {
                info!("Connected via QUIC/HTTP3");
                return Ok(conn);
            }
            Err(e) => {
                warn!(err = %e, "QUIC connection failed, trying TCP/TLS fallback");
            }
        }

        // TCP/TLS fallback
        connect_tcp(server, token, profile).await
    }

    /// Open a new bidirectional relay stream for a TCP destination
    pub async fn relay_tcp(&self, dest: String, client: TcpStream) -> Result<()> {
        match &self.inner {
            ConnectionInner::Quic(conn) => {
                let (mut send, mut recv) = conn.open_bi().await?;

                // Send destination
                send.write_all(dest.as_bytes()).await?;

                // Pipe traffic
                let (mut cr, mut cw) = client.into_split();
                let c2s = async {
                    let mut buf = vec![0u8; 32768];
                    loop {
                        let n = tokio::io::AsyncReadExt::read(&mut cr, &mut buf).await?;
                        if n == 0 { break; }
                        send.write_all(&buf[..n]).await?;
                    }
                    send.finish()?;
                    Ok::<_, anyhow::Error>(())
                };
                let s2c = async {
                    let mut buf = vec![0u8; 32768];
                    loop {
                        let n = recv.read(&mut buf).await?.unwrap_or(0);
                        if n == 0 { break; }
                        tokio::io::AsyncWriteExt::write_all(&mut cw, &buf[..n]).await?;
                    }
                    Ok::<_, anyhow::Error>(())
                };
                tokio::try_join!(c2s, s2c)?;
            }
        }
        Ok(())
    }

    /// Send a raw datagram (for UDP relay)
    pub async fn send_datagram(&self, dest: &str, data: Bytes) -> Result<()> {
        match &self.inner {
            ConnectionInner::Quic(conn) => {
                let dest_bytes = dest.as_bytes();
                let mut buf = Vec::with_capacity(2 + dest_bytes.len() + data.len());
                buf.extend_from_slice(&(dest_bytes.len() as u16).to_be_bytes());
                buf.extend_from_slice(dest_bytes);
                buf.extend_from_slice(&data);
                conn.send_datagram(Bytes::from(buf))?;
            }
        }
        Ok(())
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

async fn connect_quic(server: &str, token: &str, profile: &str) -> Result<VeilConnection> {
    let tls_config = build_client_tls()?;
    let quinn_config = QuinnClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)?
    ));

    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(quinn_config);

    let addr: SocketAddr = resolve(server).await?;
    let host = server.split(':').next().unwrap_or(server);

    let conn = endpoint.connect(addr, host)?.await?;
    debug!(remote = %conn.remote_address(), "QUIC handshake complete");

    // Authenticate
    let session_id = authenticate_quic(&conn, token).await?;

    Ok(VeilConnection {
        inner: ConnectionInner::Quic(conn),
        session_id,
    })
}

async fn authenticate_quic(conn: &quinn::Connection, token: &str) -> Result<String> {
    let (mut send, mut recv) = conn.open_bi().await?;

    let auth_req = AuthRequest {
        challenge_id: "init".into(),
        method: AuthMethod::Token,
        credential: token.to_string(),
        client_pubkey: None,
    };
    let payload = serde_json::to_vec(&auth_req)?;
    send.write_all(&payload).await?;
    send.finish()?;

    let mut buf = vec![0u8; 1024];
    let n = recv.read(&mut buf).await?.unwrap_or(0);
    let resp: serde_json::Value = serde_json::from_slice(&buf[..n])?;

    if resp.get("status").and_then(|s| s.as_str()) == Some("ok") {
        let session_id = resp.get("session_id")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        Ok(session_id)
    } else {
        anyhow::bail!("Authentication failed: {:?}", resp)
    }
}

async fn connect_tcp(server: &str, token: &str, _profile: &str) -> Result<VeilConnection> {
    anyhow::bail!("TCP/TLS fallback not yet implemented")
}

fn build_client_tls() -> Result<Arc<RustlsClientConfig>> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = RustlsClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

async fn resolve(addr: &str) -> Result<SocketAddr> {
    // Parse host:port, resolve DNS
    let addr: SocketAddr = if addr.parse::<SocketAddr>().is_ok() {
        addr.parse()?
    } else {
        tokio::net::lookup_host(addr)
            .await?
            .next()
            .ok_or_else(|| anyhow::anyhow!("Could not resolve {}", addr))?
    };
    Ok(addr)
}
