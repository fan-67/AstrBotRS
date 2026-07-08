use std::sync::Arc;

use astrbot_core::lifecycle::CoreLifecycle;
use astrbot_utils::logging::LogBroker;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(true)
        .init();

    info!("AstrBotRS v{}", env!("CARGO_PKG_VERSION"));

    let config_path =
        std::env::var("ASTRBOT_CONFIG").unwrap_or_else(|_| "data/astrbot_config.toml".to_string());
    let db_path =
        std::env::var("ASTRBOT_DB").unwrap_or_else(|_| "data/astrbot.db".to_string());

    if let Some(parent) = std::path::Path::new(&config_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let log_broker = Arc::new(LogBroker::new(1000));

    let mut core = CoreLifecycle::initialize(&config_path, &db_path, log_broker)
        .await
        .expect("Failed to initialize AstrBot");

    // Start core in background
    let core_handle = tokio::spawn(async move {
        core.start().await.expect("Failed to start AstrBot");
    });

    // Wait for Ctrl+C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = core_handle => {}
    }

    info!("AstrBotRS stopped");
}
