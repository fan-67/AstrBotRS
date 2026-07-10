use std::path::PathBuf;
use std::sync::Arc;

use astrbot_core::lifecycle::CoreLifecycle;
use astrbot_plugin::PluginManager;
use astrbot_provider::manager::ProviderManager;
use astrbot_utils::logging::LogBroker;
use axum::Router;
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

use crate::frontend::FrontendService;
use crate::middleware::auth as auth_mw;
use crate::routes::{
    auth, bots, config, conversations, logs, operations, plugins, providers, stats,
};

#[derive(Clone)]
pub struct AppState {
    pub provider_mgr: Arc<ProviderManager>,
    pub log_broker: Arc<LogBroker>,
    pub config_path: String,
    pub config: Arc<RwLock<astrbot_config_mgr::AstrBotConfig>>,
    pub jwt_secret: Arc<RwLock<String>>,
    pub db_pool: SqlitePool,
    pub plugin_mgr: Arc<RwLock<PluginManager>>,
}

/// Create router from AppState directly (no CoreLifecycle dependency).
pub fn create_router_from_state(
    state: AppState,
    dist_dir: PathBuf,
    _plugin_mgr: Arc<RwLock<PluginManager>>,
) -> Router {
    let cors = CorsLayer::permissive();
    let frontend = FrontendService::new(dist_dir);

    let api_routes = Router::new()
        .nest("/api/v1/auth", auth::routes())
        .nest("/api/v1/config", config::routes())
        .nest("/api/v1/bots", bots::routes())
        .nest("/api/v1/providers", providers::routes())
        .nest("/api/v1/logs", logs::routes())
        .nest("/api/v1/stats", stats::routes())
        .nest("/api/v1/conversations", conversations::routes())
        .nest("/api/v1/plugins", plugins::routes())
        .nest("/api/v1/operations", operations::routes())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_mw::require_auth,
        ))
        .layer(cors)
        .with_state(state);

    Router::new()
        .merge(api_routes)
        .fallback_service(frontend.into_router())
}

/// Create router from CoreLifecycle (backward compatible).
pub fn create_router(
    core: &CoreLifecycle,
    dist_dir: PathBuf,
    plugin_mgr: Arc<RwLock<PluginManager>>,
) -> Router {
    let state = AppState {
        provider_mgr: core.provider_mgr.clone(),
        log_broker: core.log_broker.clone(),
        config_path: core.config_path.clone(),
        config: core.config.clone(),
        jwt_secret: core.jwt_secret.clone(),
        db_pool: core.db.pool().clone(),
        plugin_mgr: plugin_mgr.clone(),
    };
    create_router_from_state(state, dist_dir, plugin_mgr)
}
