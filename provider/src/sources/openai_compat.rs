use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, error, warn};

use crate::entities::{LLMResponse, ProviderMeta, ProviderRequest, ProviderType};
use crate::traits::Provider;
use crate::Result;

#[derive(Debug, Clone, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Value>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[allow(dead_code)]
    id: String,
    #[serde(default)]
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    role: String,
    content: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    cached_tokens: Option<u32>,
}

pub struct OpenAICompatProvider {
    client: Client,
    base_url: String,
    authorization: String,
    model: String,
    provider_id: String,
    max_attempts: u32,
}

impl OpenAICompatProvider {
    pub fn new(
        provider_id: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let timeout = Duration::from_secs(60);
        let authorization = format!("Bearer {}", api_key.into());
        Self {
            client: Client::builder()
                .timeout(timeout)
                .build()
                .expect("Failed to build HTTP client"),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            authorization,
            model: model.into(),
            provider_id: provider_id.into(),
            max_attempts: 3,
        }
    }

    fn build_messages(&self, req: &ProviderRequest) -> Vec<Value> {
        let mut messages: Vec<Value> = Vec::new();

        if let Some(system_prompt) = &req.system_prompt
            && !system_prompt.is_empty() {
                messages.push(json!({
                    "role": "system",
                    "content": system_prompt
                }));
            }

        // Validate context entries (skip non-objects)
        for ctx in &req.contexts {
            if ctx.is_object() {
                messages.push(ctx.clone());
            } else {
                warn!("Skipping invalid context entry: {ctx}");
            }
        }

        if let Some(prompt) = &req.prompt {
            messages.push(json!({
                "role": "user",
                "content": prompt
            }));
        }

        messages
    }
}

#[async_trait]
impl Provider for OpenAICompatProvider {
    fn meta(&self) -> ProviderMeta {
        ProviderMeta {
            id: self.provider_id.clone(),
            model: Some(self.model.clone()),
            provider_type: "openai_chat_completion".to_string(),
            provider_kind: ProviderType::ChatCompletion,
        }
    }

    async fn text_chat(&self, req: ProviderRequest) -> Result<LLMResponse> {
        let model = req.model.as_deref().unwrap_or(&self.model);
        let messages = self.build_messages(&req);

        let request_body = ChatCompletionRequest {
            model: model.to_string(),
            messages,
            stream: false,
            max_tokens: None,
            temperature: None,
        };

        let url = format!("{}/v1/chat/completions", self.base_url);
        let mut last_error = None;

        for attempt in 0..self.max_attempts {
            if attempt > 0 {
                let delay = Duration::from_millis(1000 * (1 << attempt));
                tokio::time::sleep(delay).await;
                debug!("Retry attempt {} for {}", attempt + 1, url);
            }

            match self
                .client
                .post(&url)
                .header("Authorization", &self.authorization)
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
            {
                Ok(response) => {
                    let status = response.status();
                    // Don't retry client errors (4xx)
                    if status.is_client_error() {
                        let body = response.text().await.unwrap_or_default();
                        error!("API client error ({}): {body}", status);
                        return Err(crate::AstrBotError::Provider(format!(
                            "API error {status}: {body}"
                        )));
                    }
                    if !status.is_success() {
                        let body = response.text().await.unwrap_or_default();
                        warn!("API server error ({}): {body}", status);
                        last_error = Some(format!("HTTP {status}: {body}"));
                        continue;
                    }

                    match response.json::<ChatCompletionResponse>().await {
                        Ok(chat_resp) => {
                            if let Some(choice) = chat_resp.choices.into_iter().next() {
                                let completion_text = choice
                                    .message
                                    .content
                                    .unwrap_or_default();

                                let usage = chat_resp.usage.map(|u| {
                                    let cached = u64::from(u.cached_tokens.unwrap_or(0));
                                    let prompt_total = u64::from(u.prompt_tokens);
                                    let output = u64::from(u.completion_tokens);
                                    crate::entities::TokenUsage {
                                        input_other: prompt_total.saturating_sub(cached) as u32,
                                        input_cached: cached as u32,
                                        output: output as u32,
                                    }
                                });

                                return Ok(LLMResponse {
                                    role: choice.message.role,
                                    completion_text,
                                    reasoning_content: choice.message.reasoning_content,
                                    is_chunk: false,
                                    usage,
                                });
                            }
                            last_error = Some("No choices in response".to_string());
                        }
                        Err(e) => {
                            last_error = Some(format!("Parse error: {e}"));
                        }
                    }
                }
                Err(e) => {
                    warn!("Request attempt {}/{} failed: {e}", attempt + 1, self.max_attempts);
                    last_error = Some(format!("{e}"));
                }
            }
        }

        error!("All {max_attempts} attempts failed for {url}", max_attempts = self.max_attempts);
        Err(crate::AstrBotError::Provider(
            last_error.unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }

    async fn text_chat_stream(
        &self,
        _req: ProviderRequest,
    ) -> crate::Result<
        std::pin::Pin<
            Box<dyn tokio_stream::Stream<Item = crate::Result<LLMResponse>> + Send>,
        >,
    > {
        Err(crate::AstrBotError::Provider(
            "Streaming not yet implemented".to_string(),
        ))
    }
}
