use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, warn};

use veil_core::protocol::frame::{Frame, FrameType, ChannelId};
use veil_core::protocol::session::Session;
use crate::config::ServerConfig;

pub mod tcp_relay;
pub mod udp_relay;

/// Relay engine: handles multiplexed streams for a single authenticated session
pub struct RelayEngine {
    config: Arc<ServerConfig>,
    session: Session,
}

impl RelayEngine {
    pub fn new(config: Arc<ServerConfig>, session: Session) -> Self {
        Self { config, session }
    }

    /// Run relay over established QUIC connection
    pub async fn run_quic(self, conn: quinn::Connection) -> Result<()> {
        loop {
            tokio::select! {
                // Accept new bidirectional streams (reliable TCP relay)
                stream = conn.accept_bi() => {
                    match stream {
                        Ok((send, recv)) => {
                            let config = self.config.clone();
                            let session_id = self.session.id.clone();
                            tokio::spawn(async move {
                                if let Err(e) = tcp_relay::handle_stream(
                                    send, recv, config, &session_id
                                ).await {
                                    debug!(err = %e, "Stream relay ended");
                                }
                            });
                        }
                        Err(e) => {
                            debug!("Connection closed: {}", e);
                            break;
                        }
                    }
                }

                // Accept unidirectional datagrams (UDP relay)
                datagram = conn.read_datagram() => {
                    match datagram {
                        Ok(data) => {
                            let config = self.config.clone();
                            tokio::spawn(async move {
                                if let Err(e) = udp_relay::handle_datagram(data, config).await {
                                    debug!(err = %e, "Datagram relay error");
                                }
                            });
                        }
                        Err(e) => {
                            debug!("Datagram error: {}", e);
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Run relay over TLS/TCP stream
    pub async fn run_tcp<R, W>(self, reader: R, writer: W) -> Result<()>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        use bytes::Bytes;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut reader = reader;
        let mut writer = writer;
        let mut buf = vec![0u8; 65536];

        loop {
            let n = match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    debug!(err = %e, "TCP relay read error");
                    break;
                }
            };

            let frame = Frame::decode(Bytes::copy_from_slice(&buf[..n]));
            match frame {
                Ok(f) => {
                    match f.frame_type {
                        FrameType::StreamOpen => {
                            // New TCP connection request: parse destination from payload
                            let config = self.config.clone();
                            let payload = f.payload.clone();
                            let channel = f.channel_id;
                            // Spawn TCP relay for this stream
                            debug!(channel = ?channel, "Stream open request");
                        }
                        FrameType::Ping => {
                            let pong = Frame::pong().encode();
                            let _ = writer.write_all(&pong).await;
                        }
                        FrameType::Shutdown => break,
                        _ => {}
                    }
                }
                Err(e) => {
                    warn!(err = %e, "Bad frame in TCP relay");
                    break;
                }
            }
        }

        Ok(())
    }
}
