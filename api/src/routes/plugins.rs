use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
    Router,
};
use serde_json::json;

use crate::app::AppState;

async fn list_plugins(State(state): State<AppState>) -> impl IntoResponse {
    let mgr = state.plugin_mgr.read().await;
    let plugins: Vec<_> = mgr
        .list()
        .into_iter()
        .map(|p| {
            json!({
                "name": p.name(),
                "description": p.description(),
            })
        })
        .collect();
    (StatusCode::OK, Json(plugins))
}

async fn get_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let mgr = state.plugin_mgr.read().await;
    for p in mgr.list() {
        if p.name() == name {
            return (
                StatusCode::OK,
                Json(json!({
                    "name": p.name(),
                    "description": p.description(),
                })),
            );
        }
    }
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": "plugin not found"})),
    )
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(list_plugins))
        .route("/{name}", axum::routing::get(get_plugin))
}
