use std::collections::HashMap;
use std::sync::Arc;

use astrbot_db::Database;
use astrbot_platform::manager::PlatformManager;
use astrbot_platform::traits::Platform;
use astrbot_provider::manager::ProviderManager;
use astrbot_provider::sources::OpenAICompatProvider;
use astrbot_utils::logging::LogBroker;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::info;

use crate::event_bus::EventBus;
use crate::pipeline::Pipeline;

pub struct CoreLifecycle {
    pub db: Database,
    pub provider_mgr: Arc<ProviderManager>,
    pub platform_mgr: Arc<Mutex<PlatformManager>>,
    pub config: Arc<RwLock<astrbot_config_mgr::AstrBotConfig>>,
    pub config_path: String,
    pub log_broker: Arc<LogBroker>,
    pub jwt_secret: Arc<RwLock<String>>,
    pub event_tx: Option<mpsc::UnboundedSender<astrbot_platform::AstrMessageEvent>>,
    event_bus: Option<EventBus>,
}

impl CoreLifecycle {
    pub async fn initialize(
        config_path: &str,
        db_path: &str,
        log_broker: Arc<LogBroker>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config = astrbot_config_mgr::AstrBotConfig::ensure_exists(config_path)?;
        info!("Config loaded from {config_path}");

        let db = Database::connect(db_path).await?;
        info!("Database connected at {db_path}");

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let provider_mgr = Self::build_providers(&config).await;
        info!("Providers initialized");

        let mut platform_mgr = PlatformManager::new(event_tx.clone());
        platform_mgr.load_from_config(&config.platform);
        info!("Platforms initialized: {}", platform_mgr.len());
        let platform_mgr = Arc::new(Mutex::new(platform_mgr));

        let config = Arc::new(RwLock::new(config));

        let jwt_secret_str =
            if let Some(ref s) = config.read().await.dashboard.jwt_secret {
                s.clone()
            } else {
                (0..32).map(|_| {
                    let idx = fastrand::usize(..62);
                    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"[idx] as char
                }).collect()
            };
        let jwt_secret = Arc::new(RwLock::new(jwt_secret_str));

        let pipeline = {
            let pfm = platform_mgr.lock().await;
            if pfm.is_empty() {
                info!("No platforms enabled in {config_path}");
                None
            } else {
                let platforms: HashMap<String, Arc<dyn Platform>> = pfm
                    .platforms()
                    .iter()
                    .map(|p| (p.meta().id.clone(), p.clone()))
                    .collect();
                Some(Pipeline::new(provider_mgr.clone(), platforms))
            }
        };

        let event_bus = pipeline.map(|p| EventBus::new(event_rx, p));

        Ok(Self {
            db,
            provider_mgr,
            platform_mgr,
            config,
            config_path: config_path.to_string(),
            log_broker,
            jwt_secret,
            event_tx: Some(event_tx),
            event_bus,
        })
    }

    async fn build_providers(config: &astrbot_config_mgr::AstrBotConfig) -> Arc<ProviderManager> {
        let mgr = ProviderManager::new();
        for pc in &config.provider {
            if !pc.enable { continue; }
            match pc.provider_type.as_str() {
                "openai_chat_completion" => {
                    let api_key = pc.api_key.clone().unwrap_or_default();
                    let base_url = pc.base_url.clone()
                        .unwrap_or_else(|| "https://api.deepseek.com".to_string());
                    let model = pc.model.clone()
                        .unwrap_or_else(|| "deepseek-chat".to_string());
                    let p = OpenAICompatProvider::new(&pc.id, base_url, api_key, model);
                    mgr.register(pc.id.clone(), Box::new(p)).await;
                }
                t => info!("Unsupported provider type: {t}"),
            }
        }
        Arc::new(mgr)
    }

    pub async fn reload_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let new_config = astrbot_config_mgr::AstrBotConfig::load(&self.config_path)
            .map_err(|e| format!("Failed to reload config: {e}"))?;

        // Update in-memory config
        {
            let mut w = self.config.write().await;
            *w = new_config.clone();
        }

        // Rebuild providers (clear and reload)
        self.provider_mgr.clear().await;
        for pc in &new_config.provider {
            if !pc.enable { continue; }
            match pc.provider_type.as_str() {
                "openai_chat_completion" => {
                    let api_key = pc.api_key.clone().unwrap_or_default();
                    let base_url = pc.base_url.clone()
                        .unwrap_or_else(|| "https://api.deepseek.com".to_string());
                    let model = pc.model.clone()
                        .unwrap_or_else(|| "deepseek-chat".to_string());
                    let p = OpenAICompatProvider::new(&pc.id, base_url, api_key, model);
                    self.provider_mgr.register(pc.id.clone(), Box::new(p)).await;
                }
                t => info!("Unsupported provider type: {t}"),
            }
        }
        info!("Providers reloaded from config");

        // Rebuild platforms
        {
            let mut pfm = self.platform_mgr.lock().await;
            let mut new_pfm = PlatformManager::new(
                self.event_tx.clone().ok_or("event_tx not available")?,
            );
            new_pfm.load_from_config(&new_config.platform);
            *pfm = new_pfm;
            info!("Platforms reloaded from config (restart required)");
        }

        Ok(())
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(event_bus) = self.event_bus.take() {
            let dispatch = tokio::spawn(async move { event_bus.dispatch().await });

            let handles: Vec<_> = {
                let pfm = self.platform_mgr.lock().await;
                pfm.platforms().iter().map(|p| {
                    let plat = p.clone();
                    tokio::spawn(async move {
                        if let Err(e) = plat.run().await {
                            tracing::error!("Platform: {e}");
                        }
                    })
                }).collect()
            };

            info!("AstrBot started ({} platform(s))", handles.len());
            for h in handles { let _ = h.await; }
            let _ = dispatch.await;
        }
        Ok(())
    }
}
