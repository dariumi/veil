use anyhow::Result;
use console::style;
use dialoguer::{Confirm, Input, Password, Select};

/// Interactive first-time setup wizard
pub async fn setup_wizard() -> Result<()> {
    println!();
    println!("{}", style("Welcome to Veil").bold().cyan());
    println!("{}", style("━".repeat(40)).dim());
    println!();

    let mode = Select::new()
        .with_prompt("How would you like to use Veil?")
        .items(&[
            "Connect to an existing server (I have a token)",
            "Deploy my own self-hosted server",
        ])
        .default(0)
        .interact()?;

    println!();

    match mode {
        0 => setup_connect_mode().await,
        1 => setup_deploy_mode().await,
        _ => unreachable!(),
    }
}

async fn setup_connect_mode() -> Result<()> {
    println!("{}", style("Client Setup").bold());
    println!();

    let server: String = Input::new()
        .with_prompt("Server address (host:port)")
        .interact_text()?;

    let token: String = Password::new()
        .with_prompt("Access token")
        .interact()?;

    let profile = Select::new()
        .with_prompt("Traffic profile")
        .items(&["Balanced", "Realtime (low latency)", "Throughput", "Stealth"])
        .default(0)
        .interact()?;

    let profile_name = ["balanced", "realtime", "throughput", "stealth"][profile];

    println!();
    println!("{}", style("Configuration saved.").green());
    println!();
    println!("Connect with:");
    println!("  veil connect {} --token {} --profile {}", server, token, profile_name);

    Ok(())
}

async fn setup_deploy_mode() -> Result<()> {
    println!("{}", style("Server Deployment Setup").bold());
    println!();
    println!("You'll need a Linux server with SSH access.");
    println!("Supported: Ubuntu 20+, Debian 11+, Rocky Linux 8+");
    println!();

    let host: String = Input::new()
        .with_prompt("Server address (user@host or host)")
        .interact_text()?;

    let port: u16 = Input::new()
        .with_prompt("Veil server port")
        .default("443".into())
        .interact_text::<String>()?
        .parse()
        .unwrap_or(443);

    let domain: String = Input::new()
        .with_prompt("Domain name (optional, for SNI camouflage)")
        .allow_empty(true)
        .interact_text()?;

    let use_key = Confirm::new()
        .with_prompt("Use SSH key file?")
        .default(false)
        .interact()?;

    let key_path = if use_key {
        let k: String = Input::new()
            .with_prompt("SSH key path")
            .default("~/.ssh/id_rsa".into())
            .interact_text()?;
        Some(k)
    } else {
        None
    };

    println!();
    println!("{}", style("Ready to deploy!").bold());
    println!();
    println!("Run:");
    if let Some(key) = key_path {
        println!("  veil deploy install {} --veil-port {} --key {}", host, port,
            if domain.is_empty() { String::new() } else { format!("--domain {} ", domain) } + &key
        );
    } else {
        let domain_arg = if domain.is_empty() { String::new() } else { format!(" --domain {}", domain) };
        println!("  veil deploy install {} --veil-port {}{}", host, port, domain_arg);
    }

    Ok(())
}
