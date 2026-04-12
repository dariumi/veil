use serde::{Deserialize, Serialize};
use super::{ProtocolVersion, TransportMode, TrafficProfile};

/// Client → Server: initial hello
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    pub version: ProtocolVersion,
    pub supported_transports: Vec<TransportMode>,
    pub preferred_profile: TrafficProfile,
    /// Random nonce for replay protection
    pub nonce: [u8; 32],
    /// ALPN to present (e.g. "h3", "h2", "veil/1")
    pub alpn: Vec<String>,
}

/// Server → Client: hello response with chosen transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    pub version: ProtocolVersion,
    pub chosen_transport: TransportMode,
    /// Server nonce
    pub nonce: [u8; 32],
    /// Anti-probing challenge (must be answered before relay is activated)
    pub challenge: Option<AuthChallenge>,
}

/// Server → Client: authentication challenge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthChallenge {
    pub challenge_id: String,
    pub challenge_bytes: Vec<u8>,
    pub method: AuthMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Token,
    Certificate,
    PreSharedKey,
}

/// Client → Server: authentication request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub challenge_id: String,
    pub method: AuthMethod,
    /// Encoded credential (token / cert fingerprint / PSK response)
    pub credential: String,
    /// Client public key for session key derivation
    pub client_pubkey: Option<Vec<u8>>,
}

/// Server → Client: authentication result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub success: bool,
    pub session_token: Option<String>,
    pub error: Option<String>,
    /// Server public key for session key derivation
    pub server_pubkey: Option<Vec<u8>>,
}
