use std::sync::Arc;

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json},
    routing::post,
    Router,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use astrbot_provider::sources::OpenAICompatProvider;
use astrbot_provider::{Provider, ProviderRequest};

struct MockState {
    requests: Mutex<Vec<Value>>,
}

async fn chat_handler(
    headers: HeaderMap,
    _body: Json<Value>,
) -> impl IntoResponse {
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(auth.starts_with("Bearer "), "Missing or invalid auth header");

    Json(json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello from mock!"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 15,
            "completion_tokens": 5,
            "cached_tokens": 3
        }
    }))
}

fn build_success_mock(state: Arc<MockState>) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_handler))
        .layer(middleware::from_fn(move |req: Request, next: Next| {
            let state = state.clone();
            async move {
                let (parts, body) = req.into_parts();
                let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
                if !bytes.is_empty() {
                    if let Ok(val) = serde_json::from_slice::<Value>(&bytes) {
                        state.requests.lock().await.push(val);
                    }
                }
                let req = Request::from_parts(parts, axum::body::Body::from(bytes));
                next.run(req).await
            }
        }))
}

async fn start_mock(router: Router) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (addr.port(), handle)
}

#[tokio::test]
async fn test_text_chat_success() {
    let state = Arc::new(MockState {
        requests: Mutex::new(Vec::new()),
    });
    let router = build_success_mock(state.clone());
    let (port, _handle) = start_mock(router).await;

    let provider = OpenAICompatProvider::new(
        "test",
        format!("http://127.0.0.1:{}", port),
        "test-key",
        "test-model",
    );

    let req = ProviderRequest::prompt("Say hello");
    let response = provider.text_chat(req).await.unwrap();

    assert_eq!(response.role, "assistant");
    assert_eq!(response.completion_text, "Hello from mock!");
    assert!(!response.is_chunk);

    let usage = response.usage.unwrap();
    assert_eq!(usage.input_other, 12);
    assert_eq!(usage.input_cached, 3);
    assert_eq!(usage.output, 5);

    let requests = state.requests.lock().await;
    assert_eq!(requests.len(), 1);
    let body = &requests[0];
    assert_eq!(body["model"], "test-model");
    assert!(!body["stream"].as_bool().unwrap());
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "Say hello");
}

async fn error_handler_400() -> impl IntoResponse {
    (StatusCode::BAD_REQUEST, "Bad request")
}

async fn error_handler_500() -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal error")
}

#[tokio::test]
async fn test_text_chat_4xx_error() {
    let router = Router::new()
        .route("/v1/chat/completions", post(error_handler_400));
    let (port, _handle) = start_mock(router).await;

    let provider = OpenAICompatProvider::new(
        "test",
        format!("http://127.0.0.1:{}", port),
        "test-key",
        "test-model",
    );

    let req = ProviderRequest::prompt("hello");
    let err = provider.text_chat(req).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("400") || msg.contains("Bad request"), "Error should mention 400: {msg}");
}

#[tokio::test]
async fn test_text_chat_5xx_error() {
    let router = Router::new()
        .route("/v1/chat/completions", post(error_handler_500));
    let (port, _handle) = start_mock(router).await;

    let provider = OpenAICompatProvider::new(
        "test",
        format!("http://127.0.0.1:{}", port),
        "test-key",
        "test-model",
    );

    let req = ProviderRequest::prompt("hello");
    let err = provider.text_chat(req).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("500") || msg.contains("Internal"), "Error should mention 500: {msg}");
}

#[tokio::test]
async fn test_text_chat_with_system_prompt() {
    let state = Arc::new(MockState {
        requests: Mutex::new(Vec::new()),
    });
    let router = build_success_mock(state.clone());
    let (port, _handle) = start_mock(router).await;

    let provider = OpenAICompatProvider::new(
        "test",
        format!("http://127.0.0.1:{}", port),
        "test-key",
        "test-model",
    );

    let req = ProviderRequest {
        prompt: Some("Hello".to_string()),
        system_prompt: Some("You are a helpful assistant.".to_string()),
        ..Default::default()
    };

    provider.text_chat(req).await.unwrap();

    let requests = state.requests.lock().await;
    assert_eq!(requests.len(), 1);
    let body = &requests[0];
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "You are a helpful assistant.");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "Hello");
}

#[tokio::test]
async fn test_meta() {
    let provider = OpenAICompatProvider::new(
        "my-provider",
        "http://localhost:8080",
        "key",
        "gpt-4",
    );
    let meta = provider.meta();
    assert_eq!(meta.id, "my-provider");
    assert_eq!(meta.model.as_deref(), Some("gpt-4"));
    assert_eq!(meta.provider_type, "openai_chat_completion");
}

#[tokio::test]
async fn test_missing_auth_fails() {
    let router = Router::new().route(
        "/v1/chat/completions",
        post(|| async { Json(json!({"choices": []})) }),
    );
    let (port, _handle) = start_mock(router).await;

    let provider = OpenAICompatProvider::new(
        "test",
        format!("http://127.0.0.1:{}", port),
        "", // empty key
        "test-model",
    );

    let req = ProviderRequest::prompt("hello");
    let response = provider.text_chat(req).await;
    assert!(response.is_err());
}
