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
    event_rx: Option<mpsc::UnboundedReceiver<astrbot_platform::AstrMessageEvent>>,
    pipeline: Option<Pipeline>,
    shutdown: Arc<tokio::sync::Notify>,
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

        let jwt_secret_str = config.read().await.dashboard.jwt_secret.clone()
            .unwrap_or_else(|| {
                let mut buf = [0u8; 32];
                getrandom::getrandom(&mut buf).expect("rng");
                hex::encode(buf)
            });
        let jwt_secret = Arc::new(RwLock::new(jwt_secret_str));

        let pipeline = {
            let pfm = platform_mgr.lock().await;
            if pfm.is_empty() {
                info!("No platforms enabled. API only mode.");
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

        Ok(Self {
            db,
            provider_mgr,
            platform_mgr,
            config,
            config_path: config_path.to_string(),
            log_broker,
            jwt_secret,
            event_tx: Some(event_tx),
            event_rx: Some(event_rx),
            pipeline,
            shutdown: Arc::new(tokio::sync::Notify::new()),
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

    pub async fn start(&mut self) {
        let shutdown = self.shutdown.clone();

        if let Some(pipeline) = self.pipeline.take() {
            let event_rx = self.event_rx.take().expect("event_rx consumed twice");
            let event_bus = EventBus::new(event_rx, pipeline);
            let shutdown_bus = shutdown.clone();

            tokio::spawn(async move {
                tokio::select! {
                    _ = event_bus.dispatch() => {}
                    _ = shutdown_bus.notified() => {}
                }
            });

            let handles: Vec<_> = {
                let pfm = self.platform_mgr.lock().await;
                pfm.platforms().iter().map(|p| {
                    let plat = p.clone();
                    let sd = shutdown.clone();
                    tokio::spawn(async move {
                        tokio::select! {
                            _ = plat.run() => {}
                            _ = sd.notified() => {}
                        }
                    })
                }).collect()
            };

            info!("AstrBot started ({} platform(s))", handles.len());
            for h in handles { let _ = h.await; }
        } else {
            info!("AstrBot running in headless mode (API only). Waiting for shutdown...");
            shutdown.notified().await;
        }
    }

    pub fn shutdown_signal(&self) -> Arc<tokio::sync::Notify> {
        self.shutdown.clone()
    }

    pub async fn reload_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let new_config = astrbot_config_mgr::AstrBotConfig::load(&self.config_path)
            .map_err(|e| format!("Failed to reload config: {e}"))?;

        {
            let mut w = self.config.write().await;
            *w = new_config.clone();
        }

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

        {
            let mut pfm = self.platform_mgr.lock().await;
            let mut new_pfm = PlatformManager::new(
                self.event_tx.clone().ok_or_else::<Box<dyn std::error::Error>, _>(|| "event_tx not available".into())?,
            );
            new_pfm.load_from_config(&new_config.platform);
            *pfm = new_pfm;
            info!("Platforms reloaded from config (restart required)");
        }

        Ok(())
    }
}
