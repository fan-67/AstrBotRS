use std::sync::Arc;

use astrbot_platform::message_chain::MessageChain;
use astrbot_platform::traits::Platform;
use astrbot_platform::AstrMessageEvent;
use astrbot_provider::ProviderManager;
use tracing::{error, info};

#[derive(Clone)]
pub struct Pipeline {
    provider_mgr: Arc<ProviderManager>,
    platform: Arc<dyn Platform>,
}

impl Pipeline {
    pub fn new(provider_mgr: Arc<ProviderManager>, platform: Arc<dyn Platform>) -> Self {
        Self {
            provider_mgr,
            platform,
        }
    }

    pub async fn execute(&self, event: AstrMessageEvent) {
        let text = event.get_message_str().to_string();
        if text.is_empty() {
            return;
        }

        let session_id = event.session_id.clone();

        let Some(provider) = self.provider_mgr.get_using_provider() else {
            error!("No provider available, cannot respond");
            return;
        };

        let req = astrbot_provider::ProviderRequest::prompt(&text);
        match provider.text_chat(req).await {
            Ok(response) => {
                let reply = MessageChain::text(&response.completion_text);
                info!(
                    "LLM reply to {session_id}: {}",
                    &response.completion_text.chars().take(100).collect::<String>()
                );
                if let Err(e) = self.platform.send_message(&session_id, reply).await {
                    error!("Failed to send reply: {e}");
                }
            }
            Err(e) => {
                error!("LLM request failed: {e}");
                let err_msg = MessageChain::text(format!("抱歉，我遇到了一个错误：{e}"));
                let _ = self.platform.send_message(&session_id, err_msg).await;
            }
        }
    }
}
