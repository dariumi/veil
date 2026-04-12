use super::{generate_token, hmac_sha256, verify_hmac_sha256};
use crate::error::{Result, VeilError};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    pub id: String,
    pub token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub label: Option<String>,
    pub is_admin: bool,
}

impl AccessToken {
    pub fn new(label: Option<String>, ttl_hours: Option<i64>, is_admin: bool) -> Self {
        let now = Utc::now();
        Self {
            id: generate_token(8),
            token: generate_token(32),
            created_at: now,
            expires_at: ttl_hours.map(|h| now + Duration::hours(h)),
            label,
            is_admin,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            Utc::now() > exp
        } else {
            false
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }
}

/// Manages access tokens with HMAC signing for anti-tampering
pub struct TokenManager {
    signing_key: Vec<u8>,
}

impl TokenManager {
    pub fn new(signing_key: &[u8]) -> Self {
        Self {
            signing_key: signing_key.to_vec(),
        }
    }

    /// Create a signed invite token (short-lived admission token)
    pub fn create_invite(&self, ttl_seconds: u64) -> String {
        let expires = Utc::now().timestamp() as u64 + ttl_seconds;
        let payload = format!("invite:{}:{}", generate_token(16), expires);
        let sig = hmac_sha256(&self.signing_key, payload.as_bytes());
        let sig_hex = hex::encode(sig);
        format!("{}.{}", payload, sig_hex)
    }

    /// Verify a signed invite token
    pub fn verify_invite(&self, token: &str) -> Result<()> {
        let parts: Vec<&str> = token.rsplitn(2, '.').collect();
        if parts.len() != 2 {
            return Err(VeilError::AuthFailed("Invalid token format".into()));
        }
        let (sig_hex, payload) = (parts[0], parts[1]);
        let sig = hex::decode(sig_hex)
            .map_err(|_| VeilError::AuthFailed("Invalid token signature".into()))?;

        if !verify_hmac_sha256(&self.signing_key, payload.as_bytes(), &sig) {
            return Err(VeilError::AuthFailed(
                "Token signature verification failed".into(),
            ));
        }

        // Check expiry
        let parts: Vec<&str> = payload.splitn(3, ':').collect();
        if parts.len() == 3 {
            if let Ok(exp) = parts[2].parse::<u64>() {
                if Utc::now().timestamp() as u64 > exp {
                    return Err(VeilError::AuthFailed("Token expired".into()));
                }
            }
        }

        Ok(())
    }
}

// Simple hex encoding helper (avoid extra dep)
mod hex {
    pub fn encode(bytes: Vec<u8>) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
    pub fn decode(s: &str) -> Result<Vec<u8>, ()> {
        if s.len() % 2 != 0 {
            return Err(());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
            .collect()
    }
}
