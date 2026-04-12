use tauri::{AppHandle, Manager, State};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;

mod tray;
mod state;
mod commands;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("veil=info")
        .compact()
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .manage(Arc::new(Mutex::new(AppState::default())))
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::connect,
            commands::disconnect,
            commands::get_profiles,
            commands::add_profile,
            commands::delete_profile,
            commands::deploy_server,
            commands::get_server_info,
            commands::server_add_user,
            commands::server_list_users,
            commands::server_get_sessions,
        ])
        .setup(|app| {
            tray::setup_tray(app)?;
            // Hide window to tray on startup if autostart
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide to tray instead of closing
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Veil");
}
