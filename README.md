# Veil

**Censorship-resistant VPN and proxy with HTTP/3 camouflage**

Veil is a privacy-focused network tunnel that disguises traffic as ordinary HTTPS/HTTP3, making it resistant to deep packet inspection (DPI) and active probing.

---

## Features

- **HTTP/3 camouflage** — traffic looks like normal HTTPS to DPI systems
- **QUIC + TLS 1.3** primary transport, automatic **TLS/TCP fallback**
- **VPN mode** (TUN) and **Proxy mode** (SOCKS5 + HTTP CONNECT)
- **Anti-probing** — server stays silent until a client authenticates
- **Kill switch** — OS-level firewall blocks traffic if tunnel drops
- **DNS leak protection** — all DNS over encrypted channel
- **Self-hosted** — deploy your own server in one command via SSH
- **Desktop app** — Tauri GUI for Windows, macOS, Linux

---

## Quick Start

### Option A — Connect to an existing server

```bash
veil connect myserver.com:443 --token YOUR_TOKEN
```

For proxy mode (no TUN/root required):
```bash
veil connect myserver.com:443 --token YOUR_TOKEN --proxy
# SOCKS5: 127.0.0.1:1080
# HTTP:   127.0.0.1:8080
```

### Option B — Deploy your own server

```bash
# Install Veil server on a remote VPS via SSH
veil deploy install root@1.2.3.4 --veil-port 443 --domain example.com

# The command will print your access token after installation
```

Requirements for the remote server: **Linux** (Ubuntu 20+, Debian 11+, Rocky 8+) with SSH access. Docker will be installed automatically if missing.

### Desktop App

Download the latest release for your platform from [Releases](../../releases).

---

## Installation

### Pre-built binaries

Download from [Releases](../../releases):
- `veil_linux_amd64.deb` / `.AppImage`
- `veil_macos.dmg`
- `veil_windows.msi`

### Build from source

**Prerequisites:** Rust 1.82+, `libssl-dev`, `libssh2-1-dev` (Linux)

```bash
git clone https://github.com/YOUR_ORG/veil.git
cd veil
cargo build --release -p veil-client
./target/release/veil --help
```

---

## Server

### Docker (recommended)

```bash
# Generate config
cp veil-server/config.example.toml /etc/veil/server.toml
# Edit /etc/veil/server.toml — set signing_key, admin_token, TLS paths

# Generate self-signed TLS cert
veil-server --gen-cert  # creates server.crt and server.key

# Run
docker run -d \
  --name veil-server \
  --restart unless-stopped \
  --cap-add NET_ADMIN \
  -p 443:443/udp -p 443:443/tcp \
  -p 127.0.0.1:9090:9090 \
  -v /etc/veil:/etc/veil:ro \
  ghcr.io/YOUR_ORG/veil-server:latest
```

### Server configuration

See [`veil-server/config.example.toml`](veil-server/config.example.toml) for all options.

Key settings:

```toml
[tls]
cert_path = "/etc/veil/server.crt"
key_path  = "/etc/veil/server.key"
sni       = "example.com"          # camouflage domain

[auth]
signing_key = "..."                 # openssl rand -hex 32

[admin]
admin_token = "..."                 # separate admin credential
```

### Admin API

```bash
# Status
curl -H "X-Admin-Token: $ADMIN_TOKEN" https://localhost:9090/api/v1/status

# Active sessions
curl -H "X-Admin-Token: $ADMIN_TOKEN" https://localhost:9090/api/v1/sessions

# Create invite token
curl -X POST -H "X-Admin-Token: $ADMIN_TOKEN" https://localhost:9090/api/v1/invite

# Hot reload config
curl -X POST -H "X-Admin-Token: $ADMIN_TOKEN" https://localhost:9090/api/v1/reload
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Veil Client                                                     │
│                                                                  │
│  ┌──────────┐  ┌─────────────┐  ┌─────────────────────────────┐ │
│  │ GUI App  │  │  CLI (veil) │  │   Deploy Tool (SSH)         │ │
│  │ (Tauri)  │  │             │  │   installs Docker on VPS    │ │
│  └────┬─────┘  └──────┬──────┘  └─────────────────────────────┘ │
│       └────────────────┤                                         │
│                ┌───────▼────────┐                                │
│                │  veil-core     │ protocol / crypto / config     │
│                └───────┬────────┘                                │
│          ┌─────────────┤                                         │
│    QUIC/HTTP3     TLS/TCP fallback                               │
└──────────┼─────────────┼───────────────────────────────────────-┘
           │             │
           ▼             ▼
┌─────────────────────────────────────────────────────────────────┐
│  Veil Server (Docker)                                            │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │ QUIC/HTTP3   │  │  TLS/TCP     │  │  Admin REST API        │ │
│  │ listener     │  │  fallback    │  │  :9090 (localhost only) │ │
│  └──────┬───────┘  └──────┬───────┘  └────────────────────────┘ │
│         └─────────────────┤                                      │
│                   ┌───────▼──────────┐                          │
│                   │  Auth (token)    │ tarpit + rate limit       │
│                   └───────┬──────────┘                          │
│                   ┌───────▼──────────┐                          │
│                   │  Relay Engine    │ TCP streams + UDP dgrams  │
│                   └──────────────────┘                          │
└─────────────────────────────────────────────────────────────────┘
```

### Transport modes

| Mode | Protocol | Use case |
|------|----------|----------|
| **Primary** | QUIC + TLS 1.3 + HTTP/3 | Normal operation, best performance |
| **Fallback** | TLS 1.3 over TCP | UDP blocked, aggressive DPI |
| **WG-compat** | WireGuard-style | Roadmap |

### Traffic profiles

| Profile | Optimized for |
|---------|---------------|
| `balanced` | Web browsing, general use |
| `realtime` | VoIP, video calls, gaming |
| `throughput` | Downloads, backups, large files |
| `stealth` | Heavily censored networks |

---

## Project Structure

```
veil/
├── veil-core/          Shared: protocol frames, crypto, config types
├── veil-server/        Server binary + Dockerfile
│   └── config.example.toml
├── veil-client/        CLI client + SSH deployment tool
├── veil-app/           Tauri desktop application
│   ├── src/            HTML/CSS/JS frontend
│   └── src-tauri/      Rust backend (Tauri commands)
└── .github/workflows/  CI/CD: test, Docker push, app release
```

---

## Roadmap

- [x] Protocol core (QUIC/HTTP3 + TLS/TCP fallback)
- [x] Server (Docker, Admin API, token auth)
- [x] Client (SOCKS5, HTTP proxy, SSH deploy)
- [x] Desktop app (Tauri)
- [ ] TUN/VPN mode (full tunnel)
- [ ] Kill switch (nftables/pf/WFP)
- [ ] Mobile clients (iOS, Android)
- [ ] Multi-hop routing (2-hop, 3-hop)
- [ ] Zero-knowledge auth
- [ ] Browser extension
- [ ] External security audit

---

## Security

- All cryptography via `rustls` and `ring` — no custom crypto
- Server reveals nothing before successful authentication (anti-probing)
- Minimal logging by default — no payloads, no destinations, no user IDs
- Kill switch blocks all traffic if tunnel drops (fail-closed)

Found a vulnerability? See [SECURITY.md](SECURITY.md).

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)
