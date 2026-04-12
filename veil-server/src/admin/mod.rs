use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc};
use tracing::info;

use crate::auth::AuthManager;
use crate::config::ServerConfig;

pub struct AdminServer {
    config: Arc<ServerConfig>,
    auth: Arc<AuthManager>,
}

#[derive(Clone)]
struct AppState {
    config: Arc<ServerConfig>,
    auth: Arc<AuthManager>,
}

impl AdminServer {
    pub fn new(config: Arc<ServerConfig>, auth: Arc<AuthManager>) -> Self {
        Self { config, auth }
    }

    pub async fn run(self) -> Result<()> {
        let state = AppState {
            config: self.config.clone(),
            auth: self.auth.clone(),
        };

        let admin_token = self.config.admin.admin_token.clone();

        let app = Router::new()
            .route("/api/v1/status", get(status_handler))
            .route("/api/v1/sessions", get(list_sessions))
            .route("/api/v1/sessions/:id", delete(kill_session))
            .route("/api/v1/tokens", post(create_token))
            .route("/api/v1/invite", post(create_invite))
            .route("/api/v1/reload", post(reload_config))
            .route("/api/v1/health", get(health_handler))
            .layer(middleware::from_fn_with_state(
                admin_token.clone(),
                admin_auth_middleware,
            ))
            .with_state(state);

        let addr: SocketAddr =
            format!("{}:{}", self.config.admin.bind, self.config.admin.port).parse()?;

        info!(addr = %addr, "Admin API listening");

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn admin_auth_middleware(
    State(token): State<String>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> impl IntoResponse {
    let auth_header = headers.get("X-Admin-Token").and_then(|v| v.to_str().ok());

    match auth_header {
        Some(t) if t == token => next.run(request).await.into_response(),
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response(),
    }
}

async fn health_handler() -> Json<Value> {
    Json(json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}))
}

async fn status_handler(State(state): State<AppState>) -> Json<Value> {
    let session_count = state.auth.active_session_count().await;
    Json(json!({
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
        "sessions": session_count,
        "role": state.config.node.role,
    }))
}

async fn list_sessions(State(state): State<AppState>) -> Json<Value> {
    let count = state.auth.active_session_count().await;
    Json(json!({"count": count, "sessions": []}))
}

async fn kill_session(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    state.auth.remove_session(&id).await;
    Json(json!({"removed": id}))
}

async fn create_token(State(_state): State<AppState>) -> Json<Value> {
    use veil_core::crypto::generate_token;
    let token = generate_token(32);
    // In production: hash and persist to config
    Json(json!({"token": token, "note": "Add SHA-256 hash to server config tokens list"}))
}

async fn create_invite(State(state): State<AppState>) -> Json<Value> {
    let ttl = state.config.auth.invite_ttl_seconds;
    let invite = state.auth.generate_invite(ttl);
    Json(json!({"invite_token": invite, "ttl_seconds": ttl}))
}

async fn reload_config(State(_state): State<AppState>) -> Json<Value> {
    // Hot reload: re-read config file and apply changes without restart
    // Full implementation would signal the server to reload
    Json(json!({"status": "reload_scheduled"}))
}
