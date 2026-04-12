use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use chrono::{DateTime, Utc};
use super::{TransportMode, TrafficProfile};

pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Handshaking,
    Authenticating,
    Active,
    Migrating,
    Draining,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub state: SessionState,
    pub transport: TransportMode,
    pub profile: TrafficProfile,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    /// Current remote address (can change on migration)
    pub remote_addr: Option<SocketAddr>,
    /// Bytes sent/received
    pub bytes_tx: u64,
    pub bytes_rx: u64,
}

impl Session {
    pub fn new(id: SessionId, transport: TransportMode) -> Self {
        let now = Utc::now();
        Self {
            id,
            state: SessionState::Handshaking,
            transport,
            profile: TrafficProfile::default(),
            created_at: now,
            last_seen: now,
            remote_addr: None,
            bytes_tx: 0,
            bytes_rx: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.state == SessionState::Active
    }

    pub fn touch(&mut self) {
        self.last_seen = Utc::now();
    }
}
