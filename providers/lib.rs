use std::fmt;

use futures_util::StreamExt;
use genai::{
    Client,
    adapter::AdapterKind,
    chat::{
        ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent, ContentPart, MessageContent,
        ReasoningEffort, StreamEnd, Tool, ToolCall as GenaiToolCall, ToolResponse, Usage,
    },
    resolver::{AuthData, Endpoint, ProviderConfig},
};
use reqwest::StatusCode;
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
        .map_err(ProviderConfigError::from_genai_error)?;

    Ok(models.len())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NeutralChatRequest {
    pub model_id: String,
    pub messages: Vec<NeutralChatMessage>,
    #[serde(default)]
    pub tools: Vec<NeutralToolDefinition>,
    pub thinking_level: Option<String>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NeutralChatMessage {
    pub role: NeutralChatRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<NeutralToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NeutralChatRole {
    System,
    User,
    Assistant,
    Tool,
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
    ThoughtSignatureDelta {
        delta: String,
    },
    ToolCall {
        tool_call: NeutralToolCall,
    },
    Usage {
        usage: NeutralUsage,
    },
    Complete {
        text: String,
        reasoning: Option<String>,
        tool_calls: Vec<NeutralToolCall>,
        usage: Option<NeutralUsage>,
        stop_reason: Option<String>,
        response_id: Option<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeutralToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub strict: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeutralToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signatures: Option<Vec<String>>,
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
            Err(source) => Err(ProviderConfigError::from_genai_error(source)),
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
        .map_err(ProviderConfigError::from_genai_error)?;

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
        messages.push(genai_message(message)?);
    }

    let mut chat_request = ChatRequest::from_messages(messages);
    if !request.tools.is_empty() {
        chat_request = chat_request.with_tools(request.tools.iter().map(genai_tool));
    }

    Ok(chat_request)
}

fn genai_message(message: &NeutralChatMessage) -> Result<ChatMessage, ProviderConfigError> {
    match message.role {
        NeutralChatRole::System | NeutralChatRole::User => {
            if message.content.trim().is_empty() {
                return Err(ProviderConfigError::InvalidRequest(
                    "chat message content must not be empty".to_string(),
                ));
            }
            if !message.tool_calls.is_empty() || message.tool_call_id.is_some() {
                return Err(ProviderConfigError::InvalidRequest(
                    "system and user messages cannot contain tool state".to_string(),
                ));
            }

            Ok(match message.role {
                NeutralChatRole::System => ChatMessage::system(message.content.clone()),
                NeutralChatRole::User => ChatMessage::user(message.content.clone()),
                NeutralChatRole::Assistant | NeutralChatRole::Tool => unreachable!(),
            })
        }
        NeutralChatRole::Assistant => {
            if message.tool_calls.is_empty() {
                if message.content.trim().is_empty() {
                    return Err(ProviderConfigError::InvalidRequest(
                        "assistant message content must not be empty unless it contains tool calls"
                            .to_string(),
                    ));
                }

                let mut chat_message = ChatMessage::assistant(message.content.clone());
                if let Some(reasoning) = message
                    .reasoning
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                {
                    chat_message = chat_message.with_reasoning_content(Some(reasoning.to_string()));
                }

                return Ok(chat_message);
            }

            let tool_calls = message
                .tool_calls
                .iter()
                .map(genai_tool_call)
                .collect::<Vec<_>>();
            if message.content.trim().is_empty()
                && message
                    .reasoning
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
            {
                return Ok(ChatMessage::from(tool_calls));
            }

            let mut parts = Vec::new();
            if !message.content.trim().is_empty() {
                parts.push(ContentPart::Text(message.content.clone()));
            }
            if let Some(reasoning) = message
                .reasoning
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                parts.push(ContentPart::ReasoningContent(reasoning.to_string()));
            }
            if let Some(thought_signatures) = tool_calls
                .first()
                .and_then(|tool_call| tool_call.thought_signatures.clone())
            {
                parts.extend(
                    thought_signatures
                        .into_iter()
                        .map(ContentPart::ThoughtSignature),
                );
            }
            parts.extend(tool_calls.into_iter().map(ContentPart::ToolCall));

            Ok(ChatMessage::assistant(MessageContent::from_parts(parts)))
        }
        NeutralChatRole::Tool => {
            if !message.tool_calls.is_empty() {
                return Err(ProviderConfigError::InvalidRequest(
                    "tool messages cannot contain tool calls".to_string(),
                ));
            }
            if message.content.trim().is_empty() {
                return Err(ProviderConfigError::InvalidRequest(
                    "tool response content must not be empty".to_string(),
                ));
            }
            let tool_call_id = message.tool_call_id.as_deref().ok_or_else(|| {
                ProviderConfigError::InvalidRequest(
                    "tool response message is missing tool_call_id".to_string(),
                )
            })?;
            let mut response = ToolResponse::new(tool_call_id, message.content.clone());
            if let Some(tool_name) = message.tool_name.as_deref() {
                response = response.with_fn_name(tool_name);
            }

            Ok(ChatMessage::from(response))
        }
    }
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
        ChatStreamEvent::ThoughtSignatureChunk(chunk) => {
            Ok(NeutralChatStreamEvent::ThoughtSignatureDelta {
                delta: chunk.content,
            })
        }
        ChatStreamEvent::ToolCallChunk(chunk) => Ok(NeutralChatStreamEvent::ToolCall {
            tool_call: neutral_tool_call(&chunk.tool_call),
        }),
        ChatStreamEvent::End(end) => normalize_stream_end(end),
    }
}

