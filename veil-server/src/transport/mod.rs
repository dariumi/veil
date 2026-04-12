use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, info};

pub mod quic;
pub mod tls_tcp;

use crate::admin::AdminServer;
use crate::auth::AuthManager;
use crate::config::ServerConfig;

/// Top-level server: spawns QUIC + TCP listeners + Admin API
pub struct Server {
    config: Arc<ServerConfig>,
    auth: Arc<AuthManager>,
}

impl Server {
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let auth = Arc::new(AuthManager::new(&config.auth)?);
        Ok(Self {
            config: Arc::new(config),
            auth,
        })
    }

    pub async fn run(self) -> Result<()> {
        let mut tasks = JoinSet::new();

        // QUIC / HTTP3 listener (primary)
        {
            let config = self.config.clone();
            let auth = self.auth.clone();
            tasks.spawn(async move {
                if let Err(e) = quic::run_listener(config, auth).await {
                    error!("QUIC listener failed: {}", e);
                }
            });
        }

        // TLS/TCP fallback listener
        {
            let config = self.config.clone();
            let auth = self.auth.clone();
            tasks.spawn(async move {
                if let Err(e) = tls_tcp::run_listener(config, auth).await {
                    error!("TCP/TLS listener failed: {}", e);
                }
            });
        }

        // Admin API
        if self.config.admin.enabled {
            let config = self.config.clone();
            let auth = self.auth.clone();
            tasks.spawn(async move {
                let admin = AdminServer::new(config, auth);
                if let Err(e) = admin.run().await {
                    error!("Admin API failed: {}", e);
                }
            });
        }

        info!("All listeners started");

        // Wait for any task to finish (unexpected exit = error)
        while let Some(res) = tasks.join_next().await {
            if let Err(e) = res {
                error!("Task panicked: {}", e);
            }
        }

        Ok(())
    }
}
