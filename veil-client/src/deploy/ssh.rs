use anyhow::{bail, Result};
use ssh2::Session;
use std::{
    io::Read,
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
};
use tracing::debug;

/// SSH client wrapper for server deployment
pub struct SshClient {
    session: Session,
}

impl SshClient {
    /// Connect with explicit credentials
    pub async fn connect(
        host: &str,
        port: u16,
        user: &str,
        password: Option<&str>,
        key_path: Option<&PathBuf>,
    ) -> Result<Self> {
        let addr = format!("{}:{}", host, port);
        let tcp = TcpStream::connect(&addr)
            .map_err(|e| anyhow::anyhow!("SSH connect to {}: {}", addr, e))?;

        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;

        // Authenticate
        if let Some(pass) = password {
            session.userauth_password(user, pass)?;
        } else if let Some(key) = key_path {
            session.userauth_pubkey_file(user, None, key, None)?;
        } else {
            // Try SSH agent
            let mut agent = session.agent()?;
            agent.connect()?;
            agent.list_identities()?;
            let identities: Vec<_> = agent.identities()?.collect::<std::result::Result<_, _>>()?;
            let mut authed = false;
            for identity in &identities {
                if agent.userauth(user, identity).is_ok() {
                    authed = true;
                    break;
                }
            }
            if !authed {
                bail!("SSH authentication failed: no valid credentials. Use --password or --key");
            }
        }

        if !session.authenticated() {
            bail!("SSH authentication failed");
        }

        debug!(host = %host, user = %user, "SSH authenticated");
        Ok(Self { session })
    }

    /// Interactive connect: prompt for password if no key works
    pub async fn connect_interactive(host: &str, port: u16, user: &str) -> Result<Self> {
        // Try SSH agent first, then prompt
        let password = rpassword::prompt_password(format!("SSH password for {}@{}: ", user, host))
            .unwrap_or_default();
        Self::connect(host, port, user, Some(&password), None).await
    }

    /// Execute a command and return stdout
    pub async fn run(&mut self, cmd: &str) -> Result<String> {
        debug!(cmd = %cmd, "SSH exec");
        let mut channel = self.session.channel_session()?;
        channel.exec(cmd)?;

        let mut output = String::new();
        channel.read_to_string(&mut output)?;
        channel.wait_close()?;

        let exit_code = channel.exit_status()?;
        if exit_code != 0 {
            bail!("Command `{}` exited with code {}", cmd, exit_code);
        }

        Ok(output)
    }

    /// Write a file to the remote machine via SFTP
    pub async fn write_file(&mut self, remote_path: &str, content: &[u8]) -> Result<()> {
        use std::io::Write;
        let sftp = self.session.sftp()?;
        let mut file = sftp.create(std::path::Path::new(remote_path))?;
        file.write_all(content)?;
        debug!(path = %remote_path, "File written via SFTP");
        Ok(())
    }

    /// Upload a local file to remote
    pub async fn upload_file(&mut self, local_path: &PathBuf, remote_path: &str) -> Result<()> {
        let content = std::fs::read(local_path)?;
        self.write_file(remote_path, &content).await
    }
}
