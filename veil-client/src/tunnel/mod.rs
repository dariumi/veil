use anyhow::Result;
use std::net::Ipv4Addr;
use tracing::{debug, info};

use crate::transport::VeilConnection;

/// TUN virtual network device
pub struct TunDevice {
    name: String,
    // tun: tun::AsyncDevice, // platform-specific
}

impl TunDevice {
    pub async fn create(name: &str) -> Result<Self> {
        info!(name = %name, "Creating TUN device");
        // Platform-specific TUN creation:
        // Linux/macOS: use `tun` crate
        // Windows: use `wintun` crate
        Ok(Self {
            name: name.to_string(),
        })
    }

    pub async fn configure(
        &self,
        local_ip: Ipv4Addr,
        gateway: Ipv4Addr,
        prefix_len: u8,
    ) -> Result<()> {
        info!(
            device = %self.name,
            ip = %local_ip,
            gw = %gateway,
            prefix = %prefix_len,
            "Configuring TUN device"
        );

        // Add default route through TUN
        // Linux: `ip route add default via <gateway> dev <name>`
        // macOS: `route add default <gateway>`
        // Windows: via wintun API

        Ok(())
    }

    /// Pump packets between TUN device and Veil connection
    pub async fn run(self, _conn: VeilConnection) -> Result<()> {
        let buf = vec![0u8; 65536];
        let _ = buf;

        // Read IP packets from TUN → send as datagrams through Veil
        // Receive datagrams from Veil → write IP packets to TUN
        loop {
            // Placeholder: real implementation reads from tun device
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            debug!(device = %self.name, "TUN pump tick");
        }
    }
}
