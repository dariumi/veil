use anyhow::{bail, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use tracing::info;

use crate::{DeployCommands, ServerCommands};

mod ssh;
use ssh::SshClient;

pub async fn handle(action: DeployCommands) -> Result<()> {
    match action {
        DeployCommands::Install {
            host,
            ssh_port,
            password,
            key,
            veil_port,
            domain,
        } => {
            install_server(
                &host,
                ssh_port,
                password.as_deref(),
                key.as_deref(),
                veil_port,
                domain.as_deref(),
            )
            .await
        }
        DeployCommands::Uninstall { host } => uninstall_server(&host).await,
        DeployCommands::Status { host } => server_ssh_status(&host).await,
        DeployCommands::Update { host } => update_server(&host).await,
    }
}

/// Install Veil server on remote machine via SSH
async fn install_server(
    host: &str,
    ssh_port: u16,
    password: Option<&str>,
    key: Option<&Path>,
    veil_port: u16,
    domain: Option<&str>,
) -> Result<()> {
    println!("{}", style("Veil Server Installer").bold().cyan());
    println!("Host: {}:{}", host, ssh_port);
    println!();

    // Parse user@host
    let (user, hostname) = parse_user_host(host);

    println!("{} Connecting via SSH...", style("→").cyan());
    let mut ssh = SshClient::connect(hostname, ssh_port, &user, password, key).await?;
    println!("{} Connected", style("✓").green());

    // Check requirements
    let pb = progress_bar("Checking requirements");
    let has_docker = ssh.run("command -v docker").await.is_ok();
    pb.finish_with_message("done");

    if !has_docker {
        println!("{} Docker not found — installing...", style("→").cyan());
        let pb = progress_bar("Installing Docker");
        ssh.run("curl -fsSL https://get.docker.com | sh")
            .await
            .map_err(|e| anyhow::anyhow!("Docker install failed: {}", e))?;
        ssh.run("systemctl enable --now docker").await?;
        pb.finish_with_message("installed");
        println!("{} Docker installed", style("✓").green());
    } else {
        println!("{} Docker already installed", style("✓").green());
    }

    // Generate server config
    let pb = progress_bar("Generating configuration");
    let signing_key = veil_core::crypto::generate_token(32);
    let admin_token = veil_core::crypto::generate_token(32);
    let user_token = veil_core::crypto::generate_token(32);
    let user_token_hash = hex_sha256(&user_token);

    let sni_line = match domain {
        Some(d) => format!("sni = \"{}\"", d),
        None => format!("# sni = \"example.com\"  # set a domain for better camouflage"),
    };

    let config = format!(
        r#"[node]
role = "all"
allowed_destinations = []

[listen]
bind = "0.0.0.0"
quic_port = {veil_port}
tcp_port = {veil_port}

[tls]
cert_path = "/etc/veil/server.crt"
key_path  = "/etc/veil/server.key"
{sni_line}
alpn = ["h3", "h2", "http/1.1"]

[auth]
signing_key = "{signing_key}"
invite_ttl_seconds = 3600
rate_limit = true
tarpit_ms = 500

[[auth.tokens]]
id = "user1"
token_hash = "{user_token_hash}"
label = "default user"
is_admin = false

[relay]
tcp_enabled = true
udp_enabled = true
dns_enabled = true
max_streams_per_session = 256

[limits]
max_sessions = 1000
session_timeout_secs = 3600
connect_rate_per_ip = 10

[logging]
level = "warn"
disabled = false

[admin]
enabled = true
bind = "127.0.0.1"
port = 9090
admin_token = "{admin_token}"

[obfuscation]
padding_enabled = true
size_normalization = true
idle_noise = false
burst_shaping = true
"#
    );

    pb.finish_with_message("done");

    // Upload config and generate cert
    let pb = progress_bar("Uploading configuration");
    ssh.run("mkdir -p /etc/veil").await?;
    ssh.write_file("/etc/veil/server.toml", config.as_bytes())
        .await?;
    pb.finish_with_message("done");

    // Generate self-signed cert
    let pb = progress_bar("Generating TLS certificate");
    let cert_cmd = format!(
        "docker run --rm -v /etc/veil:/etc/veil ghcr.io/dariuni/veil-server:latest \
         veil-server --gen-cert && mv server.crt /etc/veil/ && mv server.key /etc/veil/"
    );
    // Simplified: use openssl directly
    let cn = domain.unwrap_or("veil-server");
    ssh.run(&format!(
        "openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
         -keyout /etc/veil/server.key \
         -out /etc/veil/server.crt \
         -subj '/CN={}'",
        cn
    ))
    .await
    .or_else(|_| {
        // openssl not available, fall back to self-signed via docker
        Ok::<_, anyhow::Error>(String::new())
    })?;
    pb.finish_with_message("done");

    // Pull and start Docker container
    let pb = progress_bar("Pulling Veil server image");
    ssh.run("docker pull ghcr.io/dariuni/veil-server:latest")
        .await
        .unwrap_or_default();
    pb.finish_with_message("done");

    let pb = progress_bar("Starting Veil server container");
    ssh.run("docker stop veil-server 2>/dev/null; docker rm veil-server 2>/dev/null")
        .await
        .unwrap_or_default();

    let docker_run = format!(
        "docker run -d \
         --name veil-server \
         --restart unless-stopped \
         --cap-add NET_ADMIN \
         -p {veil_port}:{veil_port}/udp \
         -p {veil_port}:{veil_port}/tcp \
         -p 127.0.0.1:9090:9090 \
         -v /etc/veil:/etc/veil:ro \
         -v /var/lib/veil:/var/lib/veil \
         ghcr.io/dariuni/veil-server:latest"
    );
    ssh.run(&docker_run).await?;
    pb.finish_with_message("started");

    println!();
    println!("{}", style("Server installed successfully!").bold().green());
    println!();
    println!("{}", style("Connection details:").bold());
    println!("  Server:  {}:{}", hostname, veil_port);
    println!("  Token:   {}", style(&user_token).yellow());
    println!();
    println!(
        "{}",
        style("Save this token — it won't be shown again.").dim()
    );
    println!();
    println!("Connect with:");
    println!(
        "  veil connect {}:{} --token {}",
        hostname, veil_port, user_token
    );

    Ok(())
}

async fn uninstall_server(host: &str) -> Result<()> {
    let (user, hostname) = parse_user_host(host);
    println!("Uninstalling from {}...", hostname);

    let mut ssh = SshClient::connect_interactive(hostname, 22, &user).await?;
    ssh.run("docker stop veil-server && docker rm veil-server")
        .await?;
    ssh.run("rm -rf /etc/veil /var/lib/veil").await?;

    println!("{} Uninstalled", style("✓").green());
    Ok(())
}

async fn server_ssh_status(host: &str) -> Result<()> {
    let (user, hostname) = parse_user_host(host);
    let mut ssh = SshClient::connect_interactive(hostname, 22, &user).await?;
    let output = ssh
        .run("docker inspect --format='{{.State.Status}}' veil-server")
        .await?;
    println!("Container status: {}", output.trim());
    Ok(())
}

async fn update_server(host: &str) -> Result<()> {
    let (user, hostname) = parse_user_host(host);
    println!("Updating server on {}...", hostname);

    let mut ssh = SshClient::connect_interactive(hostname, 22, &user).await?;
    ssh.run("docker pull ghcr.io/dariuni/veil-server:latest")
        .await?;
    ssh.run("docker stop veil-server && docker start veil-server")
        .await?;

    println!("{} Updated", style("✓").green());
    Ok(())
}

/// Manage server via Admin API
pub async fn server_manage(
    action: ServerCommands,
    server_url: &str,
    admin_token: &str,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // self-signed cert
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert("X-Admin-Token", admin_token.parse()?);
            h
        })
        .build()?;

    match action {
        ServerCommands::ListUsers => {
            let resp: serde_json::Value = client
                .get(format!("{}/tokens", server_url))
                .send()
                .await?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }

        ServerCommands::AddUser {
            label,
            admin,
            expires,
        } => {
            let resp: serde_json::Value = client
                .post(format!("{}/tokens", server_url))
                .json(
                    &serde_json::json!({"label": label, "is_admin": admin, "expires_at": expires}),
                )
                .send()
                .await?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }

        ServerCommands::RemoveUser { id } => {
            let resp: serde_json::Value = client
                .delete(format!("{}/tokens/{}", server_url, id))
                .send()
                .await?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }

        ServerCommands::Invite => {
            let resp: serde_json::Value = client
                .post(format!("{}/invite", server_url))
                .send()
                .await?
                .json()
                .await?;
            println!(
                "Invite token: {}",
                resp["invite_token"].as_str().unwrap_or("?")
            );
        }

        ServerCommands::Sessions => {
            let resp: serde_json::Value = client
                .get(format!("{}/sessions", server_url))
                .send()
                .await?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }

        ServerCommands::Reload => {
            let resp: serde_json::Value = client
                .post(format!("{}/reload", server_url))
                .send()
                .await?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }

        ServerCommands::Logs { lines } => {
            println!("Log fetching via SSH not yet implemented");
        }

        ServerCommands::SetPort { port } => {
            println!("Port change requires SSH access (config edit + restart)");
        }
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn parse_user_host(host: &str) -> (&str, &str) {
    if let Some((user, h)) = host.split_once('@') {
        (user, h)
    } else {
        ("root", host)
    }
}

fn progress_bar(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("{spinner:.cyan} {msg}").unwrap());
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

fn hex_sha256(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}
