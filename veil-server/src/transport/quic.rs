use anyhow::Result;
use quinn::{Endpoint, ServerConfig as QuinnServerConfig};
use rustls::ServerConfig as RustlsConfig;
use rustls_pemfile::{certs, private_key};
use std::{fs::File, io::BufReader, net::SocketAddr, sync::Arc};
use tracing::{debug, info, warn};

use crate::auth::AuthManager;
use crate::config::ServerConfig;
use crate::relay::RelayEngine;

pub async fn run_listener(config: Arc<ServerConfig>, auth: Arc<AuthManager>) -> Result<()> {
    let tls_config = build_tls_config(&config)?;
    let quinn_config = QuinnServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)?,
    ));

    let addr: SocketAddr = format!("{}:{}", config.listen.bind, config.listen.quic_port).parse()?;

    let endpoint = Endpoint::server(quinn_config, addr)?;
    info!(addr = %addr, "QUIC/HTTP3 listener ready");

    loop {
        let incoming = endpoint.accept().await;
        match incoming {
            Some(conn) => {
                let auth = auth.clone();
                let config = config.clone();
                tokio::spawn(async move {
                    match conn.await {
                        Ok(connection) => {
                            let remote = connection.remote_address();
                            debug!(remote = %remote, "New QUIC connection");
                            if let Err(e) = handle_quic_connection(connection, config, auth).await {
                                debug!(remote = %remote, err = %e, "Connection ended");
                            }
                        }
                        Err(e) => {
                            // Failed TLS handshake — anti-probing: log minimally
                            debug!("Incoming connection rejected: {}", e);
                        }
                    }
                });
            }
            None => {
                warn!("QUIC endpoint closed");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_quic_connection(
    conn: quinn::Connection,
    config: Arc<ServerConfig>,
    auth: Arc<AuthManager>,
) -> Result<()> {
    // Anti-probing: first message must be valid Veil auth before relaying anything
    // Accept first bidirectional stream as the auth channel
    let (mut send, mut recv) = conn.accept_bi().await?;

    // Read ClientHello
    let mut buf = vec![0u8; 4096];
    let n = recv.read(&mut buf).await?.unwrap_or(0);
    if n == 0 {
        return Ok(());
    }

    let session = auth.authenticate(&buf[..n]).await?;
    tracing::info!(session_id = %session.id, "Session authenticated");

    // Send AuthOk
    let ok = serde_json::to_vec(&serde_json::json!({"status": "ok", "session_id": session.id}))?;
    send.write_all(&ok).await?;
    send.finish()?;

    // Hand off to relay engine
    let relay = RelayEngine::new(config, session);
    relay.run_quic(conn).await?;

    Ok(())
}

fn build_tls_config(config: &ServerConfig) -> Result<Arc<RustlsConfig>> {
    let cert_file = File::open(&config.tls.cert_path)?;
    let key_file = File::open(&config.tls.key_path)?;

    let certs: Vec<_> =
        certs(&mut BufReader::new(cert_file)).collect::<std::result::Result<_, _>>()?;

    let key = private_key(&mut BufReader::new(key_file))?
        .ok_or_else(|| anyhow::anyhow!("No private key found"))?;

    let alpn: Vec<Vec<u8>> = config
        .tls
        .alpn
        .iter()
        .map(|a| a.as_bytes().to_vec())
        .collect();

    let mut tls = RustlsConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    tls.alpn_protocols = alpn;

    Ok(Arc::new(tls))
}
