use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router};

use crate::app::AppState;

use astrbot_config_mgr::config::{DashboardConfig, PlatformConfig, ProviderConfig};

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    let json = serde_json::to_value(&*config).unwrap_or_default();
    (StatusCode::OK, Json(json))
}

async fn update_config(
    State(state): State<AppState>,
    Json(updates): Json<serde_json::Value>,
) -> impl IntoResponse {
    let result: Result<(), String> = async {
        let mut config = state.config.write().await;
        if let Some(d) = updates.get("dashboard") {
            if let Ok(dc) = serde_json::from_value::<DashboardConfig>(d.clone()) {
                config.dashboard = dc;
            }
        }
        if let Some(p) = updates.get("provider") {
            if let Ok(pc) = serde_json::from_value::<Vec<ProviderConfig>>(p.clone()) {
                config.provider = pc;
            }
        }
        if let Some(p) = updates.get("platform") {
            if let Ok(pc) = serde_json::from_value::<Vec<PlatformConfig>>(p.clone()) {
                config.platform = pc;
            }
        }
        config.save(&state.config_path).map_err(|e| e.to_string())?;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(get_config).put(update_config))
}
