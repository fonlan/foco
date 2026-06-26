use std::fmt;

use futures_util::StreamExt;
use genai::{
    Client, Headers, WebConfig,
    adapter::AdapterKind,
    chat::{
        CacheControl, ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent, ContentPart,
        MessageContent, ReasoningEffort, StreamEnd, Tool, ToolCall as GenaiToolCall, ToolResponse,
        Usage,
    },
    resolver::{AuthData, Endpoint, ProviderConfig},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const OPENAI_CHAT_KIND: &str = "openai-chat";
pub const OPENAI_RESPONSES_KIND: &str = "openai-responses";
pub const GEMINI_KIND: &str = "gemini";
pub const ANTHROPIC_KIND: &str = "anthropic";
pub const FIREWORKS_KIND: &str = "fireworks";
pub const TOGETHER_KIND: &str = "together";
pub const GROQ_KIND: &str = "groq";
pub const AIHUBMIX_KIND: &str = "aihubmix";
pub const MIMO_KIND: &str = "mimo";
pub const MOONSHOT_KIND: &str = "moonshot";
pub const NEBIUS_KIND: &str = "nebius";
pub const XAI_KIND: &str = "xai";
pub const DEEPSEEK_KIND: &str = "deepseek";
pub const ZAI_KIND: &str = "zai";
pub const BIGMODEL_KIND: &str = "bigmodel";
pub const ALIYUN_KIND: &str = "aliyun";
pub const BAIDU_KIND: &str = "baidu";
pub const COHERE_KIND: &str = "cohere";
pub const OLLAMA_KIND: &str = "ollama";
pub const OLLAMA_CLOUD_KIND: &str = "ollama-cloud";
pub const VERTEX_KIND: &str = "vertex";
pub const GITHUB_COPILOT_KIND: &str = "github-copilot";
pub const OPENCODE_GO_KIND: &str = "opencode-go";
pub const BEDROCK_API_KIND: &str = "bedrock-api";
pub const OPEN_ROUTER_KIND: &str = "open-router";
pub const MINIMAX_KIND: &str = "minimax";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1/";
pub const HTTP_PROXY_KIND: &str = "http";
pub const SOCKS_PROXY_KIND: &str = "socks";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderKind {
    kind: &'static str,
    label: &'static str,
    adapter_kind: AdapterKind,
    default_base_url: &'static str,
    requires_api_key: bool,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        self.kind
    }

    pub fn adapter_kind(self) -> AdapterKind {
        self.adapter_kind
    }

    pub fn label(self) -> &'static str {
        self.label
    }

    pub fn default_base_url(self) -> &'static str {
        self.default_base_url
    }

    pub fn requires_api_key(self) -> bool {
        self.requires_api_key
    }

    fn adapter_label(self) -> &'static str {
        self.adapter_kind.as_str()
    }
}

macro_rules! provider_kind {
    ($kind:ident, $label:literal, $adapter_kind:ident, $base_url:expr) => {
        ProviderKind {
            kind: $kind,
            label: $label,
            adapter_kind: AdapterKind::$adapter_kind,
            default_base_url: $base_url,
            requires_api_key: true,
        }
    };
    ($kind:ident, $label:literal, $adapter_kind:ident, $base_url:expr, no_api_key) => {
        ProviderKind {
            kind: $kind,
            label: $label,
            adapter_kind: AdapterKind::$adapter_kind,
            default_base_url: $base_url,
            requires_api_key: false,
        }
    };
}

pub const SUPPORTED_PROVIDER_KINDS: &[ProviderKind] = &[
    provider_kind!(
        OPENAI_CHAT_KIND,
        "OpenAI Chat",
        OpenAI,
        DEFAULT_OPENAI_BASE_URL
    ),
    provider_kind!(
        OPENAI_RESPONSES_KIND,
        "OpenAI Responses",
        OpenAIResp,
        DEFAULT_OPENAI_BASE_URL
    ),
    provider_kind!(
        GEMINI_KIND,
        "Gemini",
        Gemini,
        "https://generativelanguage.googleapis.com/v1beta/"
    ),
    provider_kind!(
        ANTHROPIC_KIND,
        "Anthropic",
        Anthropic,
        "https://api.anthropic.com/v1/"
    ),
    provider_kind!(
        FIREWORKS_KIND,
        "Fireworks",
        Fireworks,
        "https://api.fireworks.ai/inference/v1/"
    ),
    provider_kind!(
        TOGETHER_KIND,
        "Together",
        Together,
        "https://api.together.xyz/v1/"
    ),
    provider_kind!(GROQ_KIND, "Groq", Groq, "https://api.groq.com/openai/v1/"),
    provider_kind!(
        AIHUBMIX_KIND,
        "AIHubMix",
        Aihubmix,
        "https://aihubmix.com/v1/"
    ),
    provider_kind!(MIMO_KIND, "Mimo", Mimo, "https://api.xiaomimimo.com/v1/"),
    provider_kind!(
        MOONSHOT_KIND,
        "Moonshot",
        Moonshot,
        "https://api.moonshot.cn/v1/"
    ),
    provider_kind!(
        NEBIUS_KIND,
        "Nebius",
        Nebius,
        "https://api.studio.nebius.ai/v1/"
    ),
    provider_kind!(XAI_KIND, "xAI", Xai, "https://api.x.ai/v1/"),
    provider_kind!(
        DEEPSEEK_KIND,
        "DeepSeek",
        DeepSeek,
        "https://api.deepseek.com/v1/"
    ),
    provider_kind!(ZAI_KIND, "ZAI", Zai, "https://api.z.ai/api/paas/v4/"),
    provider_kind!(
        BIGMODEL_KIND,
        "BigModel",
        BigModel,
        "https://open.bigmodel.cn/api/paas/v4/"
    ),
    provider_kind!(
        ALIYUN_KIND,
        "Aliyun",
        Aliyun,
        "https://dashscope.aliyuncs.com/compatible-mode/v1/"
    ),
    provider_kind!(
        BAIDU_KIND,
        "Baidu",
        Baidu,
        "https://qianfan.baidubce.com/v2/"
    ),
    provider_kind!(COHERE_KIND, "Cohere", Cohere, "https://api.cohere.com/v1/"),
    provider_kind!(
        OLLAMA_KIND,
        "Ollama",
        Ollama,
        "http://localhost:11434/",
        no_api_key
    ),
    provider_kind!(
        OLLAMA_CLOUD_KIND,
        "Ollama Cloud",
        OllamaCloud,
        "https://ollama.com/"
    ),
    provider_kind!(
        VERTEX_KIND,
        "Vertex AI",
        Vertex,
        "https://aiplatform.googleapis.com/v1/projects/PROJECT_ID/locations/global/"
    ),
    provider_kind!(
        GITHUB_COPILOT_KIND,
        "GitHub Copilot",
        GithubCopilot,
        "https://models.github.ai/inference/"
    ),
    provider_kind!(
        OPENCODE_GO_KIND,
        "OpenCode Go",
        OpenCodeGo,
        "https://opencode.ai/zen/go/v1/"
    ),
    provider_kind!(
        BEDROCK_API_KIND,
        "Bedrock API",
        BedrockApi,
        "https://bedrock-runtime.us-east-1.amazonaws.com/"
    ),
    provider_kind!(
        OPEN_ROUTER_KIND,
        "OpenRouter",
        OpenRouter,
        "https://openrouter.ai/api/v1/"
    ),
    provider_kind!(
        MINIMAX_KIND,
        "MiniMax",
        MiniMax,
        "https://api.minimax.io/anthropic/v1/"
    ),
];

