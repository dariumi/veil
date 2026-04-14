// veil-client library interface — exposes only what veil-app (Tauri) needs.
// The CLI-specific modules (config, deploy, tui, killswitch) are binary-only.

pub mod transport;
pub mod tunnel;
pub mod modes;
