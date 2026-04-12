# Changelog

All notable changes will be documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)

## [Unreleased]

### Added
- Initial project structure (Rust workspace)
- `veil-core`: protocol types, frame format, handshake, session model, AEAD crypto, token manager
- `veil-server`: QUIC/HTTP3 listener, TLS/TCP fallback, TCP/UDP relay, token auth with tarpit, REST Admin API, obfuscation layer, Docker image
- `veil-client`: SOCKS5 proxy, HTTP CONNECT proxy, VPN (TUN) mode skeleton, SSH-based server deployment, server management via Admin API, interactive setup wizard
- `veil-app`: Tauri desktop application with 5-tab UI (Home, Servers, Deploy, Manage, Settings), system tray support
- GitHub Actions: CI for Linux/macOS/Windows, Docker image publish to ghcr.io, release automation