pub fn supported_provider_kinds() -> &'static [ProviderKind] {
    SUPPORTED_PROVIDER_KINDS
}

const REQUEST_OVERRIDE_TARGET_HEADER: &str = "header";
const REQUEST_OVERRIDE_TARGET_BODY: &str = "body";
const REQUEST_OVERRIDE_VALUE_TYPE_STRING: &str = "string";
const REQUEST_OVERRIDE_VALUE_TYPE_NUMBER: &str = "number";
const REQUEST_OVERRIDE_VALUE_TYPE_BOOLEAN: &str = "boolean";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRequestOverride {
    pub target: String,
    pub name: String,
    pub value_type: String,
    pub value: Value,
}

impl ProviderRequestOverride {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        self.normalized_target()?;
        self.normalized_name()?;
        self.normalized_value()?;
        Ok(())
    }

    fn normalized_target(&self) -> Result<&str, ProviderConfigError> {
        let target = self.target.trim();

        match target {
            REQUEST_OVERRIDE_TARGET_HEADER | REQUEST_OVERRIDE_TARGET_BODY => Ok(target),
            _ => Err(ProviderConfigError::InvalidRequest(format!(
                "request override target must be '{REQUEST_OVERRIDE_TARGET_HEADER}' or '{REQUEST_OVERRIDE_TARGET_BODY}': {target}"
            ))),
        }
    }

    fn normalized_name(&self) -> Result<&str, ProviderConfigError> {
        let name = self.name.trim();

        if name.is_empty() {
            return Err(ProviderConfigError::InvalidRequest(
                "request override name must not be empty".to_string(),
            ));
        }

        Ok(name)
    }

    fn normalized_value(&self) -> Result<Value, ProviderConfigError> {
        let value_type = self.value_type.trim();

        match value_type {
            REQUEST_OVERRIDE_VALUE_TYPE_STRING => self
                .value
                .as_str()
                .map(|value| Value::String(value.to_string()))
                .ok_or_else(|| {
                    ProviderConfigError::InvalidRequest(format!(
                        "request override '{}' value must be a string",
                        self.name
                    ))
                }),
            REQUEST_OVERRIDE_VALUE_TYPE_NUMBER => {
                if self.value.is_number() {
                    Ok(self.value.clone())
                } else {
                    Err(ProviderConfigError::InvalidRequest(format!(
                        "request override '{}' value must be a number",
                        self.name
                    )))
                }
            }
            REQUEST_OVERRIDE_VALUE_TYPE_BOOLEAN => {
                self.value.as_bool().map(Value::Bool).ok_or_else(|| {
                    ProviderConfigError::InvalidRequest(format!(
                        "request override '{}' value must be a boolean",
                        self.name
                    ))
                })
            }
            _ => Err(ProviderConfigError::InvalidRequest(format!(
                "request override value type must be '{REQUEST_OVERRIDE_VALUE_TYPE_STRING}', '{REQUEST_OVERRIDE_VALUE_TYPE_NUMBER}', or '{REQUEST_OVERRIDE_VALUE_TYPE_BOOLEAN}': {value_type}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderConnectionConfig {
    pub kind: ProviderKind,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub proxy_url: Option<String>,
    pub request_overrides: Vec<ProviderRequestOverride>,
}
impl ProviderConnectionConfig {
    fn provider_error_context(
        &self,
        phase: &'static str,
        model_id: &str,
    ) -> Result<ProviderErrorContext, ProviderConfigError> {
        Ok(ProviderErrorContext::new(self, phase, model_id))
    }

    pub fn genai_client(&self) -> Result<Client, ProviderConfigError> {
        let auth = self.auth_data()?;
        let resolver_auth = auth.clone();
        let endpoint = self.custom_endpoint()?;
        let resolver_endpoint = endpoint.clone();
        let mut builder = Client::builder()
            .with_adapter_kind(self.kind.adapter_kind())
            .with_service_target_resolver_fn(move |mut target: genai::ServiceTarget| {
                if let Some(endpoint) = resolver_endpoint.clone() {
                    target.endpoint = endpoint;
                }
                target.auth = resolver_auth.clone();
                Ok(target)
            });

        if let Some(proxy_url) = self.proxy_url.as_deref() {
            let proxy = self.reqwest_proxy(proxy_url)?;
            builder = builder.with_web_config(WebConfig::default().with_proxy(proxy));
        }

        Ok(builder.build())
    }

    pub fn genai_provider_config(&self) -> Result<ProviderConfig, ProviderConfigError> {
        let mut config = ProviderConfig::default().with_auth(self.auth_data()?);

        if let Some(endpoint) = self.custom_endpoint()? {
            config = config.with_endpoint(endpoint);
        }

        Ok(config)
    }

    fn custom_endpoint(&self) -> Result<Option<Endpoint>, ProviderConfigError> {
        self.base_url
            .as_deref()
            .map(|base_url| normalized_genai_endpoint_url(self.kind, base_url))
            .map(|result| result.map(Endpoint::from_owned))
            .transpose()
    }

    fn diagnostic_endpoint_url(&self) -> Result<String, ProviderConfigError> {
        normalized_genai_endpoint_url(
            self.kind,
            self.base_url
                .as_deref()
                .unwrap_or_else(|| self.kind.default_base_url()),
        )
    }

    fn auth_data(&self) -> Result<AuthData, ProviderConfigError> {
        match self
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(api_key) => Ok(AuthData::from_single(api_key.to_string())),
            None if !self.kind.requires_api_key() => Ok(AuthData::None),
            None => Err(ProviderConfigError::MissingApiKey),
        }
    }

    fn reqwest_proxy(&self, proxy_url: &str) -> Result<reqwest::Proxy, ProviderConfigError> {
        reqwest::Proxy::all(proxy_url).map_err(|source| ProviderConfigError::InvalidProxyUrl {
            value: proxy_url.to_string(),
            source: source.to_string(),
        })
    }
}

pub async fn fetch_provider_model_ids(
    config: &ProviderConnectionConfig,
) -> Result<Vec<String>, ProviderConfigError> {
    match fetch_provider_model_ids_once(config, "models").await {
        Ok(models) => Ok(models),
        Err(source) if should_retry_model_list_with_v1_endpoint(config, &source) => {
            let Some(retry_config) = model_list_v1_retry_config(config)? else {
                return Err(source);
            };

            fetch_provider_model_ids_once(&retry_config, "v1/models").await
        }
        Err(source) => Err(source),
    }
}

async fn fetch_provider_model_ids_once(
    config: &ProviderConnectionConfig,
    diagnostic_model_id: &str,
) -> Result<Vec<String>, ProviderConfigError> {
    let client = config.genai_client()?;
    let context = config.provider_error_context("listing provider models", diagnostic_model_id)?;
    let models = client
        .all_model_names(config.kind.adapter_kind(), config.genai_provider_config()?)
        .await
        .map_err(|source| ProviderConfigError::from_genai_error_with_context(source, &context))?;

    Ok(unique_sorted_model_ids(models))
}

fn should_retry_model_list_with_v1_endpoint(
    config: &ProviderConnectionConfig,
    error: &ProviderConfigError,
) -> bool {
    config.base_url.is_some() && matches!(error, ProviderConfigError::Connection { .. })
}

fn model_list_v1_retry_config(
    config: &ProviderConnectionConfig,
) -> Result<Option<ProviderConnectionConfig>, ProviderConfigError> {
    let Some(base_url) = config.base_url.as_deref() else {
        return Ok(None);
    };
    let normalized = normalized_base_url(base_url)?;
    let mut url =
        reqwest::Url::parse(&normalized).map_err(|source| ProviderConfigError::InvalidBaseUrl {
            value: base_url.to_string(),
            source: source.to_string(),
        })?;
    let already_v1 = url
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        == Some("v1");

    if already_v1 {
        return Ok(None);
    }

    {
        let mut segments =
            url.path_segments_mut()
                .map_err(|_| ProviderConfigError::InvalidBaseUrl {
                    value: base_url.to_string(),
                    source: "base URL cannot be used as a path base".to_string(),
                })?;
        segments.pop_if_empty();
        segments.push("v1");
    }

    let mut retry_config = config.clone();
    retry_config.base_url = Some(normalized_base_url(url.as_str())?);
    Ok(Some(retry_config))
}

fn unique_sorted_model_ids(mut models: Vec<String>) -> Vec<String> {
    models.retain(|model| !model.trim().is_empty());
    models.sort();
    models.dedup();
    models
}

pub async fn test_provider_connection(
    config: &ProviderConnectionConfig,
) -> Result<usize, ProviderConfigError> {
    Ok(fetch_provider_model_ids(config).await?.len())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NeutralChatRequest {
    pub model_id: String,
    pub messages: Vec<NeutralChatMessage>,
    #[serde(default)]
    pub tools: Vec<NeutralToolDefinition>,
    pub thinking_level: Option<String>,
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NeutralChatMessage {
    pub role: NeutralChatRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<NeutralChatAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<NeutralToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeutralChatAttachment {
    pub id: String,
    pub name: String,
    pub content_type: String,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NeutralChatRole {
    System,
    Developer,
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
    error_context: ProviderErrorContext,
}

impl NeutralChatStream {
    pub async fn next_event(
        &mut self,
    ) -> Option<Result<NeutralChatStreamEvent, ProviderConfigError>> {
        let event = self.stream.next().await?;
        Some(match event {
            Ok(event) => normalize_stream_event(event),
            Err(source) => Err(ProviderConfigError::from_genai_error_with_context(
                source,
                &self.error_context,
            )),
        })
    }
}

pub async fn stream_chat(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
) -> Result<NeutralChatStream, ProviderConfigError> {
    let client = config.genai_client()?;
    let chat_request = genai_chat_request(&request)?;
    let error_context =
        config.provider_error_context("opening provider stream", &request.model_id)?;
    let options = genai_chat_options(config, &request)?;
    let model = genai::ModelIden::new(config.kind.adapter_kind(), request.model_id.clone());
    let response = client
        .exec_chat_stream(model, chat_request, Some(&options))
        .await
        .map_err(|source| {
            ProviderConfigError::from_genai_error_with_context(source, &error_context)
        })?;

    Ok(NeutralChatStream {
        stream: response.stream,
        error_context: error_context.with_phase("reading provider stream"),
    })
}

pub fn parse_provider_kind(value: &str) -> Result<ProviderKind, ProviderConfigError> {
    let value = value.trim();
    supported_provider_kinds()
        .iter()
        .copied()
        .find(|kind| kind.as_str() == value)
        .ok_or_else(|| ProviderConfigError::UnsupportedKind(value.to_string()))
}

pub fn normalized_proxy_url(proxy_type: &str, value: &str) -> Result<String, ProviderConfigError> {
    let proxy_type = proxy_type.trim();
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(ProviderConfigError::EmptyProxyUrl);
    }

    let normalized = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        match proxy_type {
            HTTP_PROXY_KIND => format!("http://{trimmed}"),
            SOCKS_PROXY_KIND => format!("socks5h://{trimmed}"),
            other => return Err(ProviderConfigError::UnsupportedProxyKind(other.to_string())),
        }
    };
    let url = reqwest::Url::parse(&normalized).map_err(|source| {
        ProviderConfigError::InvalidProxyUrl {
            value: normalized.clone(),
            source: source.to_string(),
        }
    })?;
    let scheme = url.scheme();
    let scheme_matches = match proxy_type {
        HTTP_PROXY_KIND => scheme == "http",
        SOCKS_PROXY_KIND => {
            scheme == "socks4" || scheme == "socks4a" || scheme == "socks5" || scheme == "socks5h"
        }
        other => return Err(ProviderConfigError::UnsupportedProxyKind(other.to_string())),
    };

    if !scheme_matches {
        return Err(ProviderConfigError::InvalidProxyUrl {
            value: normalized,
            source: format!("scheme '{scheme}' does not match proxy type '{proxy_type}'"),
        });
    }

    if url.host_str().is_none() {
        return Err(ProviderConfigError::InvalidProxyUrl {
            value: url.to_string(),
            source: "host is required".to_string(),
        });
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ProviderConfigError::InvalidProxyUrl {
            value: url.to_string(),
            source: "proxy credentials in URL are not supported".to_string(),
        });
    }

    Ok(url.to_string())
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

fn normalized_genai_endpoint_url(
    kind: ProviderKind,
    value: &str,
) -> Result<String, ProviderConfigError> {
    let normalized = normalized_base_url(value)?;
    if kind.adapter_kind() != AdapterKind::Anthropic {
        return Ok(normalized);
    }

    append_v1_path_segment(&normalized, value)
}

fn append_v1_path_segment(
    value: &str,
    original_value: &str,
) -> Result<String, ProviderConfigError> {
    let mut url =
        reqwest::Url::parse(value).map_err(|source| ProviderConfigError::InvalidBaseUrl {
            value: original_value.to_string(),
            source: source.to_string(),
        })?;
    let already_v1 = url
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        == Some("v1");

    if already_v1 {
        return Ok(value.to_string());
    }

    {
        let mut segments =
            url.path_segments_mut()
                .map_err(|_| ProviderConfigError::InvalidBaseUrl {
                    value: original_value.to_string(),
                    source: "base URL cannot be used as a path base".to_string(),
                })?;
        segments.pop_if_empty();
        segments.push("v1");
    }

    normalized_base_url(url.as_str())
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

    let leading_system_count = request
        .messages
        .iter()
        .take_while(|message| message.role == NeutralChatRole::System)
        .count();
    let leading_system = leading_system_prompt(&request.messages[..leading_system_count])?;

    let mut developer_parts = Vec::new();
    let mut messages = Vec::with_capacity(request.messages.len() - leading_system_count);
    for message in &request.messages[leading_system_count..] {
        if message.role == NeutralChatRole::Developer {
            validate_instruction_message(message, "developer")?;
            developer_parts.push(message.content.clone());
            continue;
        }

        messages.push(genai_message(message)?);
    }

    let mut chat_request = ChatRequest::from_messages(messages);
    if let Some(system) = combined_instruction_prompt(leading_system, developer_parts) {
        chat_request = chat_request.with_system(system);
    }
    if !request.tools.is_empty() {
        chat_request = chat_request.with_tools(request.tools.iter().map(genai_tool));
    }

    Ok(chat_request)
}

fn leading_system_prompt(
    messages: &[NeutralChatMessage],
) -> Result<Option<String>, ProviderConfigError> {
    if messages.is_empty() {
        return Ok(None);
    }

    let mut parts = Vec::with_capacity(messages.len());
    for message in messages {
        validate_instruction_message(message, "system")?;
        parts.push(message.content.clone());
    }

    Ok(Some(parts.join("\n\n")))
}

fn combined_instruction_prompt(
    leading_system: Option<String>,
    developer_parts: Vec<String>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(system) = leading_system {
        parts.push(system);
    }
    parts.extend(developer_parts);

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

fn validate_instruction_message(
    message: &NeutralChatMessage,
    role_label: &str,
) -> Result<(), ProviderConfigError> {
    if !message.attachments.is_empty() {
        return Err(ProviderConfigError::InvalidRequest(format!(
            "{role_label} messages cannot contain attachments"
        )));
    }
    if message.content.trim().is_empty() {
        return Err(ProviderConfigError::InvalidRequest(
            "chat message content must not be empty".to_string(),
        ));
    }
    if !message.tool_calls.is_empty() || message.tool_call_id.is_some() {
        return Err(ProviderConfigError::InvalidRequest(format!(
            "{role_label} messages cannot contain tool state"
        )));
    }

    Ok(())
}

fn genai_message(message: &NeutralChatMessage) -> Result<ChatMessage, ProviderConfigError> {
    match message.role {
        NeutralChatRole::System => {
            validate_instruction_message(message, "system")?;

            Ok(ChatMessage::system(message.content.clone()))
        }
        NeutralChatRole::Developer => {
            validate_instruction_message(message, "developer")?;

            Ok(ChatMessage::system(message.content.clone()))
        }
        NeutralChatRole::User => {
            if message.content.trim().is_empty() && message.attachments.is_empty() {
                return Err(ProviderConfigError::InvalidRequest(
                    "user message content must not be empty unless it contains attachments"
                        .to_string(),
                ));
            }
            if !message.tool_calls.is_empty() || message.tool_call_id.is_some() {
                return Err(ProviderConfigError::InvalidRequest(
                    "user messages cannot contain tool state".to_string(),
                ));
            }

            if message.attachments.is_empty() {
                return Ok(ChatMessage::user(message.content.clone()));
            }

            let mut parts = Vec::new();
            if !message.content.trim().is_empty() {
                parts.push(ContentPart::Text(message.content.clone()));
            }
            for attachment in &message.attachments {
                if let Some(content_base64) = &attachment.content_base64 {
                    parts.push(ContentPart::from_binary_base64(
                        attachment.content_type.clone(),
                        content_base64.clone(),
                        Some(attachment.name.clone()),
                    ));
                    continue;
                }

                if attachment.path.is_none() {
                    return Err(ProviderConfigError::InvalidRequest(format!(
                        "attachment '{}' must have contentBase64 or path",
                        attachment.name
                    )));
                }
            }

            if parts.is_empty() {
                return Err(ProviderConfigError::InvalidRequest(
                    "user message content must not be empty unless it contains binary attachments"
                        .to_string(),
                ));
            }

            Ok(ChatMessage::user(MessageContent::from_parts(parts)))
        }
        NeutralChatRole::Assistant => {
            if !message.attachments.is_empty() {
                return Err(ProviderConfigError::InvalidRequest(
                    "assistant messages cannot contain attachments".to_string(),
                ));
            }
            let reasoning = message
                .reasoning
                .as_deref()
                .filter(|value| !value.trim().is_empty());
            if message.tool_calls.is_empty() {
                if message.content.trim().is_empty() && reasoning.is_none() {
                    return Err(ProviderConfigError::InvalidRequest(
                        "assistant message content or reasoning must not be empty unless it contains tool calls"
                            .to_string(),
                    ));
                }

                if message.content.trim().is_empty() {
                    return Ok(ChatMessage::assistant(MessageContent::from_parts(vec![
                        ContentPart::ReasoningContent(
                            reasoning.expect("reasoning was checked above").to_string(),
                        ),
                    ])));
                }

                let mut chat_message = ChatMessage::assistant(message.content.clone());
                if let Some(reasoning) = reasoning {
                    chat_message = chat_message.with_reasoning_content(Some(reasoning.to_string()));
                }

                return Ok(chat_message);
            }

            let tool_calls = message
                .tool_calls
                .iter()
                .map(genai_tool_call)
                .collect::<Vec<_>>();
            if message.content.trim().is_empty() && reasoning.is_none() {
                return Ok(ChatMessage::from(tool_calls));
            }

            let mut parts = Vec::new();
            if !message.content.trim().is_empty() {
                parts.push(ContentPart::Text(message.content.clone()));
            }
            if let Some(reasoning) = reasoning {
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
            if !message.attachments.is_empty() {
                return Err(ProviderConfigError::InvalidRequest(
                    "tool messages cannot contain attachments".to_string(),
                ));
            }
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

fn genai_chat_options(
    config: &ProviderConnectionConfig,
    request: &NeutralChatRequest,
) -> Result<ChatOptions, ProviderConfigError> {
    // ponytail: model-id heuristic; add provider metadata if non-Claude ids ever contain "claude".
    let is_claude = request.model_id.to_ascii_lowercase().contains("claude");
    let temperature = if is_claude { 1.0 } else { 0.0 };
    let mut options = ChatOptions::default()
        .with_temperature(temperature)
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(true)
        .with_capture_tool_calls(true);

    if !is_claude {
        options = options.with_top_p(1.0);
    }

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

    if let Some(prompt_cache_key) = request.prompt_cache_key.as_deref() {
        options = options.with_prompt_cache_key(prompt_cache_key);
    }

    if let Some(prompt_cache_retention) = request.prompt_cache_retention.as_deref() {
        let cache_control = match prompt_cache_retention {
            "24h" => CacheControl::Ephemeral24h,
            other => {
                return Err(ProviderConfigError::InvalidRequest(format!(
                    "unsupported prompt cache retention '{other}'"
                )));
            }
        };
        options = options.with_cache_control(cache_control);
    }

    apply_request_overrides(options, &config.request_overrides)
}

fn apply_request_overrides(
    mut options: ChatOptions,
    overrides: &[ProviderRequestOverride],
) -> Result<ChatOptions, ProviderConfigError> {
    let mut headers = Vec::new();
    let mut body = Map::new();

    for override_rule in overrides {
        let target = override_rule.normalized_target()?;
        let name = override_rule.normalized_name()?.to_string();
        let value = override_rule.normalized_value()?;

        match target {
            REQUEST_OVERRIDE_TARGET_HEADER => {
                let Some(header_value) = value.as_str() else {
                    return Err(ProviderConfigError::InvalidRequest(format!(
                        "header request override '{name}' value must be a string"
                    )));
                };
                headers.push((name, header_value.to_string()));
            }
            REQUEST_OVERRIDE_TARGET_BODY => {
                body.insert(name, value);
            }
            _ => unreachable!("request override target was validated"),
        }
    }

    if !headers.is_empty() {
        options = options.with_extra_headers(Headers::from(headers));
    }

    if !body.is_empty() {
        options = options.with_extra_body(Value::Object(body));
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
struct ProviderErrorContext {
    phase: &'static str,
    model_id: String,
    adapter: &'static str,
    base_url: String,
    proxy_configured: bool,
}

impl ProviderErrorContext {
    fn new(config: &ProviderConnectionConfig, phase: &'static str, model_id: &str) -> Self {
        Self {
            phase,
            model_id: model_id.to_string(),
            adapter: config.kind.adapter_label(),
            base_url: config.diagnostic_base_url(),
            proxy_configured: config.proxy_url.is_some(),
        }
    }

    fn with_phase(&self, phase: &'static str) -> Self {
        Self {
            phase,
            model_id: self.model_id.clone(),
            adapter: self.adapter,
            base_url: self.base_url.clone(),
            proxy_configured: self.proxy_configured,
        }
    }
}

impl fmt::Display for ProviderErrorContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let proxy = if self.proxy_configured {
            "enabled"
        } else {
            "disabled"
        };
        write!(
            formatter,
            "{} (model '{}', adapter {}, base URL '{}', proxy {})",
            self.phase, self.model_id, self.adapter, self.base_url, proxy
        )
    }
}

impl ProviderConnectionConfig {
    fn diagnostic_base_url(&self) -> String {
        let Ok(mut url) = reqwest::Url::parse(&self.diagnostic_endpoint_url().unwrap_or_default())
        else {
            return "<invalid>".to_string();
        };
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        url.to_string()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderConfigError {
    Connection {
        message: String,
        status_code: Option<u16>,
    },
    EmptyBaseUrl,
    EmptyProxyUrl,
    InvalidBaseUrl {
        value: String,
        source: String,
    },
    InvalidProxyUrl {
        value: String,
        source: String,
    },
    InvalidRequest(String),
    MissingRequiredField(String),
    MissingApiKey,
    UnsupportedKind(String),
    UnsupportedProxyKind(String),
}

impl ProviderConfigError {
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Connection { status_code, .. } => *status_code,
            Self::EmptyBaseUrl
            | Self::EmptyProxyUrl
            | Self::InvalidBaseUrl { .. }
            | Self::InvalidProxyUrl { .. }
            | Self::InvalidRequest(_)
            | Self::MissingRequiredField(_)
            | Self::MissingApiKey
            | Self::UnsupportedKind(_)
            | Self::UnsupportedProxyKind(_) => None,
        }
    }

    fn from_genai_error_with_context(source: genai::Error, context: &ProviderErrorContext) -> Self {
        let status_code = genai_error_status_code(&source).map(|status| status.as_u16());

        Self::Connection {
            message: format!("{context}: {source}"),
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
            Self::EmptyProxyUrl => write!(formatter, "AI API proxy URL must not be empty"),
            Self::InvalidBaseUrl { value, source } => {
                write!(
                    formatter,
                    "provider base URL '{value}' is invalid: {source}"
                )
            }
            Self::InvalidProxyUrl { value, source } => {
                write!(formatter, "AI API proxy URL '{value}' is invalid: {source}")
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
                "unsupported provider kind '{kind}'; expected one of: {}",
                supported_provider_kinds()
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::UnsupportedProxyKind(kind) => write!(
                formatter,
                "unsupported AI API proxy type '{kind}'; expected '{HTTP_PROXY_KIND}' or '{SOCKS_PROXY_KIND}'"
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

    fn openai_responses_kind() -> ProviderKind {
        parse_provider_kind(OPENAI_RESPONSES_KIND).expect("responses kind")
    }

    fn neutral_request(messages: Vec<NeutralChatMessage>) -> NeutralChatRequest {
        NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages,
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        }
    }

    fn neutral_text_message(role: NeutralChatRole, content: &str) -> NeutralChatMessage {
        NeutralChatMessage {
            role,
            content: content.to_string(),
            attachments: Vec::new(),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
        }
    }

    #[test]
    fn parses_supported_provider_kinds() {
        assert_eq!(
            parse_provider_kind(OPENAI_CHAT_KIND)
                .expect("chat kind")
                .adapter_kind(),
            AdapterKind::OpenAI
        );
        assert_eq!(
            parse_provider_kind(OPENAI_RESPONSES_KIND)
                .expect("responses kind")
                .adapter_kind(),
            AdapterKind::OpenAIResp
        );
        assert_eq!(
            parse_provider_kind(ANTHROPIC_KIND)
                .expect("anthropic kind")
                .adapter_kind(),
            AdapterKind::Anthropic
        );
        assert_eq!(
            parse_provider_kind(GEMINI_KIND)
                .expect("gemini kind")
                .adapter_kind(),
            AdapterKind::Gemini
        );
        assert_eq!(
            parse_provider_kind(XAI_KIND)
                .expect("xai kind")
                .adapter_kind(),
            AdapterKind::Xai
        );
        assert_eq!(
            parse_provider_kind(DEEPSEEK_KIND)
                .expect("deepseek kind")
                .adapter_kind(),
            AdapterKind::DeepSeek
        );
    }

    #[test]
    fn rejects_unknown_provider_kind() {
        let error = parse_provider_kind("openai").expect_err("unsupported kind should fail");

        assert!(error.to_string().contains("unsupported provider kind"));
    }

    #[test]
    fn provider_kind_catalog_exposes_genai_adapters() {
        let kinds = supported_provider_kinds()
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>();

        assert!(kinds.contains(&ANTHROPIC_KIND));
        assert!(kinds.contains(&GEMINI_KIND));
        assert!(kinds.contains(&XAI_KIND));
        assert!(kinds.contains(&DEEPSEEK_KIND));
        assert!(kinds.contains(&OLLAMA_KIND));
        assert!(
            !parse_provider_kind(OLLAMA_KIND)
                .expect("ollama kind")
                .requires_api_key()
        );
    }

    #[test]
    fn normalizes_base_url_for_genai_joining() {
        assert_eq!(
            normalized_base_url("https://api.openai.com/v1").expect("base url"),
            DEFAULT_OPENAI_BASE_URL
        );
    }

    #[test]
    fn anthropic_custom_endpoint_adds_v1_for_genai() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(ANTHROPIC_KIND).expect("anthropic kind"),
            base_url: Some("https://api.krill-ai.com/".to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        };

        let endpoint = config
            .custom_endpoint()
            .expect("custom endpoint")
            .expect("configured endpoint");

        assert_eq!(endpoint.base_url(), "https://api.krill-ai.com/v1/");
    }

    #[test]
    fn anthropic_custom_endpoint_keeps_existing_v1_for_genai() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(ANTHROPIC_KIND).expect("anthropic kind"),
            base_url: Some("https://api.krill-ai.com/coding/v1/".to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        };

        let endpoint = config
            .custom_endpoint()
            .expect("custom endpoint")
            .expect("configured endpoint");

        assert_eq!(endpoint.base_url(), "https://api.krill-ai.com/coding/v1/");
    }

    #[test]
    fn builds_v1_retry_base_url_for_missing_model_list_endpoint() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(ANTHROPIC_KIND).expect("anthropic kind"),
            base_url: Some("https://api.krill-ai.com/coding/".to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        };

        let retry_config = model_list_v1_retry_config(&config)
            .expect("retry config")
            .expect("custom base url retry");

        assert_eq!(
            retry_config.base_url.as_deref(),
            Some("https://api.krill-ai.com/coding/v1/")
        );
    }

    #[test]
    fn skips_v1_retry_when_base_url_already_ends_with_v1() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(ANTHROPIC_KIND).expect("anthropic kind"),
            base_url: Some("https://api.anthropic.com/v1/".to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        };

        assert_eq!(
            model_list_v1_retry_config(&config).expect("retry config"),
            None
        );
    }

    #[test]
    fn normalizes_proxy_urls_for_supported_types() {
        assert_eq!(
            normalized_proxy_url(HTTP_PROXY_KIND, "127.0.0.1:7890").expect("http proxy"),
            "http://127.0.0.1:7890/"
        );
        assert_eq!(
            normalized_proxy_url(SOCKS_PROXY_KIND, "127.0.0.1:7891").expect("socks proxy"),
            "socks5h://127.0.0.1:7891"
        );
        assert_eq!(
            normalized_proxy_url(SOCKS_PROXY_KIND, "socks5://127.0.0.1:7891")
                .expect("explicit socks proxy"),
            "socks5://127.0.0.1:7891"
        );
    }

    #[test]
    fn provider_error_context_redacts_url_credentials_query_and_fragment() {
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: Some("https://user:secret@example.test/v1?api_key=hidden#frag".to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: Some("http://127.0.0.1:7890".to_string()),
            request_overrides: Vec::new(),
        };

        let context = config
            .provider_error_context("reading provider stream", "gpt-5.5")
            .expect("provider error context")
            .to_string();

        assert!(context.contains("reading provider stream"));
        assert!(context.contains("model 'gpt-5.5'"));
        assert!(context.contains("adapter OpenAIResp"));
        assert!(context.contains("base URL 'https://example.test/v1/'"));
        assert!(context.contains("proxy enabled"));
        assert!(!context.contains("secret"));
        assert!(!context.contains("api_key"));
        assert!(!context.contains("frag"));
    }

    #[test]
    fn rejects_proxy_url_type_mismatches_and_credentials() {
        let mismatch = normalized_proxy_url(HTTP_PROXY_KIND, "socks5://127.0.0.1:7891")
            .expect_err("scheme mismatch should fail");
        assert!(mismatch.to_string().contains("does not match proxy type"));

        let credentials =
            normalized_proxy_url(SOCKS_PROXY_KIND, "socks5://user:pass@127.0.0.1:7891")
                .expect_err("proxy credentials should fail");
        assert!(credentials.to_string().contains("credentials"));
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
    fn moves_leading_system_messages_to_genai_system() {
        let mut request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Core prompt."),
            neutral_text_message(NeutralChatRole::System, "Tool guidance."),
            neutral_text_message(NeutralChatRole::User, "Do it."),
        ]);
        request.tools.push(NeutralToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file.".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
            strict: true,
        });

        let chat_request = genai_chat_request(&request).expect("chat request");

        assert_eq!(
            chat_request.system.as_deref(),
            Some("Core prompt.\n\nTool guidance.")
        );
        assert_eq!(chat_request.messages.len(), 1);
        assert_eq!(chat_request.messages[0].role, genai::chat::ChatRole::User);
        assert_eq!(
            chat_request.messages[0].content.first_text(),
            Some("Do it.")
        );
        assert_eq!(chat_request.tools.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn keeps_non_leading_system_messages_inline() {
        let request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Initial system."),
            neutral_text_message(NeutralChatRole::User, "User turn."),
            neutral_text_message(NeutralChatRole::System, "Runtime guard."),
        ]);

        let chat_request = genai_chat_request(&request).expect("chat request");

        assert_eq!(chat_request.system.as_deref(), Some("Initial system."));
        assert_eq!(chat_request.messages.len(), 2);
        assert_eq!(chat_request.messages[0].role, genai::chat::ChatRole::User);
        assert_eq!(chat_request.messages[1].role, genai::chat::ChatRole::System);
        assert_eq!(
            chat_request.messages[1].content.first_text(),
            Some("Runtime guard.")
        );
    }

    #[test]
    fn folds_developer_messages_into_genai_system() {
        let request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Base system."),
            neutral_text_message(NeutralChatRole::User, "User turn."),
            neutral_text_message(NeutralChatRole::Developer, "Skill instructions."),
            neutral_text_message(NeutralChatRole::User, "Continue."),
        ]);

        let chat_request = genai_chat_request(&request).expect("chat request");

        assert_eq!(
            chat_request.system.as_deref(),
            Some("Base system.\n\nSkill instructions.")
        );
        assert_eq!(chat_request.messages.len(), 2);
        assert_eq!(chat_request.messages[0].role, genai::chat::ChatRole::User);
        assert_eq!(chat_request.messages[1].role, genai::chat::ChatRole::User);
    }

    #[test]
    fn converts_tool_state_messages_for_genai_continuation() {
        let request = NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages: vec![
                NeutralChatMessage {
                    role: NeutralChatRole::User,
                    content: "Read the note.".to_string(),
                    attachments: Vec::new(),
                    reasoning: None,
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::Assistant,
                    content: String::new(),
                    attachments: Vec::new(),
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
                    attachments: Vec::new(),
                    reasoning: None,
                    tool_calls: Vec::new(),
                    tool_call_id: Some("call-1".to_string()),
                    tool_name: Some("read_file".to_string()),
                },
            ],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        };

        let chat_request = genai_chat_request(&request).expect("chat request");

        assert!(chat_request.messages[1].content.contains_tool_call());
        assert!(chat_request.messages[2].content.contains_tool_response());
    }

    #[test]
    fn converts_reasoning_only_assistant_messages_for_genai_continuation() {
        let request = NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::Assistant,
                content: String::new(),
                attachments: Vec::new(),
                reasoning: Some("Thinking.".to_string()),
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            }],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        };

        genai_chat_request(&request).expect("reasoning-only assistant message should convert");
    }

    #[test]
    fn converts_user_image_attachments_to_binary_parts() {
        let request = NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "Inspect this image.".to_string(),
                attachments: vec![NeutralChatAttachment {
                    id: "att-1".to_string(),
                    name: "image.png".to_string(),
                    content_type: "image/png".to_string(),
                    size_bytes: 5,
                    content_base64: Some("SGVsbG8=".to_string()),
                    path: None,
                }],
                reasoning: None,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            }],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        };

        let chat_request = genai_chat_request(&request).expect("chat request");
        let parts = (&chat_request.messages[0].content)
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(parts.len(), 2);
        assert!(parts[0].is_text());
        assert!(parts[1].is_binary());
    }

    #[test]
    fn keeps_path_attachments_as_text_only_messages() {
        let request = NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "# Files mentioned by the user:\n\n## note.txt: C:\\Users\\fonla\\Desktop\\note.txt\n\n## My request for Foco:\nReview it"
                    .to_string(),
                attachments: vec![NeutralChatAttachment {
                    id: "att-1".to_string(),
                    name: "note.txt".to_string(),
                    content_type: "text/plain".to_string(),
                    size_bytes: 5,
                    content_base64: None,
                    path: Some("C:\\Users\\fonla\\Desktop\\note.txt".to_string()),
                }],
                reasoning: None,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            }],
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        };

        let chat_request = genai_chat_request(&request).expect("chat request");
        let parts = (&chat_request.messages[0].content)
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(parts.len(), 1);
        assert!(parts[0].is_text());
    }

    #[test]
    fn maps_prompt_cache_options_to_genai_chat_options() {
        let request = NeutralChatRequest {
            model_id: "gpt-5.5".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: Some("foco:workspace:chat".to_string()),
            prompt_cache_retention: Some("24h".to_string()),
        };

        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        };
        let options = genai_chat_options(&config, &request).expect("chat options");

        assert_eq!(options.temperature, Some(0.0));
        assert_eq!(options.top_p, Some(1.0));
        assert_eq!(
            options.prompt_cache_key.as_deref(),
            Some("foco:workspace:chat")
        );
        assert_eq!(options.cache_control, Some(CacheControl::Ephemeral24h));
    }

    #[test]
    fn uses_temperature_one_and_omits_top_p_for_claude_models() {
        let request = NeutralChatRequest {
            model_id: "anthropic/claude-sonnet-4".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        };

        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        };
        let options = genai_chat_options(&config, &request).expect("chat options");

        assert_eq!(options.temperature, Some(1.0));
        assert_eq!(options.top_p, None);
    }

    #[test]
    fn rejects_unsupported_prompt_cache_retention() {
        let request = NeutralChatRequest {
            model_id: "gpt-5.5".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: Some("foco:workspace:chat".to_string()),
            prompt_cache_retention: Some("1h".to_string()),
        };

        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
        };
        let error =
            genai_chat_options(&config, &request).expect_err("unsupported retention should fail");

        assert!(
            error
                .to_string()
                .contains("unsupported prompt cache retention")
        );
    }
}
