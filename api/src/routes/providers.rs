use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router};
use serde_json::Value;

use crate::app::AppState;

async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    let providers: Vec<Value> = config
        .provider
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or_default())
        .collect();
    (StatusCode::OK, Json(providers))
}

async fn list_provider_instances(State(state): State<AppState>) -> impl IntoResponse {
    let instances: Vec<Value> = state
        .provider_mgr
        .list_providers()
        .into_iter()
        .map(|(_id, p)| {
            let m = p.meta();
            serde_json::json!({
                "id": m.id,
                "model": m.model,
                "type": m.provider_type,
                "provider_kind": m.provider_kind.as_str(),
            })
        })
        .collect();
    (StatusCode::OK, Json(instances))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(list_providers))
        .route("/instances", axum::routing::get(list_provider_instances))
}
