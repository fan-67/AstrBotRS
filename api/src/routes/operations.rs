use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router};
use serde_json::json;

use crate::app::AppState;
use astrbot_config_mgr::AstrBotConfig;

async fn reload_config(State(state): State<AppState>) -> impl IntoResponse {
    match AstrBotConfig::load(&state.config_path) {
        Ok(cfg) => {
            let mut w = state.config.write().await;
            *w = cfg;
            (StatusCode::OK, Json(json!({"status": "config reloaded"})))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

async fn health() -> impl IntoResponse {
    Json(json!({"status": "healthy"}))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/reload-config", axum::routing::post(reload_config))
        .route("/health", axum::routing::get(health))
}
