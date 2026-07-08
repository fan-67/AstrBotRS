use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProviderType {
    #[serde(rename = "chat_completion")]
    ChatCompletion,
    #[serde(rename = "speech_to_text")]
    SpeechToText,
    #[serde(rename = "text_to_speech")]
    TextToSpeech,
    #[serde(rename = "embedding")]
    Embedding,
    #[serde(rename = "rerank")]
    Rerank,
}

impl ProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderType::ChatCompletion => "chat_completion",
            ProviderType::SpeechToText => "speech_to_text",
            ProviderType::TextToSpeech => "text_to_speech",
            ProviderType::Embedding => "embedding",
            ProviderType::Rerank => "rerank",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMeta {
    pub id: String,
    pub model: Option<String>,
    pub provider_type: String,
    pub provider_kind: ProviderType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct TokenUsage {
    pub input_other: u32,
    pub input_cached: u32,
    pub output: u32,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        (self.input_other as u64)
            .saturating_add(self.input_cached as u64)
            .saturating_add(self.output as u64)
    }

    pub fn input(&self) -> u64 {
        (self.input_other as u64).saturating_add(self.input_cached as u64)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub role: String,
    pub completion_text: String,
    pub reasoning_content: Option<String>,
    pub is_chunk: bool,
    pub usage: Option<TokenUsage>,
}

impl LLMResponse {
    pub fn text(completion_text: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            completion_text: completion_text.into(),
            reasoning_content: None,
            is_chunk: false,
            usage: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderRequest {
    pub prompt: Option<String>,
    pub system_prompt: Option<String>,
    pub contexts: Vec<serde_json::Value>,
    pub model: Option<String>,
    pub stream: bool,
}

impl ProviderRequest {
    pub fn prompt(prompt: impl Into<String>) -> Self {
        Self {
            prompt: Some(prompt.into()),
            ..Default::default()
        }
    }
}
