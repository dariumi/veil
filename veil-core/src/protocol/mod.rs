use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod frame;
pub mod handshake;
pub mod session;

pub use frame::{ChannelId, Frame, FrameType};
pub use handshake::{AuthRequest, AuthResponse, ClientHello, ServerHello};
pub use session::{Session, SessionId, SessionState};

/// Protocol version negotiation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl ProtocolVersion {
    pub const CURRENT: Self = Self {
        major: 0,
        minor: 1,
        patch: 0,
    };

    pub fn is_compatible(&self, other: &Self) -> bool {
        self.major == other.major
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Transport mode selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransportMode {
    /// Primary: QUIC + TLS 1.3 + HTTP/3 camouflage
    QuicHttp3,
    /// Fallback: TLS over TCP (HTTP/2)
    TlsTcp,
    /// WireGuard compatibility mode
    WireGuardCompat,
}

/// Traffic profile
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrafficProfile {
    #[default]
    Balanced,
    Realtime,
    Throughput,
    Stealth,
}

/// Node role in multi-hop topology
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    Entry,
    Relay,
    Exit,
    /// Combines all roles (single-hop mode)
    All,
}

/// Unique session identifier — short-lived, not persistent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionToken(pub String);

impl SessionToken {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}
