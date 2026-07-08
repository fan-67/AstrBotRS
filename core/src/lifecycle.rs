use std::sync::Arc;

use astrbot_db::Database;
use astrbot_platform::manager::PlatformManager;
use astrbot_provider::manager::ProviderManager;
use astrbot_provider::sources::OpenAICompatProvider;
use tokio::sync::mpsc;
use tracing::info;

use crate::event_bus::EventBus;
use crate::pipeline::Pipeline;

pub struct CoreLifecycle {
    pub db: Database,
    pub provider_mgr: Arc<ProviderManager>,
    pub platform_mgr: PlatformManager,
    pub event_bus: Option<EventBus>,
}

impl CoreLifecycle {
    pub async fn initialize(config_path: &str, db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Load config
        let config = astrbot_config_mgr::AstrBotConfig::ensure_exists(config_path)?;
        info!("Config loaded from {config_path}");

        // Connect DB
        let db = Database::connect(db_path).await?;
        info!("Database connected at {db_path}");

        // Event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Provider manager
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
                    let provider = OpenAICompatProvider::new(&pc.id, base_url, api_key, model);
                    provider_mgr.register(pc.id.clone(), Box::new(provider));
                }
                t => {
                    info!("Unsupported provider type: {t}");
                }
            }
        }
        let provider_mgr = Arc::new(provider_mgr);
        info!("Providers initialized");

        // Platform manager
        let mut platform_mgr = PlatformManager::new(event_tx);
        platform_mgr.load_from_config(&config.platform);
        info!("Platforms initialized: {}", platform_mgr.len());

        // Check if we have at least one platform and provider
        if platform_mgr.is_empty() {
            info!("No platforms enabled. Configure a platform in {config_path}");
        }
        if provider_mgr.is_empty() {
            info!("No providers enabled. Configure a provider in {config_path}");
        }

        // Pipeline
        let first_platform = platform_mgr.platforms().first().cloned();
        let pipeline = first_platform.map(|p| {
            Pipeline::new(provider_mgr.clone(), p.clone())
        });

        let event_bus = pipeline.map(|p| EventBus::new(event_rx, p));

        Ok(Self {
            db,
            provider_mgr,
            platform_mgr,
            event_bus,
        })
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Start event bus dispatch
        if let Some(mut event_bus) = self.event_bus.take() {
            let dispatch_handle = tokio::spawn(async move {
                event_bus.dispatch().await;
            });

            // Start all platforms
            let platform_handles: Vec<_> = self
                .platform_mgr
                .platforms()
                .iter()
                .map(|p| {
                    let plat = p.clone();
                    tokio::spawn(async move {
                        if let Err(e) = plat.run().await {
                            tracing::error!("Platform error: {e}");
                        }
                    })
                })
                .collect();

            info!("AstrBot started with {} platform(s)", platform_handles.len());

            // Wait for any platform or dispatch task to complete
            for handle in platform_handles {
                let _ = handle.await;
            }
            let _ = dispatch_handle.await;
        }

        Ok(())
    }
}
