use std::sync::Arc;

use astrbot_core::lifecycle::CoreLifecycle;
use astrbot_provider::manager::ProviderManager;
use astrbot_utils::logging::LogBroker;
use axum::Router;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

use crate::routes::{auth, bots, config, logs, providers, stats};

#[derive(Clone)]
pub struct AppState {
    pub provider_mgr: Arc<ProviderManager>,
    pub log_broker: Arc<LogBroker>,
    pub config_path: String,
    pub config: Arc<RwLock<astrbot_config_mgr::AstrBotConfig>>,
}

pub fn create_router(core: &CoreLifecycle) -> Router {
    let state = AppState {
        provider_mgr: core.provider_mgr.clone(),
        log_broker: core.log_broker.clone(),
        config_path: core.config_path.clone(),
        config: core.config.clone(),
    };

    let cors = CorsLayer::permissive();

    Router::new()
        .nest("/api/v1/auth", auth::routes())
        .nest("/api/v1/config", config::routes())
        .nest("/api/v1/bots", bots::routes())
        .nest("/api/v1/providers", providers::routes())
        .nest("/api/v1/logs", logs::routes())
        .nest("/api/v1/stats", stats::routes())
        .layer(cors)
        .with_state(state)
}
