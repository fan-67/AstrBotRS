use async_trait::async_trait;

use crate::event::AstrMessageEvent;
use crate::message_chain::MessageChain;
use crate::metadata::PlatformMetadata;

#[async_trait]
pub trait Platform: Send + Sync {
    fn meta(&self) -> PlatformMetadata;

    async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn send_message(
        &self,
        session_id: &str,
        message: MessageChain,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn commit_event(&self, event: AstrMessageEvent);

    async fn start_typing(&self, _session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn stop_typing(&self, _session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn terminate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
