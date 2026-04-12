use anyhow::Result;
use rustls::ServerConfig as RustlsConfig;
use rustls_pemfile::{certs, private_key};
use std::{fs::File, io::BufReader, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::{debug, info, warn};

use crate::auth::AuthManager;
use crate::config::ServerConfig;
use crate::relay::RelayEngine;

pub async fn run_listener(config: Arc<ServerConfig>, auth: Arc<AuthManager>) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", config.listen.bind, config.listen.tcp_port).parse()?;

    let listener = TcpListener::bind(addr).await?;
    info!(addr = %addr, "TCP/TLS fallback listener ready");

    let tls_config = build_tls_config(&config)?;
    let acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

    loop {
        match listener.accept().await {
            Ok((stream, remote)) => {
                let acceptor = acceptor.clone();
                let auth = auth.clone();
                let config = config.clone();
                tokio::spawn(async move {
                    match acceptor.accept(stream).await {
                        Ok(tls_stream) => {
                            debug!(remote = %remote, "New TCP/TLS connection");
                            if let Err(e) = handle_tls_connection(tls_stream, config, auth).await {
                                debug!(remote = %remote, err = %e, "TCP/TLS session ended");
                            }
                        }
                        Err(e) => {
                            // Failed TLS — respond like plain HTTPS for anti-probing
                            debug!("TLS handshake failed: {}", e);
                        }
                    }
                });
            }
            Err(e) => {
                warn!("TCP accept error: {}", e);
            }
        }
    }
}

async fn handle_tls_connection(
    stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    config: Arc<ServerConfig>,
    auth: Arc<AuthManager>,
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut reader, mut writer) = tokio::io::split(stream);

    // Read auth request
    let mut buf = vec![0u8; 4096];
    let n = reader.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }

    let session = auth.authenticate(&buf[..n]).await?;
    tracing::info!(session_id = %session.id, transport = "tcp", "Session authenticated");

    let ok = serde_json::to_vec(&serde_json::json!({"status": "ok", "session_id": session.id}))?;
    writer.write_all(&ok).await?;

    let relay = RelayEngine::new(config, session);
    relay.run_tcp(reader, writer).await?;

    Ok(())
}

fn build_tls_config(config: &ServerConfig) -> Result<Arc<RustlsConfig>> {
    let cert_file = File::open(&config.tls.cert_path)?;
    let key_file = File::open(&config.tls.key_path)?;

    let certs: Vec<_> =
        certs(&mut BufReader::new(cert_file)).collect::<std::result::Result<_, _>>()?;
    let key = private_key(&mut BufReader::new(key_file))?
        .ok_or_else(|| anyhow::anyhow!("No private key"))?;

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
