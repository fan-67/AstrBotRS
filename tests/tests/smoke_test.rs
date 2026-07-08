use std::sync::Arc;

use astrbot_config_mgr::AstrBotConfig;
use astrbot_core::lifecycle::CoreLifecycle;
use astrbot_utils::logging::LogBroker;

/// Test that the binary can initialize with default config and create all components
#[tokio::test]
async fn test_initialization_with_defaults() {
    let dir = std::env::temp_dir().join(format!("astrbot_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let config_path = dir.join("config.toml");
    let db_path = dir.join("test.db");

    let log_broker = Arc::new(LogBroker::new(100));

    let result = CoreLifecycle::initialize(
        config_path.to_str().unwrap(),
        db_path.to_str().unwrap(),
        log_broker,
    )
    .await;

    assert!(result.is_ok(), "Initialization should succeed: {:?}", result.err());

    let mut core = result.unwrap();

    // Verify components exist
    assert!(core.provider_mgr.is_empty().await, "Should have no providers by default");
    assert!(core.event_tx.is_some(), "event_tx should exist");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Test default config generation
#[tokio::test]
async fn test_default_config_generates_valid() {
    let dir = std::env::temp_dir().join(format!("astrbot_config_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("default.toml");

    let config = AstrBotConfig::ensure_exists(path.to_str().unwrap());
    assert!(config.is_ok(), "Default config should be created");

    let config = config.unwrap();
    assert_eq!(config.dashboard.host, "0.0.0.0");
    assert_eq!(config.dashboard.port, 6185);
    assert_eq!(config.provider.len(), 1);
    assert_eq!(config.platform.len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}
