use async_trait::async_trait;

use astrbot_platform::event::AstrMessageEvent;
use astrbot_platform::message_chain::MessageChain;
use astrbot_provider::entities::{LLMResponse, ProviderRequest};

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;

    fn description(&self) -> &str {
        ""
    }

    async fn on_message(&self, _event: &AstrMessageEvent) -> Option<MessageChain> {
        None
    }

    async fn on_llm_request(&self, _req: &ProviderRequest) -> Option<ProviderRequest> {
        None
    }

    async fn on_llm_response(
        &self,
        _req: &ProviderRequest,
        _resp: &LLMResponse,
    ) -> Option<LLMResponse> {
        None
    }

    async fn on_bot_start(&self) {}

    async fn on_bot_stop(&self) {}

    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn terminate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
