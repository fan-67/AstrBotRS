use std::path::PathBuf;
use std::sync::Arc;

use astrbot_core::lifecycle::CoreLifecycle;
use astrbot_utils::logging::{LogBroker, LogLayer};
use tokio::sync::Mutex;
use tracing::info;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() {
    let log_broker = Arc::new(LogBroker::new(1000));

    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(LogLayer::new(Arc::downgrade(&log_broker)));
    subscriber.init();

    info!("AstrBotRS v{}", env!("CARGO_PKG_VERSION"));

    let config_path =
        std::env::var("ASTRBOT_CONFIG").unwrap_or_else(|_| "data/config.toml".to_string());
    let db_path = std::env::var("ASTRBOT_DB").unwrap_or_else(|_| "data/astrbot.db".to_string());

    if let Some(parent) = std::path::Path::new(&config_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let dist_dir = std::env::var("ASTRBOT_DIST_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            let p = PathBuf::from("data/dist");
            if p.join("index.html").exists() {
                Some(p)
            } else {
                None
            }
        });

    let core = Arc::new(Mutex::new(
        CoreLifecycle::initialize(&config_path, &db_path, log_broker.clone())
            .await
            .expect("Failed to initialize AstrBot"),
    ));

    // Extract API server state without holding the core lock
    // This avoids the deadlock where API and core fight over the same Mutex
    let api_state = {
        let lock = core.lock().await;
        let config = lock.config.read().await;
        let addr = format!("{}:{}", config.dashboard.host, config.dashboard.port);
        let provider_mgr = lock.provider_mgr.clone();
        let log_broker = lock.log_broker.clone();
        let config_path = lock.config_path.clone();
        let lock_config = lock.config.clone();
        let jwt_secret = lock.jwt_secret.clone();
        let db_pool = lock.db.pool().clone();
        drop(config);
        drop(lock);
        (
            addr,
            provider_mgr,
            log_broker,
            config_path,
            lock_config,
            jwt_secret,
            db_pool,
            dist_dir,
        )
    };

    let core_run = core.clone();
    let core_handle = tokio::spawn(async move {
        let mut lock = core_run.lock().await;
        lock.start().await;
    });

    let api_handle = tokio::spawn(async move {
        let (addr, provider_mgr, log_broker, config_path, config, jwt_secret, db_pool, dist_dir) =
            api_state;
        astrbot_api::server::start_server_on(
            &addr,
            provider_mgr,
            log_broker,
            &config_path,
            config,
            jwt_secret,
            db_pool,
            dist_dir,
        )
        .await;
    });

    // Wait for Ctrl+C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = api_handle => {}
        _ = core_handle => {}
    }

    info!("AstrBotRS stopped");
}
