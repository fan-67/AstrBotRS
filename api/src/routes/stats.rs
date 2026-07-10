use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::State, Json, Router};
use serde_json::json;

use crate::app::AppState;

static START_TIME: std::sync::OnceLock<u64> = std::sync::OnceLock::new();

async fn get_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let start = *START_TIME.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Json(json!({
        "status": "running",
        "providers": state.provider_mgr.len().await,
        "uptime_secs": now.saturating_sub(start),
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", axum::routing::get(get_stats))
}