fn normalize_stream_end(end: StreamEnd) -> Result<NeutralChatStreamEvent, ProviderConfigError> {
    let text = end.captured_first_text().unwrap_or_default().to_string();
    let tool_calls = end
        .captured_tool_calls()
        .unwrap_or_default()
        .into_iter()
        .map(neutral_tool_call)
        .collect();
    let usage = end.captured_usage.as_ref().map(neutral_usage);
    let stop_reason = end
        .captured_stop_reason
        .as_ref()
        .map(|reason| reason.raw().to_string());

    Ok(NeutralChatStreamEvent::Complete {
        text,
        reasoning: end.captured_reasoning_content,
        tool_calls,
        usage,
        stop_reason,
        response_id: end.captured_response_id,
    })
}

fn genai_tool(tool: &NeutralToolDefinition) -> Tool {
    Tool::new(tool.name.clone())
        .with_description(tool.description.clone())
        .with_schema(tool.input_schema.clone())
        .with_strict(tool.strict)
}

fn neutral_tool_call(tool_call: &GenaiToolCall) -> NeutralToolCall {
    NeutralToolCall {
        call_id: tool_call.call_id.clone(),
        name: tool_call.fn_name.clone(),
        arguments: normalized_tool_arguments(&tool_call.fn_arguments),
        thought_signatures: tool_call.thought_signatures.clone(),
    }
}

fn normalized_tool_arguments(arguments: &serde_json::Value) -> serde_json::Value {
    let mut current = arguments.clone();

    for _ in 0..4 {
        let serde_json::Value::String(text) = &current else {
            return current;
        };

        let trimmed = text.trim();
        let looks_like_json = trimmed.starts_with('{')
            || trimmed.starts_with('[')
            || trimmed.starts_with("\"{")
            || trimmed.starts_with("\"[");
        if !looks_like_json {
            return current;
        }

        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            return current;
        };
        current = parsed;
    }

    current
}

fn genai_tool_call(tool_call: &NeutralToolCall) -> GenaiToolCall {
    GenaiToolCall {
        call_id: tool_call.call_id.clone(),
        fn_name: tool_call.name.clone(),
        fn_arguments: tool_call.arguments.clone(),
        thought_signatures: tool_call.thought_signatures.clone(),
    }
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
    Connection {
        message: String,
        status_code: Option<u16>,
    },
    EmptyBaseUrl,
    InvalidBaseUrl {
        value: String,
        source: String,
    },
    InvalidRequest(String),
    MissingRequiredField(String),
    MissingApiKey,
    UnsupportedKind(String),
}

impl ProviderConfigError {
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Connection { status_code, .. } => *status_code,
            Self::EmptyBaseUrl
            | Self::InvalidBaseUrl { .. }
            | Self::InvalidRequest(_)
            | Self::MissingRequiredField(_)
            | Self::MissingApiKey
            | Self::UnsupportedKind(_) => None,
        }
    }

    fn from_genai_error(source: genai::Error) -> Self {
        let status_code = genai_error_status_code(&source).map(|status| status.as_u16());

        Self::Connection {
            message: source.to_string(),
            status_code,
        }
    }
}

