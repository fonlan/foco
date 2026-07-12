use std::{
    collections::{BTreeMap, HashSet},
    fmt,
    sync::{Arc, Mutex},
};

use base64::Engine;
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
use reqwest::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const PROVIDER_WIRE_REQUEST_DUMP_VERSION: u32 = 1;
pub const PROVIDER_FINAL_RESPONSE_DUMP_VERSION: u32 = 1;
pub const PROVIDER_WIRE_REQUEST_DUMP_FORMAT: &str = "provider_request_v1";
pub const PROVIDER_FINAL_RESPONSE_DUMP_FORMAT: &str = "provider_final_response_v1";
const REDACTED_CREDENTIAL_VALUE: &str = "[REDACTED]";
const MASKED_AUTHORIZATION_VALUE: &str = "********";
pub const OPENAI_CHAT_KIND: &str = "openai-chat";

pub type ProviderHttpHeadersDump = BTreeMap<String, Vec<String>>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderWireRequestDump {
    pub format: String,
    pub version: u32,
    pub method: String,
    pub url: String,
    pub headers: ProviderHttpHeadersDump,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_encoding: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHttpResponseHeadDump {
    pub status: u16,
    pub version: String,
    pub headers: ProviderHttpHeadersDump,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "state"
)]
pub enum ProviderFinalResponseDump {
    Succeeded {
        format: String,
        version: u32,
        #[serde(default)]
        http: Option<ProviderHttpResponseHeadDump>,
        text: String,
        reasoning: Option<String>,
        tool_calls: Vec<NeutralToolCall>,
        usage: Option<NeutralUsage>,
        stop_reason: Option<String>,
        response_id: Option<String>,
    },
    Failed {
        format: String,
        version: u32,
        #[serde(default)]
        http: Option<ProviderHttpResponseHeadDump>,
        partial: bool,
        error: String,
        status_code: Option<u16>,
    },
}

impl ProviderFinalResponseDump {
    fn succeeded(
        http: Option<ProviderHttpResponseHeadDump>,
        text: String,
        reasoning: Option<String>,
        tool_calls: Vec<NeutralToolCall>,
        usage: Option<NeutralUsage>,
        stop_reason: Option<String>,
        response_id: Option<String>,
    ) -> Self {
        Self::Succeeded {
            format: PROVIDER_FINAL_RESPONSE_DUMP_FORMAT.to_string(),
            version: PROVIDER_FINAL_RESPONSE_DUMP_VERSION,
            http,
            text,
            reasoning,
            tool_calls,
            usage,
            stop_reason,
            response_id,
        }
    }

    pub fn failed(error: impl Into<String>, status_code: Option<u16>, partial: bool) -> Self {
        Self::failed_with_http(None, error, status_code, partial)
    }

    fn failed_with_http(
        http: Option<ProviderHttpResponseHeadDump>,
        error: impl Into<String>,
        status_code: Option<u16>,
        partial: bool,
    ) -> Self {
        Self::Failed {
            format: PROVIDER_FINAL_RESPONSE_DUMP_FORMAT.to_string(),
            version: PROVIDER_FINAL_RESPONSE_DUMP_VERSION,
            http,
            partial,
            error: error.into(),
            status_code,
        }
    }
}

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderModelRedirect {
    pub from: String,
    pub to: String,
}

impl ProviderModelRedirect {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        self.normalized_from()?;
        self.normalized_to()?;
        Ok(())
    }

    fn normalized_from(&self) -> Result<&str, ProviderConfigError> {
        normalized_model_redirect_value("from", &self.from)
    }

    fn normalized_to(&self) -> Result<&str, ProviderConfigError> {
        normalized_model_redirect_value("to", &self.to)
    }
}

fn normalized_model_redirect_value<'a>(
    field: &str,
    value: &'a str,
) -> Result<&'a str, ProviderConfigError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ProviderConfigError::InvalidRequest(format!(
            "model redirect {field} must not be empty"
        )));
    }
    if value.chars().any(char::is_whitespace) {
        return Err(ProviderConfigError::InvalidRequest(format!(
            "model redirect {field} must not contain whitespace: {value}"
        )));
    }
    Ok(value)
}

pub fn validate_model_redirects(
    redirects: &[ProviderModelRedirect],
) -> Result<(), ProviderConfigError> {
    let mut sources = HashSet::new();
    let mut targets = HashSet::new();
    for redirect in redirects {
        redirect.validate()?;
        let source = redirect.normalized_from()?;
        if !sources.insert(source) {
            return Err(ProviderConfigError::InvalidRequest(format!(
                "duplicate model redirect source '{source}'"
            )));
        }
        let target = redirect.normalized_to()?;
        if !targets.insert(target) {
            return Err(ProviderConfigError::InvalidRequest(format!(
                "duplicate model redirect target '{target}'"
            )));
        }
    }
    Ok(())
}

pub fn redirected_provider_model_ids(
    models: Vec<String>,
    redirects: &[ProviderModelRedirect],
) -> Result<Vec<String>, ProviderConfigError> {
    validate_model_redirects(redirects)?;
    let mut redirected = Vec::with_capacity(models.len());
    for model in models {
        let trimmed = model.trim();
        let model_id = redirects
            .iter()
            .find_map(|redirect| {
                (redirect.normalized_from().ok()? == trimmed)
                    .then(|| redirect.normalized_to().ok())
                    .flatten()
            })
            .unwrap_or(trimmed);
        redirected.push(model_id.to_string());
    }
    Ok(unique_sorted_model_ids(redirected))
}

