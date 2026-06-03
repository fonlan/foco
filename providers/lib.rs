use std::fmt;

use futures_util::StreamExt;
use genai::{
    Client,
    adapter::AdapterKind,
    chat::{
        ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent, ReasoningEffort, StreamEnd, Usage,
    },
    resolver::{AuthData, Endpoint, ProviderConfig},
};
use serde::{Deserialize, Serialize};

pub const OPENAI_CHAT_KIND: &str = "openai-chat";
pub const OPENAI_RESPONSES_KIND: &str = "openai-responses";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1/";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAiChat,
    OpenAiResponses,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiChat => OPENAI_CHAT_KIND,
            Self::OpenAiResponses => OPENAI_RESPONSES_KIND,
        }
    }

    pub fn adapter_kind(self) -> AdapterKind {
        match self {
            Self::OpenAiChat => AdapterKind::OpenAI,
            Self::OpenAiResponses => AdapterKind::OpenAIResp,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderConnectionConfig {
    pub kind: ProviderKind,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

impl ProviderConnectionConfig {
    pub fn genai_client(&self) -> Result<Client, ProviderConfigError> {
        let endpoint = Endpoint::from_owned(self.endpoint_url()?);
        let auth = self.auth_data()?;
        let resolver_endpoint = endpoint.clone();
        let resolver_auth = auth.clone();

        Ok(Client::builder()
            .with_adapter_kind(self.kind.adapter_kind())
            .with_service_target_resolver_fn(move |mut target: genai::ServiceTarget| {
                target.endpoint = resolver_endpoint.clone();
                target.auth = resolver_auth.clone();
                Ok(target)
            })
            .build())
    }

    pub fn genai_provider_config(&self) -> Result<ProviderConfig, ProviderConfigError> {
        Ok(ProviderConfig::default()
            .with_endpoint(Endpoint::from_owned(self.endpoint_url()?))
            .with_auth(self.auth_data()?))
    }

    fn endpoint_url(&self) -> Result<String, ProviderConfigError> {
        let base_url = self.base_url.as_deref().unwrap_or(DEFAULT_OPENAI_BASE_URL);
        normalized_base_url(base_url)
    }

    fn auth_data(&self) -> Result<AuthData, ProviderConfigError> {
        let api_key = self
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(ProviderConfigError::MissingApiKey)?;

        Ok(AuthData::from_single(api_key.to_string()))
    }
}

pub async fn test_provider_connection(
    config: &ProviderConnectionConfig,
) -> Result<usize, ProviderConfigError> {
    let client = config.genai_client()?;
    let provider_config = config.genai_provider_config()?;
    let models = client
        .all_model_names(config.kind.adapter_kind(), provider_config)
        .await
        .map_err(|source| ProviderConfigError::Connection(source.to_string()))?;

    Ok(models.len())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeutralChatRequest {
    pub model_id: String,
    pub messages: Vec<NeutralChatMessage>,
    pub thinking_level: Option<String>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeutralChatMessage {
    pub role: NeutralChatRole,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NeutralChatRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum NeutralChatStreamEvent {
    Start,
    TextDelta {
        delta: String,
    },
    ReasoningDelta {
        delta: String,
    },
    Usage {
        usage: NeutralUsage,
    },
    Complete {
        text: String,
        reasoning: Option<String>,
        usage: Option<NeutralUsage>,
        stop_reason: Option<String>,
        response_id: Option<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeutralUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
}

pub struct NeutralChatStream {
    stream: genai::chat::ChatStream,
}

impl NeutralChatStream {
    pub async fn next_event(
        &mut self,
    ) -> Option<Result<NeutralChatStreamEvent, ProviderConfigError>> {
        let event = self.stream.next().await?;
        Some(match event {
            Ok(event) => normalize_stream_event(event),
            Err(source) => Err(ProviderConfigError::Connection(source.to_string())),
        })
    }
}

pub async fn stream_chat(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
) -> Result<NeutralChatStream, ProviderConfigError> {
    match config.kind {
        ProviderKind::OpenAiChat | ProviderKind::OpenAiResponses => {}
    }

    let client = config.genai_client()?;
    let chat_request = genai_chat_request(&request)?;
    let options = genai_chat_options(&request)?;
    let model = genai::ModelIden::new(config.kind.adapter_kind(), request.model_id);
    let response = client
        .exec_chat_stream(model, chat_request, Some(&options))
        .await
        .map_err(|source| ProviderConfigError::Connection(source.to_string()))?;

    Ok(NeutralChatStream {
        stream: response.stream,
    })
}

pub fn parse_provider_kind(value: &str) -> Result<ProviderKind, ProviderConfigError> {
    match value.trim() {
        OPENAI_CHAT_KIND => Ok(ProviderKind::OpenAiChat),
        OPENAI_RESPONSES_KIND => Ok(ProviderKind::OpenAiResponses),
        other => Err(ProviderConfigError::UnsupportedKind(other.to_string())),
    }
}

pub fn normalized_base_url(value: &str) -> Result<String, ProviderConfigError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(ProviderConfigError::EmptyBaseUrl);
    }

    let mut url =
        reqwest::Url::parse(trimmed).map_err(|source| ProviderConfigError::InvalidBaseUrl {
            value: trimmed.to_string(),
            source: source.to_string(),
        })?;

    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(ProviderConfigError::InvalidBaseUrl {
            value: trimmed.to_string(),
            source: "scheme must be http or https".to_string(),
        });
    }

    if !url.path().ends_with('/') {
        let next_path = format!("{}/", url.path());
        url.set_path(&next_path);
    }

    Ok(url.to_string())
}

fn genai_chat_request(request: &NeutralChatRequest) -> Result<ChatRequest, ProviderConfigError> {
    if request.model_id.trim().is_empty() {
        return Err(ProviderConfigError::InvalidRequest(
            "model id must not be empty".to_string(),
        ));
    }

    if request.messages.is_empty() {
        return Err(ProviderConfigError::InvalidRequest(
            "chat request must contain at least one message".to_string(),
        ));
    }

    let mut messages = Vec::with_capacity(request.messages.len());
    for message in &request.messages {
        if message.content.trim().is_empty() {
            return Err(ProviderConfigError::InvalidRequest(
                "chat message content must not be empty".to_string(),
            ));
        }

        messages.push(match message.role {
            NeutralChatRole::System => ChatMessage::system(message.content.clone()),
            NeutralChatRole::User => ChatMessage::user(message.content.clone()),
            NeutralChatRole::Assistant => ChatMessage::assistant(message.content.clone()),
        });
    }

    Ok(ChatRequest::from_messages(messages))
}

fn genai_chat_options(request: &NeutralChatRequest) -> Result<ChatOptions, ProviderConfigError> {
    let mut options = ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(true)
        .with_capture_tool_calls(true);

    if let Some(max_output_tokens) = request.max_output_tokens {
        options = options.with_max_tokens(max_output_tokens);
    }

    if let Some(thinking_level) = request.thinking_level.as_deref() {
        let effort = thinking_level.parse::<ReasoningEffort>().map_err(|_| {
            ProviderConfigError::InvalidRequest(format!(
                "unsupported thinking level '{thinking_level}'"
            ))
        })?;
        options = options.with_reasoning_effort(effort);
    }

    Ok(options)
}

fn normalize_stream_event(
    event: ChatStreamEvent,
) -> Result<NeutralChatStreamEvent, ProviderConfigError> {
    match event {
        ChatStreamEvent::Start => Ok(NeutralChatStreamEvent::Start),
        ChatStreamEvent::Chunk(chunk) => Ok(NeutralChatStreamEvent::TextDelta {
            delta: chunk.content,
        }),
        ChatStreamEvent::ReasoningChunk(chunk) => Ok(NeutralChatStreamEvent::ReasoningDelta {
            delta: chunk.content,
        }),
        ChatStreamEvent::ThoughtSignatureChunk(_) | ChatStreamEvent::ToolCallChunk(_) => {
            Err(ProviderConfigError::MissingRequiredField(
                "minimal streaming chat requires text completion; tool and thought-signature chunks belong to TODO step 09"
                    .to_string(),
            ))
        }
        ChatStreamEvent::End(end) => normalize_stream_end(end),
    }
}

fn normalize_stream_end(end: StreamEnd) -> Result<NeutralChatStreamEvent, ProviderConfigError> {
    let text = end
        .captured_first_text()
        .ok_or_else(|| {
            ProviderConfigError::MissingRequiredField(
                "provider stream ended without captured assistant text".to_string(),
            )
        })?
        .to_string();
    let usage = end.captured_usage.as_ref().map(neutral_usage);
    let stop_reason = end
        .captured_stop_reason
        .as_ref()
        .map(|reason| reason.raw().to_string());

    Ok(NeutralChatStreamEvent::Complete {
        text,
        reasoning: end.captured_reasoning_content,
        usage,
        stop_reason,
        response_id: end.captured_response_id,
    })
}

fn neutral_usage(usage: &Usage) -> NeutralUsage {
    NeutralUsage {
        input_tokens: usage.prompt_tokens.map(i64::from),
        output_tokens: usage.completion_tokens.map(i64::from),
        cache_read_tokens: usage
            .prompt_tokens_details
            .as_ref()
            .and_then(|details| details.cached_tokens)
            .map(i64::from),
        cache_write_tokens: usage
            .prompt_tokens_details
            .as_ref()
            .and_then(|details| details.cache_creation_tokens)
            .map(i64::from),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderConfigError {
    Connection(String),
    EmptyBaseUrl,
    InvalidBaseUrl { value: String, source: String },
    InvalidRequest(String),
    MissingRequiredField(String),
    MissingApiKey,
    UnsupportedKind(String),
}

impl fmt::Display for ProviderConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(source) => write!(formatter, "provider connection failed: {source}"),
            Self::EmptyBaseUrl => write!(formatter, "provider base URL must not be empty"),
            Self::InvalidBaseUrl { value, source } => {
                write!(
                    formatter,
                    "provider base URL '{value}' is invalid: {source}"
                )
            }
            Self::InvalidRequest(message) => {
                write!(formatter, "invalid provider request: {message}")
            }
            Self::MissingRequiredField(message) => write!(
                formatter,
                "provider did not return required streaming field: {message}"
            ),
            Self::MissingApiKey => write!(formatter, "provider API key must not be empty"),
            Self::UnsupportedKind(kind) => write!(
                formatter,
                "unsupported provider kind '{kind}'; expected '{OPENAI_CHAT_KIND}' or '{OPENAI_RESPONSES_KIND}'"
            ),
        }
    }
}

impl std::error::Error for ProviderConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_provider_kinds() {
        assert_eq!(
            parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind"),
            ProviderKind::OpenAiChat
        );
        assert_eq!(
            parse_provider_kind(OPENAI_RESPONSES_KIND).expect("responses kind"),
            ProviderKind::OpenAiResponses
        );
    }

    #[test]
    fn rejects_unknown_provider_kind() {
        let error = parse_provider_kind("openai").expect_err("unsupported kind should fail");

        assert!(error.to_string().contains("unsupported provider kind"));
    }

    #[test]
    fn normalizes_base_url_for_genai_joining() {
        assert_eq!(
            normalized_base_url("https://api.openai.com/v1").expect("base url"),
            DEFAULT_OPENAI_BASE_URL
        );
    }
}
