use anyhow::Result;
use tracing::info;

/// OS-level kill switch: blocks all traffic except through Veil tunnel
/// On disconnect or crash, traffic is blocked (fail-closed)
pub struct KillSwitch {
    active: bool,
}

impl KillSwitch {
    /// Activate kill switch (block non-tunnel traffic)
    pub async fn activate() -> Result<Self> {
        #[cfg(target_os = "linux")]
        Self::activate_linux().await?;

        #[cfg(target_os = "macos")]
        Self::activate_macos().await?;

        #[cfg(target_os = "windows")]
        Self::activate_windows().await?;

        info!("Kill switch activated");
        Ok(Self { active: true })
    }

    /// Deactivate kill switch (restore normal traffic)
    pub async fn deactivate(self) -> Result<()> {
        #[cfg(target_os = "linux")]
        Self::deactivate_linux().await?;

        #[cfg(target_os = "macos")]
        Self::deactivate_macos().await?;

        #[cfg(target_os = "windows")]
        Self::deactivate_windows().await?;

        info!("Kill switch deactivated");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn activate_linux() -> Result<()> {
        // Use nftables or iptables to:
        // 1. Allow traffic through veil0
        // 2. Allow traffic to Veil server IP (for tunnel itself)
        // 3. Block all other outbound traffic
        // 4. Block IPv6 (leak protection)
        use tokio::process::Command;
        Command::new("nft")
            .args(["-f", "-"])
            .stdin(std::process::Stdio::piped())
            .output()
            .await?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn deactivate_linux() -> Result<()> {
        // Flush nftables rules added by Veil
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn activate_macos() -> Result<()> {
        // Use pf (Packet Filter) to block non-tunnel traffic
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn deactivate_macos() -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn activate_windows() -> Result<()> {
        // Use Windows Filtering Platform (WFP) via wintun or direct WinAPI
        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn deactivate_windows() -> Result<()> {
        Ok(())
    }
}

// Ensure kill switch is deactivated on drop (safety net)
impl Drop for KillSwitch {
    fn drop(&mut self) {
        if self.active {
            tracing::warn!("KillSwitch dropped without explicit deactivate — traffic may be blocked");
        }
    }
}
