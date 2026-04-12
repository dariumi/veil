use anyhow::Result;
use tracing::info;

pub mod proxy;
pub mod vpn;

/// Connect to a Veil server
pub async fn connect(
    server: Option<String>,
    token: Option<String>,
    proxy_mode: bool,
    profile: &str,
) -> Result<()> {
    let server = server.unwrap_or_else(|| {
        // Load from active profile
        "127.0.0.1:443".to_string()
    });

    let token = token.unwrap_or_else(|| {
        std::env::var("VEIL_TOKEN").unwrap_or_default()
    });

    if token.is_empty() {
        anyhow::bail!("Token required. Use --token or set VEIL_TOKEN env var.");
    }

    println!("Connecting to {} ({})", server, if proxy_mode { "proxy" } else { "VPN" });

    if proxy_mode {
        proxy::run(&server, &token, profile).await
    } else {
        vpn::run(&server, &token, profile).await
    }
}

/// Disconnect from current server
pub async fn disconnect() -> Result<()> {
    // Signal the daemon to disconnect
    // Full implementation: write to IPC socket / PID file
    println!("Disconnecting...");
    Ok(())
}

/// Show connection status
pub async fn status() -> Result<()> {
    // Query local daemon status
    // Full implementation: read from IPC socket
    println!("Status: disconnected");
    Ok(())
}