pub fn upstream_provider_model_id<'a>(
    model_id: &'a str,
    redirects: &'a [ProviderModelRedirect],
) -> Result<&'a str, ProviderConfigError> {
    validate_model_redirects(redirects)?;
    let trimmed = model_id.trim();
    Ok(redirects
        .iter()
        .find_map(|redirect| {
            (redirect.normalized_to().ok()? == trimmed)
                .then(|| redirect.normalized_from().ok())
                .flatten()
        })
        .unwrap_or(model_id))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderConnectionConfig {
    pub kind: ProviderKind,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub proxy_url: Option<String>,
    pub request_overrides: Vec<ProviderRequestOverride>,
    pub model_redirects: Vec<ProviderModelRedirect>,
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
    let models = match fetch_provider_model_ids_once(config, "models").await {
        Ok(models) => models,
        Err(source) if should_retry_model_list_with_v1_endpoint(config, &source) => {
            let Some(retry_config) = model_list_v1_retry_config(config)? else {
                return Err(source);
            };

            fetch_provider_model_ids_once(&retry_config, "v1/models").await?
        }
        Err(source) => return Err(source),
    };

    redirected_provider_model_ids(models, &config.model_redirects)
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
    pub reasoning_tokens: Option<i64>,
}

#[derive(Debug)]
pub struct ProviderRequestFailure {
    pub error: ProviderConfigError,
    pub request_dump: Option<ProviderWireRequestDump>,
}

impl ProviderRequestFailure {
    pub fn status_code(&self) -> Option<u16> {
        self.error.status_code()
    }
}

impl fmt::Display for ProviderRequestFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for ProviderRequestFailure {}

pub struct NeutralChatStream {
    stream: genai::chat::ChatStream,
    error_context: ProviderErrorContext,
    wire_request_dump: Option<ProviderWireRequestDump>,
    response_head: Option<Arc<Mutex<Option<ProviderHttpResponseHeadDump>>>>,
    saw_response_event: bool,
    final_response_dump: Option<ProviderFinalResponseDump>,
}

impl NeutralChatStream {
    pub fn wire_request_dump(&self) -> Option<&ProviderWireRequestDump> {
        self.wire_request_dump.as_ref()
    }

    pub fn final_response_dump(&self) -> Option<&ProviderFinalResponseDump> {
        self.final_response_dump.as_ref()
    }

    fn response_head_dump(&self) -> Option<ProviderHttpResponseHeadDump> {
        self.response_head
            .as_ref()
            .and_then(|response_head| response_head.lock().ok()?.clone())
    }

    pub fn interrupted_final_response_dump(
        &self,
        message: impl Into<String>,
    ) -> Option<ProviderFinalResponseDump> {
        self.wire_request_dump.as_ref().map(|_| {
            ProviderFinalResponseDump::failed_with_http(
                self.response_head_dump(),
                message,
                None,
                self.saw_response_event,
            )
        })
    }

    pub async fn next_event(
        &mut self,
    ) -> Option<Result<NeutralChatStreamEvent, ProviderConfigError>> {
        let event = self.stream.next().await?;
        let normalized = match event {
            Ok(event) => normalize_stream_event(event),
            Err(source) => Err(ProviderConfigError::from_genai_error_with_context(
                source,
                &self.error_context,
            )),
        };

        match &normalized {
            Ok(
                NeutralChatStreamEvent::Start
                | NeutralChatStreamEvent::TextDelta { .. }
                | NeutralChatStreamEvent::ReasoningDelta { .. }
                | NeutralChatStreamEvent::ThoughtSignatureDelta { .. }
                | NeutralChatStreamEvent::ToolCall { .. }
                | NeutralChatStreamEvent::Usage { .. },
            ) => {
                self.saw_response_event = true;
            }
            Ok(NeutralChatStreamEvent::Complete {
                text,
                reasoning,
                tool_calls,
                usage,
                stop_reason,
                response_id,
            }) => {
                self.saw_response_event = true;
                if self.wire_request_dump.is_some() {
                    self.final_response_dump = Some(ProviderFinalResponseDump::succeeded(
                        self.response_head_dump(),
                        text.clone(),
                        reasoning.clone(),
                        tool_calls.clone(),
                        usage.clone(),
                        stop_reason.clone(),
                        response_id.clone(),
                    ));
                }
            }
            Ok(NeutralChatStreamEvent::Error { message }) => {
                if self.wire_request_dump.is_some() {
                    self.final_response_dump = Some(ProviderFinalResponseDump::failed_with_http(
                        self.response_head_dump(),
                        message.clone(),
                        None,
                        self.saw_response_event,
                    ));
                }
            }
            Err(error) => {
                if self.wire_request_dump.is_some() {
                    self.final_response_dump = Some(ProviderFinalResponseDump::failed_with_http(
                        self.response_head_dump(),
                        error.to_string(),
                        error.status_code(),
                        self.saw_response_event,
                    ));
                }
            }
        }

        Some(normalized)
    }
}

pub type ProviderRequestDumpObserver = Arc<dyn Fn(&ProviderWireRequestDump) + Send + Sync>;

pub async fn stream_chat(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
) -> Result<NeutralChatStream, ProviderConfigError> {
    stream_chat_with_capture(config, request, false)
        .await
        .map_err(|failure| failure.error)
}

pub async fn stream_chat_with_capture(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    capture_details: bool,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    stream_chat_with_capture_observer(config, request, capture_details, None).await
}

