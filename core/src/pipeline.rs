use std::collections::HashMap;
use std::sync::Arc;

use astrbot_platform::message_chain::MessageChain;
use astrbot_platform::traits::Platform;
use astrbot_platform::AstrMessageEvent;
use astrbot_provider::ProviderManager;
use tracing::{error, info};

#[derive(Clone)]
pub struct Pipeline {
    provider_mgr: Arc<ProviderManager>,
    platforms: HashMap<String, Arc<dyn Platform>>,
}

impl Pipeline {
    pub fn new(
        provider_mgr: Arc<ProviderManager>,
        platforms: HashMap<String, Arc<dyn Platform>>,
    ) -> Self {
        Self {
            provider_mgr,
            platforms,
        }
    }

    pub async fn execute(&self, event: AstrMessageEvent) {
        let text = event.get_message_str().to_string();
        if text.is_empty() {
            return;
        }

        let platform_id = event.platform_meta.id.clone();
        let session_id = event.session_id.clone();

        let Some(provider) = self.provider_mgr.get_using_provider().await else {
            error!("No provider available, cannot respond to {session_id}");
            return;
        };

        let platform = match self.platforms.get(&platform_id) {
            Some(p) => p,
            None => {
                error!("Platform {platform_id} not found for session {session_id}");
                return;
            }
        };

        let req = astrbot_provider::ProviderRequest::prompt(&text);
        match provider.text_chat(req).await {
            Ok(response) => {
                let reply = MessageChain::text(&response.completion_text);
                info!(
                    "LLM reply to {session_id} via {platform_id}: {}",
                    &response.completion_text.chars().take(100).collect::<String>()
                );
                if let Err(e) = platform.send_message(&session_id, reply).await {
                    error!("Failed to send reply on {platform_id}: {e}");
                }
            }
            Err(e) => {
                error!("LLM request failed for {session_id}: {e}");
                let err_msg = MessageChain::text(format!("抱歉，我遇到了一个错误：{e}"));
                let _ = platform.send_message(&session_id, err_msg).await;
            }
        }
    }
}
