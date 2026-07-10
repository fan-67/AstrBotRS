use std::path::PathBuf;
use std::sync::Arc;

use astrbot_plugin::PluginManager;
use astrbot_provider::manager::ProviderManager;
use astrbot_utils::logging::LogBroker;
use axum::Router;
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use tracing::info;

use crate::app::{create_router_from_state, AppState};

/// Start API server from pre-extracted fields (no lock needed).
pub async fn start_server_on(
    addr: &str,
    provider_mgr: Arc<ProviderManager>,
    log_broker: Arc<LogBroker>,
    config_path: &str,
    config: Arc<RwLock<astrbot_config_mgr::AstrBotConfig>>,
    jwt_secret: Arc<RwLock<String>>,
    db_pool: SqlitePool,
    dist_dir: Option<PathBuf>,
) {
    let dist = dist_dir.unwrap_or_else(|| PathBuf::from("data/dist"));
    let plugin_mgr = Arc::new(RwLock::new(PluginManager::new()));

    let state = AppState {
        provider_mgr,
        log_broker,
        config_path: config_path.to_string(),
        config,
        jwt_secret,
        db_pool,
        plugin_mgr: plugin_mgr.clone(),
    };

    let app = create_router_from_state(state, dist, plugin_mgr);

    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            info!("Dashboard listening on http://{addr}");
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("API server error: {e}");
            }
        }
        Err(e) => {
            tracing::error!("Failed to bind {addr}: {e}");
        }
    }
}
