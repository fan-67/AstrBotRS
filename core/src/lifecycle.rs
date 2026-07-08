use std::sync::Arc;

use astrbot_db::Database;
use astrbot_platform::manager::PlatformManager;
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

        let mut provider_mgr = ProviderManager::new();
        for pc in &config.provider {
            if !pc.enable {
                continue;
            }
            match pc.provider_type.as_str() {
                "openai_chat_completion" => {
                    let api_key = pc.api_key.clone().unwrap_or_default();
                    let base_url = pc
                        .base_url
                        .clone()
                        .unwrap_or_else(|| "https://api.deepseek.com".to_string());
                    let model = pc
                        .model
                        .clone()
                        .unwrap_or_else(|| "deepseek-chat".to_string());
                    let p = OpenAICompatProvider::new(&pc.id, base_url, api_key, model);
                    provider_mgr.register(pc.id.clone(), Box::new(p));
                }
                t => info!("Unsupported provider type: {t}"),
            }
        }
        let provider_mgr = Arc::new(provider_mgr);
        info!("Providers initialized");

        let mut platform_mgr = PlatformManager::new(event_tx.clone());
        platform_mgr.load_from_config(&config.platform);
        info!("Platforms initialized: {}", platform_mgr.len());
        let platform_mgr = Arc::new(Mutex::new(platform_mgr));

        let config = Arc::new(RwLock::new(config));

        let pipeline = {
            let pfm = platform_mgr.lock().await;
            pfm.platforms().first().cloned().map(|p| {
                Pipeline::new(provider_mgr.clone(), p)
            })
        };

        let event_bus = pipeline.map(|p| EventBus::new(event_rx, p));

        Ok(Self {
            db,
            provider_mgr,
            platform_mgr,
            config,
            config_path: config_path.to_string(),
            log_broker,
            event_tx: Some(event_tx),
            event_bus,
        })
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut event_bus) = self.event_bus.take() {
            let dispatch = tokio::spawn(async move { event_bus.dispatch().await });

            let handles: Vec<_> = {
                let pfm = self.platform_mgr.lock().await;
                pfm.platforms()
                    .iter()
                    .map(|p| {
                        let plat = p.clone();
                        tokio::spawn(async move {
                            if let Err(e) = plat.run().await {
                                tracing::error!("Platform: {e}");
                            }
                        })
                    })
                    .collect()
            };

            info!("AstrBot started ({} platform(s))", handles.len());
            for h in handles {
                let _ = h.await;
            }
            let _ = dispatch.await;
        }
        Ok(())
    }

    pub async fn reload_config(&self) {
        match astrbot_config_mgr::AstrBotConfig::load(&self.config_path) {
            Ok(cfg) => {
                let mut w = self.config.write().await;
                *w = cfg;
            }
            Err(e) => tracing::error!("Failed to reload config: {e}"),
        }
    }
}
