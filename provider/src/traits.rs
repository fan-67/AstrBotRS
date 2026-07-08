use async_trait::async_trait;
use std::pin::Pin;

use crate::entities::{LLMResponse, ProviderMeta, ProviderRequest};

#[async_trait]
pub trait Provider: Send + Sync {
    fn meta(&self) -> ProviderMeta;

    async fn text_chat(&self, req: ProviderRequest) -> crate::Result<LLMResponse>;

    async fn text_chat_stream(
        &self,
        req: ProviderRequest,
    ) -> crate::Result<Pin<Box<dyn tokio_stream::Stream<Item = crate::Result<LLMResponse>> + Send>>>;
}