impl fmt::Display for ProviderConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection { message, .. } => {
                write!(formatter, "provider connection failed: {message}")
            }
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

fn genai_error_status_code(source: &genai::Error) -> Option<StatusCode> {
    match source {
        genai::Error::HttpError { status, .. } => Some(*status),
        genai::Error::WebModelCall { webc_error, .. }
        | genai::Error::WebAdapterCall { webc_error, .. } => webc_error_status_code(webc_error),
        genai::Error::WebStream { error, .. } => error
            .downcast_ref::<genai::Error>()
            .and_then(genai_error_status_code),
        genai::Error::ChatReqHasNoMessages { .. }
        | genai::Error::LastChatMessageIsNotUser { .. }
        | genai::Error::MessageRoleNotSupported { .. }
        | genai::Error::MessageContentTypeNotSupported { .. }
        | genai::Error::JsonModeWithoutInstruction
        | genai::Error::VerbosityParsing { .. }
        | genai::Error::ReasoningParsingError { .. }
        | genai::Error::ServiceTierParsing { .. }
        | genai::Error::PromptCacheRetentionParsing { .. }
        | genai::Error::NoChatResponse { .. }
        | genai::Error::InvalidJsonResponseElement { .. }
        | genai::Error::RequiresApiKey { .. }
        | genai::Error::NoAuthResolver { .. }
        | genai::Error::NoAuthData { .. }
        | genai::Error::ModelMapperFailed { .. }
        | genai::Error::ChatResponseGeneration { .. }
        | genai::Error::ChatResponse { .. }
        | genai::Error::StreamParse { .. }
        | genai::Error::Resolver { .. }
        | genai::Error::AdapterNotSupported { .. }
        | genai::Error::AdapterKindMismatch { .. }
        | genai::Error::Internal(_)
        | genai::Error::JsonValueExt(_)
        | genai::Error::SerdeJson(_) => None,
    }
}

fn webc_error_status_code(source: &genai::webc::Error) -> Option<StatusCode> {
    match source {
        genai::webc::Error::ResponseFailedStatus { status, .. } => Some(*status),
        genai::webc::Error::ResponseFailedNotJson { .. }
        | genai::webc::Error::ResponseFailedInvalidJson { .. }
        | genai::webc::Error::JsonValueExt(_)
        | genai::webc::Error::Reqwest(_) => None,
    }
}

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

    #[test]
    fn normalizes_json_string_tool_arguments() {
        assert_eq!(
            normalized_tool_arguments(&serde_json::Value::String(
                r#"{"path":"note.txt"}"#.to_string()
            )),
            serde_json::json!({ "path": "note.txt" })
        );

        let double_encoded =
            serde_json::to_string(r#"{"path":"note.txt"}"#).expect("double encoded JSON argument");
        assert_eq!(
            normalized_tool_arguments(&serde_json::Value::String(double_encoded)),
            serde_json::json!({ "path": "note.txt" })
        );

        assert_eq!(
            normalized_tool_arguments(&serde_json::Value::String("plain text".to_string())),
            serde_json::Value::String("plain text".to_string())
        );
    }

    #[test]
    fn converts_tool_state_messages_for_genai_continuation() {
        let request = NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages: vec![
                NeutralChatMessage {
                    role: NeutralChatRole::User,
                    content: "Read the note.".to_string(),
                    reasoning: None,
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::Assistant,
                    content: String::new(),
                    reasoning: None,
                    tool_calls: vec![NeutralToolCall {
                        call_id: "call-1".to_string(),
                        name: "read_file".to_string(),
                        arguments: serde_json::json!({ "path": "note.txt" }),
                        thought_signatures: None,
                    }],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::Tool,
                    content: r#"{"content":"hello"}"#.to_string(),
                    reasoning: None,
                    tool_calls: Vec::new(),
                    tool_call_id: Some("call-1".to_string()),
                    tool_name: Some("read_file".to_string()),
                },
            ],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
        };

        let chat_request = genai_chat_request(&request).expect("chat request");

        assert!(chat_request.messages[1].content.contains_tool_call());
        assert!(chat_request.messages[2].content.contains_tool_response());
    }
}
