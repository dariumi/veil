use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, path::Path};
use veil_core::protocol::NodeRole;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub node: NodeConfig,
    pub listen: ListenConfig,
    pub tls: TlsConfig,
    pub auth: AuthConfig,
    pub relay: RelayConfig,
    pub limits: LimitsConfig,
    pub logging: LoggingConfig,
    pub admin: AdminConfig,
    pub obfuscation: ObfuscationConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeConfig {
    pub role: NodeRole,
    pub name: Option<String>,
    /// Allowed exit destinations (CIDR or hostnames). Empty = allow all.
    pub allowed_destinations: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListenConfig {
    pub bind: IpAddr,
    /// Primary QUIC/HTTP3 port
    pub quic_port: u16,
    /// Fallback TCP/TLS port
    pub tcp_port: u16,
    /// Additional ports to listen on for camouflage
    #[serde(default)]
    pub extra_ports: Vec<u16>,
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0".parse().unwrap(),
            quic_port: 443,
            tcp_port: 443,
            extra_ports: vec![8443, 2053],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    /// Server Name Indication - domain to present
    pub sni: Option<String>,
    /// ALPN protocols (for HTTP/3 camouflage use h3)
    #[serde(default = "default_alpn")]
    pub alpn: Vec<String>,
    /// Key rotation interval in hours
    pub key_rotation_hours: Option<u64>,
}

fn default_alpn() -> Vec<String> {
    vec!["h3".into(), "h2".into(), "http/1.1".into()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    /// Master signing key for tokens (hex-encoded 32 bytes)
    pub signing_key: String,
    /// Static access tokens
    #[serde(default)]
    pub tokens: Vec<TokenEntry>,
    /// Short-lived invite token TTL in seconds
    pub invite_ttl_seconds: u64,
    /// Enable rate limiting on auth attempts
    pub rate_limit: bool,
    /// Tarpitting delay on failed auth (ms)
    pub tarpit_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenEntry {
    pub id: String,
    pub token_hash: String, // SHA-256 of the actual token
    pub label: Option<String>,
    pub is_admin: bool,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelayConfig {
    pub tcp_enabled: bool,
    pub udp_enabled: bool,
    pub dns_enabled: bool,
    /// Max concurrent relay streams per session
    pub max_streams_per_session: usize,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            tcp_enabled: true,
            udp_enabled: true,
            dns_enabled: true,
            max_streams_per_session: 256,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LimitsConfig {
    pub max_sessions: usize,
    pub bandwidth_mbps: Option<u32>,
    pub session_timeout_secs: u64,
    /// DDoS rate limit: connections per second per IP
    pub connect_rate_per_ip: u32,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_sessions: 1000,
            bandwidth_mbps: None,
            session_timeout_secs: 3600,
            connect_rate_per_ip: 10,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Log level: error, warn, info, debug, trace
    pub level: String,
    /// Disable logging entirely
    pub disabled: bool,
    /// Log to file
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "warn".into(),
            disabled: false,
            file: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdminConfig {
    pub enabled: bool,
    pub bind: IpAddr,
    pub port: u16,
    /// Admin API token (separate from user tokens)
    pub admin_token: String,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: "127.0.0.1".parse().unwrap(),
            port: 9090,
            admin_token: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ObfuscationConfig {
    /// Add random padding to frames
    pub padding_enabled: bool,
    /// Packet size normalization to common HTTPS sizes
    pub size_normalization: bool,
    /// Add idle noise when session is quiet
    pub idle_noise: bool,
    /// Burst shaping
    pub burst_shaping: bool,
}

impl Default for ObfuscationConfig {
    fn default() -> Self {
        Self {
            padding_enabled: true,
            size_normalization: true,
            idle_noise: false,
            burst_shaping: true,
        }
    }
}

impl ServerConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.tls.cert_path.is_empty() || self.tls.key_path.is_empty() {
            bail!("TLS cert and key paths must be set");
        }
        if self.auth.signing_key.is_empty() {
            bail!("auth.signing_key must be set");
        }
        if self.admin.enabled && self.admin.admin_token.is_empty() {
            bail!("admin.admin_token must be set when admin API is enabled");
        }
        Ok(())
    }
}
