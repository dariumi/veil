// veil-client library interface — exposes what veil-app (Tauri) and the shared
// connection modes need. The CLI-specific modules remain binary-only.

pub mod killswitch;
pub mod transport;
pub mod tunnel;
pub mod modes;
