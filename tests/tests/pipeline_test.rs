use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use astrbot_core::pipeline::Pipeline;
use astrbot_platform::event::AstrMessageEvent;
use astrbot_platform::message::{AstrBotMessage, MessageMember, MessageType};
use astrbot_platform::message_chain::MessageChain;
use astrbot_platform::metadata::PlatformMetadata;
use astrbot_platform::traits::Platform;
use astrbot_provider::entities::{LLMResponse, ProviderMeta, ProviderRequest, ProviderType};
use astrbot_provider::manager::ProviderManager;
use astrbot_provider::traits::Provider;

struct MockProvider {
    response: LLMResponse,
}

#[async_trait]
impl Provider for MockProvider {
    fn meta(&self) -> ProviderMeta {
        ProviderMeta {
            id: "mock".to_string(),
            model: Some("mock-model".to_string()),
            provider_type: "chat_completion".to_string(),
            provider_kind: ProviderType::ChatCompletion,
        }
    }

    async fn text_chat(&self, _req: ProviderRequest) -> astrbot_provider::Result<LLMResponse> {
        Ok(self.response.clone())
    }

    async fn text_chat_stream(
        &self,
        _req: ProviderRequest,
    ) -> astrbot_provider::Result<
        std::pin::Pin<
            Box<dyn tokio_stream::Stream<Item = astrbot_provider::Result<LLMResponse>> + Send>,
        >,
    > {
        Err(astrbot_provider::AstrBotError::Provider(
            "streaming not supported".to_string(),
        ))
    }
}

struct MockPlatform {
    sent: Arc<Mutex<Vec<(String, MessageChain)>>>,
    meta: PlatformMetadata,
}

#[async_trait]
impl Platform for MockPlatform {
    fn meta(&self) -> PlatformMetadata {
        self.meta.clone()
    }

    async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn send_message(
        &self,
        session_id: &str,
        message: MessageChain,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sent
            .lock()
            .await
            .push((session_id.to_string(), message));
        Ok(())
    }

    fn commit_event(&self, _event: AstrMessageEvent) {}
}

fn make_event(text: &str, session_id: &str, platform_id: &str) -> AstrMessageEvent {
    let msg = AstrBotMessage::new(
        MessageType::FriendMessage,
        "bot",
        session_id,
        "msg-1",
        MessageMember::new("user-1"),
        text,
    );
    let meta = PlatformMetadata::new(platform_id, platform_id, platform_id);
    AstrMessageEvent::new(msg, meta, session_id)
}

#[tokio::test]
async fn test_pipeline_execute_sends_reply() {
    let provider = Box::new(MockProvider {
        response: LLMResponse::text("Hello from LLM!"),
    });
    let provider_mgr = Arc::new(ProviderManager::new());
    provider_mgr.register("mock".to_string(), provider).await;

    let sent = Arc::new(Mutex::new(Vec::new()));
    let platform = MockPlatform {
        sent: sent.clone(),
        meta: PlatformMetadata::new("test_platform", "test", "Test Platform"),
    };
    let mut platforms: HashMap<String, Arc<dyn Platform>> = HashMap::new();
    platforms.insert("test_platform".to_string(), Arc::new(platform));

    let pipeline = Pipeline::new(provider_mgr, platforms);
    let event = make_event("What's up?", "session-1", "test_platform");
    pipeline.execute(event).await;

    let records = sent.lock().await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, "session-1");
    assert_eq!(records[0].1.get_plain_text(), "Hello from LLM!");
}

#[tokio::test]
async fn test_pipeline_empty_message_does_nothing() {
    let provider = Box::new(MockProvider {
        response: LLMResponse::text("should not be sent"),
    });
    let provider_mgr = Arc::new(ProviderManager::new());
    provider_mgr.register("mock".to_string(), provider).await;

    let sent = Arc::new(Mutex::new(Vec::new()));
    let platform = MockPlatform {
        sent: sent.clone(),
        meta: PlatformMetadata::new("test_platform", "test", "Test Platform"),
    };
    let mut platforms: HashMap<String, Arc<dyn Platform>> = HashMap::new();
    platforms.insert("test_platform".to_string(), Arc::new(platform));

    let pipeline = Pipeline::new(provider_mgr, platforms);
    let event = make_event("", "session-1", "test_platform");
    pipeline.execute(event).await;

    let records = sent.lock().await;
    assert_eq!(records.len(), 0);
}

#[tokio::test]
async fn test_pipeline_no_provider_sends_error_reply() {
    let provider_mgr = Arc::new(ProviderManager::new());

    let sent = Arc::new(Mutex::new(Vec::new()));
    let platform = MockPlatform {
        sent: sent.clone(),
        meta: PlatformMetadata::new("test_platform", "test", "Test Platform"),
    };
    let mut platforms: HashMap<String, Arc<dyn Platform>> = HashMap::new();
    platforms.insert("test_platform".to_string(), Arc::new(platform));

    let pipeline = Pipeline::new(provider_mgr, platforms);
    let event = make_event("Hello", "session-1", "test_platform");
    pipeline.execute(event).await;

    let records = sent.lock().await;
    assert_eq!(records.len(), 0);
}

#[tokio::test]
async fn test_pipeline_unknown_platform_does_nothing() {
    let provider = Box::new(MockProvider {
        response: LLMResponse::text("test"),
    });
    let provider_mgr = Arc::new(ProviderManager::new());
    provider_mgr.register("mock".to_string(), provider).await;

    let sent = Arc::new(Mutex::new(Vec::new()));
    let platform = MockPlatform {
        sent: sent.clone(),
        meta: PlatformMetadata::new("real_platform", "real", "Real Platform"),
    };
    let mut platforms: HashMap<String, Arc<dyn Platform>> = HashMap::new();
    platforms.insert("real_platform".to_string(), Arc::new(platform));

    let pipeline = Pipeline::new(provider_mgr, platforms);
    let event = make_event("Hello", "session-1", "unknown_platform");
    pipeline.execute(event).await;

    let records = sent.lock().await;
    assert_eq!(records.len(), 0);
}

#[tokio::test]
async fn test_multiple_conversations() {
    let provider = Box::new(MockProvider {
        response: LLMResponse::text("Reply"),
    });
    let provider_mgr = Arc::new(ProviderManager::new());
    provider_mgr.register("mock".to_string(), provider).await;

    let sent = Arc::new(Mutex::new(Vec::new()));
    let platform = MockPlatform {
        sent: sent.clone(),
        meta: PlatformMetadata::new("p", "p", "P"),
    };
    let mut platforms: HashMap<String, Arc<dyn Platform>> = HashMap::new();
    platforms.insert("p".to_string(), Arc::new(platform));

    let pipeline = Pipeline::new(provider_mgr, platforms);

    pipeline
        .execute(make_event("First", "session-a", "p"))
        .await;
    pipeline
        .execute(make_event("Second", "session-b", "p"))
        .await;

    let records = sent.lock().await;
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].0, "session-a");
    assert_eq!(records[1].0, "session-b");
}
