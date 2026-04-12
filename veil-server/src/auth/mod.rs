use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::config::AuthConfig;
use veil_core::crypto::{sha256, token::TokenManager};
use veil_core::error::VeilError;
use veil_core::protocol::session::{Session, SessionId, SessionState};
use veil_core::protocol::TransportMode;

pub struct AuthManager {
    config: AuthConfig,
    token_manager: TokenManager,
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    /// Token hash → token entry index
    token_lookup: HashMap<String, usize>,
}

impl AuthManager {
    pub fn new(config: &AuthConfig) -> Result<Self> {
        let signing_key = hex::decode(&config.signing_key)?;
        let token_manager = TokenManager::new(&signing_key);

        // Build lookup table for fast token verification
        let token_lookup = config
            .tokens
            .iter()
            .enumerate()
            .map(|(i, t)| (t.token_hash.clone(), i))
            .collect();

        Ok(Self {
            config: config.clone(),
            token_manager,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            token_lookup,
        })
    }

    /// Authenticate a raw auth request payload and return a new session if valid
    pub async fn authenticate(&self, payload: &[u8]) -> Result<Session> {
        use veil_core::protocol::handshake::AuthRequest;

        let req: AuthRequest =
            serde_json::from_slice(payload).map_err(|_| anyhow::anyhow!("Invalid auth request"))?;

        // Rate limiting / tarpit for failed attempts would be applied here

        match req.method {
            veil_core::protocol::handshake::AuthMethod::Token => {
                self.verify_token(&req.credential).await
            }
            veil_core::protocol::handshake::AuthMethod::PreSharedKey => {
                self.verify_psk(&req.credential).await
            }
            veil_core::protocol::handshake::AuthMethod::Certificate => {
                Err(anyhow::anyhow!("Certificate auth not yet implemented"))
            }
        }
    }

    async fn verify_token(&self, token: &str) -> Result<Session> {
        // Hash the presented token and look it up
        let hash = hex::encode(sha256(token.as_bytes()));

        if let Some(&idx) = self.token_lookup.get(&hash) {
            let entry = &self.config.tokens[idx];

            // Check expiry
            if let Some(exp_str) = &entry.expires_at {
                let exp: chrono::DateTime<chrono::Utc> = exp_str
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid expiry date"))?;
                if chrono::Utc::now() > exp {
                    warn!(token_id = %entry.id, "Expired token used");
                    return Err(VeilError::AuthFailed("Token expired".into()).into());
                }
            }

            let session_id = uuid::Uuid::new_v4().to_string();
            let mut session = Session::new(session_id.clone(), TransportMode::QuicHttp3);
            session.state = SessionState::Active;

            self.sessions
                .write()
                .await
                .insert(session_id, session.clone());
            debug!(token_id = %entry.id, session_id = %session.id, "Session created");

            Ok(session)
        } else {
            // Tarpit: delay response to slow down brute-force
            if self.config.tarpit_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(self.config.tarpit_ms)).await;
            }
            warn!("Invalid token presented");
            Err(VeilError::AuthFailed("Invalid token".into()).into())
        }
    }

    async fn verify_psk(&self, _psk: &str) -> Result<Session> {
        // PSK auth: compare against configured PSK hash
        Err(anyhow::anyhow!("PSK auth not configured"))
    }

    pub async fn get_session(&self, id: &str) -> Option<Session> {
        self.sessions.read().await.get(id).cloned()
    }

    pub async fn remove_session(&self, id: &str) {
        self.sessions.write().await.remove(id);
    }

    pub async fn active_session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Generate a short-lived invite token
    pub fn generate_invite(&self, ttl_secs: u64) -> String {
        self.token_manager.create_invite(ttl_secs)
    }
}

mod hex {
    pub fn encode(bytes: Vec<u8>) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
    pub fn decode(s: &str) -> Result<Vec<u8>, anyhow::Error> {
        if s.len() % 2 != 0 {
            anyhow::bail!("odd hex string length");
        }
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16)
                    .map_err(|e| anyhow::anyhow!("invalid hex: {}", e))
            })
            .collect()
    }
}
