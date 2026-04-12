use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{info, warn};

mod auth;
mod admin;
mod config;
mod obfuscation;
mod relay;
mod transport;

use config::ServerConfig;

#[derive(Parser, Debug)]
#[command(name = "veil-server", about = "Veil VPN/Proxy Server", version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "/etc/veil/server.toml")]
    config: PathBuf,

    /// Generate a new self-signed TLS certificate and exit
    #[arg(long)]
    gen_cert: bool,

    /// Generate a new access token and exit
    #[arg(long)]
    gen_token: bool,

    /// Validate config and exit
    #[arg(long)]
    check_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging (minimal by default for privacy)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("veil_server=info".parse()?)
        )
        .json()
        .init();

    let cli = Cli::parse();

    if cli.gen_cert {
        return generate_self_signed_cert();
    }

    let config = ServerConfig::load(&cli.config)?;

    if cli.check_config {
        config.validate()?;
        println!("Configuration OK");
        return Ok(());
    }

    if cli.gen_token {
        let token = veil_core::crypto::generate_token(32);
        println!("Access token: {}", token);
        return Ok(());
    }

    config.validate()?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        mode = ?config.node.role,
        "Veil Server starting"
    );

    let server = transport::Server::new(config).await?;
    server.run().await?;

    Ok(())
}

fn generate_self_signed_cert() -> Result<()> {
    use rcgen::{CertificateParams, DistinguishedName, DnType, SanType};
    use std::fs;

    let mut params = CertificateParams::default();
    params.distinguished_name = {
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "veil-server");
        dn
    };
    params.subject_alt_names = vec![
        SanType::DnsName("localhost".try_into()?),
        SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
    ];

    let key_pair = rcgen::KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    fs::write("server.crt", cert.pem())?;
    fs::write("server.key", key_pair.serialize_pem())?;

    println!("Generated server.crt and server.key");
    Ok(())
}