pub async fn stream_chat_with_capture_observer(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    capture_details: bool,
    request_observer: Option<ProviderRequestDumpObserver>,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    let client = config
        .genai_client()
        .map_err(|error| ProviderRequestFailure {
            error,
            request_dump: None,
        })?;
    let chat_request = genai_chat_request_for_adapter(&request, config.kind.adapter_kind())
        .map_err(|error| ProviderRequestFailure {
            error,
            request_dump: None,
        })?;
    let upstream_model_id = upstream_provider_model_id(&request.model_id, &config.model_redirects)
        .map_err(|error| ProviderRequestFailure {
            error,
            request_dump: None,
        })?;
    let error_context = config
        .provider_error_context("opening provider stream", upstream_model_id)
        .map_err(|error| ProviderRequestFailure {
            error,
            request_dump: None,
        })?;
    let options = genai_chat_options(config, &request).map_err(|error| ProviderRequestFailure {
        error,
        request_dump: None,
    })?;
    let model = genai::ModelIden::new(config.kind.adapter_kind(), upstream_model_id.to_string());
    let captured_request = capture_details.then(|| Arc::new(Mutex::new(None)));
    let captured_response_head = capture_details.then(|| Arc::new(Mutex::new(None)));
    let observer = if capture_details || request_observer.is_some() {
        let captured_request = captured_request.clone();
        Some(Arc::new(move |request: &Request| {
            let dump = provider_wire_request_dump(request);
            if let Some(request_observer) = request_observer.as_ref() {
                request_observer(&dump);
            }
            if let Some(captured_request) = captured_request.as_ref()
                && let Ok(mut slot) = captured_request.lock()
            {
                *slot = Some(dump);
            }
        }) as genai::PreparedRequestObserver)
    } else {
        None
    };
    let response_observer = captured_response_head
        .as_ref()
        .map(|captured_response_head| {
            let captured_response_head = captured_response_head.clone();
            Arc::new(move |response: &Response| {
                if let Ok(mut slot) = captured_response_head.lock() {
                    *slot = Some(provider_http_response_head_dump(response));
                }
            }) as genai::ResponseHeadObserver
        });
    let response = client
        .exec_chat_stream_observed_with_response(
            model,
            chat_request,
            Some(&options),
            observer,
            response_observer,
        )
        .await
        .map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::from_genai_error_with_context(source, &error_context),
            request_dump: take_captured_request_dump(&captured_request),
        })?;
    let wire_request_dump = take_captured_request_dump(&captured_request);

    Ok(NeutralChatStream {
        stream: response.stream,
        error_context: error_context.with_phase("reading provider stream"),
        wire_request_dump,
        response_head: captured_response_head,
        saw_response_event: false,
        final_response_dump: None,
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

#[cfg(test)]
fn genai_chat_request(request: &NeutralChatRequest) -> Result<ChatRequest, ProviderConfigError> {
    genai_chat_request_for_adapter(request, AdapterKind::OpenAI)
}

fn genai_chat_request_for_adapter(
    request: &NeutralChatRequest,
    _adapter_kind: AdapterKind,
) -> Result<ChatRequest, ProviderConfigError> {
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

    let mut messages = Vec::with_capacity(request.messages.len() - leading_system_count);
    for message in &request.messages[leading_system_count..] {
        messages.push(genai_message(message)?);
    }

    let mut chat_request = ChatRequest::from_messages(messages);
    if let Some(system) = leading_system {
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

                if let Some(path) = &attachment.path {
                    // ponytail: only media/PDF path attachments become model-visible binary input;
                    // text/code paths stay in the prompt for workspace tools. Add provider-aware
                    // filtering here if a provider needs stricter media support than the model config.
                    if is_model_visible_binary_attachment(&attachment.content_type) {
                        let mut part =
                            ContentPart::from_binary_file(path.as_str()).map_err(|source| {
                                ProviderConfigError::InvalidRequest(format!(
                                    "attachment '{}' could not be read: {source}",
                                    attachment.name
                                ))
                            })?;
                        if let ContentPart::Binary(binary) = &mut part {
                            binary.content_type = attachment.content_type.clone();
                            binary.name = Some(attachment.name.clone());
                        }
                        parts.push(part);
                    }
                    continue;
                }

                return Err(ProviderConfigError::InvalidRequest(format!(
                    "attachment '{}' must have contentBase64 or path",
                    attachment.name
                )));
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

fn is_model_visible_binary_attachment(content_type: &str) -> bool {
    let content_type = content_type
        .trim()
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    content_type.starts_with("image/")
        || content_type.starts_with("audio/")
        || content_type.starts_with("video/")
        || content_type == "application/pdf"
}

fn genai_chat_options(
    config: &ProviderConnectionConfig,
    request: &NeutralChatRequest,
) -> Result<ChatOptions, ProviderConfigError> {
    let model_id = upstream_provider_model_id(&request.model_id, &config.model_redirects)?;
    // ponytail: model-id heuristic; add provider metadata if non-Claude ids ever contain "claude".
    let is_claude = model_id.to_ascii_lowercase().contains("claude");
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
                insert_nested_body_override(&mut body, &name, value)?;
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

fn insert_nested_body_override(
    body: &mut Map<String, Value>,
    name: &str,
    value: Value,
) -> Result<(), ProviderConfigError> {
    let mut parts = name.split('.').peekable();
    let mut current = body;

    while let Some(raw_part) = parts.next() {
        let part = raw_part.trim();
        if part.is_empty() {
            return Err(ProviderConfigError::InvalidRequest(format!(
                "body request override path '{name}' must not contain empty segments"
            )));
        }

        if parts.peek().is_none() {
            current.insert(part.to_string(), value);
            return Ok(());
        }

        let entry = current
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(next) = entry else {
            return Err(ProviderConfigError::InvalidRequest(format!(
                "body request override path '{name}' conflicts with non-object segment '{part}'"
            )));
        };
        current = next;
    }

    Err(ProviderConfigError::InvalidRequest(
        "body request override path must not be empty".to_string(),
    ))
}

fn take_captured_request_dump(
    captured_request: &Option<Arc<Mutex<Option<ProviderWireRequestDump>>>>,
) -> Option<ProviderWireRequestDump> {
    captured_request
        .as_ref()
        .and_then(|captured_request| captured_request.lock().ok()?.take())
}

fn provider_wire_request_dump(request: &Request) -> ProviderWireRequestDump {
    let (body, body_encoding) = request
        .body()
        .and_then(|body| body.as_bytes())
        .map(|bytes| match std::str::from_utf8(bytes) {
            Ok(body) => (
                Some(redact_json_body_credentials(body)),
                Some("utf8".to_string()),
            ),
            Err(_) => (
                Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
                Some("base64".to_string()),
            ),
        })
        .unwrap_or((None, None));

    ProviderWireRequestDump {
        format: PROVIDER_WIRE_REQUEST_DUMP_FORMAT.to_string(),
        version: PROVIDER_WIRE_REQUEST_DUMP_VERSION,
        method: request.method().as_str().to_string(),
        url: redact_url_credentials(request.url()),
        headers: provider_http_headers_dump(request.headers()),
        body,
        body_encoding,
    }
}

fn provider_http_response_head_dump(response: &Response) -> ProviderHttpResponseHeadDump {
    ProviderHttpResponseHeadDump {
        status: response.status().as_u16(),
        version: format!("{:?}", response.version()),
        headers: provider_http_headers_dump(response.headers()),
    }
}

fn provider_http_headers_dump(headers: &reqwest::header::HeaderMap) -> ProviderHttpHeadersDump {
    headers
        .keys()
        .map(|name| {
            let values = headers
                .get_all(name)
                .iter()
                .map(|value| {
                    if name.as_str().eq_ignore_ascii_case("authorization") {
                        MASKED_AUTHORIZATION_VALUE.to_string()
                    } else {
                        value
                            .to_str()
                            .map(str::to_string)
                            .unwrap_or_else(|_| "[NON_UTF8]".to_string())
                    }
                })
                .collect();
            (name.as_str().to_string(), values)
        })
        .collect()
}

fn redact_url_credentials(url: &reqwest::Url) -> String {
    let mut redacted = url.clone();
    if !redacted.username().is_empty() {
        let _ = redacted.set_username(REDACTED_CREDENTIAL_VALUE);
    }
    if redacted.password().is_some() {
        let _ = redacted.set_password(Some(REDACTED_CREDENTIAL_VALUE));
    }
    if redacted.query().is_some() {
        let query = redacted
            .query_pairs()
            .map(|(name, value)| {
                let value = if is_sensitive_query_name(&name) {
                    REDACTED_CREDENTIAL_VALUE.to_string()
                } else {
                    value.into_owned()
                };
                (name.into_owned(), value)
            })
            .collect::<Vec<_>>();
        redacted.query_pairs_mut().clear().extend_pairs(query);
    }
    redacted.to_string()
}

fn redact_json_body_credentials(body: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(body) else {
        return body.to_string();
    };
    redact_json_credentials(&mut value);
    serde_json::to_string(&value).unwrap_or_else(|_| body.to_string())
}

fn redact_json_credentials(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (name, value) in object {
                if is_sensitive_credential_name(name) {
                    *value = Value::String(REDACTED_CREDENTIAL_VALUE.to_string());
                } else {
                    redact_json_credentials(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_json_credentials(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_sensitive_query_name(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|character| *character != '-' && *character != '_')
        .flat_map(char::to_lowercase)
        .collect::<String>();
    normalized == "key" || normalized.contains("apikey") || normalized.contains("accesskey")
}

fn is_sensitive_credential_name(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|character| *character != '-' && *character != '_')
        .flat_map(char::to_lowercase)
        .collect::<String>();
    normalized == "key"
        || normalized == "authorization"
        || normalized == "proxyauthorization"
        || normalized == "cookie"
        || normalized == "setcookie"
        || normalized == "password"
        || normalized.contains("apikey")
        || normalized.contains("signature")
        || normalized.contains("credential")
        || normalized.contains("secret")
        || normalized.ends_with("token")
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
        reasoning_tokens: usage
            .completion_tokens_details
            .as_ref()
            .and_then(|details| details.reasoning_tokens)
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
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::{Duration, timeout},
    };

    #[derive(Debug)]
    struct RawHttpRequest {
        method: String,
        target: String,
        headers: BTreeMap<String, String>,
        body: String,
    }

    fn parse_raw_http_request(bytes: Vec<u8>) -> RawHttpRequest {
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
            .expect("raw HTTP header terminator");
        let head = String::from_utf8(bytes[..header_end].to_vec()).expect("raw HTTP headers");
        let mut lines = head.lines();
        let request_line = lines.next().expect("HTTP request line");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().expect("HTTP method").to_string();
        let target = request_parts.next().expect("HTTP target").to_string();
        let headers = lines
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.to_ascii_lowercase(), value.trim().to_string()))
            })
            .collect();
        let body = String::from_utf8(bytes[header_end..].to_vec()).expect("raw HTTP body");
        RawHttpRequest {
            method,
            target,
            headers,
            body,
        }
    }

    async fn read_raw_http_request(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let header_end = loop {
            let read = socket
                .read(&mut buffer)
                .await
                .expect("read fixture request");
            assert!(read > 0, "fixture client closed before headers");
            request.extend_from_slice(&buffer[..read]);
            if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("content length"))
            })
            .unwrap_or_default();
        while request.len() < header_end + content_length {
            let read = socket.read(&mut buffer).await.expect("read fixture body");
            assert!(read > 0, "fixture client closed before body");
            request.extend_from_slice(&buffer[..read]);
        }
        request.truncate(header_end + content_length);
        request
    }

    async fn spawn_raw_http_fixture(
        status: &'static str,
        content_type: &'static str,
        response_body: &'static str,
    ) -> (String, tokio::task::JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            loop {
                let accepted = if requests.is_empty() {
                    Some(listener.accept().await.expect("accept fixture request"))
                } else {
                    timeout(Duration::from_millis(100), listener.accept())
                        .await
                        .ok()
                        .map(|result| result.expect("accept repeated fixture request"))
                };
                let Some((mut socket, _)) = accepted else {
                    break;
                };
                requests.push(read_raw_http_request(&mut socket).await);
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\nauthorization: Bearer response-secret\r\nx-api-key: response-api-key\r\nset-cookie: session=response-cookie\r\nx-fixture-response: response-header-value\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response_body}",
                    response_body.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("write fixture response");
            }
            requests
        });
        (format!("http://{address}/"), task)
    }

    async fn assert_adapter_captures_finalized_request_once(
        kind_name: &str,
        model_id: &str,
        expected_target_fragment: &str,
        response_body: &'static str,
    ) -> (ProviderWireRequestDump, RawHttpRequest) {
        let (base_url, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response_body).await;
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(kind_name).expect("provider kind"),
            base_url: Some(base_url),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: vec![
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_HEADER.to_string(),
                    name: "x-fixture-header".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("fixture-header-value".to_string()),
                },
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_BODY.to_string(),
                    name: "foco_fixture".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("finalized-body-override".to_string()),
                },
            ],
            model_redirects: vec![ProviderModelRedirect {
                from: format!("upstream-{model_id}"),
                to: model_id.to_string(),
            }],
        };
        let mut request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "adapter system"),
            neutral_text_message(NeutralChatRole::User, "adapter user"),
        ]);
        request.model_id = model_id.to_string();
        request.thinking_level = Some("low".to_string());
        request.prompt_cache_key = Some("fixture-cache-key".to_string());
        request.prompt_cache_retention = Some("24h".to_string());
        request.tools.push(NeutralToolDefinition {
            name: "fixture_tool".to_string(),
            description: "fixture tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            strict: true,
        });

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open adapter fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("adapter wire request dump")
            .clone();
        while stream.next_event().await.is_some() {}

        let requests = fixture.await.expect("adapter fixture task");
        assert_eq!(
            requests.len(),
            1,
            "capture must not duplicate provider sends"
        );
        let raw = parse_raw_http_request(requests.into_iter().next().expect("raw request"));
        assert_eq!(dump.method, raw.method);
        assert!(
            raw.target.contains(expected_target_fragment),
            "{}",
            raw.target
        );
        assert!(dump.url.ends_with(&raw.target));
        assert_eq!(dump.body.as_deref(), Some(raw.body.as_str()));
        assert_eq!(dump.body_encoding.as_deref(), Some("utf8"));
        assert_eq!(
            dump.headers
                .get("x-fixture-header")
                .and_then(|values| values.first())
                .map(String::as_str),
            Some("fixture-header-value")
        );
        let raw_secret_header = raw
            .headers
            .iter()
            .find(|(_, value)| value.contains("fixture-api-key"))
            .map(|(name, _)| name)
            .expect("provider authentication header");
        let expected_secret_header_value =
            if raw_secret_header.eq_ignore_ascii_case("authorization") {
                MASKED_AUTHORIZATION_VALUE
            } else {
                "fixture-api-key"
            };
        assert_eq!(
            dump.headers
                .get(raw_secret_header)
                .and_then(|values| values.first())
                .map(String::as_str),
            Some(expected_secret_header_value)
        );
        let body: Value = serde_json::from_str(&raw.body).expect("adapter request JSON");
        match kind_name {
            OPENAI_CHAT_KIND | OPENAI_RESPONSES_KIND => {
                assert_eq!(
                    body.get("foco_fixture").and_then(Value::as_str),
                    Some("finalized-body-override"),
                    "adapter {kind_name} did not preserve extra_body: {}",
                    raw.body
                );
            }
            ANTHROPIC_KIND => {
                assert_eq!(body["thinking"]["type"], "enabled");
                assert_eq!(body["thinking"]["budget_tokens"], 1024);
            }
            GEMINI_KIND => {
                assert_eq!(
                    body["generationConfig"]["thinkingConfig"]["includeThoughts"],
                    true
                );
                assert_eq!(
                    body["generationConfig"]["thinkingConfig"]["thinkingBudget"],
                    1000
                );
            }
            _ => unreachable!("fixture covers primary adapters only"),
        }
        if kind_name != GEMINI_KIND {
            assert!(raw.body.contains(&format!("upstream-{model_id}")));
        } else {
            assert!(raw.target.contains(&format!("upstream-{model_id}")));
        }
        assert!(raw.body.contains("adapter system"));
        assert!(raw.body.contains("fixture_tool"));
        let final_response = stream
            .final_response_dump()
            .expect("adapter final response dump");
        assert!(
            !serde_json::to_string(final_response)
                .expect("adapter final response JSON")
                .contains("chunk-only-secret")
        );
        (dump, raw)
    }

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
    fn redirects_provider_model_ids_to_local_ids() {
        let models = redirected_provider_model_ids(
            vec![
                "qwen/qwen3.6-35b-a3b".to_string(),
                "qwen3.6-35b-a3b".to_string(),
                "  ".to_string(),
            ],
            &[ProviderModelRedirect {
                from: "qwen/qwen3.6-35b-a3b".to_string(),
                to: "qwen3.6-35b-a3b".to_string(),
            }],
        )
        .expect("redirect model ids");

        assert_eq!(models, vec!["qwen3.6-35b-a3b"]);
    }

    #[test]
    fn maps_local_model_id_to_upstream_provider_id() {
        let redirects = [ProviderModelRedirect {
            from: "qwen/qwen3.6-35b-a3b".to_string(),
            to: "qwen3.6-35b-a3b".to_string(),
        }];

        assert_eq!(
            upstream_provider_model_id("qwen3.6-35b-a3b", &redirects)
                .expect("redirected upstream model id"),
            "qwen/qwen3.6-35b-a3b"
        );
        assert_eq!(
            upstream_provider_model_id("gpt-4o", &redirects).expect("unchanged upstream model id"),
            "gpt-4o"
        );
    }

    #[test]
    fn rejects_ambiguous_model_redirect_targets() {
        let error = validate_model_redirects(&[
            ProviderModelRedirect {
                from: "first/full".to_string(),
                to: "short".to_string(),
            },
            ProviderModelRedirect {
                from: "second/full".to_string(),
                to: "short".to_string(),
            },
        ])
        .expect_err("duplicate redirect target should fail");

        assert!(
            error
                .to_string()
                .contains("duplicate model redirect target")
        );
    }

    #[test]
    fn rejects_model_redirect_values_with_whitespace() {
        let error = validate_model_redirects(&[ProviderModelRedirect {
            from: "qwen/qwen3.6 35b".to_string(),
            to: "qwen3.6-35b-a3b".to_string(),
        }])
        .expect_err("whitespace in redirect should fail");

        assert!(error.to_string().contains("must not contain whitespace"));
    }

    #[test]
    fn inserts_nested_body_request_override() {
        let mut body = Map::new();

        insert_nested_body_override(
            &mut body,
            "text.verbosity",
            Value::String("low".to_string()),
        )
        .expect("nested body override");

        assert_eq!(
            Value::Object(body),
            serde_json::json!({ "text": { "verbosity": "low" } })
        );
    }

    #[test]
    fn rejects_empty_body_request_override_path_segment() {
        let mut body = Map::new();

        let error = insert_nested_body_override(
            &mut body,
            "text..verbosity",
            Value::String("low".to_string()),
        )
        .expect_err("empty path segment should fail");

        assert!(error.to_string().contains("empty segments"));
    }

    #[test]
    fn rejects_nested_body_request_override_conflict() {
        let mut body = Map::new();

        insert_nested_body_override(&mut body, "text", Value::String("x".to_string()))
            .expect("top-level body override");
        let error = insert_nested_body_override(
            &mut body,
            "text.verbosity",
            Value::String("low".to_string()),
        )
        .expect_err("non-object path segment should fail");

        assert!(
            error
                .to_string()
                .contains("conflicts with non-object segment")
        );
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
            model_redirects: Vec::new(),
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
            model_redirects: Vec::new(),
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
            model_redirects: Vec::new(),
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
            model_redirects: Vec::new(),
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
            model_redirects: Vec::new(),
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
            neutral_text_message(NeutralChatRole::System, "## Skills\n\n- Name: html-ppt"),
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
            Some("Core prompt.\n\n## Skills\n\n- Name: html-ppt")
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
    fn keeps_developer_messages_inline() {
        let request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Base system."),
            neutral_text_message(NeutralChatRole::Developer, "## Skills\n\n- Name: html-ppt"),
            neutral_text_message(NeutralChatRole::User, "Continue."),
        ]);

        let chat_request = genai_chat_request(&request).expect("chat request");

        assert_eq!(chat_request.system.as_deref(), Some("Base system."));
        assert_eq!(chat_request.messages.len(), 2);
        assert_eq!(chat_request.messages[0].role, genai::chat::ChatRole::System);
        assert_eq!(
            chat_request.messages[0].content.first_text(),
            Some("## Skills\n\n- Name: html-ppt")
        );
        assert_eq!(chat_request.messages[1].role, genai::chat::ChatRole::User);
        assert_eq!(
            chat_request.messages[1].content.first_text(),
            Some("Continue.")
        );
    }

    #[test]
    fn openai_responses_moves_leading_system_messages_to_genai_system() {
        let request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Base system."),
            neutral_text_message(NeutralChatRole::System, "Project spec."),
            neutral_text_message(NeutralChatRole::Developer, "## Skills\n\n- Name: html-ppt"),
            neutral_text_message(NeutralChatRole::User, "Continue."),
        ]);

        let chat_request = genai_chat_request_for_adapter(&request, AdapterKind::OpenAIResp)
            .expect("responses chat request");

        assert_eq!(
            chat_request.system.as_deref(),
            Some("Base system.\n\nProject spec.")
        );
        assert_eq!(chat_request.messages.len(), 2);
        assert_eq!(chat_request.messages[0].role, genai::chat::ChatRole::System);
        assert_eq!(
            chat_request.messages[0].content.first_text(),
            Some("## Skills\n\n- Name: html-ppt")
        );
        assert_eq!(chat_request.messages[1].role, genai::chat::ChatRole::User);
        assert_eq!(
            chat_request.messages[1].content.first_text(),
            Some("Continue.")
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
    fn keeps_text_path_attachments_as_text_only_messages() {
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
    fn converts_user_image_path_attachments_to_binary_parts() {
        let temp_path = std::env::temp_dir().join(format!(
            "foco-provider-image-attachment-{}.png",
            std::process::id()
        ));
        std::fs::write(&temp_path, b"not a real png, but enough bytes")
            .expect("write image fixture");

        let request = NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "Inspect this image.".to_string(),
                attachments: vec![NeutralChatAttachment {
                    id: "att-1".to_string(),
                    name: "image.png".to_string(),
                    content_type: "image/png".to_string(),
                    size_bytes: 29,
                    content_base64: None,
                    path: Some(temp_path.display().to_string()),
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

        let result = genai_chat_request(&request);
        let _ = std::fs::remove_file(&temp_path);
        let chat_request = result.expect("chat request");
        let parts = (&chat_request.messages[0].content)
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(parts.len(), 2);
        assert!(parts[0].is_text());
        let binary = parts[1].as_binary().expect("binary part");
        assert_eq!(binary.content_type, "image/png");
        assert_eq!(binary.name.as_deref(), Some("image.png"));
    }

    #[test]
    fn converts_user_pdf_path_attachments_to_binary_parts() {
        let temp_path = std::env::temp_dir().join(format!(
            "foco-provider-pdf-attachment-{}.pdf",
            std::process::id()
        ));
        std::fs::write(&temp_path, b"%PDF-1.4\n").expect("write pdf fixture");

        let request = NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "Read this PDF.".to_string(),
                attachments: vec![NeutralChatAttachment {
                    id: "att-1".to_string(),
                    name: "paper.pdf".to_string(),
                    content_type: "application/pdf".to_string(),
                    size_bytes: 9,
                    content_base64: None,
                    path: Some(temp_path.display().to_string()),
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

        let result = genai_chat_request(&request);
        let _ = std::fs::remove_file(&temp_path);
        let chat_request = result.expect("chat request");
        let parts = (&chat_request.messages[0].content)
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(parts.len(), 2);
        assert!(parts[0].is_text());
        let binary = parts[1].as_binary().expect("binary part");
        assert_eq!(binary.content_type, "application/pdf");
        assert_eq!(binary.name.as_deref(), Some("paper.pdf"));
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
            model_redirects: Vec::new(),
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
    fn accepts_none_thinking_level() {
        let request = NeutralChatRequest {
            model_id: "gpt-5.5".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: Some("none".to_string()),
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
            model_redirects: Vec::new(),
        };

        genai_chat_options(&config, &request).expect("none thinking level should parse");
    }

    #[test]
    fn accepts_max_thinking_level() {
        let request = NeutralChatRequest {
            model_id: "gpt-5.6".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: Some("max".to_string()),
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
            model_redirects: Vec::new(),
        };

        let options = genai_chat_options(&config, &request).expect("max thinking level");

        assert_eq!(
            options.reasoning_effort.map(|effort| effort.to_string()),
            Some("max".to_string())
        );
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
            model_redirects: Vec::new(),
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
            model_redirects: Vec::new(),
        };
        let error =
            genai_chat_options(&config, &request).expect_err("unsupported retention should fail");
        assert!(
            error
                .to_string()
                .contains("unsupported prompt cache retention")
        );
    }

    #[test]
    fn legacy_provider_final_response_v1_without_http_head_remains_readable() {
        let dump: ProviderFinalResponseDump = serde_json::from_value(serde_json::json!({
            "format": PROVIDER_FINAL_RESPONSE_DUMP_FORMAT,
            "version": PROVIDER_FINAL_RESPONSE_DUMP_VERSION,
            "state": "succeeded",
            "text": "historical response",
            "reasoning": null,
            "toolCalls": [],
            "usage": null,
            "stopReason": "stop",
            "responseId": null
        }))
        .expect("legacy response dump");

        assert!(matches!(
            dump,
            ProviderFinalResponseDump::Succeeded {
                http: None,
                text,
                ..
            } if text == "historical response"
        ));
    }

    #[test]
    fn wire_dump_only_masks_authorization_while_preserving_url_and_body_redaction() {
        let mut request = Request::new(
            reqwest::Method::POST,
            reqwest::Url::parse(
                "https://user:password@example.test/v1/chat?key=query-secret&topic=api-key-is-a-prompt",
            )
            .expect("url"),
        );
        request.headers_mut().insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_static("Bearer header-secret"),
        );
        request.headers_mut().insert(
            reqwest::header::HeaderName::from_static("x-provider-signature"),
            reqwest::header::HeaderValue::from_static("signature-secret"),
        );
        request.headers_mut().append(
            reqwest::header::HeaderName::from_static("x-provider-signature"),
            reqwest::header::HeaderValue::from_static("second-signature"),
        );
        *request.body_mut() = Some(
            serde_json::to_vec(&serde_json::json!({
                "api_key": "body-secret",
                "nested": { "accessToken": "nested-secret" },
                "prompt": "Keep api_key and token words in ordinary prompt text",
            }))
            .expect("body")
            .into(),
        );

        let dump = provider_wire_request_dump(&request);
        assert!(dump.url.contains("%5BREDACTED%5D:%5BREDACTED%5D@"));
        assert!(dump.url.contains("key=%5BREDACTED%5D"));
        assert!(dump.url.contains("topic=api-key-is-a-prompt"));
        assert_eq!(
            dump.headers
                .get("authorization")
                .and_then(|values| values.first())
                .map(String::as_str),
            Some(MASKED_AUTHORIZATION_VALUE)
        );
        assert_eq!(
            dump.headers.get("x-provider-signature"),
            Some(&vec![
                "signature-secret".to_string(),
                "second-signature".to_string()
            ])
        );
        let body: Value = serde_json::from_str(dump.body.as_deref().expect("body")).expect("json");
        assert_eq!(body["api_key"], REDACTED_CREDENTIAL_VALUE);
        assert_eq!(body["nested"]["accessToken"], REDACTED_CREDENTIAL_VALUE);
        assert_eq!(
            body["prompt"],
            "Keep api_key and token words in ordinary prompt text"
        );
        assert_eq!(dump.body_encoding.as_deref(), Some("utf8"));
    }

    #[test]
    fn wire_dump_preserves_empty_and_binary_body_semantics() {
        let empty = Request::new(
            reqwest::Method::GET,
            reqwest::Url::parse("https://example.test/").expect("url"),
        );
        let empty_dump = provider_wire_request_dump(&empty);
        assert_eq!(empty_dump.body, None);
        assert_eq!(empty_dump.body_encoding, None);

        let mut binary = Request::new(
            reqwest::Method::POST,
            reqwest::Url::parse("https://example.test/").expect("url"),
        );
        *binary.body_mut() = Some(vec![0xff, 0x00, 0x80].into());
        let binary_dump = provider_wire_request_dump(&binary);
        assert_eq!(binary_dump.body.as_deref(), Some("/wCA"));
        assert_eq!(binary_dump.body_encoding.as_deref(), Some("base64"));
    }

    #[tokio::test]
    async fn captures_finalized_requests_for_four_primary_adapters() {
        let openai_chat = concat!(
            "data: {\"raw_chunk_secret\":\"chunk-only-secret\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"chat ok\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        assert_adapter_captures_finalized_request_once(
            OPENAI_CHAT_KIND,
            "fixture-chat-model",
            "/chat/completions",
            openai_chat,
        )
        .await;

        let openai_responses = concat!(
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"responses ok\",\"chunk_secret\":\"chunk-only-secret\"}\n\n",
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-fixture\",\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"responses ok\",\"annotations\":[]}]}],\"usage\":{\"input_tokens\":3,\"output_tokens\":2,\"total_tokens\":5}}}\n\n"
        );
        assert_adapter_captures_finalized_request_once(
            OPENAI_RESPONSES_KIND,
            "fixture-responses-model",
            "/responses",
            openai_responses,
        )
        .await;

        let anthropic = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg-fixture\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-fixture\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"anthropic ok\"},\"chunk_secret\":\"chunk-only-secret\"}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":2}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
        );
        assert_adapter_captures_finalized_request_once(
            ANTHROPIC_KIND,
            "fixture-anthropic-model",
            "/messages",
            anthropic,
        )
        .await;

        let gemini = concat!(
            "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"gemini ok\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":3,\"candidatesTokenCount\":2,\"totalTokenCount\":5},\"chunk_secret\":\"chunk-only-secret\"}\n\n"
        );
        assert_adapter_captures_finalized_request_once(
            GEMINI_KIND,
            "fixture-gemini-model",
            ":streamGenerateContent",
            gemini,
        )
        .await;
    }

    #[tokio::test]
    async fn captures_final_wire_request_and_only_final_response() {
        const CHUNK_SENTINEL: &str = "chunk-only-secret";
        let response = concat!(
            "data: {\"id\":\"resp-fixture\",\"raw_chunk_secret\":\"chunk-only-secret\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"final text\"}}]}\n\n",
            "data: {\"id\":\"resp-fixture\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\n\n",
            "data: [DONE]\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response).await;
        let base_url = format!("{fixture_root}v1/");
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("openai kind"),
            base_url: Some(base_url),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: vec![
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_HEADER.to_string(),
                    name: "x-fixture-header".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("fixture-header-value".to_string()),
                },
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_BODY.to_string(),
                    name: "text.verbosity".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("low".to_string()),
                },
            ],
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "wire system"),
            neutral_text_message(NeutralChatRole::User, "wire user"),
        ]);
        request.tools.push(NeutralToolDefinition {
            name: "fixture_tool".to_string(),
            description: "fixture tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            strict: true,
        });

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("wire request dump")
            .clone();
        assert_eq!(dump.method, "POST");
        assert!(dump.url.ends_with("/v1/chat/completions"));
        assert_eq!(
            dump.headers
                .get("authorization")
                .and_then(|values| values.first())
                .map(String::as_str),
            Some(MASKED_AUTHORIZATION_VALUE)
        );
        assert_eq!(
            dump.headers
                .get("x-fixture-header")
                .and_then(|values| values.first())
                .map(String::as_str),
            Some("fixture-header-value")
        );
        let body = dump.body.as_deref().expect("request body");
        let body_json: Value = serde_json::from_str(body).expect("request JSON");
        assert_eq!(body_json["messages"][0]["role"], "system");
        assert_eq!(body_json["messages"][0]["content"], "wire system");
        assert_eq!(body_json["tools"][0]["function"]["name"], "fixture_tool");
        assert_eq!(body_json["text"]["verbosity"], "low");

        while stream.next_event().await.is_some() {}
        let final_dump = stream.final_response_dump().expect("final response dump");
        let final_json = serde_json::to_string(final_dump).expect("final response JSON");
        assert!(!final_json.contains(CHUNK_SENTINEL));
        assert!(matches!(
            final_dump,
            ProviderFinalResponseDump::Succeeded {
                http: Some(ProviderHttpResponseHeadDump {
                    status: 200,
                    version,
                    headers,
                }),
                text,
                stop_reason: Some(stop_reason),
                ..
            } if version == "HTTP/1.1"
                && text == "final text"
                && stop_reason == "stop"
                && headers.get("authorization").and_then(|values| values.first()).map(String::as_str)
                    == Some(MASKED_AUTHORIZATION_VALUE)
                && headers.get("x-api-key").and_then(|values| values.first()).map(String::as_str)
                    == Some("response-api-key")
                && headers.get("set-cookie").and_then(|values| values.first()).map(String::as_str)
                    == Some("session=response-cookie")
                && headers.get("x-fixture-response").and_then(|values| values.first()).map(String::as_str)
                    == Some("response-header-value")
        ));

        let raw_requests = fixture.await.expect("fixture task");
        assert_eq!(raw_requests.len(), 1);
        let raw_request = String::from_utf8(raw_requests.into_iter().next().expect("raw request"))
            .expect("raw HTTP request UTF-8");
        assert!(raw_request.contains("authorization: Bearer fixture-api-key"));
        assert!(raw_request.contains(body));
    }

    #[tokio::test]
    async fn captures_http_response_head_for_non_success_stream() {
        let (fixture_root, fixture) = spawn_raw_http_fixture(
            "502 Bad Gateway",
            "application/json",
            r#"{"error":"upstream unavailable"}"#,
        )
        .await;
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("openai kind"),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "capture non-success response head",
        )]);

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open non-success fixture stream");
        while stream.next_event().await.is_some() {}

        assert!(matches!(
            stream.final_response_dump(),
            Some(ProviderFinalResponseDump::Failed {
                http: Some(ProviderHttpResponseHeadDump {
                    status: 502,
                    version,
                    headers,
                }),
                ..
            }) if version == "HTTP/1.1"
                && headers.get("authorization").and_then(|values| values.first()).map(String::as_str)
                    == Some(MASKED_AUTHORIZATION_VALUE)
                && headers.get("x-api-key").and_then(|values| values.first()).map(String::as_str)
                    == Some("response-api-key")
        ));
        assert_eq!(fixture.await.expect("fixture task").len(), 1);
    }

    #[tokio::test]
    async fn capture_disabled_keeps_request_and_response_details_empty() {
        let response = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response).await;
        let base_url = format!("{fixture_root}v1/");
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("openai kind"),
            base_url: Some(base_url),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "capture disabled",
        )]);

        let mut stream = stream_chat_with_capture(&config, request, false)
            .await
            .expect("open fixture stream");
        assert!(stream.wire_request_dump().is_none());
        while stream.next_event().await.is_some() {}
        assert!(stream.final_response_dump().is_none());
        let raw_requests = fixture.await.expect("fixture task");
        assert_eq!(raw_requests.len(), 1);
    }
}
