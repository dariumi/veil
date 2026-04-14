use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Persisted client configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientConfig {
    pub profiles: Vec<ServerProfile>,
    pub active_profile: Option<String>,
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
        std::fs::write(&expanded, toml::to_string_pretty(self)?)?;
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

// Config sub-commands — defined here, used by main.rs via `use crate::config`
pub enum ConfigAction {
    Show,
    Set { key: String, value: String },
    Reset,
}

pub fn handle_config_command(action: Option<ConfigAction>, config_path: &Path) -> Result<()> {
    let config = ClientConfig::load_or_default(config_path)?;
    match action {
        None | Some(ConfigAction::Show) => {
            println!("{}", toml::to_string_pretty(&config)?);
        }
        Some(ConfigAction::Set { key, value }) => {
            println!("Setting {} = {} (not yet implemented)", key, value);
        }
        Some(ConfigAction::Reset) => {
            ClientConfig::default().save(config_path)?;
            println!("Config reset to defaults");
        }
    }
    Ok(())
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
