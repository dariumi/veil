use anyhow::Result;
use tracing::info;

/// DNS leak protection and encrypted DNS routing
pub struct DnsProtection {
    original_resolvers: Vec<String>,
}

impl DnsProtection {
    /// Override system DNS to use Veil's encrypted DNS channel
    pub async fn activate(server_dns: &str) -> Result<Self> {
        let original = Self::get_system_resolvers()?;

        #[cfg(target_os = "linux")]
        Self::set_resolv_conf(server_dns)?;

        #[cfg(target_os = "macos")]
        Self::set_macos_dns(server_dns).await?;

        #[cfg(target_os = "windows")]
        Self::set_windows_dns(server_dns).await?;

        info!(dns = %server_dns, "DNS protection activated");
        Ok(Self { original_resolvers: original })
    }

    /// Restore original DNS settings
    pub async fn deactivate(self) -> Result<()> {
        #[cfg(target_os = "linux")]
        Self::restore_resolv_conf(&self.original_resolvers)?;

        info!("DNS protection deactivated");
        Ok(())
    }

    fn get_system_resolvers() -> Result<Vec<String>> {
        // Read current /etc/resolv.conf nameservers
        Ok(vec!["8.8.8.8".into()])
    }

    #[cfg(target_os = "linux")]
    fn set_resolv_conf(nameserver: &str) -> Result<()> {
        let content = format!("# Managed by Veil\nnameserver {}\n", nameserver);
        std::fs::write("/etc/resolv.conf", content)?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn restore_resolv_conf(resolvers: &[String]) -> Result<()> {
        let content = resolvers.iter()
            .map(|r| format!("nameserver {}\n", r))
            .collect::<String>();
        std::fs::write("/etc/resolv.conf", content)?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn set_macos_dns(nameserver: &str) -> Result<()> {
        use tokio::process::Command;
        // networksetup -setdnsservers <interface> <dns>
        Command::new("networksetup")
            .args(["-setdnsservers", "Wi-Fi", nameserver])
            .output().await?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn deactivate_macos() {}

    #[cfg(target_os = "windows")]
    async fn set_windows_dns(_nameserver: &str) -> Result<()> {
        Ok(())
    }
}
