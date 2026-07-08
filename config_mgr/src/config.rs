use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use astrbot_utils::error::{AstrBotError, Result};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DashboardConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: Option<String>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    6185
}
fn default_jwt_secret() -> Option<String> {
    None
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub enable: bool,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub platform_type: String,
    pub enable: bool,

    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AstrBotConfig {
    #[serde(default)]
    pub dashboard: DashboardConfig,
    #[serde(default)]
    pub provider: Vec<ProviderConfig>,
    #[serde(default)]
    pub platform: Vec<PlatformConfig>,
}

impl AstrBotConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| AstrBotError::Config(format!("Failed to read config: {e}")))?;
        let config: AstrBotConfig = toml::from_str(&content)
            .map_err(|e| AstrBotError::Config(format!("Failed to parse config: {e}")))?;
        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| AstrBotError::Config(format!("Failed to serialize config: {e}")))?;
        std::fs::write(path.as_ref(), content)
            .map_err(|e| AstrBotError::Config(format!("Failed to write config: {e}")))?;
        Ok(())
    }

    pub fn default_config() -> Self {
        Self {
            dashboard: DashboardConfig {
                host: "0.0.0.0".to_string(),
                port: 6185,
                username: Some("astrbot".to_string()),
                password: Some("astrbot".to_string()),
                jwt_secret: None,
            },
            provider: vec![ProviderConfig {
                id: "deepseek".to_string(),
                provider_type: "openai_chat_completion".to_string(),
                enable: false,
                model: Some("deepseek-chat".to_string()),
                api_key: None,
                base_url: Some("https://api.deepseek.com".to_string()),
                extra: HashMap::new(),
            }],
            platform: vec![PlatformConfig {
                id: "my_wechat".to_string(),
                platform_type: "weixin_oc".to_string(),
                enable: false,
                extra: HashMap::new(),
            }],
        }
    }

    pub fn ensure_exists<P: AsRef<Path>>(path: P) -> Result<Self> {
        if path.as_ref().exists() {
            Self::load(path)
        } else {
            let config = Self::default_config();
            config.save(&path)?;
            Ok(config)
        }
    }
}
