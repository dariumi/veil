use anyhow::{bail, Result};
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, warn};

use crate::config::ServerConfig;

/// Handle a single bidirectional QUIC stream as TCP relay
pub async fn handle_stream(
    mut send: quinn::SendStream,
    mut recv: quinn::RecvStream,
    config: Arc<ServerConfig>,
    session_id: &str,
) -> Result<()> {
    // Read destination from first message: "host:port\n"
    let mut header = vec![0u8; 256];
    let n = recv.read(&mut header).await?.unwrap_or(0);
    if n == 0 {
        return Ok(());
    }

    let dest = std::str::from_utf8(&header[..n])?.trim().to_string();
    debug!(session = %session_id, dest = %dest, "TCP relay request");

    if !is_destination_allowed(&dest, &config) {
        bail!("Destination not allowed: {}", dest);
    }

    let target = TcpStream::connect(&dest).await?;
    let (mut target_rx, mut target_tx) = target.into_split();

    // Bidirectional pipe: client ←→ target
    let client_to_target = async {
        let mut buf = vec![0u8; 32768];
        loop {
            let n = recv.read(&mut buf).await?.unwrap_or(0);
            if n == 0 {
                break;
            }
            target_tx.write_all(&buf[..n]).await?;
        }
        target_tx.shutdown().await?;
        Ok::<_, anyhow::Error>(())
    };

    let target_to_client = async {
        let mut buf = vec![0u8; 32768];
        loop {
            let n = target_rx.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            send.write_all(&buf[..n]).await?;
        }
        send.finish()?;
        Ok::<_, anyhow::Error>(())
    };

    tokio::try_join!(client_to_target, target_to_client)?;
    Ok(())
}

fn is_destination_allowed(dest: &str, config: &ServerConfig) -> bool {
    if config.node.allowed_destinations.is_empty() {
        return true; // Allow all
    }
    // TODO: implement ACL matching (CIDR + hostname patterns)
    config
        .node
        .allowed_destinations
        .iter()
        .any(|rule| dest.contains(rule.as_str()))
}
