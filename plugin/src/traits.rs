use async_trait::async_trait;

use astrbot_platform::event::AstrMessageEvent;
use astrbot_platform::message_chain::MessageChain;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;

    fn description(&self) -> &str {
        ""
    }

    async fn on_message(&self, _event: &AstrMessageEvent) -> Option<MessageChain> {
        None
    }

    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn terminate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
