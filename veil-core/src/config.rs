use crate::protocol::{TrafficProfile, TransportMode};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Shared client configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub server: ServerEndpoint,
    pub auth: ClientAuth,
    pub transport: TransportConfig,
    pub proxy: ProxyConfig,
    pub dns: DnsConfig,
    pub routing: RoutingConfig,
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEndpoint {
    pub host: String,
    pub port: u16,
    pub domain: Option<String>,
    /// Path to custom CA cert (for cert pinning)
    pub ca_cert: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ClientAuth {
    Token { token: String },
    PreSharedKey { key: String },
    Certificate { cert_path: String, key_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub mode: TransportMode,
    pub profile: TrafficProfile,
    pub mtu: Option<u16>,
    pub keepalive_secs: Option<u64>,
    pub reconnect_delay_secs: u64,
    pub fallback_enabled: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            mode: TransportMode::QuicHttp3,
            profile: TrafficProfile::Balanced,
            mtu: None,
            keepalive_secs: Some(30),
            reconnect_delay_secs: 2,
            fallback_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub socks5_port: Option<u16>,
    pub http_port: Option<u16>,
    pub listen_addr: IpAddr,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            socks5_port: Some(1080),
            http_port: Some(8080),
            listen_addr: "127.0.0.1".parse().unwrap(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub mode: DnsMode,
    pub servers: Vec<String>,
    pub search_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DnsMode {
    Remote,
    DnsOverTls,
    DnsOverHttps,
    System,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            mode: DnsMode::DnsOverHttps,
            servers: vec!["1.1.1.1".into(), "8.8.8.8".into()],
            search_domains: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutingConfig {
    /// Routes to exclude from the tunnel (split tunnel)
    pub bypass_routes: Vec<IpNet>,
    /// Only route these through the tunnel (split tunnel)
    pub include_routes: Vec<IpNet>,
    pub multi_hop: Option<MultiHopConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHopConfig {
    pub entry: ServerEndpoint,
    pub relay: Option<ServerEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub kill_switch: bool,
    pub dns_leak_protection: bool,
    pub ipv6_leak_protection: bool,
    pub block_on_disconnect: bool,
    pub lan_bypass: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            kill_switch: true,
            dns_leak_protection: true,
            ipv6_leak_protection: true,
            block_on_disconnect: true,
            lan_bypass: false,
        }
    }
}
