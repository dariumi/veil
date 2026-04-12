# Contributing to Veil

Thank you for your interest in contributing!

## Development Setup

### Prerequisites

- Rust 1.82+ — [rustup.rs](https://rustup.rs)
- Docker (for server testing)
- `pkg-config`, `libssl-dev`, `libssh2-1-dev` (Linux)
- Xcode CLI Tools (macOS)

```bash
# Clone
git clone https://github.com/YOUR_ORG/veil.git
cd veil

# Build all crates
cargo build

# Run tests
cargo test

# Run server (needs config)
cp veil-server/config.example.toml /tmp/server.toml
# edit /tmp/server.toml ...
cargo run -p veil-server -- --config /tmp/server.toml

# Run client
cargo run -p veil-client -- --help

# Run desktop app (dev mode)
cd veil-app && npm run dev
```

## Project Structure

```
veil-core/       Shared protocol types, crypto, config schemas
veil-server/     Server binary + Docker
veil-client/     CLI client + SSH deployment tool
veil-app/        Tauri desktop application
```

## Code Style

- `cargo fmt` before committing
- `cargo clippy -- -D warnings` must pass
- No `unwrap()` in production paths — use `?` or handle errors explicitly
- No custom cryptography — use existing primitives from `ring` / `rustls`
- Privacy first: never log payloads, destinations, or user identifiers

## Branches

| Branch  | Purpose                     |
|---------|-----------------------------|
| `main`  | Stable, triggers Docker push |
| `dev`   | Active development          |
| `feat/` | Feature branches            |
| `fix/`  | Bug fix branches            |

## Pull Request Checklist

- [ ] `cargo fmt --all` passed
- [ ] `cargo clippy` passed  
- [ ] Tests added/updated
- [ ] No hardcoded secrets or real IPs
- [ ] CHANGELOG updated (for user-visible changes)

## Security

Do **not** submit security vulnerabilities as pull requests. See [SECURITY.md](SECURITY.md).
