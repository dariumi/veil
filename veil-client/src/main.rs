use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod config;
mod deploy;
mod dns;
mod killswitch;
mod modes;
mod transport;
mod tui;
mod tunnel;

use deploy::{DeployCommands, ServerCommands};

#[derive(Parser, Debug)]
#[command(
    name = "veil",
    about = "Veil VPN/Proxy Client — connect or deploy your own server",
    version,
    long_about = None,
)]
struct Cli {
    #[arg(short, long, default_value = "~/.config/veil/config.toml")]
    config: PathBuf,
    #[arg(short, long)]
    verbose: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Connect {
        server: Option<String>,
        #[arg(short, long)] token: Option<String>,
        #[arg(long)] proxy: bool,
        #[arg(long, default_value = "balanced")] profile: String,
    },
    Disconnect,
    Status,
    Deploy { #[command(subcommand)] action: CliDeploy },
    Server { #[command(subcommand)] action: CliServer },
    Setup,
    Config { #[command(subcommand)] action: Option<CliConfig> },
}

#[derive(Subcommand, Debug)]
enum CliDeploy {
    Install {
        host: String,
        #[arg(short = 'p', long, default_value = "22")] ssh_port: u16,
        #[arg(long)] password: Option<String>,
        #[arg(long)] key: Option<PathBuf>,
        #[arg(long, default_value = "443")] veil_port: u16,
        #[arg(long)] domain: Option<String>,
    },
    Uninstall { host: String },
    Status { host: String },
    Update { host: String },
}

#[derive(Subcommand, Debug)]
enum CliServer {
    ListUsers,
    AddUser { label: String, #[arg(long)] admin: bool, #[arg(long)] expires: Option<String> },
    RemoveUser { id: String },
    Invite,
    Sessions,
    SetPort { port: u16 },
    Reload,
    Logs { #[arg(short, long, default_value = "50")] lines: u32 },
}

#[derive(Subcommand, Debug)]
enum CliConfig {
    Show,
    Set { key: String, value: String },
    Reset,
}

impl From<CliDeploy> for DeployCommands {
    fn from(c: CliDeploy) -> Self {
        match c {
            CliDeploy::Install { host, ssh_port, password, key, veil_port, domain } =>
                DeployCommands::Install { host, ssh_port, password, key, veil_port, domain },
            CliDeploy::Uninstall { host } => DeployCommands::Uninstall { host },
            CliDeploy::Status { host } => DeployCommands::Status { host },
            CliDeploy::Update { host } => DeployCommands::Update { host },
        }
    }
}

impl From<CliServer> for ServerCommands {
    fn from(c: CliServer) -> Self {
        match c {
            CliServer::ListUsers => ServerCommands::ListUsers,
            CliServer::AddUser { label, admin, expires } =>
                ServerCommands::AddUser { label, admin, expires },
            CliServer::RemoveUser { id } => ServerCommands::RemoveUser { id },
            CliServer::Invite => ServerCommands::Invite,
            CliServer::Sessions => ServerCommands::Sessions,
            CliServer::SetPort { port } => ServerCommands::SetPort { port },
            CliServer::Reload => ServerCommands::Reload,
            CliServer::Logs { lines } => ServerCommands::Logs { lines },
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("veil={}", log_level).parse()?),
        )
        .compact()
        .init();

    match cli.command {
        Commands::Connect { server, token, proxy, profile } => {
            modes::connect(server, token, proxy, &profile).await?;
        }
        Commands::Disconnect => modes::disconnect().await?,
        Commands::Status => modes::status().await?,
        Commands::Deploy { action } => deploy::handle(action.into()).await?,
        Commands::Server { action } => {
            let cfg = config::ClientConfig::load_or_default(&cli.config)?;
            let url = cfg.management_url()
                .ok_or_else(|| anyhow::anyhow!("No server configured"))?;
            deploy::server_manage(action.into(), &url, &cfg.admin_token()?).await?;
        }
        Commands::Setup => tui::setup_wizard().await?,
        Commands::Config { action } => {
            let a = action.map(|c| match c {
                CliConfig::Show => config::ConfigAction::Show,
                CliConfig::Set { key, value } => config::ConfigAction::Set { key, value },
                CliConfig::Reset => config::ConfigAction::Reset,
            });
            config::handle_config_command(a, &cli.config)?;
        }
    }

    Ok(())
}
