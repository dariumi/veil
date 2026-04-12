use anyhow::Result;
use tracing::info;

use crate::killswitch::KillSwitch;
use crate::transport::VeilConnection;
use crate::tunnel::TunDevice;

/// Run in full VPN (TUN) mode
pub async fn run(server: &str, token: &str, profile: &str) -> Result<()> {
    println!("Starting VPN mode...");

    // Activate kill switch before connecting (fail-closed)
    let ks = KillSwitch::activate().await?;
    info!("Kill switch activated");

    // Connect to Veil server
    let conn = VeilConnection::connect(server, token, profile).await?;
    info!(server = %server, "Connected to Veil server");

    println!("Connected. Setting up tunnel...");

    // Create TUN device
    let tun = TunDevice::create("veil0").await?;
    tun.configure("10.10.0.2".parse()?, "10.10.0.1".parse()?, 24)
        .await?;

    println!("VPN tunnel active: 10.10.0.2/24");
    println!("Press Ctrl+C to disconnect");

    // Pump packets between TUN device and Veil connection
    tokio::select! {
        res = tun.run(conn) => res?,
        _ = tokio::signal::ctrl_c() => {
            println!("\nDisconnecting...");
        }
    }

    // Deactivate kill switch (restore connectivity)
    ks.deactivate().await?;
    info!("Kill switch deactivated");

    Ok(())
}
