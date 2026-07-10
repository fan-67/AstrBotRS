use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router};
use serde_json::Value;

use crate::app::AppState;

async fn list_bots(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    let platforms: Vec<Value> = config
        .platform
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or_default())
        .collect();
    (StatusCode::OK, Json(platforms))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", axum::routing::get(list_bots))
}
