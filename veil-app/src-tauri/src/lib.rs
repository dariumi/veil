use std::sync::{Arc, Mutex};
use tauri::Manager;

mod commands;
mod state;
#[cfg(desktop)]
mod tray;

#[cfg(target_os = "android")]
mod vpn_android;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("veil=info")
        .compact()
        .init();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init());

    // Autostart is desktop-only (no background agents on mobile)
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ));
    }

    builder
        .manage(Arc::new(Mutex::new(AppState::default())))
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::connect,
            commands::disconnect,
            commands::get_profiles,
            commands::add_profile,
            commands::delete_profile,
            commands::get_server_info,
            commands::server_add_user,
            commands::server_list_users,
            commands::server_get_sessions,
            // Desktop: SSH server deployment
            #[cfg(desktop)]
            commands::deploy_server,
            // Android: VPN service control
            #[cfg(target_os = "android")]
            commands::start_vpn,
            #[cfg(target_os = "android")]
            commands::stop_vpn,
        ])
        .setup(|app| {
            #[cfg(desktop)]
            tray::setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide to tray on close (desktop only — mobile has no tray)
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
            #[cfg(mobile)]
            let _ = (window, event);
        })
        .run(tauri::generate_context!())
        .expect("error while running Veil");
}
