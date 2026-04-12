use anyhow::Result;
use bytes::Bytes;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::debug;

use crate::config::ServerConfig;

/// UDP datagram relay header format (prepended to each datagram):
/// | dest_len (2) | dest (N) | payload |
pub async fn handle_datagram(data: Bytes, config: Arc<ServerConfig>) -> Result<()> {
    if data.len() < 3 {
        return Ok(());
    }

    let dest_len = u16::from_be_bytes([data[0], data[1]]) as usize;
    if data.len() < 2 + dest_len {
        return Ok(());
    }

    let dest = std::str::from_utf8(&data[2..2 + dest_len])?.to_string();
    let payload = data.slice(2 + dest_len..);

    debug!(dest = %dest, payload_len = payload.len(), "UDP relay datagram");

    if !config.relay.udp_enabled {
        return Ok(());
    }

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.send_to(&payload, &dest).await?;

    // For stateless UDP relay we don't wait for response here.
    // A stateful relay would track socket → session mapping for bidirectional UDP.

    Ok(())
}
