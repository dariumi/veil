use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;

mod config;
mod deploy;
mod dns;
mod killswitch;
mod modes;
mod transport;
mod tunnel;
mod tui;

#[derive(Parser, Debug)]
#[command(
    name = "veil",
    about = "Veil VPN/Proxy Client — connect or deploy your own server",
    version,
    long_about = None,
)]
struct Cli {
    /// Config file path
    #[arg(short, long, default_value = "~/.config/veil/config.toml")]
    config: PathBuf,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Connect to a Veil server (VPN or proxy mode)
    Connect {
        /// Server address (host:port) or profile name
        server: Option<String>,
        /// Authentication token
        #[arg(short, long)]
        token: Option<String>,
        /// Use proxy mode instead of VPN (TUN) mode
        #[arg(long)]
        proxy: bool,
        /// Traffic profile: balanced, realtime, throughput, stealth
        #[arg(long, default_value = "balanced")]
        profile: String,
    },

    /// Disconnect from the current server
    Disconnect,

    /// Show connection status
    Status,

    /// Deploy a self-hosted Veil server via SSH
    Deploy {
        #[command(subcommand)]
        action: DeployCommands,
    },

    /// Manage your self-hosted server (users, config, etc.)
    Server {
        #[command(subcommand)]
        action: ServerCommands,
    },

    /// Interactive setup wizard (first-time setup)
    Setup,

    /// Show/edit client configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigCommands>,
    },
}

#[derive(Subcommand, Debug)]
enum DeployCommands {
    /// Install Veil server on a remote machine via SSH
    Install {
        /// Remote server address (user@host or host)
        host: String,
        /// SSH port
        #[arg(short = 'p', long, default_value = "22")]
        ssh_port: u16,
        /// SSH password (or use --key)
        #[arg(long)]
        password: Option<String>,
        /// SSH private key path
        #[arg(long)]
        key: Option<PathBuf>,
        /// Veil port to listen on
        #[arg(long, default_value = "443")]
        veil_port: u16,
        /// Domain for TLS SNI (optional)
        #[arg(long)]
        domain: Option<String>,
    },

    /// Uninstall Veil server from a remote machine
    Uninstall { host: String },

    /// Check server status via SSH
    Status { host: String },

    /// Update server to the latest version
    Update { host: String },
}

#[derive(Subcommand, Debug)]
enum ServerCommands {
    /// List users / tokens
    ListUsers,
    /// Create a new user token
    AddUser {
        label: String,
        #[arg(long)]
        admin: bool,
        #[arg(long)]
        expires: Option<String>,
    },
    /// Remove a user token
    RemoveUser { id: String },
    /// Generate a short-lived invite link
    Invite,
    /// Show active sessions
    Sessions,
    /// Change server port
    SetPort { port: u16 },
    /// Reload server configuration
    Reload,
    /// Show server logs (last N lines)
    Logs {
        #[arg(short, long, default_value = "50")]
        lines: u32,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommands {
    /// Show current config
    Show,
    /// Set a config value
    Set { key: String, value: String },
    /// Reset to defaults
    Reset,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("veil={}", log_level).parse()?)
        )
        .compact()
        .init();

    match cli.command {
        Commands::Connect { server, token, proxy, profile } => {
            modes::connect(server, token, proxy, &profile).await?;
        }

        Commands::Disconnect => {
            modes::disconnect().await?;
        }

        Commands::Status => {
            modes::status().await?;
        }

        Commands::Deploy { action } => {
            deploy::handle(action).await?;
        }

        Commands::Server { action } => {
            let cfg = config::ClientConfig::load_or_default(&cli.config)?;
            let server_url = cfg.management_url()
                .ok_or_else(|| anyhow::anyhow!(
                    "No server configured. Run `veil deploy install` first."
                ))?;
            deploy::server_manage(action, &server_url, &cfg.admin_token()?).await?;
        }

        Commands::Setup => {
            tui::setup_wizard().await?;
        }

        Commands::Config { action } => {
            config::handle_config_command(action, &cli.config)?;
        }
    }

    Ok(())
}
