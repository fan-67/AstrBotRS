use astrbot_core::lifecycle::CoreLifecycle;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize logging
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(true)
        .init();

    info!("AstrBotRS v{}", env!("CARGO_PKG_VERSION"));

    let config_path = std::env::var("ASTRBOT_CONFIG")
        .unwrap_or_else(|_| "data/astrbot_config.toml".to_string());
    let db_path = std::env::var("ASTRBOT_DB")
        .unwrap_or_else(|_| "data/astrbot.db".to_string());

    // Ensure data directory exists
    if let Some(parent) = std::path::Path::new(&config_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut core = CoreLifecycle::initialize(&config_path, &db_path)
        .await
        .expect("Failed to initialize AstrBot");

    info!("AstrBotRS initialized. Starting...");

    core.start().await.expect("Failed to start AstrBot");
}
