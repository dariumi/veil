use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::state::{AppState, ClientMode, ConnectionStatus, ManagedServer, ServerProfile};

type AppStateRef = Arc<Mutex<AppState>>;

// ── Status ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StatusResponse {
    pub status: ConnectionStatus,
    pub active_profile: Option<String>,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub elapsed_secs: u64,
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppStateRef>) -> Result<StatusResponse, String> {
    let s = state.lock().unwrap();
    Ok(StatusResponse {
        status: s.status.clone(),
        active_profile: s.active_profile.clone(),
        bytes_up: s.bytes_up,
        bytes_down: s.bytes_down,
        elapsed_secs: s.elapsed_secs(),
    })
}

// ── Connection ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ConnectRequest {
    pub profile_id: String,
    pub mode: ClientMode,
}

#[tauri::command]
pub async fn connect(
    req: ConnectRequest,
    state: State<'_, AppStateRef>,
) -> Result<(), String> {
    {
        let mut s = state.lock().unwrap();
        s.status = ConnectionStatus::Connecting;
        s.active_profile = Some(req.profile_id.clone());
    }

    // Spawn connection in background
    // In full implementation: call veil_client::modes::connect(...)
    // and update state as connection succeeds/fails
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        // Simulate connection
        tracing::info!("Connected (stub)");
    });

    Ok(())
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppStateRef>) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    s.status = ConnectionStatus::Disconnected;
    s.active_profile = None;
    s.connected_since = None;
    s.bytes_up = 0;
    s.bytes_down = 0;
    Ok(())
}

// ── Profiles ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_profiles(state: State<'_, AppStateRef>) -> Result<Vec<ServerProfile>, String> {
    Ok(state.lock().unwrap().profiles.clone())
}

#[derive(Deserialize)]
pub struct AddProfileRequest {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
    pub domain: Option<String>,
    pub mode: ClientMode,
}

#[tauri::command]
pub async fn add_profile(
    req: AddProfileRequest,
    state: State<'_, AppStateRef>,
) -> Result<ServerProfile, String> {
    let profile = ServerProfile {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        host: req.host,
        port: req.port,
        token: req.token,
        domain: req.domain,
        mode: req.mode,
    };
    state.lock().unwrap().profiles.push(profile.clone());
    Ok(profile)
}

#[tauri::command]
pub async fn delete_profile(
    id: String,
    state: State<'_, AppStateRef>,
) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    s.profiles.retain(|p| p.id != id);
    Ok(())
}

// ── Android VPN ───────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
#[tauri::command]
pub async fn start_vpn(
    profile_id: String,
    state: State<'_, AppStateRef>,
) -> Result<(), String> {
    let (server, token) = {
        let s = state.lock().unwrap();
        let profile = s.profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or("Profile not found")?
            .clone();
        (
            format!("{}:{}", profile.host, profile.port),
            profile.token.clone(),
        )
    };

    {
        let mut s = state.lock().unwrap();
        s.status = ConnectionStatus::Connecting;
        s.active_profile = Some(profile_id);
    }

    let state_arc = state.inner().clone();
    tokio::spawn(async move {
        match crate::vpn_android::start(server, token).await {
            Ok(()) => {
                let mut s = state_arc.lock().unwrap();
                s.status = ConnectionStatus::Disconnected;
                s.active_profile = None;
                s.connected_since = None;
            }
            Err(e) => {
                tracing::error!(err = %e, "Android VPN error");
                let mut s = state_arc.lock().unwrap();
                s.status = ConnectionStatus::Error(e.to_string());
            }
        }
    });

    Ok(())
}

#[cfg(target_os = "android")]
#[tauri::command]
pub async fn stop_vpn(state: State<'_, AppStateRef>) -> Result<(), String> {
    crate::vpn_android::stop();
    let mut s = state.lock().unwrap();
    s.status = ConnectionStatus::Disconnected;
    s.active_profile = None;
    s.connected_since = None;
    s.bytes_up = 0;
    s.bytes_down = 0;
    Ok(())
}

// ── Server Deployment (desktop only — SSH not available on mobile) ────────────

#[derive(Deserialize)]
pub struct DeployRequest {
    pub host: String,
    pub ssh_port: u16,
    pub user: String,
    pub password: Option<String>,
    pub veil_port: u16,
    pub domain: Option<String>,
}

#[derive(Serialize)]
pub struct DeployResult {
    pub success: bool,
    pub token: String,
    pub admin_token: String,
    pub message: String,
}

#[cfg(desktop)]
#[tauri::command]
pub async fn deploy_server(
    req: DeployRequest,
    state: State<'_, AppStateRef>,
) -> Result<DeployResult, String> {
    // In full implementation: call veil_client::deploy::install_server(...)
    // This runs the SSH deployment pipeline
    tracing::info!(host = %req.host, "Deploying Veil server (stub)");

    let user_token = veil_core::crypto::generate_token(32);
    let admin_token = veil_core::crypto::generate_token(32);

    let managed = ManagedServer {
        host: req.host.clone(),
        port: req.veil_port,
        admin_token: admin_token.clone(),
        domain: req.domain,
    };
    state.lock().unwrap().managed_server = Some(managed);

    Ok(DeployResult {
        success: true,
        token: user_token,
        admin_token,
        message: format!("Server deployed on {}:{}", req.host, req.veil_port),
    })
}

// ── Server Management ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ServerInfo {
    pub status: String,
    pub version: String,
    pub sessions: u32,
    pub uptime_secs: u64,
}

#[tauri::command]
pub async fn get_server_info(state: State<'_, AppStateRef>) -> Result<ServerInfo, String> {
    let s = state.lock().unwrap();
    let _server = s.managed_server.as_ref()
        .ok_or("No managed server configured")?;

    // In full impl: HTTP GET to admin API
    Ok(ServerInfo {
        status: "running".into(),
        version: "0.1.0".into(),
        sessions: 0,
        uptime_secs: 0,
    })
}

#[derive(Deserialize)]
pub struct AddUserRequest {
    pub label: String,
    pub is_admin: bool,
}

#[derive(Serialize)]
pub struct UserToken {
    pub id: String,
    pub label: String,
    pub token: String,
    pub is_admin: bool,
}

#[tauri::command]
pub async fn server_add_user(
    req: AddUserRequest,
    state: State<'_, AppStateRef>,
) -> Result<UserToken, String> {
    let _s = state.lock().unwrap();
    let token = veil_core::crypto::generate_token(32);
    Ok(UserToken {
        id: Uuid::new_v4().to_string(),
        label: req.label,
        token,
        is_admin: req.is_admin,
    })
}

#[tauri::command]
pub async fn server_list_users(
    _state: State<'_, AppStateRef>,
) -> Result<Vec<UserToken>, String> {
    Ok(vec![])
}

#[derive(Serialize)]
pub struct Session {
    pub id: String,
    pub connected_at: String,
    pub bytes_up: u64,
    pub bytes_down: u64,
}

#[tauri::command]
pub async fn server_get_sessions(
    _state: State<'_, AppStateRef>,
) -> Result<Vec<Session>, String> {
    Ok(vec![])
}
