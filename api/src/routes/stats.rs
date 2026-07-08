use axum::{extract::State, Json, Router};
use serde_json::json;

use crate::app::AppState;

async fn get_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "running",
        "providers": state.provider_mgr.len(),
        "uptime_secs": 0,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", axum::routing::get(get_stats))
}
