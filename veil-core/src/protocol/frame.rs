use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use crate::error::{Result, VeilError};

/// Logical channel identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub u32);

impl ChannelId {
    pub const CONTROL: Self = Self(0);
    pub const AUTH: Self = Self(1);
    pub const DNS: Self = Self(2);
    pub const TELEMETRY: Self = Self(3);
    /// Data relay channels start from 1000
    pub const RELAY_BASE: u32 = 1000;
}

/// Veil frame types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum FrameType {
    // Control plane
    Ping = 0x01,
    Pong = 0x02,
    Error = 0x03,
    Shutdown = 0x04,
    // Auth
    AuthHello = 0x10,
    AuthChallenge = 0x11,
    AuthResponse = 0x12,
    AuthOk = 0x13,
    AuthFail = 0x14,
    // Data relay
    StreamOpen = 0x20,
    StreamData = 0x21,
    StreamClose = 0x22,
    StreamReset = 0x23,
    // Datagram (unreliable, low-latency)
    Datagram = 0x30,
    // DNS
    DnsQuery = 0x40,
    DnsResponse = 0x41,
    // Config / management
    ConfigPush = 0x50,
    ConfigAck = 0x51,
    // Obfuscation padding
    Padding = 0xFF,
}

impl TryFrom<u8> for FrameType {
    type Error = VeilError;
    fn try_from(v: u8) -> Result<Self> {
        match v {
            0x01 => Ok(Self::Ping),
            0x02 => Ok(Self::Pong),
            0x03 => Ok(Self::Error),
            0x04 => Ok(Self::Shutdown),
            0x10 => Ok(Self::AuthHello),
            0x11 => Ok(Self::AuthChallenge),
            0x12 => Ok(Self::AuthResponse),
            0x13 => Ok(Self::AuthOk),
            0x14 => Ok(Self::AuthFail),
            0x20 => Ok(Self::StreamOpen),
            0x21 => Ok(Self::StreamData),
            0x22 => Ok(Self::StreamClose),
            0x23 => Ok(Self::StreamReset),
            0x30 => Ok(Self::Datagram),
            0x40 => Ok(Self::DnsQuery),
            0x41 => Ok(Self::DnsResponse),
            0x50 => Ok(Self::ConfigPush),
            0x51 => Ok(Self::ConfigAck),
            0xFF => Ok(Self::Padding),
            _ => Err(VeilError::Protocol(format!("Unknown frame type: 0x{:02x}", v))),
        }
    }
}

/// Wire-format Veil frame
///
/// Layout:
/// | version (1) | type (1) | channel_id (4) | length (4) | payload (N) |
#[derive(Debug, Clone)]
pub struct Frame {
    pub version: u8,
    pub frame_type: FrameType,
    pub channel_id: ChannelId,
    pub payload: Bytes,
}

impl Frame {
    pub const HEADER_SIZE: usize = 10; // 1 + 1 + 4 + 4

    pub fn new(frame_type: FrameType, channel_id: ChannelId, payload: Bytes) -> Self {
        Self {
            version: 1,
            frame_type,
            channel_id,
            payload,
        }
    }

    pub fn ping() -> Self {
        Self::new(FrameType::Ping, ChannelId::CONTROL, Bytes::new())
    }

    pub fn pong() -> Self {
        Self::new(FrameType::Pong, ChannelId::CONTROL, Bytes::new())
    }

    pub fn padding(size: usize) -> Self {
        let pad = vec![0u8; size];
        Self::new(FrameType::Padding, ChannelId::CONTROL, Bytes::from(pad))
    }

    /// Encode frame to bytes
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(Self::HEADER_SIZE + self.payload.len());
        buf.put_u8(self.version);
        buf.put_u8(self.frame_type as u8);
        buf.put_u32(self.channel_id.0);
        buf.put_u32(self.payload.len() as u32);
        buf.put_slice(&self.payload);
        buf.freeze()
    }

    /// Decode frame from bytes
    pub fn decode(mut buf: Bytes) -> Result<Self> {
        if buf.len() < Self::HEADER_SIZE {
            return Err(VeilError::Protocol("Frame too short".into()));
        }
        let version = buf.get_u8();
        let frame_type = FrameType::try_from(buf.get_u8())?;
        let channel_id = ChannelId(buf.get_u32());
        let length = buf.get_u32() as usize;

        if buf.remaining() < length {
            return Err(VeilError::Protocol("Truncated frame payload".into()));
        }
        let payload = buf.copy_to_bytes(length);

        Ok(Self { version, frame_type, channel_id, payload })
    }
}
