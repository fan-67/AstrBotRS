use std::path::PathBuf;

use astrbot_config_mgr::config::AstrBotConfig;

fn temp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("astrbot_test_{}", name));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn test_default_config() {
    let config = AstrBotConfig::default_config();

    assert_eq!(config.dashboard.host, "0.0.0.0");
    assert_eq!(config.dashboard.port, 6185);
    assert_eq!(
        config.dashboard.username.as_deref(),
        Some("astrbot")
    );
    assert_eq!(
        config.dashboard.password.as_deref(),
        Some("astrbot")
    );

    assert_eq!(config.provider.len(), 1);
    assert_eq!(config.provider[0].id, "deepseek");
    assert_eq!(config.provider[0].provider_type, "openai_chat_completion");
    assert!(!config.provider[0].enable);
    assert_eq!(config.provider[0].model.as_deref(), Some("deepseek-chat"));
    assert_eq!(
        config.provider[0].base_url.as_deref(),
        Some("https://api.deepseek.com")
    );

    assert_eq!(config.platform.len(), 1);
    assert_eq!(config.platform[0].id, "my_wechat");
    assert_eq!(config.platform[0].platform_type, "weixin_oc");
    assert!(!config.platform[0].enable);
}

#[test]
fn test_load_valid_toml() {
    let toml_str = r#"
[dashboard]
host = "127.0.0.1"
port = 8080

[[provider]]
id = "test_provider"
type = "openai_chat_completion"
enable = true
model = "gpt-4"
api_key = "sk-test"
base_url = "https://api.openai.com"
"#;
    let path = temp_path("load_test.toml");
    std::fs::write(&path, toml_str).unwrap();

    let config = AstrBotConfig::load(&path).unwrap();
    assert_eq!(config.dashboard.host, "127.0.0.1");
    assert_eq!(config.dashboard.port, 8080);
    assert_eq!(config.provider.len(), 1);
    assert_eq!(config.provider[0].id, "test_provider");
    assert!(config.provider[0].enable);
    assert_eq!(config.provider[0].model.as_deref(), Some("gpt-4"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_load_invalid_toml() {
    let path = temp_path("load_invalid_test.toml");
    std::fs::write(&path, "this is not valid toml [[[").unwrap();

    let result = AstrBotConfig::load(&path);
    assert!(result.is_err());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_save_and_reload() {
    let config = AstrBotConfig::default_config();
    let path = temp_path("save_test.toml");

    config.save(&path).unwrap();
    assert!(path.exists());

    let loaded = AstrBotConfig::load(&path).unwrap();
    assert_eq!(loaded.dashboard.host, config.dashboard.host);
    assert_eq!(loaded.dashboard.port, config.dashboard.port);
    assert_eq!(loaded.provider.len(), config.provider.len());
    assert_eq!(loaded.provider[0].id, config.provider[0].id);
    assert_eq!(loaded.platform.len(), config.platform.len());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_ensure_exists_creates_file() {
    let path = temp_path("ensure_test.toml");

    let config = AstrBotConfig::ensure_exists(&path).unwrap();
    assert!(path.exists());
    assert_eq!(config.dashboard.host, "0.0.0.0");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_ensure_exists_loads_existing() {
    let path = temp_path("ensure_exists_test.toml");
    std::fs::write(
        &path,
        r#"
[dashboard]
host = "192.168.1.1"
port = 9090
"#,
    )
    .unwrap();

    let config = AstrBotConfig::ensure_exists(&path).unwrap();
    assert_eq!(config.dashboard.host, "192.168.1.1");
    assert_eq!(config.dashboard.port, 9090);

    let _ = std::fs::remove_file(&path);
}
