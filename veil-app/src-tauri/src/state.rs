use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
    pub domain: Option<String>,
    pub mode: ClientMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientMode {
    Vpn,
    Proxy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManagedServer {
    pub host: String,
    pub port: u16,
    pub admin_token: String,
    pub domain: Option<String>,
}

#[derive(Debug, Default)]
pub struct AppState {
    pub status: ConnectionStatus,
    pub active_profile: Option<String>,
    pub profiles: Vec<ServerProfile>,
    pub managed_server: Option<ManagedServer>,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub connected_since: Option<std::time::Instant>,
}

impl AppState {
    pub fn elapsed_secs(&self) -> u64 {
        self.connected_since
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0)
    }
}
