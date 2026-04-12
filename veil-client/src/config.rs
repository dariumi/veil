use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use veil_core::config::ClientConfig as CoreClientConfig;

use crate::{ConfigCommands, ServerCommands};

/// Persisted client configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientConfig {
    /// Saved server profiles
    pub profiles: Vec<ServerProfile>,
    /// Currently active profile name
    pub active_profile: Option<String>,
    /// Managed server (self-hosted)
    pub managed_server: Option<ManagedServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
    pub domain: Option<String>,
    pub ca_cert: Option<String>,
}

/// Self-hosted server metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedServer {
    pub host: String,
    pub ssh_port: u16,
    pub veil_port: u16,
    pub admin_token: String,
    pub domain: Option<String>,
    pub installed_at: String,
}

impl ClientConfig {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        let expanded = expand_tilde(path);
        if expanded.exists() {
            let content = std::fs::read_to_string(&expanded)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let expanded = expand_tilde(path);
        if let Some(parent) = expanded.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&expanded, content)?;
        Ok(())
    }

    pub fn management_url(&self) -> Option<String> {
        self.managed_server
            .as_ref()
            .map(|s| format!("https://{}:{}/api/v1", s.host, s.veil_port))
    }

    pub fn admin_token(&self) -> Result<String> {
        self.managed_server
            .as_ref()
            .map(|s| s.admin_token.clone())
            .ok_or_else(|| anyhow::anyhow!("No admin token configured"))
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&s[2..]);
        }
    }
    path.to_path_buf()
}

pub fn handle_config_command(action: Option<ConfigCommands>, config_path: &Path) -> Result<()> {
    let config = ClientConfig::load_or_default(config_path)?;

    match action {
        None | Some(ConfigCommands::Show) => {
            println!("{}", toml::to_string_pretty(&config)?);
        }
        Some(ConfigCommands::Set { key, value }) => {
            println!("Setting {} = {} (not yet implemented)", key, value);
        }
        Some(ConfigCommands::Reset) => {
            let default = ClientConfig::default();
            default.save(config_path)?;
            println!("Config reset to defaults");
        }
    }

    Ok(())
}
