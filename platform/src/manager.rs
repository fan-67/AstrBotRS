use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::info;

use crate::event::AstrMessageEvent;
use crate::sources::weixin_oc::WeixinOCAdapter;
use crate::traits::Platform;

pub struct PlatformManager {
    pub event_tx: mpsc::UnboundedSender<AstrMessageEvent>,
    platforms: Vec<Arc<dyn Platform>>,
}

impl PlatformManager {
    pub fn new(event_tx: mpsc::UnboundedSender<AstrMessageEvent>) -> Self {
        Self {
            event_tx,
            platforms: Vec::new(),
        }
    }

    pub fn add(&mut self, platform: Arc<dyn Platform>) {
        info!("Adding platform: {}", platform.meta().id);
        self.platforms.push(platform);
    }

    pub fn load_from_config(
        &mut self,
        configs: &[astrbot_config_mgr::config::PlatformConfig],
    ) {
        for cfg in configs {
            if !cfg.enable {
                continue;
            }

            match cfg.platform_type.as_str() {
                "weixin_oc" => {
                    let base_url = cfg
                        .extra
                        .get("weixin_oc_base_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("https://ilinkai.weixin.qq.com");
                    let cdn_base_url = cfg
                        .extra
                        .get("weixin_oc_cdn_base_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("https://novac2c.cdn.weixin.qq.com/c2c");
                    let token = cfg
                        .extra
                        .get("weixin_oc_token")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let adapter = WeixinOCAdapter::new(
                        &cfg.id,
                        base_url,
                        cdn_base_url,
                        token,
                        self.event_tx.clone(),
                    );
                    self.add(Arc::new(adapter));
                }
                t => {
                    info!("Unsupported platform type: {t}");
                }
            }
        }
    }

    pub fn platforms(&self) -> &[Arc<dyn Platform>] {
        &self.platforms
    }

    pub fn len(&self) -> usize {
        self.platforms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.platforms.is_empty()
    }
}
