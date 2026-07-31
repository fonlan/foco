mod openai_resp_websocket;
mod openai_resp_ws_session;

pub use openai_resp_ws_session::{
    OpenAiRespWsSessionKey, OpenAiRespWsSessionRegistry, ProviderWsSessionContext,
};

use std::{
    collections::{BTreeMap, HashSet},
    fmt,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::Engine;
use futures_util::{Stream, StreamExt};
use genai::{
    Client, Headers, WebConfig,
    adapter::AdapterKind,
    chat::{
        CacheControl, ChatMessage, ChatOptions, ChatRequest, ChatStreamEvent, ContentPart,
        MessageContent, ReasoningEffort, StreamEnd, Tool, ToolCall as GenaiToolCall,
        ToolChoice as GenaiToolChoice, ToolResponse, Usage,
    },
    resolver::{AuthData, Endpoint, ProviderConfig},
};
use reqwest::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub const PROVIDER_WIRE_REQUEST_DUMP_VERSION: u32 = 1;
pub const PROVIDER_FINAL_RESPONSE_DUMP_VERSION: u32 = 1;
pub const PROVIDER_WIRE_REQUEST_DUMP_FORMAT: &str = "provider_request_v1";
pub const PROVIDER_WEBSOCKET_REQUEST_DUMP_FORMAT: &str = "provider_websocket_request_v1";
pub const PROVIDER_WEBSOCKET_REQUEST_DUMP_VERSION: u32 = 1;
pub const PROVIDER_FINAL_RESPONSE_DUMP_FORMAT: &str = "provider_final_response_v1";
/// Maximum persisted JSON size for a failed final provider response envelope.
///
/// The bounded decoder diagnostic is compacted first, then non-correlation response
/// headers and the error message if necessary. This keeps the v1 envelope useful for
/// upstream correlation without creating an unbounded second stream archive.
pub const MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES: usize = 32 * 1024;
const MAX_CORRELATION_RESPONSE_HEADER_VALUES: usize = 8;
const TRUNCATION_SUFFIX: &str = "…[truncated]";
const REDACTED_CREDENTIAL_VALUE: &str = "[REDACTED]";
pub(crate) const MASKED_AUTHORIZATION_VALUE: &str = "********";
pub const OPENAI_CHAT_KIND: &str = "openai-chat";
pub const OPENAI_RESPONSES_KIND: &str = "openai-responses";
pub const OPENAI_RESPONSES_WEBSOCKET_KIND: &str = "openai-responses-websocket";
pub const XAI_RESPONSES_KIND: &str = "xai-responses";

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

/// Real WebSocket wire request dump for `openai-responses-websocket`.
///
/// Not an HTTP POST body: captures the derived `ws`/`wss` URL, handshake headers,
/// the `response.create` client frame, whether this turn reused a live socket, and
/// optional upgrade handshake metadata observed on this turn only.
///
/// `frame_sent` is true only after the client successfully wrote `response.create`
/// to the socket. Pre-send connect failures may still persist a dump with
/// `frame_sent=false` for diagnostics; do not treat those as observed wire frames.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderWebSocketRequestDump {
    pub format: String,
    pub version: u32,
    pub url: String,
    pub headers: ProviderHttpHeadersDump,
    /// Serialized `response.create` client event (UTF-8 JSON), credentials redacted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_frame: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_frame_encoding: Option<String>,
    /// True only after `response.create` was successfully written to the socket.
    /// Missing on historical rows defaults to true (those dumps were produced for
    /// completed turns that intended real-wire semantics).
    #[serde(default = "default_true")]
    pub frame_sent: bool,
    pub connection_reused: bool,
    /// Present only when this turn performed a WebSocket upgrade (not on reuse).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handshake: Option<ProviderWebSocketHandshakeDump>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderWebSocketHandshakeDump {
    pub status: u16,
    pub version: String,
    pub headers: ProviderHttpHeadersDump,
}

/// Versioned request detail accepted in `llm_requests.request_body_json`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderAuditRequestDump {
    WebSocket(ProviderWebSocketRequestDump),
    Http(ProviderWireRequestDump),
}

impl ProviderAuditRequestDump {
    pub fn format(&self) -> &str {
        match self {
            Self::Http(dump) => dump.format.as_str(),
            Self::WebSocket(dump) => dump.format.as_str(),
        }
    }

    pub fn from_http(dump: ProviderWireRequestDump) -> Self {
        Self::Http(dump)
    }

    pub fn from_websocket(dump: ProviderWebSocketRequestDump) -> Self {
        Self::WebSocket(dump)
    }

    pub fn as_http(&self) -> Option<&ProviderWireRequestDump> {
        match self {
            Self::Http(dump) => Some(dump),
            Self::WebSocket(_) => None,
        }
    }

    pub fn as_websocket(&self) -> Option<&ProviderWebSocketRequestDump> {
        match self {
            Self::WebSocket(dump) => Some(dump),
            Self::Http(_) => None,
        }
    }

    pub fn into_http(self) -> Option<ProviderWireRequestDump> {
        match self {
            Self::Http(dump) => Some(dump),
            Self::WebSocket(_) => None,
        }
    }
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
        /// Optional, bounded diagnostic from the OpenAI Responses stream decoder.
        ///
        /// Stored as JSON so an oversized diagnostic can be summarized without making
        /// historical v1 envelopes or future decoder additions impossible to read.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stream_diagnostic: Option<Value>,
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
        Self::failed_with_http_and_stream_diagnostic(http, error, status_code, partial, None)
    }

    fn failed_with_http_and_stream_diagnostic(
        http: Option<ProviderHttpResponseHeadDump>,
        error: impl Into<String>,
        status_code: Option<u16>,
        partial: bool,
        stream_diagnostic: Option<genai::OpenAIRespStreamDiagnostic>,
    ) -> Self {
        Self::Failed {
            format: PROVIDER_FINAL_RESPONSE_DUMP_FORMAT.to_string(),
            version: PROVIDER_FINAL_RESPONSE_DUMP_VERSION,
            http,
            partial,
            error: error.into(),
            status_code,
            stream_diagnostic: stream_diagnostic.and_then(provider_stream_diagnostic_value),
        }
    }

    /// Serializes the terminal provider envelope for audit persistence.
    ///
    /// Successful envelopes retain the existing behavior. Failed envelopes are capped
    /// so a malformed upstream response cannot make a single audit record unbounded.
    pub fn audit_json(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        let serialized = serde_json::to_string(&value)?;
        if !matches!(self, Self::Failed { .. })
            || serialized.len() <= MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES
        {
            return Ok(serialized);
        }

        let original_bytes = serialized.len();
        let original_sha256 = sha256_hex(serialized.as_bytes());
        compact_failed_stream_diagnostic(&mut value)?;
        record_audit_truncation(
            &mut value,
            "originalBytes",
            Value::from(original_bytes as u64),
        );
        record_audit_truncation(&mut value, "sha256", Value::String(original_sha256));
        record_audit_truncation(
            &mut value,
            "maxBytes",
            Value::from(MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES as u64),
        );

        let serialized = serde_json::to_string(&value)?;
        if serialized.len() <= MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES {
            return Ok(serialized);
        }

        compact_failed_error_message(&mut value);
        compact_failed_response_headers(&mut value);
        record_audit_truncation(&mut value, "envelopeTruncated", Value::Bool(true));

        let serialized = serde_json::to_string(&value)?;
        if serialized.len() <= MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES {
            return Ok(serialized);
        }

        // A malicious upstream can still repeat correlation headers enough times to
        // exceed the cap. Fall back to the irreducible audit fields rather than
        // persisting an oversized row or dropping the diagnostic entirely.
        let compacted = emergency_compact_failed_response(&value);
        let serialized = serde_json::to_string(&compacted)?;
        debug_assert!(
            serialized.len() <= MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES,
            "emergency-compacted failed provider audit envelope must stay bounded"
        );
        Ok(serialized)
    }
}

fn provider_stream_diagnostic_value(
    diagnostic: genai::OpenAIRespStreamDiagnostic,
) -> Option<Value> {
    let mut value = serde_json::to_value(diagnostic).ok()?;
    // The decoder already redacts its bounded payload snapshot. Reuse Foco's
    // provider credential-key redactor before the diagnostic joins a v1 envelope.
    redact_json_credentials(&mut value);
    Some(value)
}

fn compact_failed_stream_diagnostic(value: &mut Value) -> Result<(), serde_json::Error> {
    let Some(object) = value.as_object_mut() else {
        return Ok(());
    };
    let Some(diagnostic) = object.remove("streamDiagnostic") else {
        return Ok(());
    };
    let diagnostic = serde_json::to_vec(&diagnostic)?;
    object.insert(
        "streamDiagnostic".to_string(),
        json!({
            "truncated": true,
            "originalBytes": diagnostic.len(),
            "sha256": sha256_hex(&diagnostic),
        }),
    );
    record_audit_truncation(value, "streamDiagnosticTruncated", Value::Bool(true));
    Ok(())
}

fn compact_failed_error_message(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let Some(error) = object.get("error").and_then(Value::as_str) else {
        return;
    };
    let original_bytes = error.len();
    let original_sha256 = sha256_hex(error.as_bytes());
    object.insert(
        "error".to_string(),
        Value::String(truncate_utf8(error, 512)),
    );
    record_audit_truncation(
        value,
        "errorOriginalBytes",
        Value::from(original_bytes as u64),
    );
    record_audit_truncation(value, "errorSha256", Value::String(original_sha256));
}

fn compact_failed_response_headers(value: &mut Value) {
    let Some(headers) = value
        .as_object_mut()
        .and_then(|object| object.get_mut("http"))
        .and_then(Value::as_object_mut)
        .and_then(|http| http.remove("headers"))
    else {
        return;
    };

    let mut correlation_headers = Map::new();
    if let Some(headers) = headers.as_object() {
        for (name, value) in headers {
            if is_correlation_response_header(name) {
                correlation_headers.insert(name.clone(), truncate_header_value(value));
            }
        }
    }
    if let Some(http) = value
        .as_object_mut()
        .and_then(|object| object.get_mut("http"))
        .and_then(Value::as_object_mut)
    {
        http.insert("headers".to_string(), Value::Object(correlation_headers));
    }
    record_audit_truncation(value, "responseHeadersTruncated", Value::Bool(true));
}

fn record_audit_truncation(value: &mut Value, key: &str, field: Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let truncation = object
        .entry("auditTruncation".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(truncation) = truncation.as_object_mut() {
        truncation.insert(key.to_string(), field);
    }
}

fn is_correlation_response_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "x-cpa-trace-id"
            | "x-request-id"
            | "x-correlation-id"
            | "traceparent"
            | "tracestate"
            | "x-amzn-trace-id"
            | "content-type"
    )
}

fn truncate_header_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(
            values
                .iter()
                .take(MAX_CORRELATION_RESPONSE_HEADER_VALUES)
                .filter_map(Value::as_str)
                .map(|value| Value::String(truncate_utf8(value, 256)))
                .collect(),
        ),
        Value::String(value) => Value::String(truncate_utf8(value, 256)),
        _ => Value::Null,
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    if max_bytes <= TRUNCATION_SUFFIX.len() {
        return TRUNCATION_SUFFIX[..utf8_prefix_end(TRUNCATION_SUFFIX, max_bytes)].to_string();
    }
    let end = utf8_prefix_end(value, max_bytes - TRUNCATION_SUFFIX.len());
    format!("{}{TRUNCATION_SUFFIX}", &value[..end])
}

fn utf8_prefix_end(value: &str, max_bytes: usize) -> usize {
    value
        .char_indices()
        .take_while(|(index, character)| *index + character.len_utf8() <= max_bytes)
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or_default()
}

fn emergency_compact_failed_response(value: &Value) -> Value {
    let object = value.as_object();
    let mut compacted = Map::new();
    for key in ["format", "version", "state", "partial", "statusCode"] {
        if let Some(field) = object.and_then(|object| object.get(key)) {
            compacted.insert(key.to_string(), field.clone());
        }
    }
    if let Some(status) = object
        .and_then(|object| object.get("http"))
        .and_then(Value::as_object)
        .and_then(|http| http.get("status"))
    {
        compacted.insert("http".to_string(), json!({ "status": status }));
    }
    if let Some(error) = object
        .and_then(|object| object.get("error"))
        .and_then(Value::as_str)
    {
        compacted.insert(
            "error".to_string(),
            Value::String(truncate_utf8(error, 256)),
        );
    }
    if let Some(diagnostic) = object.and_then(|object| object.get("streamDiagnostic")) {
        compacted.insert("streamDiagnostic".to_string(), diagnostic.clone());
    }
    let mut truncation = object
        .and_then(|object| object.get("auditTruncation"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    if let Some(truncation) = truncation.as_object_mut() {
        truncation.insert("emergencyCompacted".to_string(), Value::Bool(true));
    }
    compacted.insert("auditTruncation".to_string(), truncation);
    Value::Object(compacted)
}

fn sha256_hex(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

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
    /// When true, chat/completions use the WebSocket Responses transport.
    /// Model listing and other HTTP endpoints still use the HTTP `base_url`.
    uses_websocket: bool,
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

    pub fn uses_websocket(self) -> bool {
        self.uses_websocket
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
            uses_websocket: false,
        }
    };
    ($kind:ident, $label:literal, $adapter_kind:ident, $base_url:expr, no_api_key) => {
        ProviderKind {
            kind: $kind,
            label: $label,
            adapter_kind: AdapterKind::$adapter_kind,
            default_base_url: $base_url,
            requires_api_key: false,
            uses_websocket: false,
        }
    };
    ($kind:ident, $label:literal, $adapter_kind:ident, $base_url:expr, websocket) => {
        ProviderKind {
            kind: $kind,
            label: $label,
            adapter_kind: AdapterKind::$adapter_kind,
            default_base_url: $base_url,
            requires_api_key: true,
            uses_websocket: true,
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
        OPENAI_RESPONSES_WEBSOCKET_KIND,
        "OpenAI Responses WebSocket",
        OpenAIResp,
        DEFAULT_OPENAI_BASE_URL,
        websocket
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
        XAI_RESPONSES_KIND,
        "xAI Responses",
        OpenAIResp,
        "https://api.x.ai/v1/"
    ),
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
        ensure_proxy_compatible_with_kind(self.kind, self.proxy_url.is_some())?;
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

    pub fn supports_fast_latency_mode(&self, model_id: &str) -> Result<bool, ProviderConfigError> {
        supports_fast_latency_mode(self.kind, model_id, &self.model_redirects)
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LatencyMode {
    #[default]
    Standard,
    Fast,
}

/// Runtime parameters that affect a user-visible chat request without changing its prompt.
///
/// These parameters are intentionally separate from `NeutralChatRequest` so internal one-shot
/// calls retain the default behavior unless an explicit user chat/agent run opts in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatRequestRuntimeOptions {
    #[serde(default)]
    pub latency_mode: LatencyMode,
    /// Whether the provider wire may retain the neutral `developer` role.
    ///
    /// Kept out of [`NeutralChatRequest`] so persisted prompt/history messages retain their
    /// original role. Omitting this option preserves the existing developer-role behavior.
    #[serde(
        default = "default_developer_role_enabled",
        skip_serializing_if = "is_true"
    )]
    pub developer_role_enabled: bool,
    /// Optional per-request sampling override for user-visible chat runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

impl Default for ChatRequestRuntimeOptions {
    fn default() -> Self {
        Self {
            latency_mode: LatencyMode::Standard,
            developer_role_enabled: true,
            temperature: None,
        }
    }
}

fn default_developer_role_enabled() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

/// Return whether this provider route can expose the Fast latency mode for `model_id`.
///
/// The decision is centralized here so callers must resolve the active provider and its upstream
/// model redirect before presenting Fast to users.
pub fn supports_fast_latency_mode(
    kind: ProviderKind,
    model_id: &str,
    redirects: &[ProviderModelRedirect],
) -> Result<bool, ProviderConfigError> {
    // Fast/priority is an OpenAI Responses product capability, not a generic OpenAIResp
    // transport feature. xAI Responses reuses the adapter but must not claim Fast.
    if !is_openai_responses_provider_kind(kind) {
        return Ok(false);
    }

    let upstream_model_id = upstream_provider_model_id(model_id, redirects)?;
    Ok(openai_priority_processing_supports_model(upstream_model_id))
}

/// True for Foco OpenAI Responses HTTP/WebSocket kinds only (not xAI Responses).
pub fn is_openai_responses_provider_kind(kind: ProviderKind) -> bool {
    matches!(
        kind.as_str(),
        OPENAI_RESPONSES_KIND | OPENAI_RESPONSES_WEBSOCKET_KIND
    )
}

/// True when Agent default headers (originator/User-Agent/session/WS beta) apply.
pub fn uses_openai_resp_agent_headers(kind: ProviderKind) -> bool {
    is_openai_responses_provider_kind(kind)
}

fn openai_priority_processing_supports_model(model_id: &str) -> bool {
    let model_id = model_id.trim().to_ascii_lowercase();
    model_id == "gpt-5" || model_id.starts_with("gpt-5-") || model_id.starts_with("gpt-5.")
}

/// Per-model preference for how Foco should expose web search when the global switch is on.
///
/// Stored on `ModelSettings` and resolved together with provider protocol/model capability into a
/// single runtime [`WebSearchRoute`]. Unknown JSON values are rejected by serde.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebSearchMode {
    /// Prefer provider-native search only when the central capability table confirms support;
    /// otherwise use the Tavily/Brave function fallback when configured.
    #[default]
    Auto,
    /// Force provider-native search when the active provider protocol supports it.
    Native,
    /// Force Foco's Tavily/Brave function tool when a fallback key is available.
    Function,
    /// Never expose web search for this model, even if the global switch is on.
    Disabled,
}

/// Runtime decision for which web-search path (if any) a chat turn may use.
///
/// At most one path is active per turn. Callers must not invent parallel native + function tools.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebSearchRoute {
    Disabled,
    ProviderNative,
    FocoFunction,
}

/// Inputs for the central web-search route state machine.
///
/// Callers resolve `active_provider_id` and `model_redirects` first, then pass the active provider
/// kind and redirected upstream model id. Capability must not be inferred from tool names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WebSearchRouteInput<'a> {
    /// Global `web_search.enabled` master switch.
    pub enabled: bool,
    /// Whether the configured Tavily/Brave active provider has a usable API key.
    pub fallback_available: bool,
    /// Active provider kind after model route resolution; `None` when the route is incomplete.
    pub provider_kind: Option<ProviderKind>,
    /// Upstream model id after applying provider `model_redirects`.
    pub upstream_model_id: &'a str,
    /// Explicit per-model mode (defaults to [`WebSearchMode::Auto`]).
    pub mode: WebSearchMode,
}

/// Whether the provider protocol can carry a native web-search tool at all.
///
/// This is independent of per-model capability. Phase 1 covers OpenAI Responses (HTTP + WebSocket).
/// xAI Responses and other protocols can be added here without scattering checks.
pub fn provider_protocol_supports_native_web_search(kind: ProviderKind) -> bool {
    kind.adapter_kind() == AdapterKind::OpenAIResp
}

/// Central native web-search capability for a provider kind + upstream model id.
///
/// - [`NativeWebSearchSupport::Supported`]: confirmed by the maintained capability table
/// - [`NativeWebSearchSupport::Unsupported`]: protocol or model is known not to support native search
/// - [`NativeWebSearchSupport::Unknown`]: protocol may support it, but model capability is unconfirmed
///
/// `auto` must not treat Unknown as Supported. Future model-metadata signals should plug in here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWebSearchSupport {
    Supported,
    Unsupported,
    Unknown,
}

pub fn native_web_search_support(
    kind: ProviderKind,
    upstream_model_id: &str,
) -> NativeWebSearchSupport {
    if !provider_protocol_supports_native_web_search(kind) {
        return NativeWebSearchSupport::Unsupported;
    }

    let model_id = upstream_model_id.trim();
    if model_id.is_empty() {
        return NativeWebSearchSupport::Unknown;
    }

    let known = if kind.as_str() == XAI_RESPONSES_KIND {
        xai_responses_native_web_search_model_support(model_id)
    } else {
        openai_responses_native_web_search_model_support(model_id)
    };

    match known {
        Some(true) => NativeWebSearchSupport::Supported,
        Some(false) => NativeWebSearchSupport::Unsupported,
        None => NativeWebSearchSupport::Unknown,
    }
}

/// True only when native web search is positively confirmed for this route.
pub fn supports_native_web_search(kind: ProviderKind, upstream_model_id: &str) -> bool {
    matches!(
        native_web_search_support(kind, upstream_model_id),
        NativeWebSearchSupport::Supported
    )
}

/// Resolve the single web-search route for a turn.
///
/// Rules:
/// - Global switch off or mode `disabled` → [`WebSearchRoute::Disabled`]
/// - Mode `function` → Foco function only when a fallback key is available
/// - Mode `native` → provider native only when the protocol supports native search
/// - Mode `auto` → native only when capability is Supported; otherwise Foco function if available
pub fn resolve_web_search_route(input: WebSearchRouteInput<'_>) -> WebSearchRoute {
    if !input.enabled {
        return WebSearchRoute::Disabled;
    }

    match input.mode {
        WebSearchMode::Disabled => WebSearchRoute::Disabled,
        WebSearchMode::Function => {
            if input.fallback_available {
                WebSearchRoute::FocoFunction
            } else {
                WebSearchRoute::Disabled
            }
        }
        WebSearchMode::Native => match input.provider_kind {
            Some(kind) if provider_protocol_supports_native_web_search(kind) => {
                WebSearchRoute::ProviderNative
            }
            _ => WebSearchRoute::Disabled,
        },
        WebSearchMode::Auto => {
            let native = input
                .provider_kind
                .is_some_and(|kind| supports_native_web_search(kind, input.upstream_model_id));
            if native {
                WebSearchRoute::ProviderNative
            } else if input.fallback_available {
                WebSearchRoute::FocoFunction
            } else {
                WebSearchRoute::Disabled
            }
        }
    }
}

/// Maintained OpenAI Responses native web-search model table.
///
/// Returns `Some(true)` / `Some(false)` when known, `None` when unconfirmed (auto must not
/// optimistically send a native tool).
fn openai_responses_native_web_search_model_support(model_id: &str) -> Option<bool> {
    let model_id = model_id.trim().to_ascii_lowercase();
    // Known-supported families on OpenAI Responses web_search.
    if model_id == "gpt-4o"
        || model_id.starts_with("gpt-4o-")
        || model_id == "gpt-4.1"
        || model_id.starts_with("gpt-4.1-")
        || model_id == "gpt-5"
        || model_id.starts_with("gpt-5-")
        || model_id.starts_with("gpt-5.")
        || model_id == "o3"
        || model_id.starts_with("o3-")
        || model_id == "o4-mini"
        || model_id.starts_with("o4-mini-")
    {
        return Some(true);
    }

    // Explicitly non-search / embedding-style ids when they appear on Responses-compatible routes.
    if model_id.contains("embed") || model_id.contains("tts") || model_id.contains("whisper") {
        return Some(false);
    }

    None
}

fn xai_responses_native_web_search_model_support(model_id: &str) -> Option<bool> {
    let model_id = model_id.trim().to_ascii_lowercase();
    // Grok models on xAI Responses support native web_search.
    if model_id.starts_with("grok-") || model_id == "grok" {
        return Some(true);
    }
    if model_id.contains("embed") {
        return Some(false);
    }
    None
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
    /// OpenAIResp Agent correlation headers (session/thread/request ids).
    /// Optional; when absent, Foco still injects fixed identity (+ WS beta) for OpenAIResp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_correlation: Option<AgentRequestCorrelation>,
    /// Provider-neutral tool selection strategy.
    ///
    /// Default is `Auto` (normal chat). Internal single-tool structured requests may set
    /// `RequiredSingleTool` so supporting adapters send a native forced tool-choice field.
    #[serde(default)]
    pub tool_choice: NeutralToolChoice,
}

/// Provider-neutral tool selection preference for a chat request.
///
/// Only `Auto` and `RequiredSingleTool` are exposed on the Foco transport model today.
/// Normal agent chat keeps `Auto`; internal single-tool structured requests use
/// `RequiredSingleTool`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NeutralToolChoice {
    /// Let the model decide whether (and which) tool to call.
    #[default]
    Auto,
    /// Require the model to call exactly the named tool when the provider supports it.
    RequiredSingleTool {
        #[serde(rename = "toolName")]
        tool_name: String,
    },
}

impl NeutralToolChoice {
    pub fn required_single_tool(tool_name: impl Into<String>) -> Self {
        Self::RequiredSingleTool {
            tool_name: tool_name.into(),
        }
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    pub fn required_tool_name(&self) -> Option<&str> {
        match self {
            Self::RequiredSingleTool { tool_name } => Some(tool_name.as_str()),
            Self::Auto => None,
        }
    }
}

/// Whether a requested `RequiredSingleTool` was applied on the provider wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolChoiceEnforcement {
    /// No forced single-tool strategy was requested.
    Auto,
    /// Native forced tool-choice was applied for this adapter.
    Applied,
    /// Forced single-tool was requested but this adapter does not support it; the
    /// request falls back to tools + prompt (+ later repair retry). Never claim forced.
    UnsupportedDegraded,
}

/// Return whether this adapter can emit a native forced single-tool selection field.
///
/// Unsupported adapters (Ollama native protocol, Cohere, DeepSeek, Bedrock) keep tools +
/// prompt only; callers must treat that as an explicit degradation, not silent enforcement.
pub fn adapter_supports_required_single_tool(adapter_kind: AdapterKind) -> bool {
    match adapter_kind {
        AdapterKind::OpenAI
        | AdapterKind::OpenAIResp
        | AdapterKind::Gemini
        | AdapterKind::Anthropic
        | AdapterKind::Fireworks
        | AdapterKind::Together
        | AdapterKind::Groq
        | AdapterKind::Aihubmix
        | AdapterKind::Mimo
        | AdapterKind::Moonshot
        | AdapterKind::Nebius
        | AdapterKind::Xai
        | AdapterKind::Zai
        | AdapterKind::BigModel
        | AdapterKind::Aliyun
        | AdapterKind::Baidu
        | AdapterKind::Vertex
        | AdapterKind::GithubCopilot
        | AdapterKind::OpenCodeGo
        | AdapterKind::OpenRouter
        | AdapterKind::MiniMax => true,
        AdapterKind::Ollama
        | AdapterKind::OllamaCloud
        | AdapterKind::Cohere
        | AdapterKind::DeepSeek
        | AdapterKind::BedrockApi => false,
    }
}

/// Resolve enforcement for a request against a concrete adapter.
pub fn resolve_tool_choice_enforcement(
    adapter_kind: AdapterKind,
    tool_choice: &NeutralToolChoice,
) -> ToolChoiceEnforcement {
    match tool_choice {
        NeutralToolChoice::Auto => ToolChoiceEnforcement::Auto,
        NeutralToolChoice::RequiredSingleTool { .. } => {
            if adapter_supports_required_single_tool(adapter_kind) {
                ToolChoiceEnforcement::Applied
            } else {
                ToolChoiceEnforcement::UnsupportedDegraded
            }
        }
    }
}

/// Session / multi-turn correlation for OpenAI Responses (HTTP and WebSocket).
/// See `docs/agent-openai-request-headers-contract.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequestCorrelation {
    pub session_id: String,
    pub thread_id: String,
    /// Prefer the durable LLM audit / broker request id.
    pub client_request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// Fixed Foco client originator (not Codex).
pub const FOCO_AGENT_ORIGINATOR: &str = "foco";
/// Responses WebSocket capability header name.
pub const OPENAI_RESP_WS_BETA_HEADER: &str = "OpenAI-Beta";
/// Responses WebSocket capability header value.
pub const OPENAI_RESP_WS_BETA_VALUE: &str = "responses_websockets=2026-02-06";
pub const AGENT_HEADER_SESSION_ID: &str = "session-id";
pub const AGENT_HEADER_THREAD_ID: &str = "thread-id";
pub const AGENT_HEADER_CLIENT_REQUEST_ID: &str = "x-client-request-id";
pub const AGENT_HEADER_FOCO_RUN_ID: &str = "x-foco-run-id";
pub const AGENT_HEADER_FOCO_WORKSPACE_ID: &str = "x-foco-workspace-id";
pub const AGENT_HEADER_ORIGINATOR: &str = "originator";
pub const AGENT_HEADER_USER_AGENT: &str = "User-Agent";

/// Headers that must not affect OpenAI Responses WebSocket continuation fingerprints.
pub const AGENT_VOLATILE_HEADER_NAMES: &[&str] =
    &[AGENT_HEADER_CLIENT_REQUEST_ID, AGENT_HEADER_FOCO_RUN_ID];

impl AgentRequestCorrelation {
    pub fn new(
        session_id: impl Into<String>,
        thread_id: impl Into<String>,
        client_request_id: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            thread_id: thread_id.into(),
            client_request_id: client_request_id.into(),
            run_id: None,
            workspace_id: None,
        }
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    pub fn with_workspace_id(mut self, workspace_id: impl Into<String>) -> Self {
        self.workspace_id = Some(workspace_id.into());
        self
    }
}

pub fn foco_agent_user_agent() -> String {
    format!(
        "foco/{} ({} {}; {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::FAMILY
    )
}

fn header_name_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

/// Build default OpenAIResp Agent headers (L2–L4). Callers merge `request_overrides` after.
///
/// Does not send a bare `version` header: some third-party/CPA gateways misread it as a
/// Codex client version. App version remains in the Foco `User-Agent`. Operators may still
/// add `version` via `request_overrides`.
pub fn default_openai_resp_agent_headers(
    uses_websocket: bool,
    correlation: Option<&AgentRequestCorrelation>,
) -> Vec<(String, String)> {
    // originator + User-Agent + optional WS beta + up to 5 correlation headers
    let mut headers = Vec::with_capacity(8);
    headers.push((
        AGENT_HEADER_ORIGINATOR.to_string(),
        FOCO_AGENT_ORIGINATOR.to_string(),
    ));
    headers.push((AGENT_HEADER_USER_AGENT.to_string(), foco_agent_user_agent()));
    if uses_websocket {
        headers.push((
            OPENAI_RESP_WS_BETA_HEADER.to_string(),
            OPENAI_RESP_WS_BETA_VALUE.to_string(),
        ));
    }
    if let Some(correlation) = correlation {
        let session_id = correlation.session_id.trim();
        let thread_id = correlation.thread_id.trim();
        let client_request_id = correlation.client_request_id.trim();
        if !session_id.is_empty() {
            headers.push((AGENT_HEADER_SESSION_ID.to_string(), session_id.to_string()));
        }
        if !thread_id.is_empty() {
            headers.push((AGENT_HEADER_THREAD_ID.to_string(), thread_id.to_string()));
        }
        if !client_request_id.is_empty() {
            headers.push((
                AGENT_HEADER_CLIENT_REQUEST_ID.to_string(),
                client_request_id.to_string(),
            ));
        }
        if let Some(run_id) = correlation
            .run_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            headers.push((AGENT_HEADER_FOCO_RUN_ID.to_string(), run_id.to_string()));
        }
        if let Some(workspace_id) = correlation
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            headers.push((
                AGENT_HEADER_FOCO_WORKSPACE_ID.to_string(),
                workspace_id.to_string(),
            ));
        }
    }
    headers
}

/// Merge default headers then override headers; later same-name (case-insensitive) wins.
pub fn merge_header_pairs(
    defaults: Vec<(String, String)>,
    overrides: Vec<(String, String)>,
) -> Vec<(String, String)> {
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for (name, value) in defaults.into_iter().chain(overrides) {
        let key = header_name_key(&name);
        if key.is_empty() {
            continue;
        }
        if !map.contains_key(&key) {
            order.push(key.clone());
        }
        map.insert(key, (name, value));
    }
    order
        .into_iter()
        .filter_map(|key| map.remove(&key))
        .collect()
}

/// Resolve session/thread ids per Agent header contract (no DB).
///
/// - If `chat_id` is a plan phase implementation (or merge) chat → session=`plan_id`, thread=`chat_id`
/// - Else if parent chat is plan-bound → session=`plan_id`, thread=`chat_id` (subagent)
/// - Else normal → session=thread=`chat_id`
pub fn resolve_agent_session_thread_ids(
    chat_id: &str,
    plan_id_for_chat: Option<&str>,
    plan_id_for_parent_chat: Option<&str>,
) -> (String, String) {
    let chat_id = chat_id.trim();
    if chat_id.is_empty() {
        return (String::new(), String::new());
    }
    if let Some(plan_id) = plan_id_for_chat
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return (plan_id.to_string(), chat_id.to_string());
    }
    if let Some(plan_id) = plan_id_for_parent_chat
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return (plan_id.to_string(), chat_id.to_string());
    }
    (chat_id.to_string(), chat_id.to_string())
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

/// How a neutral tool should be mapped onto the provider wire.
///
/// Kind is explicit and never inferred from `name`. A function tool named `web_search`
/// must remain a function tool; only [`NeutralToolKind::ProviderWebSearch`] becomes a
/// provider-native web search tool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NeutralToolKind {
    /// Ordinary function/custom tool (default for old serialized data).
    #[default]
    Function,
    /// Provider-native web search (`Tool::new_web_search()` / `type=web_search`).
    ProviderWebSearch,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeutralToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub strict: bool,
    /// Wire kind. Missing field deserializes as [`NeutralToolKind::Function`].
    #[serde(default)]
    pub kind: NeutralToolKind,
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
    pub request_dump: Option<ProviderAuditRequestDump>,
    /// Observed `Retry-After` delay from the provider Response head, if any.
    ///
    /// Captured for HTTP open failures (and WebSocket upgrade failures when the
    /// handshake response is available) so production retry paths can prefer it
    /// over pure exponential backoff.
    pub retry_after: Option<Duration>,
}

impl ProviderRequestFailure {
    pub fn new(error: ProviderConfigError) -> Self {
        Self {
            error,
            request_dump: None,
            retry_after: None,
        }
    }

    pub fn with_request_dump(mut self, request_dump: Option<ProviderAuditRequestDump>) -> Self {
        self.request_dump = request_dump;
        self
    }

    pub fn with_retry_after(mut self, retry_after: Option<Duration>) -> Self {
        self.retry_after = retry_after;
        self
    }

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

type GenaiChatEventStream =
    Pin<Box<dyn Stream<Item = Result<ChatStreamEvent, genai::Error>> + Send>>;

pub struct NeutralChatStream {
    pub(crate) stream: GenaiChatEventStream,
    pub(crate) error_context: ProviderErrorContext,
    pub(crate) wire_request_dump: Option<ProviderAuditRequestDump>,
    /// Always captured when a real HTTP Response is observed; independent of detail dumps.
    pub(crate) response_status: Arc<Mutex<Option<u16>>>,
    /// Always captured when a real HTTP Response includes a parseable Retry-After header.
    pub(crate) response_retry_after: Arc<Mutex<Option<Duration>>>,
    /// Full head dump (version/headers) only when `capture_details` is enabled.
    pub(crate) response_head: Option<Arc<Mutex<Option<ProviderHttpResponseHeadDump>>>>,
    pub(crate) saw_response_event: bool,
    pub(crate) final_response_dump: Option<ProviderFinalResponseDump>,
    /// Bounded OpenAI Responses decoder diagnostics, retained only for detail-enabled turns.
    pub(crate) stream_diagnostics: Option<genai::OpenAIRespStreamDiagnostics>,
}

impl NeutralChatStream {
    pub fn wire_request_dump(&self) -> Option<&ProviderAuditRequestDump> {
        self.wire_request_dump.as_ref()
    }

    pub fn final_response_dump(&self) -> Option<&ProviderFinalResponseDump> {
        self.final_response_dump.as_ref()
    }

    /// Returns the bounded OpenAI Responses stream diagnostic captured for this detail-enabled turn.
    pub fn stream_diagnostic(&self) -> Option<genai::OpenAIRespStreamDiagnostic> {
        self.stream_diagnostics
            .as_ref()
            .and_then(genai::OpenAIRespStreamDiagnostics::latest)
    }

    /// Observed HTTP response status from the provider Response head, if any.
    ///
    /// Independent of `capture_details`: status is recorded whenever a Response exists.
    /// DNS/TLS/connect failures before a Response remain `None`.
    pub fn http_status(&self) -> Option<u16> {
        self.response_status
            .lock()
            .ok()
            .and_then(|status| *status)
            .or_else(|| self.response_head_dump().map(|head| head.status))
    }

    /// Observed `Retry-After` delay from the provider Response head, if any.
    ///
    /// Independent of `capture_details`. Only integer-second values are accepted and the
    /// result is clamped to 30 seconds so callers can feed it into retry backoff safely.
    pub fn retry_after(&self) -> Option<Duration> {
        self.response_retry_after
            .lock()
            .ok()
            .and_then(|value| *value)
            .or_else(|| {
                self.response_head_dump()
                    .and_then(|head| retry_after_from_http_headers(&head.headers))
            })
    }

    fn response_head_dump(&self) -> Option<ProviderHttpResponseHeadDump> {
        self.response_head
            .as_ref()
            .and_then(|response_head| response_head.lock().ok()?.clone())
    }

    fn synthetic_failed_final_response_dump(
        &self,
        message: impl Into<String>,
        status_code: Option<u16>,
        partial: bool,
    ) -> Option<ProviderFinalResponseDump> {
        self.wire_request_dump.as_ref().map(|_| {
            ProviderFinalResponseDump::failed_with_http_and_stream_diagnostic(
                self.response_head_dump(),
                message,
                status_code,
                partial,
                self.stream_diagnostic(),
            )
        })
    }

    /// Returns the observed terminal dump, or creates a bounded failed envelope from
    /// the current stream state when cancellation/timeout interrupted the decoder.
    pub fn failed_final_response_dump(
        &self,
        message: impl Into<String>,
        status_code: Option<u16>,
        partial: bool,
    ) -> Option<ProviderFinalResponseDump> {
        self.final_response_dump
            .clone()
            .or_else(|| self.synthetic_failed_final_response_dump(message, status_code, partial))
    }

    pub fn interrupted_final_response_dump(
        &self,
        message: impl Into<String>,
    ) -> Option<ProviderFinalResponseDump> {
        self.synthetic_failed_final_response_dump(
            message,
            self.http_status(),
            self.saw_response_event,
        )
    }

    pub async fn next_event(
        &mut self,
    ) -> Option<Result<NeutralChatStreamEvent, ProviderConfigError>> {
        loop {
            let event = self.stream.next().await?;
            let normalized = match event {
                Ok(event) => match normalize_stream_event(event) {
                    Ok(Some(event)) => Ok(event),
                    Ok(None) => continue,
                    Err(error) => Err(error),
                },
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
                    self.final_response_dump = self.synthetic_failed_final_response_dump(
                        message.clone(),
                        None,
                        self.saw_response_event,
                    );
                }
                Err(error) => {
                    // A decoder error after a successful HTTP response (for example a 200
                    // SSE `response.failed`) has no error status of its own. Preserve the
                    // observed response status in the terminal audit envelope without
                    // changing ProviderConfigError, which callers use for retry policy.
                    let status_code = error.status_code().or_else(|| self.http_status());
                    self.final_response_dump = self.synthetic_failed_final_response_dump(
                        error.to_string(),
                        status_code,
                        self.saw_response_event,
                    );
                }
            }

            return Some(normalized);
        }
    }
}

pub type ProviderRequestDumpObserver = Arc<dyn Fn(&ProviderAuditRequestDump) + Send + Sync>;

pub async fn stream_chat(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
) -> Result<NeutralChatStream, ProviderConfigError> {
    stream_chat_with_runtime_options(config, request, ChatRequestRuntimeOptions::default())
        .await
        .map_err(|failure| failure.error)
}

pub async fn stream_chat_with_runtime_options(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    runtime_options: ChatRequestRuntimeOptions,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    stream_chat_with_capture_runtime_options(config, request, runtime_options, false).await
}

pub async fn stream_chat_with_capture(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    capture_details: bool,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    stream_chat_with_capture_runtime_options(
        config,
        request,
        ChatRequestRuntimeOptions::default(),
        capture_details,
    )
    .await
}

pub async fn stream_chat_with_capture_runtime_options(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    runtime_options: ChatRequestRuntimeOptions,
    capture_details: bool,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    stream_chat_with_capture_observer_runtime_options(
        config,
        request,
        runtime_options,
        capture_details,
        None,
        None,
    )
    .await
}

pub async fn stream_chat_with_capture_observer(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    capture_details: bool,
    request_observer: Option<ProviderRequestDumpObserver>,
    session_ctx: Option<ProviderWsSessionContext>,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    stream_chat_with_capture_observer_runtime_options(
        config,
        request,
        ChatRequestRuntimeOptions::default(),
        capture_details,
        request_observer,
        session_ctx,
    )
    .await
}

pub async fn stream_chat_with_capture_observer_runtime_options(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    runtime_options: ChatRequestRuntimeOptions,
    capture_details: bool,
    request_observer: Option<ProviderRequestDumpObserver>,
    session_ctx: Option<ProviderWsSessionContext>,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    // Normalize once, before adapters, request overrides, and wire observers. This is the sole
    // outbound role boundary: callers keep Developer semantics in prompt construction/history.
    let request = normalize_request_developer_role(request, &runtime_options);
    if config.kind.uses_websocket() {
        return stream_chat_with_capture_observer_websocket(
            config,
            request,
            runtime_options,
            capture_details,
            request_observer,
            session_ctx,
        )
        .await;
    }
    // HTTP path ignores session_ctx (Responses WebSocket only).
    let _ = session_ctx;

    let client = config
        .genai_client()
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let chat_request = genai_chat_request_for_adapter(&request, config.kind.adapter_kind())
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let upstream_model_id = upstream_provider_model_id(&request.model_id, &config.model_redirects)
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let error_context = config
        .provider_error_context("opening provider stream", upstream_model_id)
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let options = genai_chat_options_with_runtime_options(config, &request, &runtime_options)
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let model = genai::ModelIden::new(config.kind.adapter_kind(), upstream_model_id.to_string());
    let captured_request = capture_details.then(|| Arc::new(Mutex::new(None)));
    let captured_response_status = Arc::new(Mutex::new(None));
    let captured_response_retry_after = Arc::new(Mutex::new(None));
    let captured_response_head = capture_details.then(|| Arc::new(Mutex::new(None)));
    let observer = if capture_details || request_observer.is_some() {
        let captured_request = captured_request.clone();
        Some(Arc::new(move |request: &Request| {
            let dump = ProviderAuditRequestDump::from_http(provider_wire_request_dump(request));
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
    let response_observer = {
        let captured_response_status = captured_response_status.clone();
        let captured_response_retry_after = captured_response_retry_after.clone();
        let captured_response_head = captured_response_head.clone();
        Some(Arc::new(move |response: &Response| {
            let status = response.status().as_u16();
            if let Ok(mut slot) = captured_response_status.lock() {
                *slot = Some(status);
            }
            if let Ok(mut slot) = captured_response_retry_after.lock() {
                *slot = retry_after_from_reqwest_headers(response.headers());
            }
            if let Some(captured_response_head) = captured_response_head.as_ref()
                && let Ok(mut slot) = captured_response_head.lock()
            {
                // Only copy version/headers when detail capture is enabled.
                *slot = Some(provider_http_response_head_dump(response));
            }
        }) as genai::ResponseHeadObserver)
    };
    let (response, stream_diagnostics) = if capture_details {
        let response = client
            .exec_chat_stream_observed_with_response_and_diagnostics(
                model,
                chat_request,
                Some(&options),
                observer,
                response_observer,
            )
            .await
            .map_err(|source| {
                ProviderRequestFailure::new(ProviderConfigError::from_genai_error_with_context(
                    source,
                    &error_context,
                ))
                .with_request_dump(take_captured_request_dump(&captured_request))
                .with_retry_after(take_captured_retry_after(&captured_response_retry_after))
            })?;
        (response.response, Some(response.diagnostics))
    } else {
        let response = client
            .exec_chat_stream_observed_with_response(
                model,
                chat_request,
                Some(&options),
                observer,
                response_observer,
            )
            .await
            .map_err(|source| {
                ProviderRequestFailure::new(ProviderConfigError::from_genai_error_with_context(
                    source,
                    &error_context,
                ))
                .with_request_dump(take_captured_request_dump(&captured_request))
                .with_retry_after(take_captured_retry_after(&captured_response_retry_after))
            })?;
        (response, None)
    };
    let wire_request_dump = take_captured_request_dump(&captured_request);

    Ok(NeutralChatStream {
        stream: Box::pin(response.stream),
        error_context: error_context.with_phase("reading provider stream"),
        wire_request_dump,
        response_status: captured_response_status,
        response_retry_after: captured_response_retry_after,
        response_head: captured_response_head,
        saw_response_event: false,
        final_response_dump: None,
        stream_diagnostics,
    })
}

fn normalize_request_developer_role(
    mut request: NeutralChatRequest,
    runtime_options: &ChatRequestRuntimeOptions,
) -> NeutralChatRequest {
    if !runtime_options.developer_role_enabled {
        for message in &mut request.messages {
            if message.role == NeutralChatRole::Developer {
                message.role = NeutralChatRole::System;
            }
        }
    }
    request
}

async fn stream_chat_with_capture_observer_websocket(
    config: &ProviderConnectionConfig,
    request: NeutralChatRequest,
    runtime_options: ChatRequestRuntimeOptions,
    capture_details: bool,
    request_observer: Option<ProviderRequestDumpObserver>,
    session_ctx: Option<ProviderWsSessionContext>,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    ensure_proxy_compatible_with_kind(config.kind, config.proxy_url.is_some())
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let chat_request = genai_chat_request_for_adapter(&request, config.kind.adapter_kind())
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let upstream_model_id = upstream_provider_model_id(&request.model_id, &config.model_redirects)
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let error_context = config
        .provider_error_context("opening provider stream", upstream_model_id)
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let options = genai_chat_options_with_runtime_options(config, &request, &runtime_options)
        .map_err(|error| ProviderRequestFailure::new(error))?;
    let model = genai::ModelIden::new(config.kind.adapter_kind(), upstream_model_id.to_string());
    openai_resp_websocket::stream_chat_openai_resp_websocket(
        config,
        chat_request,
        options,
        model,
        error_context,
        capture_details,
        request_observer,
        session_ctx,
    )
    .await
}

pub fn parse_provider_kind(value: &str) -> Result<ProviderKind, ProviderConfigError> {
    let value = value.trim();
    supported_provider_kinds()
        .iter()
        .copied()
        .find(|kind| kind.as_str() == value)
        .ok_or_else(|| ProviderConfigError::UnsupportedKind(value.to_string()))
}

/// Reject API proxy when the provider kind uses the WebSocket Responses transport.
/// First release does not implement proxy tunneling for WebSocket.
pub fn ensure_proxy_compatible_with_kind(
    kind: ProviderKind,
    proxy_enabled: bool,
) -> Result<(), ProviderConfigError> {
    if proxy_enabled && kind.uses_websocket() {
        return Err(ProviderConfigError::UnsupportedProxyForWebSocket {
            kind: kind.as_str().to_string(),
        });
    }
    Ok(())
}

/// Build the full HTTP OpenAI Responses URL the adapter would call from an HTTP API base.
/// Example: `https://gateway.example/v1/` → `https://gateway.example/v1/responses`
pub fn openai_responses_http_url_from_base(base_url: &str) -> Result<String, ProviderConfigError> {
    let normalized = normalized_base_url(base_url)?;
    let mut url =
        reqwest::Url::parse(&normalized).map_err(|source| ProviderConfigError::InvalidBaseUrl {
            value: base_url.to_string(),
            source: source.to_string(),
        })?;
    if url.host_str().is_none() {
        return Err(ProviderConfigError::InvalidBaseUrl {
            value: base_url.to_string(),
            source: "host is required".to_string(),
        });
    }
    {
        let mut segments =
            url.path_segments_mut()
                .map_err(|_| ProviderConfigError::InvalidBaseUrl {
                    value: base_url.to_string(),
                    source: "base URL cannot be used as a path base".to_string(),
                })?;
        segments.pop_if_empty();
        segments.push("responses");
    }
    Ok(url.to_string())
}

/// Derive a WebSocket endpoint from a complete OpenAI Responses HTTP URL.
/// Only `http→ws` and `https→wss` are allowed; host, port, path, and query are preserved.
pub fn websocket_url_from_responses_http_url(
    http_url: &str,
) -> Result<String, ProviderConfigError> {
    let trimmed = http_url.trim();
    if trimmed.is_empty() {
        return Err(ProviderConfigError::InvalidBaseUrl {
            value: http_url.to_string(),
            source: "Responses HTTP URL must not be empty".to_string(),
        });
    }
    let mut url =
        reqwest::Url::parse(trimmed).map_err(|source| ProviderConfigError::InvalidBaseUrl {
            value: trimmed.to_string(),
            source: source.to_string(),
        })?;
    if url.host_str().is_none() {
        return Err(ProviderConfigError::InvalidBaseUrl {
            value: trimmed.to_string(),
            source: "host is required".to_string(),
        });
    }
    let ws_scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        other => {
            return Err(ProviderConfigError::InvalidBaseUrl {
                value: trimmed.to_string(),
                source: format!(
                    "WebSocket URL must be derived from http or https Responses URL, got '{other}'"
                ),
            });
        }
    };
    url.set_scheme(ws_scheme)
        .map_err(|()| ProviderConfigError::InvalidBaseUrl {
            value: trimmed.to_string(),
            source: format!("failed to convert scheme to '{ws_scheme}'"),
        })?;
    Ok(url.to_string())
}

/// HTTP `base_url` → full Responses HTTP URL → WebSocket endpoint.
pub fn openai_responses_websocket_url_from_base(
    base_url: &str,
) -> Result<String, ProviderConfigError> {
    let http_url = openai_responses_http_url_from_base(base_url)?;
    websocket_url_from_responses_http_url(&http_url)
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

            Ok(ChatMessage::developer(message.content.clone()))
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

#[cfg(test)]
fn genai_chat_options(
    config: &ProviderConnectionConfig,
    request: &NeutralChatRequest,
) -> Result<ChatOptions, ProviderConfigError> {
    genai_chat_options_with_runtime_options(config, request, &ChatRequestRuntimeOptions::default())
}

fn genai_chat_options_with_runtime_options(
    config: &ProviderConnectionConfig,
    request: &NeutralChatRequest,
    runtime_options: &ChatRequestRuntimeOptions,
) -> Result<ChatOptions, ProviderConfigError> {
    let model_id = upstream_provider_model_id(&request.model_id, &config.model_redirects)?;
    // ponytail: model-id heuristic; add provider metadata if non-Claude ids ever contain "claude".
    let is_claude = model_id.to_ascii_lowercase().contains("claude");
    let default_temperature = if is_claude { 1.0 } else { 0.0 };
    let temperature = runtime_options.temperature.unwrap_or(default_temperature);
    if !temperature.is_finite() || !(0.0..=2.0).contains(&temperature) {
        return Err(ProviderConfigError::InvalidRequest(format!(
            "temperature must be a finite value between 0 and 2: {temperature}"
        )));
    }
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

    options = apply_neutral_tool_choice(options, config.kind.adapter_kind(), request)?;

    apply_request_overrides_and_agent_headers(options, config, request, runtime_options)
}

/// Map Foco `NeutralToolChoice` onto genai `ChatOptions` when the adapter supports it.
///
/// Unsupported adapters leave tool_choice unset (tools + prompt only) and log an explicit
/// degradation so callers never assume native enforcement happened.
fn apply_neutral_tool_choice(
    mut options: ChatOptions,
    adapter_kind: AdapterKind,
    request: &NeutralChatRequest,
) -> Result<ChatOptions, ProviderConfigError> {
    let NeutralToolChoice::RequiredSingleTool { tool_name } = &request.tool_choice else {
        return Ok(options);
    };

    let tool_name = tool_name.trim();
    if tool_name.is_empty() {
        return Err(ProviderConfigError::InvalidRequest(
            "RequiredSingleTool tool name must not be empty".to_string(),
        ));
    }
    if !request.tools.iter().any(|tool| tool.name == tool_name) {
        return Err(ProviderConfigError::InvalidRequest(format!(
            "RequiredSingleTool '{tool_name}' is not present in request.tools"
        )));
    }

    match resolve_tool_choice_enforcement(adapter_kind, &request.tool_choice) {
        ToolChoiceEnforcement::Applied => {
            options = options.with_tool_choice(GenaiToolChoice::tool(tool_name));
            Ok(options)
        }
        ToolChoiceEnforcement::UnsupportedDegraded => {
            tracing::warn!(
                adapter = adapter_kind.as_str(),
                tool_name,
                "RequiredSingleTool requested but adapter does not support native forced tool choice; degrading to tools + prompt (+ repair retry)"
            );
            Ok(options)
        }
        ToolChoiceEnforcement::Auto => Ok(options),
    }
}

fn apply_request_overrides_and_agent_headers(
    mut options: ChatOptions,
    config: &ProviderConnectionConfig,
    request: &NeutralChatRequest,
    runtime_options: &ChatRequestRuntimeOptions,
) -> Result<ChatOptions, ProviderConfigError> {
    let mut override_headers = Vec::new();
    let mut body = Map::new();

    for override_rule in &config.request_overrides {
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
                override_headers.push((name, header_value.to_string()));
            }
            REQUEST_OVERRIDE_TARGET_BODY => {
                insert_nested_body_override(&mut body, &name, value)?;
            }
            _ => unreachable!("request override target was validated"),
        }
    }

    // Fast is a first-class runtime choice. Apply it after generic body overrides so the
    // persisted UI mode always describes the service tier sent to OpenAI Responses.
    apply_fast_latency_mode(&mut body, config, request, runtime_options)?;

    // OpenAI Responses only (not xAI Responses): built-in Agent headers first, then overrides.
    let headers = if uses_openai_resp_agent_headers(config.kind) {
        merge_header_pairs(
            default_openai_resp_agent_headers(
                config.kind.uses_websocket(),
                request.agent_correlation.as_ref(),
            ),
            override_headers,
        )
    } else {
        override_headers
    };

    if !headers.is_empty() {
        options = options.with_extra_headers(Headers::from(headers));
    }

    if !body.is_empty() {
        options = options.with_extra_body(Value::Object(body));
    }

    Ok(options)
}

fn apply_fast_latency_mode(
    body: &mut Map<String, Value>,
    config: &ProviderConnectionConfig,
    request: &NeutralChatRequest,
    runtime_options: &ChatRequestRuntimeOptions,
) -> Result<(), ProviderConfigError> {
    if runtime_options.latency_mode != LatencyMode::Fast {
        return Ok(());
    }

    if !config.supports_fast_latency_mode(&request.model_id)? {
        return Err(ProviderConfigError::InvalidRequest(format!(
            "Fast latency mode is not supported for provider '{}' and model '{}'",
            config.kind.as_str(),
            request.model_id
        )));
    }

    body.insert(
        "service_tier".to_string(),
        Value::String("priority".to_string()),
    );
    Ok(())
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
    captured_request: &Option<Arc<Mutex<Option<ProviderAuditRequestDump>>>>,
) -> Option<ProviderAuditRequestDump> {
    captured_request
        .as_ref()
        .and_then(|captured_request| captured_request.lock().ok()?.take())
}

fn take_captured_retry_after(
    captured_response_retry_after: &Arc<Mutex<Option<Duration>>>,
) -> Option<Duration> {
    captured_response_retry_after
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
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

/// Parse a provider `Retry-After` header into a bounded delay.
///
/// Only integer-second values are accepted (HTTP-date forms are ignored). Values are
/// clamped to 30 seconds so retry storms cannot wait unbounded on a malicious header.
pub fn parse_retry_after_seconds(raw: &str) -> Option<Duration> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let seconds = trimmed.parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds.min(30)))
}

pub fn retry_after_from_http_headers(headers: &ProviderHttpHeadersDump) -> Option<Duration> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, values)| values.first())
        .and_then(|value| parse_retry_after_seconds(value))
}

pub(crate) fn retry_after_from_reqwest_headers(
    headers: &reqwest::header::HeaderMap,
) -> Option<Duration> {
    headers
        .get_all(reqwest::header::RETRY_AFTER)
        .iter()
        .find_map(|value| value.to_str().ok().and_then(parse_retry_after_seconds))
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

pub(crate) fn redact_json_body_credentials(body: &str) -> String {
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
) -> Result<Option<NeutralChatStreamEvent>, ProviderConfigError> {
    match event {
        ChatStreamEvent::Start => Ok(Some(NeutralChatStreamEvent::Start)),
        ChatStreamEvent::Chunk(chunk) => Ok(Some(NeutralChatStreamEvent::TextDelta {
            delta: chunk.content,
        })),
        ChatStreamEvent::ReasoningChunk(chunk) => {
            Ok(Some(NeutralChatStreamEvent::ReasoningDelta {
                delta: chunk.content,
            }))
        }
        ChatStreamEvent::ThoughtSignatureChunk(chunk) => {
            Ok(Some(NeutralChatStreamEvent::ThoughtSignatureDelta {
                delta: chunk.content,
            }))
        }
        ChatStreamEvent::ToolCallChunk(chunk) => {
            // Provider-native web search is server-side. Adapters should not surface it as a
            // function ToolCall; if a stream still emits one, drop it so Foco never executes
            // Tavily/Brave for a native search lifecycle event.
            if is_provider_native_web_search_tool_call_name(&chunk.tool_call.fn_name) {
                tracing::debug!(
                    tool_name = %chunk.tool_call.fn_name,
                    "ignoring provider-native web search stream tool call chunk"
                );
                return Ok(None);
            }
            Ok(Some(NeutralChatStreamEvent::ToolCall {
                tool_call: neutral_tool_call(&chunk.tool_call),
            }))
        }
        ChatStreamEvent::End(end) => normalize_stream_end(end).map(Some),
    }
}

fn normalize_stream_end(end: StreamEnd) -> Result<NeutralChatStreamEvent, ProviderConfigError> {
    let text = end.captured_first_text().unwrap_or_default().to_string();
    let tool_calls = end
        .captured_tool_calls()
        .unwrap_or_default()
        .into_iter()
        .filter(|tool_call| !is_provider_native_web_search_tool_call_name(&tool_call.fn_name))
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

/// Names that must never become Foco-executable `web_search` function calls.
///
/// OpenAI/xAI Responses native search uses `type=web_search` and is not a function_call.
/// Genai's builtin display name is exactly `WebSearch` (not the Foco function name `web_search`).
fn is_provider_native_web_search_tool_call_name(name: &str) -> bool {
    name.trim() == "WebSearch"
}

fn genai_tool(tool: &NeutralToolDefinition) -> Tool {
    match tool.kind {
        NeutralToolKind::ProviderWebSearch => Tool::new_web_search(),
        NeutralToolKind::Function => Tool::new(tool.name.clone())
            .with_description(tool.description.clone())
            .with_schema(tool.input_schema.clone())
            .with_strict(tool.strict),
    }
}

fn neutral_tool_call(tool_call: &GenaiToolCall) -> NeutralToolCall {
    NeutralToolCall {
        call_id: tool_call.call_id.clone(),
        name: tool_call.fn_name.clone(),
        arguments: normalized_tool_arguments(&tool_call.fn_arguments),
        thought_signatures: tool_call.thought_signatures.clone(),
    }
}

/// Maximum recursive unwraps of JSON-encoded string values from models.
/// Shared by ToolCall argument normalization and plain-text JSON recovery.
const MODEL_JSON_STRING_UNWRAP_DEPTH: usize = 4;

/// Normalize ToolCall `arguments` that providers sometimes deliver as JSON strings
/// (including limited nested string encoding). Non-string values and non-JSON strings
/// are returned unchanged. Never panics and never repairs invalid JSON syntax.
pub fn normalized_tool_arguments(arguments: &Value) -> Value {
    unwrap_model_json_encoded_value(arguments.clone())
}

/// Recover a `serde_json::Value` from model text that was intended as structured tool
/// arguments rather than a native ToolCall.
///
/// Accepts, conservatively:
/// - pure JSON object/array text (after trim)
/// - limited nested JSON string encoding of those values
/// - a single Markdown fenced code block whose language is empty or `json` (case
///   insensitive), with no surrounding prose
///
/// Rejects extra prose, non-`json` fences, invalid JSON, and inputs that would require
/// guessing (single quotes, trailing commas, unquoted keys, etc.).
pub fn recover_model_json_from_text(text: &str) -> Option<Value> {
    let candidate = extract_model_json_candidate_text(text)?;
    let parsed = serde_json::from_str::<Value>(candidate.trim()).ok()?;
    Some(unwrap_model_json_encoded_value(parsed))
}

fn extract_model_json_candidate_text(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(inner) = extract_single_json_fenced_block(trimmed) {
        return Some(inner);
    }

    if model_json_text_looks_like_encoded_json(trimmed) {
        return Some(trimmed);
    }

    None
}

/// Extract the body of a single fenced code block that is the entire `text`.
/// Language must be empty or `json` (ASCII case insensitive). No surrounding prose.
fn extract_single_json_fenced_block(text: &str) -> Option<&str> {
    let text = text.trim();
    if !text.starts_with("```") {
        return None;
    }

    let after_open = &text[3..];
    let newline_idx = after_open.find('\n')?;
    let lang = after_open[..newline_idx].trim().trim_end_matches('\r');
    if !lang.is_empty() && !lang.eq_ignore_ascii_case("json") {
        return None;
    }

    let body = &after_open[newline_idx + 1..];
    let body_trimmed_end = body.trim_end();
    let content = body_trimmed_end.strip_suffix("```")?;
    let content = content.trim();
    if content.is_empty() {
        return None;
    }

    Some(content)
}

fn model_json_text_looks_like_encoded_json(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with('{')
        || trimmed.starts_with('[')
        || trimmed.starts_with("\"{")
        || trimmed.starts_with("\"[")
}

fn unwrap_model_json_encoded_value(mut current: Value) -> Value {
    for _ in 0..MODEL_JSON_STRING_UNWRAP_DEPTH {
        let Value::String(text) = &current else {
            return current;
        };

        let trimmed = text.trim();
        if !model_json_text_looks_like_encoded_json(trimmed) {
            return current;
        }

        let Ok(parsed) = serde_json::from_str::<Value>(trimmed) else {
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
#[derive(Clone)]
pub(crate) struct ProviderErrorContext {
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

    pub(crate) fn with_phase(&self, phase: &'static str) -> Self {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStreamFailureKind {
    Capacity,
    RateLimit,
    ServerError,
    Auth,
    Permission,
    InvalidRequest,
    ContextLength,
    ProtocolParse,
    Other,
}

impl ProviderStreamFailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Capacity => "capacity",
            Self::RateLimit => "rate_limit",
            Self::ServerError => "server_error",
            Self::Auth => "auth",
            Self::Permission => "permission",
            Self::InvalidRequest => "invalid_request",
            Self::ContextLength => "context_length",
            Self::ProtocolParse => "protocol_parse",
            Self::Other => "other",
        }
    }

    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Capacity | Self::RateLimit | Self::ServerError)
    }
}

/// Structured OpenAI Responses / provider stream failure retained across the Foco boundary.
///
/// Raw frames stay on audit `streamDiagnostic`; this type only carries control-flow fields and a
/// short, safe user-facing message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderStreamErrorDetail {
    pub message: String,
    pub status_code: Option<u16>,
    pub kind: ProviderStreamFailureKind,
    pub code: Option<String>,
    pub error_type: Option<String>,
    pub param: Option<String>,
    pub event_type: Option<String>,
    pub diagnostic_kind: Option<String>,
    pub model_id: Option<String>,
    pub adapter: Option<String>,
}

impl ProviderStreamErrorDetail {
    pub fn user_message(&self) -> &str {
        self.message.as_str()
    }

    pub fn summary_with_model_context(&self) -> String {
        match self.model_id.as_deref().filter(|value| !value.is_empty()) {
            Some(model_id) => format!("{} (model '{model_id}')", self.message),
            None => self.message.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderConfigError {
    Connection {
        message: String,
        status_code: Option<u16>,
    },
    /// Successfully decoded provider stream failure (`type:error` / `response.failed`) or a true
    /// stream parse failure classified as [`ProviderStreamFailureKind::ProtocolParse`].
    ProviderStream(Box<ProviderStreamErrorDetail>),
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
    /// API proxy is not supported for WebSocket Responses transport in the first release.
    UnsupportedProxyForWebSocket {
        kind: String,
    },
}

impl ProviderConfigError {
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Connection { status_code, .. } => *status_code,
            Self::ProviderStream(detail) => detail.status_code,
            Self::EmptyBaseUrl
            | Self::EmptyProxyUrl
            | Self::InvalidBaseUrl { .. }
            | Self::InvalidProxyUrl { .. }
            | Self::InvalidRequest(_)
            | Self::MissingRequiredField(_)
            | Self::MissingApiKey
            | Self::UnsupportedKind(_)
            | Self::UnsupportedProxyKind(_)
            | Self::UnsupportedProxyForWebSocket { .. } => None,
        }
    }

    /// Short, safe message suitable for chat failure bubbles.
    pub fn user_message(&self) -> String {
        match self {
            Self::ProviderStream(detail) => detail.summary_with_model_context(),
            other => other.to_string(),
        }
    }

    pub fn stream_detail(&self) -> Option<&ProviderStreamErrorDetail> {
        match self {
            Self::ProviderStream(detail) => Some(detail),
            _ => None,
        }
    }

    pub fn stream_failure_kind(&self) -> Option<ProviderStreamFailureKind> {
        self.stream_detail().map(|detail| detail.kind)
    }

    pub(crate) fn from_genai_error_with_context(
        source: genai::Error,
        context: &ProviderErrorContext,
    ) -> Self {
        match source {
            genai::Error::ProviderStream(inner) => {
                let message = sanitize_provider_stream_message(&inner.message);
                let kind = classify_provider_stream_failure_kind(
                    inner.code.as_deref(),
                    inner.error_type.as_deref(),
                    &message,
                    None,
                );
                Self::ProviderStream(Box::new(ProviderStreamErrorDetail {
                    message,
                    // SSE error events commonly arrive on HTTP 200; leave status unset so
                    // classification uses structured code/type/message instead of transport status.
                    status_code: None,
                    kind,
                    code: inner.code,
                    error_type: inner.error_type,
                    param: inner.param,
                    event_type: Some(inner.event_type),
                    diagnostic_kind: Some(provider_stream_diagnostic_kind_label(
                        inner.diagnostic_kind,
                    )),
                    model_id: Some(context.model_id.clone()),
                    adapter: Some(context.adapter.to_string()),
                }))
            }
            genai::Error::StreamParse {
                model_iden,
                serde_error,
            } => {
                let message = format!("Failed to parse stream data: {serde_error}");
                Self::ProviderStream(Box::new(ProviderStreamErrorDetail {
                    message,
                    status_code: None,
                    kind: ProviderStreamFailureKind::ProtocolParse,
                    code: None,
                    error_type: None,
                    param: None,
                    event_type: None,
                    diagnostic_kind: Some("invalid_json".to_string()),
                    model_id: Some(model_iden.model_name.to_string()),
                    adapter: Some(context.adapter.to_string()),
                }))
            }
            other => {
                let status_code = genai_error_status_code(&other).map(|status| status.as_u16());
                Self::Connection {
                    message: format!("{context}: {other}"),
                    status_code,
                }
            }
        }
    }
}

impl fmt::Display for ProviderConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection { message, .. } => {
                write!(formatter, "provider connection failed: {message}")
            }
            Self::ProviderStream(detail) => {
                write!(formatter, "{}", detail.summary_with_model_context())
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
            Self::UnsupportedProxyForWebSocket { kind } => write!(
                formatter,
                "AI API proxy is not supported for WebSocket provider kind '{kind}'; disable the proxy or use an HTTP Responses protocol"
            ),
        }
    }
}

impl std::error::Error for ProviderConfigError {}

const PROVIDER_STREAM_MESSAGE_LIMIT_CHARS: usize = 512;

fn sanitize_provider_stream_message(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return "provider stream error".to_string();
    }
    // Redact common credential shapes before the text reaches chat bubbles / broker payloads.
    // streamDiagnostic already redacts structured JSON; this covers free-text provider messages.
    let mut redacted = trimmed.to_string();
    for pattern in [
        "Bearer ",
        "bearer ",
        "api_key=",
        "api-key=",
        "apiKey=",
        "access_token=",
        "accessToken=",
        "sk-",
        "rk-",
    ] {
        if let Some(idx) = redacted.find(pattern) {
            let start = idx + pattern.len();
            let end = redacted[start..]
                .find(|ch: char| {
                    ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ',' || ch == '}'
                })
                .map(|offset| start + offset)
                .unwrap_or(redacted.len());
            if end > start {
                redacted.replace_range(start..end, REDACTED_CREDENTIAL_VALUE);
            }
        }
    }
    let mut out = String::new();
    for (index, ch) in redacted.chars().enumerate() {
        if index >= PROVIDER_STREAM_MESSAGE_LIMIT_CHARS {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn provider_stream_diagnostic_kind_label(kind: genai::OpenAIRespStreamDiagnosticKind) -> String {
    match kind {
        genai::OpenAIRespStreamDiagnosticKind::ProviderErrorEvent => {
            "provider_error_event".to_string()
        }
        genai::OpenAIRespStreamDiagnosticKind::ResponseFailed => "response_failed".to_string(),
        genai::OpenAIRespStreamDiagnosticKind::InvalidJson => "invalid_json".to_string(),
        genai::OpenAIRespStreamDiagnosticKind::TransportError => "transport_error".to_string(),
        genai::OpenAIRespStreamDiagnosticKind::UnexpectedEof => "unexpected_eof".to_string(),
    }
}

/// Classify a provider stream failure from structured fields and optional HTTP status.
pub fn classify_provider_stream_failure_kind(
    code: Option<&str>,
    error_type: Option<&str>,
    message: &str,
    status_code: Option<u16>,
) -> ProviderStreamFailureKind {
    let code = code.unwrap_or("").to_ascii_lowercase();
    let error_type = error_type.unwrap_or("").to_ascii_lowercase();
    let message = message.to_ascii_lowercase();
    let haystack = format!("{code} {error_type} {message}");

    if matches!(status_code, Some(401))
        || contains_any(
            &haystack,
            &[
                "invalid_api_key",
                "authentication",
                "unauthorized",
                "invalid auth",
                "incorrect api key",
                "invalid api key",
            ],
        )
    {
        return ProviderStreamFailureKind::Auth;
    }
    if matches!(status_code, Some(403))
        || contains_any(
            &haystack,
            &["permission", "forbidden", "access_denied", "not allowed"],
        )
    {
        return ProviderStreamFailureKind::Permission;
    }
    if contains_any(
        &haystack,
        &[
            "context_length",
            "context length",
            "maximum context",
            "max context",
            "context window",
            "too many tokens",
            "token limit",
            "max_tokens",
        ],
    ) {
        return ProviderStreamFailureKind::ContextLength;
    }
    if contains_any(
        &haystack,
        &[
            "failed to parse stream",
            "invalid_json",
            "invalid json",
            "parse stream",
            "protocol parse",
        ],
    ) {
        return ProviderStreamFailureKind::ProtocolParse;
    }
    if matches!(status_code, Some(429))
        || contains_any(
            &haystack,
            &[
                "rate_limit",
                "rate limit",
                "too many requests",
                "rate_limited",
                "requests per",
            ],
        )
    {
        return ProviderStreamFailureKind::RateLimit;
    }
    // Billing / account quota exhaustion is not a transient capacity blip; do not retry.
    if contains_any(
        &haystack,
        &[
            "insufficient_quota",
            "insufficient quota",
            "billing",
            "payment required",
            "quota exceeded",
            "exceeded your current quota",
        ],
    ) {
        return ProviderStreamFailureKind::InvalidRequest;
    }
    if contains_any(
        &haystack,
        &[
            "capacity",
            "overloaded",
            "model_capacity",
            "no capacity",
            "out of capacity",
            "resource_exhausted",
            "server is busy",
            "high demand",
        ],
    ) {
        return ProviderStreamFailureKind::Capacity;
    }
    if matches!(status_code, Some(500..=599))
        || contains_any(
            &haystack,
            &[
                "server_error",
                "internal_error",
                "internal server",
                "service_unavailable",
                "bad_gateway",
                "gateway_timeout",
                "temporarily unavailable",
            ],
        )
    {
        return ProviderStreamFailureKind::ServerError;
    }
    if matches!(status_code, Some(400 | 404 | 422))
        || contains_any(
            &haystack,
            &[
                "invalid_request",
                "invalid request",
                "bad request",
                "not found",
                "validation_error",
            ],
        )
    {
        return ProviderStreamFailureKind::InvalidRequest;
    }
    ProviderStreamFailureKind::Other
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

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
        | genai::Error::ProviderStream(_)
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
            neutral_text_message(NeutralChatRole::Developer, "adapter developer"),
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
            kind: NeutralToolKind::Function,
        });

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open adapter fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("adapter wire request dump")
            .as_http()
            .expect("http request dump")
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
            OPENAI_CHAT_KIND | OPENAI_RESPONSES_KIND | DEEPSEEK_KIND => {
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
            _ => unreachable!("fixture covers primary adapters and compatible fallback"),
        }
        match kind_name {
            OPENAI_CHAT_KIND => {
                assert_eq!(
                    body["messages"],
                    serde_json::json!([
                        {"role": "system", "content": "adapter system"},
                        {"role": "developer", "content": "adapter developer"},
                        {"role": "user", "content": "adapter user"}
                    ])
                );
            }
            OPENAI_RESPONSES_KIND => {
                assert_eq!(body["instructions"], "adapter system");
                assert_eq!(
                    body["input"],
                    serde_json::json!([
                        {"role": "developer", "content": "adapter developer"},
                        {"role": "user", "content": "adapter user"}
                    ])
                );
            }
            ANTHROPIC_KIND => {
                assert_eq!(body["system"], "adapter system\n\nadapter developer");
                assert_eq!(
                    body["messages"],
                    serde_json::json!([
                        {"role": "user", "content": "adapter user"}
                    ])
                );
                assert!(!raw.body.contains("\"role\":\"developer\""));
            }
            GEMINI_KIND => {
                assert_eq!(
                    body["systemInstruction"],
                    serde_json::json!({
                        "parts": [{"text": "adapter system\nadapter developer"}]
                    })
                );
                assert_eq!(
                    body["contents"],
                    serde_json::json!([
                        {"role": "user", "parts": [{"text": "adapter user"}]}
                    ])
                );
                assert!(!raw.body.contains("\"role\":\"developer\""));
            }
            DEEPSEEK_KIND => {
                assert_eq!(
                    body["messages"],
                    serde_json::json!([
                        {"role": "system", "content": "adapter system"},
                        {"role": "system", "content": "adapter developer"},
                        {"role": "user", "content": "adapter user"}
                    ])
                );
                assert!(!raw.body.contains("\"role\":\"developer\""));
            }
            _ => unreachable!("fixture covers primary adapters and compatible fallback"),
        }
        if kind_name != GEMINI_KIND {
            assert!(raw.body.contains(&format!("upstream-{model_id}")));
        } else {
            assert!(raw.target.contains(&format!("upstream-{model_id}")));
        }
        assert!(raw.body.contains("adapter system"));
        assert!(raw.body.contains("adapter developer"));
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
    fn fast_latency_capability_uses_active_adapter_and_redirected_upstream_model() {
        let redirects = [ProviderModelRedirect {
            from: "gpt-5.4".to_string(),
            to: "friendly-fast-model".to_string(),
        }];
        let responses_kind = openai_responses_kind();
        let chat_kind = parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind");

        assert!(
            supports_fast_latency_mode(responses_kind, "friendly-fast-model", &redirects)
                .expect("redirected Responses capability")
        );
        assert!(
            !supports_fast_latency_mode(responses_kind, "gpt-4o", &redirects)
                .expect("older model is not Fast-capable")
        );
        assert!(
            !supports_fast_latency_mode(chat_kind, "friendly-fast-model", &redirects)
                .expect("non-Responses adapter is not Fast-capable")
        );
    }

    #[test]
    fn web_search_mode_defaults_and_serializes_as_camel_case() {
        assert_eq!(
            serde_json::from_value::<WebSearchMode>(serde_json::json!("auto")).expect("auto mode"),
            WebSearchMode::Auto
        );
        assert_eq!(
            serde_json::to_value(WebSearchMode::Native).expect("serialize native"),
            serde_json::json!("native")
        );
        assert_eq!(
            serde_json::from_value::<WebSearchMode>(serde_json::Value::Null).unwrap_or_default(),
            WebSearchMode::Auto
        );
        assert!(
            serde_json::from_value::<WebSearchMode>(serde_json::json!("bogus")).is_err(),
            "invalid webSearchMode must fail"
        );
    }

    #[test]
    fn resolve_web_search_route_prefers_confirmed_native_and_falls_back_conservatively() {
        let responses = openai_responses_kind();
        let chat = parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind");

        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: false,
                fallback_available: true,
                provider_kind: Some(responses),
                upstream_model_id: "gpt-4o",
                mode: WebSearchMode::Auto,
            }),
            WebSearchRoute::Disabled
        );

        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: true,
                provider_kind: Some(responses),
                upstream_model_id: "gpt-4o",
                mode: WebSearchMode::Auto,
            }),
            WebSearchRoute::ProviderNative
        );

        // Unknown model capability must not optimistically send a native tool.
        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: true,
                provider_kind: Some(responses),
                upstream_model_id: "custom-gateway-model",
                mode: WebSearchMode::Auto,
            }),
            WebSearchRoute::FocoFunction
        );
        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: false,
                provider_kind: Some(responses),
                upstream_model_id: "custom-gateway-model",
                mode: WebSearchMode::Auto,
            }),
            WebSearchRoute::Disabled
        );

        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: true,
                provider_kind: Some(chat),
                upstream_model_id: "gpt-4o",
                mode: WebSearchMode::Auto,
            }),
            WebSearchRoute::FocoFunction
        );

        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: false,
                provider_kind: Some(responses),
                upstream_model_id: "gpt-4o",
                mode: WebSearchMode::Native,
            }),
            WebSearchRoute::ProviderNative
        );
        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: true,
                provider_kind: Some(chat),
                upstream_model_id: "gpt-4o",
                mode: WebSearchMode::Native,
            }),
            WebSearchRoute::Disabled
        );

        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: true,
                provider_kind: Some(responses),
                upstream_model_id: "gpt-4o",
                mode: WebSearchMode::Function,
            }),
            WebSearchRoute::FocoFunction
        );
        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: false,
                provider_kind: Some(responses),
                upstream_model_id: "gpt-4o",
                mode: WebSearchMode::Function,
            }),
            WebSearchRoute::Disabled
        );

        assert_eq!(
            resolve_web_search_route(WebSearchRouteInput {
                enabled: true,
                fallback_available: true,
                provider_kind: Some(responses),
                upstream_model_id: "gpt-4o",
                mode: WebSearchMode::Disabled,
            }),
            WebSearchRoute::Disabled
        );
    }

    /// Provider × model × config route matrix (table-driven).
    ///
    /// Each case asserts the single runtime route. Native is only available on OpenAIResp
    /// protocols (OpenAI Responses HTTP/WS and xAI Responses). Claude/Gemini adapters can
    /// serialize a builtin web-search tool in genai, but Foco's central capability gate does
    /// not treat those protocols as native-capable yet — auto falls back to FocoFunction.
    #[test]
    fn web_search_route_matrix_covers_providers_modes_and_fallback() {
        let openai_resp = openai_responses_kind();
        let openai_ws = parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws");
        let openai_chat = parse_provider_kind(OPENAI_CHAT_KIND).expect("chat");
        let xai_resp = parse_provider_kind(XAI_RESPONSES_KIND).expect("xai responses");
        let xai_chat = parse_provider_kind(XAI_KIND).expect("xai chat");
        let anthropic = parse_provider_kind(ANTHROPIC_KIND).expect("anthropic");
        let gemini = parse_provider_kind(GEMINI_KIND).expect("gemini");
        let deepseek = parse_provider_kind(DEEPSEEK_KIND).expect("deepseek");
        let ollama = parse_provider_kind(OLLAMA_KIND).expect("ollama");

        struct Case {
            name: &'static str,
            enabled: bool,
            fallback: bool,
            kind: Option<ProviderKind>,
            model: &'static str,
            mode: WebSearchMode,
            expected: WebSearchRoute,
        }

        let cases = [
            Case {
                name: "master_switch_off_disables_even_with_native_model",
                enabled: false,
                fallback: true,
                kind: Some(openai_resp),
                model: "gpt-4o",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::Disabled,
            },
            Case {
                name: "openai_responses_gpt4o_auto_native",
                enabled: true,
                fallback: true,
                kind: Some(openai_resp),
                model: "gpt-4o",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::ProviderNative,
            },
            Case {
                name: "openai_responses_ws_gpt5_auto_native",
                enabled: true,
                fallback: false,
                kind: Some(openai_ws),
                model: "gpt-5.4",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::ProviderNative,
            },
            Case {
                name: "xai_responses_grok_auto_native",
                enabled: true,
                fallback: true,
                kind: Some(xai_resp),
                model: "grok-3",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::ProviderNative,
            },
            Case {
                name: "xai_chat_grok_auto_function_fallback",
                enabled: true,
                fallback: true,
                kind: Some(xai_chat),
                model: "grok-3",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::FocoFunction,
            },
            Case {
                name: "anthropic_claude_auto_function_not_native",
                enabled: true,
                fallback: true,
                kind: Some(anthropic),
                model: "claude-sonnet-4-20250514",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::FocoFunction,
            },
            Case {
                name: "gemini_auto_function_not_native",
                enabled: true,
                fallback: true,
                kind: Some(gemini),
                model: "gemini-2.5-pro",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::FocoFunction,
            },
            Case {
                name: "openai_chat_auto_function_fallback",
                enabled: true,
                fallback: true,
                kind: Some(openai_chat),
                model: "gpt-4o",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::FocoFunction,
            },
            Case {
                name: "deepseek_auto_function_fallback",
                enabled: true,
                fallback: true,
                kind: Some(deepseek),
                model: "deepseek-chat",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::FocoFunction,
            },
            Case {
                name: "ollama_auto_function_fallback",
                enabled: true,
                fallback: true,
                kind: Some(ollama),
                model: "llama3.2",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::FocoFunction,
            },
            Case {
                name: "no_fallback_key_auto_unknown_model_disabled",
                enabled: true,
                fallback: false,
                kind: Some(openai_resp),
                model: "custom-gateway-model",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::Disabled,
            },
            Case {
                name: "no_fallback_key_chat_auto_disabled",
                enabled: true,
                fallback: false,
                kind: Some(openai_chat),
                model: "gpt-4o",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::Disabled,
            },
            Case {
                name: "explicit_native_openai_responses",
                enabled: true,
                fallback: false,
                kind: Some(openai_resp),
                model: "gpt-4o",
                mode: WebSearchMode::Native,
                expected: WebSearchRoute::ProviderNative,
            },
            Case {
                name: "explicit_native_on_chat_protocol_disabled",
                enabled: true,
                fallback: true,
                kind: Some(openai_chat),
                model: "gpt-4o",
                mode: WebSearchMode::Native,
                expected: WebSearchRoute::Disabled,
            },
            Case {
                name: "explicit_native_anthropic_protocol_disabled",
                enabled: true,
                fallback: true,
                kind: Some(anthropic),
                model: "claude-sonnet-4-20250514",
                mode: WebSearchMode::Native,
                expected: WebSearchRoute::Disabled,
            },
            Case {
                name: "explicit_function_with_key",
                enabled: true,
                fallback: true,
                kind: Some(openai_resp),
                model: "gpt-4o",
                mode: WebSearchMode::Function,
                expected: WebSearchRoute::FocoFunction,
            },
            Case {
                name: "explicit_function_without_key_disabled",
                enabled: true,
                fallback: false,
                kind: Some(openai_resp),
                model: "gpt-4o",
                mode: WebSearchMode::Function,
                expected: WebSearchRoute::Disabled,
            },
            Case {
                name: "explicit_disabled_mode",
                enabled: true,
                fallback: true,
                kind: Some(openai_resp),
                model: "gpt-4o",
                mode: WebSearchMode::Disabled,
                expected: WebSearchRoute::Disabled,
            },
            Case {
                name: "auto_unknown_capability_with_fallback",
                enabled: true,
                fallback: true,
                kind: Some(openai_resp),
                model: "custom-gateway-model",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::FocoFunction,
            },
            Case {
                name: "missing_provider_kind_auto_with_fallback",
                enabled: true,
                fallback: true,
                kind: None,
                model: "gpt-4o",
                mode: WebSearchMode::Auto,
                expected: WebSearchRoute::FocoFunction,
            },
        ];

        for case in cases {
            let route = resolve_web_search_route(WebSearchRouteInput {
                enabled: case.enabled,
                fallback_available: case.fallback,
                provider_kind: case.kind,
                upstream_model_id: case.model,
                mode: case.mode,
            });
            assert_eq!(
                route, case.expected,
                "case `{}`: expected {:?}, got {:?}",
                case.name, case.expected, route
            );
            // At most one search path per turn (route is a single enum variant).
            assert!(
                matches!(
                    route,
                    WebSearchRoute::Disabled
                        | WebSearchRoute::ProviderNative
                        | WebSearchRoute::FocoFunction
                ),
                "case `{}` must resolve to exactly one route variant",
                case.name
            );
        }
    }

    /// Wire-shape fixture: native vs function tools must produce distinguishable genai tools
    /// and must never both appear as a dual exposure for the same turn.
    #[test]
    fn web_search_wire_fixture_native_vs_function_and_at_most_one() {
        let native = NeutralToolDefinition {
            name: "web_search".to_string(),
            description: "native".to_string(),
            input_schema: serde_json::json!({}),
            strict: false,
            kind: NeutralToolKind::ProviderWebSearch,
        };
        let function = NeutralToolDefinition {
            name: "web_search".to_string(),
            description: "function".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "maxResults": {"type": "integer"},
                    "timeoutMs": {"type": "integer"}
                }
            }),
            strict: true,
            kind: NeutralToolKind::Function,
        };

        let native_tool = genai_tool(&native);
        let function_tool = genai_tool(&function);
        assert!(matches!(native_tool.name, genai::chat::ToolName::WebSearch));
        match &function_tool.name {
            genai::chat::ToolName::Custom(name) => assert_eq!(name, "web_search"),
            other => panic!("expected custom function tool, got {other:?}"),
        }
        assert!(function_tool.schema.is_some());
        assert!(native_tool.schema.is_none());

        // Simulate route-driven exposure: each route injects at most one web_search tool.
        for (route, expected_kind) in [
            (
                WebSearchRoute::ProviderNative,
                Some(NeutralToolKind::ProviderWebSearch),
            ),
            (
                WebSearchRoute::FocoFunction,
                Some(NeutralToolKind::Function),
            ),
            (WebSearchRoute::Disabled, None),
        ] {
            let tools: Vec<NeutralToolDefinition> = match route {
                WebSearchRoute::ProviderNative => vec![native.clone()],
                WebSearchRoute::FocoFunction => vec![function.clone()],
                WebSearchRoute::Disabled => Vec::new(),
            };
            let web_search_count = tools.iter().filter(|t| t.name == "web_search").count();
            assert!(
                web_search_count <= 1,
                "route {route:?} must expose at most one web_search"
            );
            match expected_kind {
                Some(kind) => {
                    assert_eq!(web_search_count, 1);
                    assert_eq!(tools[0].kind, kind);
                }
                None => assert_eq!(web_search_count, 0),
            }
        }

        // Dual exposure is forbidden (property-style guard used by assembly/remote).
        let dual = [native.clone(), function.clone()];
        let dual_count = dual.iter().filter(|t| t.name == "web_search").count();
        assert_eq!(dual_count, 2, "fixture dual list for negative assertion");
        assert!(
            dual_count > 1,
            "assembly must reject dual native+function exposure"
        );
    }

    #[test]
    fn supports_native_web_search_requires_protocol_and_known_model() {
        let responses = openai_responses_kind();
        let chat = parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind");
        let ws = parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind");
        let xai_chat = parse_provider_kind(XAI_KIND).expect("xai chat");
        let xai_resp = parse_provider_kind(XAI_RESPONSES_KIND).expect("xai responses");

        assert!(supports_native_web_search(responses, "gpt-4o"));
        assert!(supports_native_web_search(ws, "gpt-5.4"));
        assert!(!supports_native_web_search(chat, "gpt-4o"));
        assert!(!supports_native_web_search(
            responses,
            "custom-gateway-model"
        ));
        assert_eq!(
            native_web_search_support(responses, "custom-gateway-model"),
            NativeWebSearchSupport::Unknown
        );
        assert_eq!(
            native_web_search_support(chat, "gpt-4o"),
            NativeWebSearchSupport::Unsupported
        );
        assert!(provider_protocol_supports_native_web_search(xai_resp));
        assert!(!provider_protocol_supports_native_web_search(xai_chat));
        assert!(supports_native_web_search(xai_resp, "grok-3"));
        assert!(!supports_native_web_search(xai_chat, "grok-3"));
    }

    #[test]
    fn function_tool_adapter_preserves_delegate_target_constraints_and_strictness() {
        let input_schema = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "targetKind": { "type": "string", "enum": ["instance", "definition"] },
                "targetId": {
                    "type": "string",
                    "pattern": "^agent-(?:instance|definition)-[a-z0-9-]+$",
                    "maxLength": 128
                }
            },
            "required": ["targetKind", "targetId"]
        });
        let definition = NeutralToolDefinition {
            name: "agent_delegate_task".to_string(),
            description: "delegate".to_string(),
            input_schema: input_schema.clone(),
            strict: true,
            kind: NeutralToolKind::Function,
        };

        let tool = genai_tool(&definition);
        assert_eq!(tool.schema, Some(input_schema));
        assert_eq!(
            serde_json::to_value(tool).expect("function tool JSON")["strict"],
            true
        );
    }

    #[test]
    fn neutral_tool_kind_defaults_to_function_and_is_not_inferred_from_name() {
        let legacy = serde_json::json!({
            "name": "web_search",
            "description": "legacy",
            "inputSchema": {"type": "object"},
            "strict": false
        });
        let parsed: NeutralToolDefinition =
            serde_json::from_value(legacy).expect("legacy tool definition");
        assert_eq!(parsed.kind, NeutralToolKind::Function);
        assert_eq!(parsed.name, "web_search");

        let native = NeutralToolDefinition {
            name: "web_search".to_string(),
            description: "native".to_string(),
            input_schema: serde_json::json!({}),
            strict: false,
            kind: NeutralToolKind::ProviderWebSearch,
        };
        let function = NeutralToolDefinition {
            name: "web_search".to_string(),
            description: "function".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "maxResults": {"type": "integer"},
                    "timeoutMs": {"type": "integer"}
                }
            }),
            strict: true,
            kind: NeutralToolKind::Function,
        };

        let native_tool = genai_tool(&native);
        let function_tool = genai_tool(&function);
        assert_ne!(
            serde_json::to_string(&native_tool).expect("native tool json"),
            serde_json::to_string(&function_tool).expect("function tool json"),
            "fingerprint must distinguish native vs function web_search"
        );
        assert!(matches!(native_tool.name, genai::chat::ToolName::WebSearch));
        match &function_tool.name {
            genai::chat::ToolName::Custom(name) => assert_eq!(name, "web_search"),
            other => panic!("expected custom tool, got {other:?}"),
        }
        assert!(function_tool.schema.is_some());

        let mut native_request =
            neutral_request(vec![neutral_text_message(NeutralChatRole::User, "search")]);
        native_request.tools.push(native);
        let mut function_request = native_request.clone();
        function_request.tools[0] = function;

        let native_chat = genai_chat_request_for_adapter(&native_request, AdapterKind::OpenAIResp)
            .expect("native chat request");
        let function_chat =
            genai_chat_request_for_adapter(&function_request, AdapterKind::OpenAIResp)
                .expect("function chat request");
        let native_tools = native_chat.tools.expect("native tools");
        let function_tools = function_chat.tools.expect("function tools");
        assert!(matches!(
            native_tools[0].name,
            genai::chat::ToolName::WebSearch
        ));
        assert!(matches!(
            function_tools[0].name,
            genai::chat::ToolName::Custom(_)
        ));
        assert!(function_tools[0].schema.is_some());
        let schema = function_tools[0].schema.as_ref().expect("schema");
        assert!(
            schema
                .get("properties")
                .and_then(|p| p.get("query"))
                .is_some()
        );
        assert!(
            schema
                .get("properties")
                .and_then(|p| p.get("maxResults"))
                .is_some()
        );
        assert!(
            schema
                .get("properties")
                .and_then(|p| p.get("timeoutMs"))
                .is_some()
        );
    }

    #[test]
    fn normalize_stream_drops_provider_native_web_search_tool_calls() {
        use genai::chat::{MessageContent, StreamEnd, ToolCall as GenaiToolCall, ToolChunk};

        let native_chunk = ChatStreamEvent::ToolCallChunk(ToolChunk {
            tool_call: GenaiToolCall {
                call_id: "call-native".to_string(),
                fn_name: "WebSearch".to_string(),
                fn_arguments: serde_json::json!({}),
                thought_signatures: None,
            },
        });
        assert!(
            matches!(
                normalize_stream_event(native_chunk).expect("normalize"),
                None
            ),
            "native WebSearch chunks must be dropped"
        );

        let function_chunk = ChatStreamEvent::ToolCallChunk(ToolChunk {
            tool_call: GenaiToolCall {
                call_id: "call-fn".to_string(),
                fn_name: "web_search".to_string(),
                fn_arguments: serde_json::json!({"query": "rust"}),
                thought_signatures: None,
            },
        });
        match normalize_stream_event(function_chunk).expect("normalize") {
            Some(NeutralChatStreamEvent::ToolCall { tool_call }) => {
                assert_eq!(tool_call.name, "web_search");
                assert_eq!(tool_call.call_id, "call-fn");
            }
            other => panic!("function web_search must remain a ToolCall, got {other:?}"),
        }

        let end = StreamEnd {
            captured_content: Some(MessageContent::from_tool_calls(vec![
                GenaiToolCall {
                    call_id: "native".to_string(),
                    fn_name: "WebSearch".to_string(),
                    fn_arguments: serde_json::json!({}),
                    thought_signatures: None,
                },
                GenaiToolCall {
                    call_id: "fn".to_string(),
                    fn_name: "web_search".to_string(),
                    fn_arguments: serde_json::json!({"query": "ok"}),
                    thought_signatures: None,
                },
            ])),
            ..StreamEnd::default()
        };
        match normalize_stream_end(end).expect("end") {
            NeutralChatStreamEvent::Complete { tool_calls, .. } => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "web_search");
                assert_eq!(tool_calls[0].call_id, "fn");
            }
            other => panic!("expected Complete, got {other:?}"),
        }
    }

    #[test]
    fn xai_responses_kind_reuses_openai_resp_adapter_without_openai_product_features() {
        let xai_resp = parse_provider_kind(XAI_RESPONSES_KIND).expect("xai responses");
        let xai_chat = parse_provider_kind(XAI_KIND).expect("xai chat");
        let openai_resp = openai_responses_kind();

        assert_eq!(xai_resp.adapter_kind(), AdapterKind::OpenAIResp);
        assert_eq!(xai_chat.adapter_kind(), AdapterKind::Xai);
        assert!(!xai_resp.uses_websocket());
        assert!(is_openai_responses_provider_kind(openai_resp));
        assert!(!is_openai_responses_provider_kind(xai_resp));
        assert!(!uses_openai_resp_agent_headers(xai_resp));
        assert!(uses_openai_resp_agent_headers(openai_resp));
        assert!(!supports_fast_latency_mode(xai_resp, "grok-3", &[]).expect("fast check"));
        assert!(supports_fast_latency_mode(openai_resp, "gpt-5", &[]).expect("openai fast"));
    }

    #[test]
    fn fast_latency_runtime_options_serialize_as_camel_case() {
        let value = serde_json::to_value(ChatRequestRuntimeOptions {
            latency_mode: LatencyMode::Fast,
            developer_role_enabled: true,
            temperature: None,
        })
        .expect("serialize runtime options");

        assert_eq!(value, serde_json::json!({ "latencyMode": "fast" }));
    }

    #[test]
    fn fast_latency_mode_overrides_service_tier_request_override() {
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: vec![ProviderRequestOverride {
                target: REQUEST_OVERRIDE_TARGET_BODY.to_string(),
                name: "service_tier".to_string(),
                value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                value: Value::String("default".to_string()),
            }],
            model_redirects: Vec::new(),
        };
        let mut body = Map::new();
        insert_nested_body_override(
            &mut body,
            "service_tier",
            Value::String("default".to_string()),
        )
        .expect("apply configured override");
        let mut request = neutral_request(Vec::new());
        request.model_id = "gpt-5.4".to_string();

        apply_fast_latency_mode(
            &mut body,
            &config,
            &request,
            &ChatRequestRuntimeOptions {
                latency_mode: LatencyMode::Fast,
                developer_role_enabled: true,
                temperature: None,
            },
        )
        .expect("Fast should take precedence over service tier override");

        assert_eq!(body["service_tier"], "priority");
    }

    #[test]
    fn fast_latency_mode_rejects_non_responses_adapter() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind"),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut body = Map::new();
        let mut request = neutral_request(Vec::new());
        request.model_id = "gpt-5.4".to_string();

        let error = apply_fast_latency_mode(
            &mut body,
            &config,
            &request,
            &ChatRequestRuntimeOptions {
                latency_mode: LatencyMode::Fast,
                developer_role_enabled: true,
                temperature: None,
            },
        )
        .expect_err("non-Responses adapters must reject Fast");

        assert!(
            error
                .to_string()
                .contains("Fast latency mode is not supported")
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
        let websocket_kind =
            parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("responses websocket kind");
        assert_eq!(websocket_kind.adapter_kind(), AdapterKind::OpenAIResp);
        assert!(websocket_kind.uses_websocket());
        assert!(
            !parse_provider_kind(OPENAI_RESPONSES_KIND)
                .expect("responses kind")
                .uses_websocket()
        );
        assert!(
            !parse_provider_kind(OPENAI_CHAT_KIND)
                .expect("chat kind")
                .uses_websocket()
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
            parse_provider_kind(XAI_RESPONSES_KIND)
                .expect("xai responses kind")
                .adapter_kind(),
            AdapterKind::OpenAIResp
        );
        assert_eq!(
            parse_provider_kind(DEEPSEEK_KIND)
                .expect("deepseek kind")
                .adapter_kind(),
            AdapterKind::DeepSeek
        );
    }

    #[test]
    fn openai_responses_websocket_kind_is_catalogued_with_shared_adapter() {
        let kinds = supported_provider_kinds()
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&OPENAI_RESPONSES_KIND));
        assert!(kinds.contains(&OPENAI_RESPONSES_WEBSOCKET_KIND));
        assert!(kinds.contains(&OPENAI_CHAT_KIND));
        assert_eq!(
            parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND)
                .expect("ws kind")
                .default_base_url(),
            DEFAULT_OPENAI_BASE_URL
        );
    }

    #[test]
    fn derives_websocket_url_from_adapter_responses_http_url_not_base_root() {
        let http_url =
            openai_responses_http_url_from_base("https://gateway.example/v1/").expect("http url");
        assert_eq!(http_url, "https://gateway.example/v1/responses");
        assert_eq!(
            websocket_url_from_responses_http_url(&http_url).expect("ws url"),
            "wss://gateway.example/v1/responses"
        );
        assert_eq!(
            openai_responses_websocket_url_from_base("http://localhost:8080/v1").expect("local ws"),
            "ws://localhost:8080/v1/responses"
        );
        assert_eq!(
            openai_responses_websocket_url_from_base(
                "https://gateway.example:8443/custom/v1/?api-version=2024-01"
            )
            .expect("preserves port path query"),
            "wss://gateway.example:8443/custom/v1/responses?api-version=2024-01"
        );
    }

    #[test]
    fn rejects_non_http_responses_urls_for_websocket_derivation() {
        let relative = websocket_url_from_responses_http_url("/v1/responses")
            .expect_err("relative should fail");
        assert!(relative.to_string().contains("invalid"));
        let other_scheme = websocket_url_from_responses_http_url("ftp://example.com/v1/responses")
            .expect_err("ftp should fail");
        assert!(other_scheme.to_string().contains("http or https"));
        let already_ws = websocket_url_from_responses_http_url("wss://example.com/v1/responses")
            .expect_err("ws input should fail");
        assert!(already_ws.to_string().contains("http or https"));
    }

    #[tokio::test]
    async fn websocket_prepare_payload_matches_http_responses_minus_stream_fields() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some("https://gateway.example/v1/".to_string()),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };
        let request = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![
                NeutralChatMessage {
                    role: NeutralChatRole::System,
                    content: "sys".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::Developer,
                    content: "dev".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::User,
                    content: "hi".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
            ],
            tools: vec![],
            max_output_tokens: Some(128),
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
        };

        let client = config.genai_client().expect("client");
        let chat_request =
            genai_chat_request_for_adapter(&request, config.kind.adapter_kind()).expect("chat");
        let options = genai_chat_options(&config, &request).expect("options");
        let model = genai::ModelIden::new(config.kind.adapter_kind(), "gpt-4.1-mini");
        let prepared = client
            .prepare_chat_stream_request(model, chat_request, Some(&options))
            .await
            .expect("prepare");

        assert_eq!(prepared.url, "https://gateway.example/v1/responses");
        assert_eq!(prepared.payload["stream"], true);
        assert_eq!(prepared.payload["model"], "gpt-4.1-mini");
        assert_eq!(prepared.payload["instructions"], "sys");
        assert!(prepared.payload["input"].as_array().is_some());

        let ws_payload =
            genai::adapter::openai_resp_websocket_create_payload(prepared.payload.clone());
        assert_eq!(ws_payload["type"], "response.create");
        assert!(ws_payload.get("stream").is_none());
        assert!(ws_payload.get("background").is_none());

        let mut http_body = prepared.payload.as_object().cloned().expect("object");
        http_body.remove("stream");
        http_body.remove("background");
        let mut ws_body = ws_payload.as_object().cloned().expect("object");
        ws_body.remove("type");
        assert_eq!(http_body, ws_body);

        assert_eq!(
            websocket_url_from_responses_http_url(&prepared.url).expect("ws url"),
            "wss://gateway.example/v1/responses"
        );
    }

    #[tokio::test]
    async fn fast_latency_mode_maps_service_tier_to_http_and_websocket_bodies() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some("https://gateway.example/v1/".to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request =
            neutral_request(vec![neutral_text_message(NeutralChatRole::User, "Fast")]);
        request.model_id = "gpt-5.4".to_string();
        let client = config.genai_client().expect("client");
        let chat_request =
            genai_chat_request_for_adapter(&request, config.kind.adapter_kind()).expect("chat");
        let options = genai_chat_options_with_runtime_options(
            &config,
            &request,
            &ChatRequestRuntimeOptions {
                latency_mode: LatencyMode::Fast,
                developer_role_enabled: true,
                temperature: None,
            },
        )
        .expect("Fast options");
        let model = genai::ModelIden::new(config.kind.adapter_kind(), "gpt-5.4");
        let prepared = client
            .prepare_chat_stream_request(model, chat_request, Some(&options))
            .await
            .expect("prepare Responses request");
        let ws_payload =
            genai::adapter::openai_resp_websocket_create_payload(prepared.payload.clone());

        assert_eq!(prepared.payload["service_tier"], "priority");
        assert_eq!(ws_payload["service_tier"], "priority");
    }

    #[tokio::test]
    async fn fast_latency_mode_is_present_in_captured_responses_wire_dump() {
        let response = concat!(
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-fast-fixture\",\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response).await;
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "Fast wire",
        )]);
        request.model_id = "gpt-5.4".to_string();
        let mut stream = stream_chat_with_capture_runtime_options(
            &config,
            request,
            ChatRequestRuntimeOptions {
                latency_mode: LatencyMode::Fast,
                developer_role_enabled: true,
                temperature: None,
            },
            true,
        )
        .await
        .expect("open Fast Responses fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("captured wire request")
            .as_http()
            .expect("HTTP wire request");
        let body =
            serde_json::from_str::<Value>(dump.body.as_deref().expect("captured request body"))
                .expect("request JSON");
        assert_eq!(body["service_tier"], "priority");

        while stream.next_event().await.is_some() {}
        let raw_request = String::from_utf8(
            fixture
                .await
                .expect("fixture task")
                .into_iter()
                .next()
                .expect("fixture request"),
        )
        .expect("raw HTTP request UTF-8");
        assert!(raw_request.contains("\"service_tier\":\"priority\""));
    }

    #[tokio::test]
    async fn disabled_developer_role_is_absent_from_captured_openai_responses_wire_request() {
        let response = concat!(
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-role-fixture\",\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response).await;
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Base system."),
            neutral_text_message(NeutralChatRole::Developer, "Developer instructions."),
            neutral_text_message(NeutralChatRole::User, "Continue."),
        ]);
        request.model_id = "gpt-4.1-mini".to_string();

        let mut stream = stream_chat_with_capture_runtime_options(
            &config,
            request,
            ChatRequestRuntimeOptions {
                developer_role_enabled: false,
                ..Default::default()
            },
            true,
        )
        .await
        .expect("open Responses fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("captured wire request")
            .as_http()
            .expect("HTTP wire request");
        let body =
            serde_json::from_str::<Value>(dump.body.as_deref().expect("captured request body"))
                .expect("request JSON");

        assert_eq!(
            body["instructions"],
            "Base system.\n\nDeveloper instructions."
        );
        assert!(
            !dump
                .body
                .as_deref()
                .expect("captured request body")
                .contains("\"developer\"")
        );

        while stream.next_event().await.is_some() {}
        let raw_request = String::from_utf8(
            fixture
                .await
                .expect("fixture task")
                .into_iter()
                .next()
                .expect("fixture request"),
        )
        .expect("UTF-8 request");
        assert!(!raw_request.contains("\"developer\""));
    }

    #[tokio::test]
    async fn developer_role_policy_shapes_openai_chat_wire_request() {
        let response = concat!(
            "data: {\"id\":\"resp-role-fixture\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"}}]}\\n\\n",
            "data: {\"id\":\"resp-role-fixture\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1}}\\n\\n",
            "data: [DONE]\\n\\n"
        );
        let (disabled_fixture_root, disabled_fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response).await;
        let disabled_config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind"),
            base_url: Some(format!("{disabled_fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Base system."),
            neutral_text_message(NeutralChatRole::Developer, "Developer instructions."),
            neutral_text_message(NeutralChatRole::User, "Continue."),
        ]);

        let mut disabled_stream = stream_chat_with_capture_runtime_options(
            &disabled_config,
            request.clone(),
            ChatRequestRuntimeOptions {
                developer_role_enabled: false,
                ..Default::default()
            },
            true,
        )
        .await
        .expect("open disabled chat fixture stream");
        let disabled_dump = disabled_stream
            .wire_request_dump()
            .expect("disabled wire request")
            .as_http()
            .expect("disabled HTTP wire request");
        let disabled_body: Value = serde_json::from_str(
            disabled_dump
                .body
                .as_deref()
                .expect("disabled request body"),
        )
        .expect("disabled request JSON");
        assert_eq!(
            disabled_body["messages"][0],
            serde_json::json!({
                "role": "system",
                "content": "Base system.\n\nDeveloper instructions."
            })
        );
        assert!(!disabled_body.to_string().contains("\"developer\""));
        while disabled_stream.next_event().await.is_some() {}
        let disabled_raw = String::from_utf8(
            disabled_fixture
                .await
                .expect("disabled fixture task")
                .into_iter()
                .next()
                .expect("disabled fixture request"),
        )
        .expect("disabled raw request UTF-8");
        assert!(!disabled_raw.contains("\"developer\""));

        let (enabled_fixture_root, enabled_fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response).await;
        let enabled_config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind"),
            base_url: Some(format!("{enabled_fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut enabled_stream = stream_chat_with_capture_runtime_options(
            &enabled_config,
            request,
            ChatRequestRuntimeOptions::default(),
            true,
        )
        .await
        .expect("open enabled chat fixture stream");
        let enabled_dump = enabled_stream
            .wire_request_dump()
            .expect("enabled wire request")
            .as_http()
            .expect("enabled HTTP wire request");
        let enabled_body: Value =
            serde_json::from_str(enabled_dump.body.as_deref().expect("enabled request body"))
                .expect("enabled request JSON");
        assert_eq!(enabled_body["messages"][1]["role"], "developer");
        while enabled_stream.next_event().await.is_some() {}
        let enabled_raw = String::from_utf8(
            enabled_fixture
                .await
                .expect("enabled fixture task")
                .into_iter()
                .next()
                .expect("enabled fixture request"),
        )
        .expect("enabled raw request UTF-8");
        assert!(enabled_raw.contains("\"role\":\"developer\""));
    }

    #[tokio::test]
    async fn disabled_developer_role_is_absent_from_openai_responses_websocket_payload() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some("https://gateway.example/v1/".to_string()),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Base system."),
            neutral_text_message(NeutralChatRole::Developer, "Developer instructions."),
            neutral_text_message(NeutralChatRole::User, "Continue."),
        ]);
        let normalized = normalize_request_developer_role(
            request,
            &ChatRequestRuntimeOptions {
                developer_role_enabled: false,
                ..Default::default()
            },
        );
        let chat_request =
            genai_chat_request_for_adapter(&normalized, config.kind.adapter_kind()).expect("chat");
        let options = genai_chat_options_with_runtime_options(
            &config,
            &normalized,
            &ChatRequestRuntimeOptions {
                developer_role_enabled: false,
                ..Default::default()
            },
        )
        .expect("options");
        let client = config.genai_client().expect("client");
        let prepared = client
            .prepare_chat_stream_request(
                genai::ModelIden::new(config.kind.adapter_kind(), "gpt-4.1-mini"),
                chat_request,
                Some(&options),
            )
            .await
            .expect("prepare Responses request");
        let payload = genai::adapter::openai_resp_websocket_create_payload(prepared.payload);

        assert_eq!(
            payload["instructions"],
            "Base system.\n\nDeveloper instructions."
        );
        assert!(!payload.to_string().contains("\"developer\""));
    }

    #[tokio::test]
    async fn default_runtime_options_do_not_upgrade_responses_requests_to_fast() {
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: Some("https://gateway.example/v1/".to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "Internal request",
        )]);
        request.model_id = "gpt-5.4".to_string();
        let client = config.genai_client().expect("client");
        let chat_request =
            genai_chat_request_for_adapter(&request, config.kind.adapter_kind()).expect("chat");
        let options = genai_chat_options(&config, &request).expect("default options");
        let model = genai::ModelIden::new(config.kind.adapter_kind(), "gpt-5.4");
        let prepared = client
            .prepare_chat_stream_request(model, chat_request, Some(&options))
            .await
            .expect("prepare default Responses request");

        assert!(prepared.payload.get("service_tier").is_none());
    }

    #[tokio::test]
    async fn websocket_stream_applies_disabled_developer_role_before_the_wire_request() {
        use futures_util::{SinkExt, StreamExt};
        use tokio::net::TcpListener;
        use tokio_tungstenite::{accept_async, tungstenite::Message};

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(stream).await.expect("ws accept");
            let first = ws.next().await.expect("msg").expect("ok");
            let text = match first {
                Message::Text(text) => text.to_string(),
                other => panic!("expected text, got {other:?}"),
            };
            let create: Value = serde_json::from_str(&text).expect("json");
            assert_eq!(create["type"], "response.create");
            assert!(create.get("stream").is_none());
            assert_eq!(
                create["instructions"],
                "Base system.\n\nDeveloper instructions."
            );
            assert!(!text.contains("\"developer\""));

            for frame in [
                r#"{"type":"response.output_text.delta","delta":"Hello"}"#,
                r#"{"type":"response.completed","response":{"id":"resp_ws","status":"completed","model":"gpt-4.1-mini","output":[],"usage":null}}"#,
            ] {
                ws.send(Message::Text(frame.into())).await.expect("send");
            }
            let _ = ws.close(None).await;
        });

        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };
        let request = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![
                neutral_text_message(NeutralChatRole::System, "Base system."),
                neutral_text_message(NeutralChatRole::Developer, "Developer instructions."),
                neutral_text_message(NeutralChatRole::User, "hi"),
            ],
            tools: vec![],
            max_output_tokens: None,
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
        };

        let mut stream = stream_chat_with_capture_runtime_options(
            &config,
            request,
            ChatRequestRuntimeOptions {
                developer_role_enabled: false,
                ..Default::default()
            },
            true,
        )
        .await
        .expect("stream");
        let wire = stream
            .wire_request_dump()
            .expect("wire request dump")
            .as_websocket()
            .expect("websocket request dump");
        assert_eq!(wire.format, PROVIDER_WEBSOCKET_REQUEST_DUMP_FORMAT);
        assert_eq!(wire.version, PROVIDER_WEBSOCKET_REQUEST_DUMP_VERSION);
        assert!(wire.url.starts_with("ws://"));
        assert!(wire.url.ends_with("/v1/responses"));
        assert!(!wire.connection_reused);
        assert!(wire.frame_sent, "successful stream must record frame_sent");
        assert!(
            wire.create_frame
                .as_deref()
                .is_some_and(|frame| frame.contains("\"type\":\"response.create\""))
        );
        assert!(
            wire.create_frame
                .as_deref()
                .is_some_and(|frame| !frame.contains("\"developer\""))
        );
        assert_eq!(
            wire.headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
                .and_then(|(_, values)| values.first())
                .map(String::as_str),
            Some(MASKED_AUTHORIZATION_VALUE)
        );
        assert_eq!(
            wire.handshake.as_ref().map(|handshake| handshake.status),
            Some(101)
        );
        assert_eq!(stream.http_status(), Some(101));

        let mut events = Vec::new();
        while let Some(event) = stream.next_event().await {
            events.push(event.expect("event"));
        }

        assert!(matches!(
            events.first(),
            Some(NeutralChatStreamEvent::Start)
        ));
        assert!(events.iter().any(
            |event| matches!(event, NeutralChatStreamEvent::TextDelta { delta } if delta == "Hello")
        ));
        assert!(events.iter().any(|event| matches!(
            event,
            NeutralChatStreamEvent::Complete {
                text,
                response_id: Some(id),
                ..
            } if text == "Hello" && id == "resp_ws"
        )));
        let final_dump = stream.final_response_dump().expect("final response");
        let final_json = serde_json::to_value(final_dump).expect("final json");
        assert_eq!(final_json["format"], "provider_final_response_v1");
        assert_eq!(final_json["http"]["status"], 101);
        server.await.expect("server");
    }

    #[tokio::test]
    async fn websocket_failed_provider_event_preserves_decoder_diagnostic_and_handshake_head() {
        use futures_util::{SinkExt, StreamExt};
        use tokio::net::TcpListener;
        use tokio_tungstenite::{accept_async, tungstenite::Message};

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(stream).await.expect("ws accept");
            let create = ws
                .next()
                .await
                .expect("create frame")
                .expect("create frame ok");
            assert!(matches!(create, Message::Text(_)));
            ws.send(Message::Text(
                r#"{"type":"error","api_key":"websocket-frame-secret","error":{"code":"rate_limit","type":"rate_limit_error","message":"retry via websocket","param":"model"}}"#.into(),
            ))
            .await
            .expect("send failed event");
            let _ = ws.close(None).await;
        });

        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };
        let request = neutral_request(vec![neutral_text_message(NeutralChatRole::User, "hi")]);
        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open websocket stream");

        let stream_error = loop {
            match stream.next_event().await {
                Some(Err(error)) => break error,
                Some(Ok(_)) => continue,
                None => panic!("expected WebSocket stream error"),
            }
        };
        // HTTP SSE and WebSocket share the same ProviderStream classification path.
        assert_eq!(
            stream_error.stream_failure_kind(),
            Some(ProviderStreamFailureKind::RateLimit)
        );
        assert!(
            stream_error.user_message().contains("retry via websocket"),
            "WebSocket provider errors must keep the provider message: {}",
            stream_error.user_message()
        );
        assert!(
            !stream_error
                .user_message()
                .contains("Failed to parse stream data"),
            "legal WebSocket provider errors must not look like parse failures"
        );
        let detail = stream_error
            .stream_detail()
            .expect("structured WebSocket stream detail");
        assert_eq!(detail.code.as_deref(), Some("rate_limit"));
        assert_eq!(detail.error_type.as_deref(), Some("rate_limit_error"));
        assert_eq!(detail.param.as_deref(), Some("model"));

        let diagnostic = stream.stream_diagnostic().expect("failed frame diagnostic");
        assert_eq!(
            diagnostic.kind,
            genai::OpenAIRespStreamDiagnosticKind::ProviderErrorEvent
        );
        assert_eq!(
            diagnostic.transport,
            genai::OpenAIRespStreamTransport::WebSocket
        );
        assert_eq!(diagnostic.event_type.as_deref(), Some("error"));
        assert_eq!(
            diagnostic.provider_error.code.as_deref(),
            Some("rate_limit")
        );
        assert_eq!(
            diagnostic.provider_error.message.as_deref(),
            Some("retry via websocket")
        );

        let final_dump = stream
            .failed_final_response_dump(stream_error.to_string(), stream.http_status(), false)
            .expect("failed final response dump");
        let final_json = serde_json::to_value(final_dump).expect("final response JSON");
        assert_eq!(final_json["http"]["status"], 101);
        assert_eq!(final_json["statusCode"], 101);
        assert_eq!(
            final_json["streamDiagnostic"]["kind"],
            "provider_error_event"
        );
        assert_eq!(
            final_json["streamDiagnostic"]["provider_error"]["code"],
            "rate_limit"
        );
        assert_eq!(
            final_json["streamDiagnostic"]["payload"]["value"]["api_key"],
            REDACTED_CREDENTIAL_VALUE
        );
        assert!(
            !final_json.to_string().contains("websocket-frame-secret"),
            "WebSocket diagnostic must retain the decoder's redacted payload"
        );
        server.await.expect("server");
    }

    #[tokio::test]
    async fn websocket_session_reuses_connection_and_previous_response_id() {
        use futures_util::{SinkExt, StreamExt};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::net::TcpListener;
        use tokio_tungstenite::{accept_async, tungstenite::Message};

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let accepts = Arc::new(AtomicUsize::new(0));
        let accepts_server = Arc::clone(&accepts);
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            accepts_server.fetch_add(1, Ordering::SeqCst);
            let mut ws = accept_async(stream).await.expect("ws accept");

            // Turn 1: full input, store, no previous_response_id
            let first = ws.next().await.expect("msg").expect("ok");
            let text = match first {
                Message::Text(text) => text.to_string(),
                other => panic!("expected text, got {other:?}"),
            };
            let create: Value = serde_json::from_str(&text).expect("json");
            assert_eq!(create["type"], "response.create");
            assert_eq!(create["store"], true);
            assert!(create.get("previous_response_id").is_none());
            ws.send(Message::Text(
                r#"{"type":"response.output_text.delta","delta":"A"}"#.into(),
            ))
            .await
            .expect("send");
            ws.send(Message::Text(
                r#"{"type":"response.completed","response":{"id":"resp_1","status":"completed","model":"gpt-4.1-mini","output":[],"usage":null}}"#.into(),
            ))
            .await
            .expect("send");

            // Turn 2: same socket, previous_response_id + delta only
            let second = ws.next().await.expect("msg2").expect("ok");
            let text2 = match second {
                Message::Text(text) => text.to_string(),
                other => panic!("expected text, got {other:?}"),
            };
            let create2: Value = serde_json::from_str(&text2).expect("json");
            assert_eq!(create2["type"], "response.create");
            assert_eq!(create2["previous_response_id"], "resp_1");
            assert_eq!(create2["store"], true);
            let input = create2["input"].as_array().expect("input array");
            // Only the new assistant+tool/user tail should be sent; first user already committed.
            assert!(
                input.len() < 3,
                "continuation should send fewer items than full history, got {input:?}"
            );
            ws.send(Message::Text(
                r#"{"type":"response.output_text.delta","delta":"B"}"#.into(),
            ))
            .await
            .expect("send");
            ws.send(Message::Text(
                r#"{"type":"response.completed","response":{"id":"resp_2","status":"completed","model":"gpt-4.1-mini","output":[],"usage":null}}"#.into(),
            ))
            .await
            .expect("send");
            let _ = ws.close(None).await;
        });

        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(8));
        let session = ProviderWsSessionContext {
            registry: Arc::clone(&registry),
            key: OpenAiRespWsSessionKey::new("ws", "assistant-1", "prov", "gpt-4.1-mini"),
            enable_continuation: true,
        };
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };

        let request1 = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "hi".to_string(),
                attachments: vec![],
                reasoning: None,
                tool_calls: vec![],
                tool_call_id: None,
                tool_name: None,
            }],
            tools: vec![],
            max_output_tokens: None,
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
        };
        let mut stream1 =
            stream_chat_with_capture_observer(&config, request1, true, None, Some(session.clone()))
                .await
                .expect("stream1");
        let wire1 = stream1
            .wire_request_dump()
            .expect("turn1 wire dump")
            .as_websocket()
            .expect("websocket dump");
        assert!(!wire1.connection_reused);
        assert_eq!(
            wire1.handshake.as_ref().map(|handshake| handshake.status),
            Some(101)
        );
        while let Some(event) = stream1.next_event().await {
            let _ = event.expect("event");
        }
        drop(stream1);

        let request2 = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![
                NeutralChatMessage {
                    role: NeutralChatRole::User,
                    content: "hi".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::Assistant,
                    content: "A".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::User,
                    content: "again".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
            ],
            tools: vec![],
            max_output_tokens: None,
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
        };
        let mut stream2 =
            stream_chat_with_capture_observer(&config, request2, true, None, Some(session))
                .await
                .expect("stream2");
        let wire2 = stream2
            .wire_request_dump()
            .expect("turn2 wire dump")
            .as_websocket()
            .expect("websocket dump");
        assert_eq!(wire2.format, PROVIDER_WEBSOCKET_REQUEST_DUMP_FORMAT);
        assert!(wire2.connection_reused);
        assert!(
            wire2.handshake.is_none(),
            "reused turn must not invent handshake metadata"
        );
        let mut saw_b = false;
        let mut saw_resp2 = false;
        while let Some(event) = stream2.next_event().await {
            match event.expect("event") {
                NeutralChatStreamEvent::TextDelta { delta } if delta == "B" => saw_b = true,
                NeutralChatStreamEvent::Complete {
                    response_id: Some(id),
                    ..
                } if id == "resp_2" => saw_resp2 = true,
                _ => {}
            }
        }
        assert!(saw_b && saw_resp2);
        assert_eq!(
            accepts.load(Ordering::SeqCst),
            1,
            "must reuse one WS accept"
        );
        // Reused turn must not invent an HTTP 101 status for wire audit.
        assert_eq!(
            stream2.http_status(),
            None,
            "reused socket turn has no observed HTTP response head this turn"
        );
        let final2 = stream2.final_response_dump().expect("final2");
        let final2_json = serde_json::to_value(final2).expect("final2 json");
        assert_eq!(final2_json["format"], "provider_final_response_v1");
        assert!(
            final2_json.get("http").is_none() || final2_json["http"].is_null(),
            "must not fabricate HTTP head on reused connection"
        );
        server.await.expect("server");
    }

    #[tokio::test]
    async fn websocket_observer_notified_only_after_create_frame_sent_with_frame_sent_true() {
        use futures_util::{SinkExt, StreamExt};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::net::TcpListener;
        use tokio_tungstenite::{
            accept_hdr_async,
            tungstenite::{
                Message,
                handshake::server::{Request, Response},
            },
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let saw_bearer = Arc::new(AtomicUsize::new(0));
        let saw_bearer_server = Arc::clone(&saw_bearer);
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_hdr_async(stream, |req: &Request, response: Response| {
                let auth = req
                    .headers()
                    .get("Authorization")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("");
                assert_eq!(
                    auth, "Bearer test-key",
                    "upgrade must receive real API key once"
                );
                saw_bearer_server.fetch_add(1, Ordering::SeqCst);
                Ok(response)
            })
            .await
            .expect("ws accept");
            let first = ws.next().await.expect("msg").expect("ok");
            match first {
                Message::Text(text) => {
                    let create: Value = serde_json::from_str(&text).expect("json");
                    assert_eq!(create["type"], "response.create");
                }
                other => panic!("expected text, got {other:?}"),
            }
            ws.send(Message::Text(
                r#"{"type":"response.output_text.delta","delta":"ok"}"#.into(),
            ))
            .await
            .expect("send");
            ws.send(Message::Text(
                r#"{"type":"response.completed","response":{"id":"resp_auth","status":"completed","model":"gpt-4.1-mini","output":[],"usage":null}}"#.into(),
            ))
            .await
            .expect("send");
            let _ = ws.close(None).await;
        });

        let observer_calls = Arc::new(Mutex::new(Vec::<ProviderAuditRequestDump>::new()));
        let observer_slot = Arc::clone(&observer_calls);
        let observer: ProviderRequestDumpObserver = Arc::new(move |dump| {
            observer_slot
                .lock()
                .expect("observer lock")
                .push(dump.clone());
        });

        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };
        let request = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "hi".to_string(),
                attachments: vec![],
                reasoning: None,
                tool_calls: vec![],
                tool_call_id: None,
                tool_name: None,
            }],
            tools: vec![],
            max_output_tokens: None,
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
        };

        let mut stream =
            stream_chat_with_capture_observer(&config, request, true, Some(observer), None)
                .await
                .expect("stream");
        let dumps = observer_calls.lock().expect("lock").clone();
        assert_eq!(dumps.len(), 1, "observer once after create frame is sent");
        let wire = dumps[0].as_websocket().expect("ws dump");
        assert!(wire.frame_sent, "successful send must mark frame_sent");
        assert_eq!(
            wire.headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
                .and_then(|(_, values)| values.first())
                .map(String::as_str),
            Some(MASKED_AUTHORIZATION_VALUE)
        );
        assert!(
            !wire
                .headers
                .values()
                .flatten()
                .any(|value| value.contains("test-key"))
        );
        while let Some(event) = stream.next_event().await {
            let _ = event.expect("event");
        }
        assert_eq!(
            saw_bearer.load(Ordering::SeqCst),
            1,
            "provider upgrade must see Authorization exactly once"
        );
        server.await.expect("server");
    }

    #[tokio::test]
    async fn websocket_upgrade_receives_agent_correlation_identity_and_beta_headers() {
        use futures_util::{SinkExt, StreamExt};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::net::TcpListener;
        use tokio_tungstenite::{
            accept_hdr_async,
            tungstenite::{
                Message,
                handshake::server::{Request, Response},
            },
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let saw_agent_headers = Arc::new(AtomicUsize::new(0));
        let saw_agent_headers_server = Arc::clone(&saw_agent_headers);
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_hdr_async(stream, |req: &Request, response: Response| {
                let header = |name: &str| {
                    req.headers()
                        .get(name)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("")
                        .to_string()
                };
                assert_eq!(header("Authorization"), "Bearer test-key");
                assert_eq!(header(AGENT_HEADER_ORIGINATOR), FOCO_AGENT_ORIGINATOR);
                assert!(
                    header(AGENT_HEADER_USER_AGENT).starts_with("foco/"),
                    "User-Agent must be Foco identity, got {}",
                    header(AGENT_HEADER_USER_AGENT)
                );
                assert_eq!(header(AGENT_HEADER_SESSION_ID), "plan-ws-upgrade");
                assert_eq!(header(AGENT_HEADER_THREAD_ID), "chat-impl-ws");
                assert_eq!(header(AGENT_HEADER_CLIENT_REQUEST_ID), "llm-req-ws");
                assert_eq!(header(AGENT_HEADER_FOCO_RUN_ID), "run-ws");
                assert_eq!(header(AGENT_HEADER_FOCO_WORKSPACE_ID), "ws-ws");
                assert_eq!(
                    header(OPENAI_RESP_WS_BETA_HEADER),
                    OPENAI_RESP_WS_BETA_VALUE
                );
                assert!(
                    req.headers().get("version").is_none(),
                    "upgrade must not send bare version header by default"
                );
                saw_agent_headers_server.fetch_add(1, Ordering::SeqCst);
                Ok(response)
            })
            .await
            .expect("ws accept");
            let first = ws.next().await.expect("msg").expect("ok");
            match first {
                Message::Text(text) => {
                    let create: Value = serde_json::from_str(&text).expect("json");
                    assert_eq!(create["type"], "response.create");
                }
                other => panic!("expected text, got {other:?}"),
            }
            ws.send(Message::Text(
                r#"{"type":"response.output_text.delta","delta":"ok"}"#.into(),
            ))
            .await
            .expect("send");
            ws.send(Message::Text(
                r#"{"type":"response.completed","response":{"id":"resp_agent_hdr","status":"completed","model":"gpt-4.1-mini","output":[],"usage":null}}"#.into(),
            ))
            .await
            .expect("send");
            let _ = ws.close(None).await;
        });

        let observer_calls = Arc::new(Mutex::new(Vec::<ProviderAuditRequestDump>::new()));
        let observer_slot = Arc::clone(&observer_calls);
        let observer: ProviderRequestDumpObserver = Arc::new(move |dump| {
            observer_slot
                .lock()
                .expect("observer lock")
                .push(dump.clone());
        });

        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };
        let request = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "hi".to_string(),
                attachments: vec![],
                reasoning: None,
                tool_calls: vec![],
                tool_call_id: None,
                tool_name: None,
            }],
            tools: vec![],
            max_output_tokens: None,
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: Some(
                AgentRequestCorrelation::new("plan-ws-upgrade", "chat-impl-ws", "llm-req-ws")
                    .with_run_id("run-ws")
                    .with_workspace_id("ws-ws"),
            ),
            tool_choice: NeutralToolChoice::Auto,
        };

        let mut stream =
            stream_chat_with_capture_observer(&config, request, true, Some(observer), None)
                .await
                .expect("stream");
        let dumps = observer_calls.lock().expect("lock").clone();
        assert_eq!(dumps.len(), 1);
        let wire = dumps[0].as_websocket().expect("ws dump");
        assert_eq!(
            dump_header_first(&wire.headers, AGENT_HEADER_SESSION_ID).as_deref(),
            Some("plan-ws-upgrade")
        );
        assert_eq!(
            dump_header_first(&wire.headers, AGENT_HEADER_THREAD_ID).as_deref(),
            Some("chat-impl-ws")
        );
        assert_eq!(
            dump_header_first(&wire.headers, AGENT_HEADER_CLIENT_REQUEST_ID).as_deref(),
            Some("llm-req-ws")
        );
        assert_eq!(
            dump_header_first(&wire.headers, OPENAI_RESP_WS_BETA_HEADER).as_deref(),
            Some(OPENAI_RESP_WS_BETA_VALUE)
        );
        assert_eq!(
            dump_header_first(&wire.headers, AGENT_HEADER_ORIGINATOR).as_deref(),
            Some(FOCO_AGENT_ORIGINATOR)
        );
        assert!(
            dump_header_first(&wire.headers, "version").is_none(),
            "WS wire dump must not include default version header"
        );
        while let Some(event) = stream.next_event().await {
            let _ = event.expect("event");
        }
        assert_eq!(
            saw_agent_headers.load(Ordering::SeqCst),
            1,
            "upgrade must receive agent headers exactly once"
        );
        server.await.expect("server");
    }

    #[tokio::test]
    async fn websocket_one_shot_disables_previous_response_continuation() {
        use futures_util::{SinkExt, StreamExt};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::net::TcpListener;
        use tokio_tungstenite::{accept_async, tungstenite::Message};

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let accepts = Arc::new(AtomicUsize::new(0));
        let accepts_server = Arc::clone(&accepts);
        let server = tokio::spawn(async move {
            // One-shot turns still reuse the live socket when affinity matches, but must
            // never send previous_response_id / store continuation semantics.
            let (stream, _) = listener.accept().await.expect("accept");
            accepts_server.fetch_add(1, Ordering::SeqCst);
            let mut ws = accept_async(stream).await.expect("ws accept");

            let first = ws.next().await.expect("msg").expect("ok");
            let text = match first {
                Message::Text(text) => text.to_string(),
                other => panic!("expected text, got {other:?}"),
            };
            let create: Value = serde_json::from_str(&text).expect("json");
            assert_eq!(create["type"], "response.create");
            assert!(
                create.get("previous_response_id").is_none(),
                "one-shot turn1 must not continue"
            );
            ws.send(Message::Text(
                r#"{"type":"response.output_text.delta","delta":"A"}"#.into(),
            ))
            .await
            .expect("send");
            ws.send(Message::Text(
                r#"{"type":"response.completed","response":{"id":"resp_one_shot_1","status":"completed","model":"gpt-4.1-mini","output":[],"usage":null}}"#.into(),
            ))
            .await
            .expect("send");

            let second = ws.next().await.expect("msg2").expect("ok");
            let text2 = match second {
                Message::Text(text) => text.to_string(),
                other => panic!("expected text, got {other:?}"),
            };
            let create2: Value = serde_json::from_str(&text2).expect("json");
            assert_eq!(create2["type"], "response.create");
            assert!(
                create2.get("previous_response_id").is_none(),
                "one-shot turn2 must not send previous_response_id, got {create2}"
            );
            // Full history is re-sent when continuation is disabled.
            let input = create2["input"].as_array().expect("input array");
            assert!(
                input.len() >= 2,
                "one-shot should resend full context, got {input:?}"
            );
            ws.send(Message::Text(
                r#"{"type":"response.output_text.delta","delta":"B"}"#.into(),
            ))
            .await
            .expect("send");
            ws.send(Message::Text(
                r#"{"type":"response.completed","response":{"id":"resp_one_shot_2","status":"completed","model":"gpt-4.1-mini","output":[],"usage":null}}"#.into(),
            ))
            .await
            .expect("send");
            let _ = ws.close(None).await;
        });

        let registry = Arc::new(OpenAiRespWsSessionRegistry::new(8));
        let session = ProviderWsSessionContext {
            registry: Arc::clone(&registry),
            key: OpenAiRespWsSessionKey::new("ws", "assistant-one-shot", "prov", "gpt-4.1-mini"),
            enable_continuation: false,
        };
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };

        let request1 = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "hi".to_string(),
                attachments: vec![],
                reasoning: None,
                tool_calls: vec![],
                tool_call_id: None,
                tool_name: None,
            }],
            tools: vec![],
            max_output_tokens: None,
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: Some(AgentRequestCorrelation::new(
                "chat-one-shot",
                "chat-one-shot",
                "req-1",
            )),
            tool_choice: NeutralToolChoice::Auto,
        };
        let mut stream1 =
            stream_chat_with_capture_observer(&config, request1, true, None, Some(session.clone()))
                .await
                .expect("stream1");
        while let Some(event) = stream1.next_event().await {
            let _ = event.expect("event");
        }
        drop(stream1);

        let request2 = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![
                NeutralChatMessage {
                    role: NeutralChatRole::User,
                    content: "hi".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::Assistant,
                    content: "A".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::User,
                    content: "again".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
            ],
            tools: vec![],
            max_output_tokens: None,
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: Some(AgentRequestCorrelation::new(
                "chat-one-shot",
                "chat-one-shot",
                "req-2",
            )),
            tool_choice: NeutralToolChoice::Auto,
        };
        let mut stream2 =
            stream_chat_with_capture_observer(&config, request2, true, None, Some(session))
                .await
                .expect("stream2");
        while let Some(event) = stream2.next_event().await {
            let _ = event.expect("event");
        }
        assert_eq!(
            accepts.load(Ordering::SeqCst),
            1,
            "one-shot may reuse the socket but must not use previous_response_id"
        );
        server.await.expect("server");
    }

    #[tokio::test]
    async fn websocket_handshake_http_rejection_preserves_status_code() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buf = [0u8; 1024];
            let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
            let response =
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response).await.expect("write 401");
        });

        let observer_calls = Arc::new(Mutex::new(Vec::<ProviderAuditRequestDump>::new()));
        let observer_slot = Arc::clone(&observer_calls);
        let observer: ProviderRequestDumpObserver = Arc::new(move |dump| {
            observer_slot
                .lock()
                .expect("observer lock")
                .push(dump.clone());
        });

        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };
        let request = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![NeutralChatMessage {
                role: NeutralChatRole::User,
                content: "hi".to_string(),
                attachments: vec![],
                reasoning: None,
                tool_calls: vec![],
                tool_call_id: None,
                tool_name: None,
            }],
            tools: vec![],
            max_output_tokens: None,
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
        };

        let failure =
            match stream_chat_with_capture_observer(&config, request, true, Some(observer), None)
                .await
            {
                Ok(_) => panic!("handshake 401 must fail"),
                Err(error) => error,
            };
        match &failure.error {
            ProviderConfigError::Connection { status_code, .. } => {
                assert_eq!(*status_code, Some(401), "must keep real upgrade status");
            }
            other => panic!("expected Connection error, got {other:?}"),
        }
        let dumps = observer_calls.lock().expect("lock").clone();
        assert_eq!(dumps.len(), 1, "failure still notifies for diagnostics");
        let wire = dumps[0].as_websocket().expect("ws dump");
        assert!(
            !wire.frame_sent,
            "connect failure must not claim create frame was sent"
        );
        let dump_on_failure = failure.request_dump.expect("request dump");
        assert!(!dump_on_failure.as_websocket().expect("ws").frame_sent);
        server.await.expect("server");
    }

    #[test]
    fn websocket_protocol_does_not_silently_fallback_to_http_kind() {
        let ws = parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws");
        let http = parse_provider_kind(OPENAI_RESPONSES_KIND).expect("http");
        assert!(ws.uses_websocket());
        assert!(!http.uses_websocket());
        assert_eq!(ws.adapter_kind(), http.adapter_kind());
        assert_ne!(ws.as_str(), http.as_str());
    }

    #[test]
    fn rejects_api_proxy_for_websocket_provider_kind() {
        let kind = parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind");
        ensure_proxy_compatible_with_kind(kind, false).expect("proxy off is ok");
        let error = ensure_proxy_compatible_with_kind(kind, true).expect_err("proxy on fails");
        assert!(matches!(
            error,
            ProviderConfigError::UnsupportedProxyForWebSocket { .. }
        ));
        ensure_proxy_compatible_with_kind(
            parse_provider_kind(OPENAI_RESPONSES_KIND).expect("http responses"),
            true,
        )
        .expect("http responses allows proxy");

        let config = ProviderConnectionConfig {
            kind,
            base_url: Some(DEFAULT_OPENAI_BASE_URL.to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: Some("http://127.0.0.1:7890".to_string()),
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let client_error = config.genai_client().expect_err("proxy + websocket client");
        assert!(matches!(
            client_error,
            ProviderConfigError::UnsupportedProxyForWebSocket { .. }
        ));
    }

    #[test]
    fn websocket_kind_keeps_http_models_base_and_response_create_wire_contract() {
        let kind = parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind");
        let base = "https://gateway.example/v1/";
        let config = ProviderConnectionConfig {
            kind,
            base_url: Some(base.to_string()),
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        // Model list continues to use the HTTP base_url / OpenAIResp adapter path.
        assert_eq!(
            config.diagnostic_endpoint_url().expect("http endpoint"),
            "https://gateway.example/v1/"
        );
        assert_eq!(kind.adapter_kind(), AdapterKind::OpenAIResp);

        let ws_url = openai_responses_websocket_url_from_base(base).expect("ws endpoint");
        assert_eq!(ws_url, "wss://gateway.example/v1/responses");
        // Minimal response.create envelope sent after WebSocket handshake.
        let create = serde_json::json!({
            "type": "response.create",
            "model": "gpt-4o-mini",
            "input": [{ "role": "user", "content": "ping" }],
        });
        assert_eq!(create["type"], "response.create");
        assert_eq!(create["model"], "gpt-4o-mini");
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
        assert!(kinds.contains(&XAI_RESPONSES_KIND));
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
            normalized_tool_arguments(&Value::String(r#"{"path":"note.txt"}"#.to_string())),
            serde_json::json!({ "path": "note.txt" })
        );

        let double_encoded =
            serde_json::to_string(r#"{"path":"note.txt"}"#).expect("double encoded JSON argument");
        assert_eq!(
            normalized_tool_arguments(&Value::String(double_encoded)),
            serde_json::json!({ "path": "note.txt" })
        );

        assert_eq!(
            normalized_tool_arguments(&Value::String("plain text".to_string())),
            Value::String("plain text".to_string())
        );

        // Already-structured values are returned unchanged.
        let object = serde_json::json!({ "path": "note.txt" });
        assert_eq!(normalized_tool_arguments(&object), object);
    }

    #[test]
    fn recover_model_json_from_text_accepts_pure_json_object_and_array() {
        assert_eq!(
            recover_model_json_from_text(r#"{"path":"note.txt"}"#),
            Some(serde_json::json!({ "path": "note.txt" }))
        );
        assert_eq!(
            recover_model_json_from_text("  \n[1, 2, 3]\n  "),
            Some(serde_json::json!([1, 2, 3]))
        );
    }

    #[test]
    fn recover_model_json_from_text_unwraps_double_encoded_json_string() {
        let double_encoded =
            serde_json::to_string(r#"{"path":"note.txt"}"#).expect("double encoded JSON");
        assert_eq!(
            recover_model_json_from_text(&double_encoded),
            Some(serde_json::json!({ "path": "note.txt" }))
        );
    }

    #[test]
    fn recover_model_json_from_text_accepts_single_json_or_bare_fence() {
        let with_lang = "```json\n{\"path\":\"note.txt\"}\n```";
        assert_eq!(
            recover_model_json_from_text(with_lang),
            Some(serde_json::json!({ "path": "note.txt" }))
        );

        let bare = "```\n{\"path\":\"note.txt\"}\n```";
        assert_eq!(
            recover_model_json_from_text(bare),
            Some(serde_json::json!({ "path": "note.txt" }))
        );

        let uppercase_lang = "```JSON\n{\"path\":\"note.txt\"}\n```";
        assert_eq!(
            recover_model_json_from_text(uppercase_lang),
            Some(serde_json::json!({ "path": "note.txt" }))
        );

        let crlf = "```json\r\n{\"path\":\"note.txt\"}\r\n```";
        assert_eq!(
            recover_model_json_from_text(crlf),
            Some(serde_json::json!({ "path": "note.txt" }))
        );
    }

    #[test]
    fn recover_model_json_from_text_rejects_prose_and_non_json_fences() {
        assert_eq!(
            recover_model_json_from_text(
                "Here is the payload:\n```json\n{\"path\":\"note.txt\"}\n```"
            ),
            None
        );
        assert_eq!(
            recover_model_json_from_text("```json\n{\"path\":\"note.txt\"}\n```\nThanks!"),
            None
        );
        assert_eq!(
            recover_model_json_from_text("```javascript\n{\"path\":\"note.txt\"}\n```"),
            None
        );
        assert_eq!(
            recover_model_json_from_text("plain text without json"),
            None
        );
        assert_eq!(recover_model_json_from_text(""), None);
        assert_eq!(recover_model_json_from_text("   \n\t  "), None);
    }

    #[test]
    fn recover_model_json_from_text_does_not_repair_invalid_json_syntax() {
        // Single quotes, trailing commas, and unquoted keys are not repaired.
        assert_eq!(recover_model_json_from_text("{'path': 'note.txt'}"), None);
        assert_eq!(
            recover_model_json_from_text(r#"{"path": "note.txt",}"#),
            None
        );
        assert_eq!(recover_model_json_from_text(r#"{path: "note.txt"}"#), None);
        assert_eq!(
            normalized_tool_arguments(&Value::String(r#"{"path": "note.txt",}"#.to_string())),
            Value::String(r#"{"path": "note.txt",}"#.to_string())
        );
    }

    #[test]
    fn model_json_string_unwrap_respects_depth_limit() {
        // Shared hard cap used by ToolCall args and text recovery.
        assert_eq!(MODEL_JSON_STRING_UNWRAP_DEPTH, 4);

        let leaf = serde_json::json!({ "path": "note.txt" });

        // One encoding layer.
        assert_eq!(
            normalized_tool_arguments(&Value::String(r#"{"path":"note.txt"}"#.to_string())),
            leaf
        );

        // Two encoding layers (JSON string of object text) fully unwrap.
        let double =
            Value::String(serde_json::to_string(r#"{"path":"note.txt"}"#).expect("double encode"));
        assert_eq!(normalized_tool_arguments(&double), leaf);
        assert_eq!(
            recover_model_json_from_text(double.as_str().expect("double is string")),
            Some(leaf.clone())
        );

        // A third serde_json string layer starts with `"\` rather than `"{` /
        // `"[{`, so the conservative looks-like gate leaves it unchanged. This
        // proves unwrap is bounded and does not invent repairs for deeper nests.
        let triple = Value::String(serde_json::to_string(&double).expect("triple encode"));
        let triple_result = normalized_tool_arguments(&triple);
        assert!(triple_result.is_string());
        assert_ne!(triple_result, leaf);
        assert_eq!(
            recover_model_json_from_text(triple.as_str().expect("triple is string")),
            None
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
            kind: NeutralToolKind::Function,
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
        assert_eq!(
            chat_request.messages[0].role,
            genai::chat::ChatRole::Developer
        );
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
    fn disabled_developer_role_normalizes_only_the_outbound_request_before_system_merging() {
        let request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "Base system."),
            neutral_text_message(NeutralChatRole::Developer, "## Skills\n\n- Name: html-ppt"),
            neutral_text_message(NeutralChatRole::User, "Continue."),
        ]);

        let normalized = normalize_request_developer_role(
            request.clone(),
            &ChatRequestRuntimeOptions {
                developer_role_enabled: false,
                ..Default::default()
            },
        );

        assert_eq!(request.messages[1].role, NeutralChatRole::Developer);
        assert_eq!(normalized.messages[1].role, NeutralChatRole::System);
        assert_eq!(normalized.messages[1].content, request.messages[1].content);
        assert_eq!(normalized.messages[2].role, NeutralChatRole::User);

        let chat_request = genai_chat_request(&normalized).expect("normalized chat request");
        assert_eq!(
            chat_request.system.as_deref(),
            Some("Base system.\n\n## Skills\n\n- Name: html-ppt")
        );
        assert_eq!(chat_request.messages.len(), 1);
        assert_eq!(chat_request.messages[0].role, genai::chat::ChatRole::User);
    }

    #[test]
    fn default_developer_role_policy_preserves_developer_messages() {
        let request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::Developer,
            "Developer prompt.",
        )]);

        let normalized =
            normalize_request_developer_role(request, &ChatRequestRuntimeOptions::default());

        assert_eq!(normalized.messages[0].role, NeutralChatRole::Developer);
    }

    #[test]
    fn validates_developer_messages_as_instructions() {
        let mut with_attachment =
            neutral_text_message(NeutralChatRole::Developer, "Developer prompt.");
        with_attachment.attachments.push(NeutralChatAttachment {
            id: "attachment-1".to_string(),
            name: "prompt.txt".to_string(),
            content_type: "text/plain".to_string(),
            size_bytes: 6,
            content_base64: Some("cHJvbXB0".to_string()),
            path: None,
        });
        let attachment_error = genai_message(&with_attachment).expect_err("developer attachment");
        assert!(
            attachment_error
                .to_string()
                .contains("developer messages cannot contain attachments")
        );

        let empty_error = genai_message(&neutral_text_message(NeutralChatRole::Developer, "   "))
            .expect_err("empty developer message");
        assert!(
            empty_error
                .to_string()
                .contains("chat message content must not be empty")
        );

        let mut with_tool_state =
            neutral_text_message(NeutralChatRole::Developer, "Developer prompt.");
        with_tool_state.tool_call_id = Some("call-1".to_string());
        let tool_state_error = genai_message(&with_tool_state).expect_err("developer tool state");
        assert!(
            tool_state_error
                .to_string()
                .contains("developer messages cannot contain tool state")
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
        assert_eq!(
            chat_request.messages[0].role,
            genai::chat::ChatRole::Developer
        );
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
        agent_correlation: None,
        tool_choice: NeutralToolChoice::Auto,
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
    fn runtime_temperature_override_replaces_model_default() {
        let request = neutral_request(Vec::new());
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };

        let options = genai_chat_options_with_runtime_options(
            &config,
            &request,
            &ChatRequestRuntimeOptions {
                latency_mode: LatencyMode::Standard,
                developer_role_enabled: true,
                temperature: Some(0.7),
            },
        )
        .expect("runtime temperature override should be accepted");

        assert_eq!(options.temperature, Some(0.7));
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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
            agent_correlation: None,
            tool_choice: NeutralToolChoice::Auto,
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

    fn header_map_from_options(options: &ChatOptions) -> std::collections::HashMap<String, String> {
        let value = serde_json::to_value(options).expect("options json");
        let headers = value
            .get("extra_headers")
            .and_then(|value| value.as_object())
            .cloned()
            .unwrap_or_default();
        headers
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_ascii_lowercase(),
                    value.as_str().unwrap_or_default().to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn openai_resp_http_injects_identity_and_correlation_headers() {
        let request = NeutralChatRequest {
            model_id: "gpt-5.5".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: Some(
                AgentRequestCorrelation::new("plan-xyz", "chat-impl-1", "llm-req-1")
                    .with_run_id("run-1")
                    .with_workspace_id("ws-1"),
            ),
            tool_choice: NeutralToolChoice::Auto,
        };
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let options = genai_chat_options(&config, &request).expect("options");
        let headers = header_map_from_options(&options);
        assert_eq!(headers.get("originator").map(String::as_str), Some("foco"));
        assert!(
            headers
                .get("user-agent")
                .is_some_and(|value| value.starts_with("foco/"))
        );
        assert_eq!(
            headers.get("session-id").map(String::as_str),
            Some("plan-xyz")
        );
        assert_eq!(
            headers.get("thread-id").map(String::as_str),
            Some("chat-impl-1")
        );
        assert_eq!(
            headers.get("x-client-request-id").map(String::as_str),
            Some("llm-req-1")
        );
        assert_eq!(
            headers.get("x-foco-run-id").map(String::as_str),
            Some("run-1")
        );
        assert_eq!(
            headers.get("x-foco-workspace-id").map(String::as_str),
            Some("ws-1")
        );
        assert!(!headers.contains_key("openai-beta"));
        assert!(
            !headers.contains_key("version"),
            "default OpenAIResp HTTP headers must not send bare version (app version stays in User-Agent)"
        );
    }

    #[test]
    fn openai_resp_websocket_injects_beta_header() {
        let request = NeutralChatRequest {
            model_id: "gpt-5.5".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: Some(AgentRequestCorrelation::new(
                "chat-abc",
                "chat-abc",
                "llm-req-2",
            )),
            tool_choice: NeutralToolChoice::Auto,
        };
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_WEBSOCKET_KIND).expect("ws kind"),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let options = genai_chat_options(&config, &request).expect("options");
        let headers = header_map_from_options(&options);
        assert_eq!(
            headers.get("openai-beta").map(String::as_str),
            Some(OPENAI_RESP_WS_BETA_VALUE)
        );
        assert_eq!(headers.get("originator").map(String::as_str), Some("foco"));
        assert_eq!(
            headers.get("session-id").map(String::as_str),
            Some("chat-abc")
        );
        assert!(
            !headers.contains_key("version"),
            "default OpenAIResp WebSocket headers must not send bare version"
        );
    }

    #[test]
    fn request_overrides_replace_same_named_agent_headers() {
        let request = NeutralChatRequest {
            model_id: "gpt-5.5".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: Some(AgentRequestCorrelation::new(
                "session-default",
                "thread-default",
                "req-default",
            )),
            tool_choice: NeutralToolChoice::Auto,
        };
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: vec![
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_HEADER.to_string(),
                    name: "session-id".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("session-override".to_string()),
                },
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_HEADER.to_string(),
                    name: "originator".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("custom-gateway".to_string()),
                },
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_HEADER.to_string(),
                    name: "version".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("explicit-version-override".to_string()),
                },
            ],
            model_redirects: Vec::new(),
        };
        let options = genai_chat_options(&config, &request).expect("options");
        let headers = header_map_from_options(&options);
        assert_eq!(
            headers.get("session-id").map(String::as_str),
            Some("session-override")
        );
        assert_eq!(
            headers.get("originator").map(String::as_str),
            Some("custom-gateway")
        );
        assert_eq!(
            headers.get("thread-id").map(String::as_str),
            Some("thread-default")
        );
        assert_eq!(
            headers.get("version").map(String::as_str),
            Some("explicit-version-override"),
            "request_overrides may still add version; defaults only omit it"
        );
    }

    #[test]
    fn non_openai_resp_adapters_do_not_get_default_agent_headers() {
        let request = NeutralChatRequest {
            model_id: "gpt-4o-mini".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: Some(AgentRequestCorrelation::new("s", "t", "r")),
            tool_choice: NeutralToolChoice::Auto,
        };
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind"),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let options = genai_chat_options(&config, &request).expect("options");
        let headers = header_map_from_options(&options);
        assert!(headers.is_empty());
    }

    #[test]
    fn neutral_tool_choice_serializes_auto_and_required_single_tool() {
        let auto = NeutralToolChoice::Auto;
        assert_eq!(
            serde_json::to_value(&auto).expect("auto json"),
            serde_json::json!({ "type": "auto" })
        );
        assert_eq!(
            serde_json::from_value::<NeutralToolChoice>(serde_json::json!({ "type": "auto" }))
                .expect("auto parse"),
            NeutralToolChoice::Auto
        );

        let required = NeutralToolChoice::required_single_tool("select_relevant_memory");
        assert_eq!(
            serde_json::to_value(&required).expect("required json"),
            serde_json::json!({
                "type": "requiredSingleTool",
                "toolName": "select_relevant_memory"
            })
        );
        assert_eq!(
            serde_json::from_value::<NeutralToolChoice>(serde_json::json!({
                "type": "requiredSingleTool",
                "toolName": "submit_memory_extraction"
            }))
            .expect("required parse"),
            NeutralToolChoice::required_single_tool("submit_memory_extraction")
        );

        // Missing toolChoice deserializes as Auto for broker / older payloads.
        let request: NeutralChatRequest = serde_json::from_value(serde_json::json!({
            "model_id": "gpt-4o-mini",
            "messages": [{
                "role": "user",
                "content": "hi",
                "attachments": [],
                "tool_calls": []
            }],
            "tools": []
        }))
        .expect("legacy request without toolChoice");
        assert_eq!(request.tool_choice, NeutralToolChoice::Auto);
    }

    #[test]
    fn required_single_tool_enforcement_matrix() {
        assert_eq!(
            resolve_tool_choice_enforcement(AdapterKind::OpenAIResp, &NeutralToolChoice::Auto),
            ToolChoiceEnforcement::Auto
        );
        assert_eq!(
            resolve_tool_choice_enforcement(AdapterKind::DeepSeek, &NeutralToolChoice::Auto),
            ToolChoiceEnforcement::Auto
        );
        assert_eq!(
            resolve_tool_choice_enforcement(
                AdapterKind::OpenAIResp,
                &NeutralToolChoice::required_single_tool("select_relevant_memory")
            ),
            ToolChoiceEnforcement::Applied
        );
        assert_eq!(
            resolve_tool_choice_enforcement(
                AdapterKind::Anthropic,
                &NeutralToolChoice::required_single_tool("submit_memory_extraction")
            ),
            ToolChoiceEnforcement::Applied
        );
        assert_eq!(
            resolve_tool_choice_enforcement(
                AdapterKind::Gemini,
                &NeutralToolChoice::required_single_tool("submit_workspace_spec_update")
            ),
            ToolChoiceEnforcement::Applied
        );
        assert_eq!(
            resolve_tool_choice_enforcement(
                AdapterKind::Ollama,
                &NeutralToolChoice::required_single_tool("select_relevant_memory")
            ),
            ToolChoiceEnforcement::UnsupportedDegraded
        );
        assert_eq!(
            resolve_tool_choice_enforcement(
                AdapterKind::Cohere,
                &NeutralToolChoice::required_single_tool("select_relevant_memory")
            ),
            ToolChoiceEnforcement::UnsupportedDegraded
        );
        assert_eq!(
            resolve_tool_choice_enforcement(
                AdapterKind::BedrockApi,
                &NeutralToolChoice::required_single_tool("select_relevant_memory")
            ),
            ToolChoiceEnforcement::UnsupportedDegraded
        );
        assert_eq!(
            resolve_tool_choice_enforcement(
                AdapterKind::DeepSeek,
                &NeutralToolChoice::required_single_tool("select_relevant_memory")
            ),
            ToolChoiceEnforcement::UnsupportedDegraded
        );
        assert!(!adapter_supports_required_single_tool(
            AdapterKind::OllamaCloud
        ));
        assert!(!adapter_supports_required_single_tool(
            AdapterKind::DeepSeek
        ));
        assert!(adapter_supports_required_single_tool(AdapterKind::OpenAI));
    }

    #[test]
    fn required_single_tool_maps_to_genai_options_for_supported_adapters() {
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "use the tool",
        )]);
        request.tools.push(NeutralToolDefinition {
            name: "select_relevant_memory".to_string(),
            description: "pick memories".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            strict: true,
            kind: NeutralToolKind::Function,
        });
        request.tool_choice = NeutralToolChoice::required_single_tool("select_relevant_memory");

        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_KIND).expect("responses kind"),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let options = genai_chat_options(&config, &request).expect("options");
        let tool_choice = options.tool_choice.expect("tool choice applied");
        assert_eq!(
            tool_choice,
            genai::chat::ToolChoice::tool("select_relevant_memory")
        );
    }

    #[test]
    fn required_single_tool_degrades_without_claiming_enforcement_for_ollama() {
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "use the tool",
        )]);
        request.tools.push(NeutralToolDefinition {
            name: "select_relevant_memory".to_string(),
            description: "pick memories".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            strict: true,
            kind: NeutralToolKind::Function,
        });
        request.tool_choice = NeutralToolChoice::required_single_tool("select_relevant_memory");

        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OLLAMA_KIND).expect("ollama kind"),
            base_url: Some("http://localhost:11434/".to_string()),
            api_key: None,
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        assert_eq!(
            resolve_tool_choice_enforcement(config.kind.adapter_kind(), &request.tool_choice),
            ToolChoiceEnforcement::UnsupportedDegraded
        );
        let options = genai_chat_options(&config, &request).expect("options");
        assert!(
            options.tool_choice.is_none(),
            "unsupported adapters must not silently claim forced tool_choice"
        );
    }

    #[test]
    fn required_single_tool_rejects_missing_or_empty_tool_name() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("openai kind"),
            base_url: None,
            api_key: Some("sk-test".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut missing = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "use the tool",
        )]);
        missing.tool_choice = NeutralToolChoice::required_single_tool("select_relevant_memory");
        let err = genai_chat_options(&config, &missing).expect_err("missing tool");
        assert!(err.to_string().contains("is not present in request.tools"));

        let mut empty = missing;
        empty.tools.push(NeutralToolDefinition {
            name: "select_relevant_memory".to_string(),
            description: "pick memories".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            strict: true,
            kind: NeutralToolKind::Function,
        });
        empty.tool_choice = NeutralToolChoice::required_single_tool("   ");
        let err = genai_chat_options(&config, &empty).expect_err("empty tool name");
        assert!(err.to_string().contains("must not be empty"));
    }

    #[tokio::test]
    async fn required_single_tool_is_serialized_on_openai_chat_wire() {
        let response = concat!(
            "data: {\"id\":\"resp-fixture\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"}}]}\\n\\n",
            "data: {\"id\":\"resp-fixture\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1}}\\n\\n",
            "data: [DONE]\\n\\n"
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
        let mut request = neutral_request(vec![
            neutral_text_message(NeutralChatRole::System, "wire system"),
            neutral_text_message(NeutralChatRole::User, "wire user"),
        ]);
        request.tools.push(NeutralToolDefinition {
            name: "select_relevant_memory".to_string(),
            description: "pick memories".to_string(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
            strict: true,
            kind: NeutralToolKind::Function,
        });
        request.tool_choice = NeutralToolChoice::required_single_tool("select_relevant_memory");

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("wire request dump")
            .as_http()
            .expect("http request dump")
            .clone();
        let body = dump.body.as_deref().expect("request body");
        let body_json: Value = serde_json::from_str(body).expect("request JSON");
        assert_eq!(
            body_json["tool_choice"],
            serde_json::json!({
                "type": "function",
                "function": { "name": "select_relevant_memory" }
            })
        );
        assert_eq!(
            body_json["tools"][0]["function"]["name"],
            "select_relevant_memory"
        );

        while stream.next_event().await.is_some() {}
        let _ = fixture.await;
    }

    #[tokio::test]
    async fn required_single_tool_omits_tool_choice_on_deepseek_chat_wire() {
        let response = concat!(
            "data: {\"id\":\"resp-fixture\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"}}]}\\n\\n",
            "data: {\"id\":\"resp-fixture\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1}}\\n\\n",
            "data: [DONE]\\n\\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response).await;
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(DEEPSEEK_KIND).expect("deepseek kind"),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "use the tool",
        )]);
        request.tools.push(NeutralToolDefinition {
            name: "select_relevant_memory".to_string(),
            description: "pick memories".to_string(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
            strict: true,
            kind: NeutralToolKind::Function,
        });
        request.tool_choice = NeutralToolChoice::required_single_tool("select_relevant_memory");

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("wire request dump")
            .as_http()
            .expect("http request dump")
            .clone();
        let body = dump.body.as_deref().expect("request body");
        let body_json: Value = serde_json::from_str(body).expect("request JSON");
        assert!(
            body_json.get("tool_choice").is_none(),
            "DeepSeek must receive tools + prompt degradation without native tool_choice"
        );
        assert_eq!(
            body_json["tools"][0]["function"]["name"],
            "select_relevant_memory"
        );

        while stream.next_event().await.is_some() {}
        let _ = fixture.await;
    }

    #[tokio::test]
    async fn required_single_tool_is_serialized_on_openai_responses_prepare_payload() {
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_RESPONSES_KIND).expect("responses kind"),
            base_url: Some("https://gateway.example/v1/".to_string()),
            api_key: Some("test-key".to_string()),
            proxy_url: None,
            request_overrides: vec![],
            model_redirects: Default::default(),
        };
        let mut request = NeutralChatRequest {
            model_id: "gpt-4.1-mini".to_string(),
            messages: vec![
                NeutralChatMessage {
                    role: NeutralChatRole::System,
                    content: "sys".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
                NeutralChatMessage {
                    role: NeutralChatRole::User,
                    content: "hi".to_string(),
                    attachments: vec![],
                    reasoning: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    tool_name: None,
                },
            ],
            tools: vec![NeutralToolDefinition {
                name: "submit_workspace_spec_update".to_string(),
                description: "update spec".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
                strict: true,
                kind: NeutralToolKind::Function,
            }],
            max_output_tokens: Some(128),
            thinking_level: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
            tool_choice: NeutralToolChoice::required_single_tool("submit_workspace_spec_update"),
        };

        let client = config.genai_client().expect("client");
        let chat_request =
            genai_chat_request_for_adapter(&request, config.kind.adapter_kind()).expect("chat");
        let options = genai_chat_options(&config, &request).expect("options");
        let model = genai::ModelIden::new(config.kind.adapter_kind(), "gpt-4.1-mini");
        let prepared = client
            .prepare_chat_stream_request(model.clone(), chat_request, Some(&options))
            .await
            .expect("prepare");

        assert_eq!(
            prepared.payload["tool_choice"],
            serde_json::json!({
                "type": "function",
                "name": "submit_workspace_spec_update"
            })
        );

        let ws_payload =
            genai::adapter::openai_resp_websocket_create_payload(prepared.payload.clone());
        assert_eq!(
            ws_payload["tool_choice"],
            serde_json::json!({
                "type": "function",
                "name": "submit_workspace_spec_update"
            })
        );

        // Normal chat Auto must not emit tool_choice.
        request.tool_choice = NeutralToolChoice::Auto;
        let chat_request =
            genai_chat_request_for_adapter(&request, config.kind.adapter_kind()).expect("chat");
        let options = genai_chat_options(&config, &request).expect("options");
        let prepared = client
            .prepare_chat_stream_request(model, chat_request, Some(&options))
            .await
            .expect("prepare auto");
        assert!(prepared.payload.get("tool_choice").is_none());
    }

    #[test]
    fn resolve_agent_session_thread_mapping_table() {
        assert_eq!(
            resolve_agent_session_thread_ids("chat-abc", None, None),
            ("chat-abc".to_string(), "chat-abc".to_string())
        );
        assert_eq!(
            resolve_agent_session_thread_ids("chat-impl-1", Some("plan-xyz"), None),
            ("plan-xyz".to_string(), "chat-impl-1".to_string())
        );
        assert_eq!(
            resolve_agent_session_thread_ids("chat-sub-9", None, Some("plan-xyz")),
            ("plan-xyz".to_string(), "chat-sub-9".to_string())
        );
    }

    fn dump_header_first(headers: &ProviderHttpHeadersDump, name: &str) -> Option<String> {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, values)| values.first().cloned())
    }

    fn raw_header_value(raw: &RawHttpRequest, name: &str) -> Option<String> {
        raw.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    }

    #[tokio::test]
    async fn openai_resp_http_wire_dump_includes_agent_headers() {
        let openai_responses = concat!(
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"headers ok\"}\n\n",
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-agent-headers\",\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"headers ok\",\"annotations\":[]}]}],\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", openai_responses).await;
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "agent headers wire",
        )]);
        request.model_id = "fixture-responses-model".to_string();
        request.agent_correlation = Some(
            AgentRequestCorrelation::new("plan-wire", "chat-impl-wire", "llm-req-wire")
                .with_run_id("run-wire")
                .with_workspace_id("ws-wire"),
        );

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open openai responses fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("http wire dump")
            .as_http()
            .expect("http dump")
            .clone();
        while stream.next_event().await.is_some() {}

        let requests = fixture.await.expect("fixture task");
        assert_eq!(requests.len(), 1);
        let raw = parse_raw_http_request(requests.into_iter().next().expect("raw request"));

        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_ORIGINATOR).as_deref(),
            Some(FOCO_AGENT_ORIGINATOR)
        );
        assert!(
            dump_header_first(&dump.headers, AGENT_HEADER_USER_AGENT)
                .is_some_and(|value| value.starts_with("foco/"))
        );
        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_SESSION_ID).as_deref(),
            Some("plan-wire")
        );
        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_THREAD_ID).as_deref(),
            Some("chat-impl-wire")
        );
        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_CLIENT_REQUEST_ID).as_deref(),
            Some("llm-req-wire")
        );
        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_FOCO_RUN_ID).as_deref(),
            Some("run-wire")
        );
        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_FOCO_WORKSPACE_ID).as_deref(),
            Some("ws-wire")
        );
        assert!(
            dump_header_first(&dump.headers, OPENAI_RESP_WS_BETA_HEADER).is_none(),
            "HTTP Responses must not send OpenAI-Beta websocket capability header"
        );
        assert!(
            dump_header_first(&dump.headers, "version").is_none(),
            "HTTP wire dump must not include default version header"
        );

        assert_eq!(
            raw_header_value(&raw, AGENT_HEADER_ORIGINATOR).as_deref(),
            Some(FOCO_AGENT_ORIGINATOR)
        );
        assert_eq!(
            raw_header_value(&raw, AGENT_HEADER_SESSION_ID).as_deref(),
            Some("plan-wire")
        );
        assert_eq!(
            raw_header_value(&raw, AGENT_HEADER_THREAD_ID).as_deref(),
            Some("chat-impl-wire")
        );
        assert_eq!(
            raw_header_value(&raw, AGENT_HEADER_CLIENT_REQUEST_ID).as_deref(),
            Some("llm-req-wire")
        );
        assert!(
            raw_header_value(&raw, OPENAI_RESP_WS_BETA_HEADER).is_none(),
            "raw HTTP must not include OpenAI-Beta websocket header"
        );
        assert!(
            raw_header_value(&raw, "version").is_none(),
            "raw HTTP must not include default version header"
        );
    }

    #[tokio::test]
    async fn openai_resp_detail_stream_exposes_failed_frame_diagnostic() {
        let openai_responses = concat!(
            "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{}}\n\n",
            "event: response.failed\ndata: {\"type\":\"response.failed\",\"api_key\":\"failed-frame-secret\",\"response\":{\"id\":\"resp-failed\",\"status\":\"failed\",\"model\":\"fixture-responses-model\",\"error\":{\"code\":\"rate_limit\",\"type\":\"rate_limit_error\",\"message\":\"retry later\",\"param\":\"model\"}}}\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", openai_responses).await;
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "capture the failed responses frame",
        )]);
        request.model_id = "fixture-responses-model".to_string();

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open failed OpenAI Responses stream");
        let stream_error = loop {
            match stream.next_event().await {
                Some(Err(error)) => break error,
                Some(Ok(_)) => continue,
                None => panic!("expected a stream error"),
            }
        };

        assert!(stream_error.to_string().contains("retry later"));
        let diagnostic = stream.stream_diagnostic().expect("failed frame diagnostic");
        assert_eq!(
            diagnostic.kind,
            genai::OpenAIRespStreamDiagnosticKind::ResponseFailed
        );
        assert_eq!(
            diagnostic.transport,
            genai::OpenAIRespStreamTransport::HttpSse
        );
        assert_eq!(diagnostic.event_type.as_deref(), Some("response.failed"));
        assert_eq!(
            diagnostic.previous_event_type.as_deref(),
            Some("response.created")
        );
        assert_eq!(
            diagnostic.provider_error.code.as_deref(),
            Some("rate_limit")
        );
        assert_eq!(
            diagnostic.provider_error.error_type.as_deref(),
            Some("rate_limit_error")
        );
        assert_eq!(
            diagnostic.provider_error.message.as_deref(),
            Some("retry later")
        );
        let final_dump = stream
            .final_response_dump()
            .expect("failed final response dump");
        let final_json = serde_json::to_value(final_dump).expect("final response JSON");
        assert_eq!(final_json["http"]["status"], 200);
        assert_eq!(final_json["statusCode"], 200);
        assert_eq!(final_json["streamDiagnostic"]["kind"], "response_failed");
        assert_eq!(
            final_json["streamDiagnostic"]["provider_error"]["code"],
            "rate_limit"
        );
        assert_eq!(
            final_json["streamDiagnostic"]["payload"]["value"]["api_key"],
            REDACTED_CREDENTIAL_VALUE
        );
        assert!(
            !final_json.to_string().contains("failed-frame-secret"),
            "stream diagnostic must use the provider credential redactor"
        );

        let requests = fixture.await.expect("fixture task");
        assert_eq!(requests.len(), 1);
    }

    #[test]
    fn failed_response_dump_deserializes_legacy_v1_without_stream_diagnostic() {
        let dump: ProviderFinalResponseDump = serde_json::from_str(
            r#"{"state":"failed","format":"provider_final_response_v1","version":1,"http":null,"partial":false,"error":"upstream","statusCode":502}"#,
        )
        .expect("legacy v1 response dump");

        assert!(matches!(
            dump,
            ProviderFinalResponseDump::Failed {
                stream_diagnostic: None,
                ..
            }
        ));
    }

    #[test]
    fn failed_response_audit_json_caps_oversized_diagnostic_without_losing_trace_header() {
        let dump = ProviderFinalResponseDump::Failed {
            format: PROVIDER_FINAL_RESPONSE_DUMP_FORMAT.to_string(),
            version: PROVIDER_FINAL_RESPONSE_DUMP_VERSION,
            http: Some(ProviderHttpResponseHeadDump {
                status: 200,
                version: "HTTP/1.1".to_string(),
                headers: BTreeMap::from([
                    ("x-cpa-trace-id".to_string(), vec!["trace-123".to_string()]),
                    (
                        "x-upstream-noise".to_string(),
                        vec!["n".repeat(MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES)],
                    ),
                ]),
            }),
            partial: false,
            error: "e".repeat(MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES),
            status_code: Some(200),
            stream_diagnostic: Some(json!({
                "kind": "invalid_json",
                "payload": "p".repeat(MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES),
            })),
        };

        let audit_json = dump.audit_json().expect("bounded audit JSON");
        assert!(
            audit_json.len() <= MAX_PROVIDER_FAILED_RESPONSE_AUDIT_BYTES,
            "failed audit detail must be bounded"
        );
        let value: Value = serde_json::from_str(&audit_json).expect("bounded JSON value");
        assert_eq!(value["http"]["status"], 200);
        assert_eq!(value["http"]["headers"]["x-cpa-trace-id"][0], "trace-123");
        assert!(value["http"]["headers"]["x-upstream-noise"].is_null());
        assert_eq!(value["streamDiagnostic"]["truncated"], true);
        assert_eq!(value["auditTruncation"]["maxBytes"], 32 * 1024);
    }

    #[tokio::test]
    async fn openai_resp_http_wire_dump_request_overrides_replace_agent_headers() {
        let openai_responses = concat!(
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"override ok\"}\n\n",
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-override\",\"status\":\"completed\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"override ok\",\"annotations\":[]}]}],\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", openai_responses).await;
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: vec![
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_HEADER.to_string(),
                    name: "session-id".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("session-from-override".to_string()),
                },
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_HEADER.to_string(),
                    name: "originator".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("gateway-custom".to_string()),
                },
                ProviderRequestOverride {
                    target: REQUEST_OVERRIDE_TARGET_HEADER.to_string(),
                    name: "version".to_string(),
                    value_type: REQUEST_OVERRIDE_VALUE_TYPE_STRING.to_string(),
                    value: Value::String("explicit-version-wire".to_string()),
                },
            ],
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "override wire",
        )]);
        request.model_id = "fixture-responses-model".to_string();
        request.agent_correlation = Some(AgentRequestCorrelation::new(
            "session-default",
            "thread-default",
            "req-default",
        ));

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open override stream");
        let dump = stream
            .wire_request_dump()
            .expect("http wire dump")
            .as_http()
            .expect("http dump")
            .clone();
        while stream.next_event().await.is_some() {}
        let requests = fixture.await.expect("fixture task");
        let raw = parse_raw_http_request(requests.into_iter().next().expect("raw request"));

        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_SESSION_ID).as_deref(),
            Some("session-from-override")
        );
        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_ORIGINATOR).as_deref(),
            Some("gateway-custom")
        );
        assert_eq!(
            dump_header_first(&dump.headers, AGENT_HEADER_THREAD_ID).as_deref(),
            Some("thread-default")
        );
        assert_eq!(
            dump_header_first(&dump.headers, "version").as_deref(),
            Some("explicit-version-wire"),
            "wire dump must allow explicit version via request_overrides"
        );
        assert_eq!(
            raw_header_value(&raw, AGENT_HEADER_SESSION_ID).as_deref(),
            Some("session-from-override")
        );
        assert_eq!(
            raw_header_value(&raw, AGENT_HEADER_ORIGINATOR).as_deref(),
            Some("gateway-custom")
        );
        assert_eq!(
            raw_header_value(&raw, "version").as_deref(),
            Some("explicit-version-wire"),
            "raw HTTP must allow explicit version via request_overrides"
        );
    }

    #[tokio::test]
    async fn non_openai_resp_http_wire_dump_does_not_inject_agent_headers() {
        let openai_chat = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"chat ok\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", openai_chat).await;
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("chat kind"),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "no agent headers",
        )]);
        request.agent_correlation = Some(AgentRequestCorrelation::new(
            "should-not-appear",
            "should-not-appear",
            "should-not-appear",
        ));

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open chat stream");
        let dump = stream
            .wire_request_dump()
            .expect("http wire dump")
            .as_http()
            .expect("http dump")
            .clone();
        while stream.next_event().await.is_some() {}
        let requests = fixture.await.expect("fixture task");
        let raw = parse_raw_http_request(requests.into_iter().next().expect("raw request"));

        assert!(dump_header_first(&dump.headers, AGENT_HEADER_SESSION_ID).is_none());
        assert!(dump_header_first(&dump.headers, AGENT_HEADER_THREAD_ID).is_none());
        assert!(dump_header_first(&dump.headers, AGENT_HEADER_ORIGINATOR).is_none());
        assert!(dump_header_first(&dump.headers, AGENT_HEADER_CLIENT_REQUEST_ID).is_none());
        assert!(raw_header_value(&raw, AGENT_HEADER_SESSION_ID).is_none());
        assert!(raw_header_value(&raw, AGENT_HEADER_ORIGINATOR).is_none());
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
    async fn openai_compatible_fixture_falls_back_developer_to_system() {
        let deepseek = concat!(
            "data: {\"raw_chunk_secret\":\"chunk-only-secret\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"deepseek ok\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        assert_adapter_captures_finalized_request_once(
            DEEPSEEK_KIND,
            "fixture-deepseek-model",
            "/chat/completions",
            deepseek,
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
            kind: NeutralToolKind::Function,
        });

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open fixture stream");
        let dump = stream
            .wire_request_dump()
            .expect("wire request dump")
            .as_http()
            .expect("http request dump")
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
        assert_eq!(stream.http_status(), Some(200));

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
        assert_eq!(stream.http_status(), Some(502));
        assert_eq!(fixture.await.expect("fixture task").len(), 1);
    }

    #[tokio::test]
    async fn connection_failure_before_http_response_does_not_fabricate_response_head() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("reserve unused fixture address");
        let addr = listener.local_addr().expect("unused fixture address");
        drop(listener);
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("openai kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "fail before response head",
        )]);

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("connection failures surface through the stream");
        assert!(stream.wire_request_dump().is_some());
        while stream.next_event().await.is_some() {}

        assert!(matches!(
            stream.final_response_dump(),
            Some(ProviderFinalResponseDump::Failed {
                http: None,
                status_code: None,
                ..
            })
        ));
        assert_eq!(stream.http_status(), None);
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
        assert_eq!(stream.http_status(), Some(200));
        let raw_requests = fixture.await.expect("fixture task");
        assert_eq!(raw_requests.len(), 1);
    }

    /// Non-default success 2xx proves `http_status()` is the real Response status, not hard-coded 200.
    /// Details-off still omits request/final dumps.
    #[tokio::test]
    async fn http_status_preserves_non_default_success_status_without_detail_dumps() {
        let response = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("201 Created", "text/event-stream", response).await;
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
            "non-default success status",
        )]);

        let mut stream = stream_chat_with_capture(&config, request, false)
            .await
            .expect("open 201 fixture stream");
        assert!(stream.wire_request_dump().is_none());
        while stream.next_event().await.is_some() {}
        assert!(stream.final_response_dump().is_none());
        assert_eq!(
            stream.http_status(),
            Some(201),
            "must return observed Response status, not a hard-coded 200"
        );
        assert_eq!(fixture.await.expect("fixture task").len(), 1);
    }

    #[tokio::test]
    async fn http_status_is_captured_without_detail_dumps() {
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
            "status without details",
        )]);

        let mut stream = stream_chat_with_capture(&config, request, false)
            .await
            .expect("open non-success fixture stream");
        while stream.next_event().await.is_some() {}

        assert!(stream.wire_request_dump().is_none());
        assert!(stream.final_response_dump().is_none());
        assert_eq!(stream.http_status(), Some(502));
        assert_eq!(fixture.await.expect("fixture task").len(), 1);
    }

    /// HTTP 200 with unparseable SSE: head was established, so status stays 200 while final dump fails.
    #[tokio::test]
    async fn http_status_survives_stream_decode_failure_after_response_head() {
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", "data: {not-valid-json\n\n")
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
            "decode fails after head",
        )]);

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open decode-failure fixture stream");
        while stream.next_event().await.is_some() {}

        assert_eq!(
            stream.http_status(),
            Some(200),
            "Response head status remains available after body/SSE decode failure"
        );
        assert!(matches!(
            stream.final_response_dump(),
            Some(ProviderFinalResponseDump::Failed {
                http: Some(ProviderHttpResponseHeadDump { status: 200, .. }),
                ..
            })
        ));
        assert_eq!(fixture.await.expect("fixture task").len(), 1);
    }

    #[tokio::test]
    async fn connection_failure_http_status_is_none_without_response() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("reserve unused fixture address");
        let addr = listener.local_addr().expect("unused fixture address");
        drop(listener);
        let config = ProviderConnectionConfig {
            kind: parse_provider_kind(OPENAI_CHAT_KIND).expect("openai kind"),
            base_url: Some(format!("http://{addr}/v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "no status without response",
        )]);

        let mut stream = stream_chat_with_capture(&config, request, false)
            .await
            .expect("connection failures surface through the stream");
        while stream.next_event().await.is_some() {}
        assert_eq!(stream.http_status(), None);
    }

    #[test]
    fn classify_provider_stream_failure_kind_covers_capacity_rate_limit_and_parse() {
        assert_eq!(
            classify_provider_stream_failure_kind(
                Some("model_capacity"),
                None,
                "The model is currently overloaded",
                None,
            ),
            ProviderStreamFailureKind::Capacity
        );
        assert_eq!(
            classify_provider_stream_failure_kind(
                Some("rate_limit_exceeded"),
                Some("rate_limit_error"),
                "Too many requests",
                None,
            ),
            ProviderStreamFailureKind::RateLimit
        );
        assert_eq!(
            classify_provider_stream_failure_kind(
                Some("server_error"),
                Some("server_error"),
                "The server had an error",
                None,
            ),
            ProviderStreamFailureKind::ServerError
        );
        assert_eq!(
            classify_provider_stream_failure_kind(
                None,
                None,
                "Failed to parse stream data: invalid json",
                None,
            ),
            ProviderStreamFailureKind::ProtocolParse
        );
        assert_eq!(
            classify_provider_stream_failure_kind(
                Some("invalid_api_key"),
                Some("invalid_request_error"),
                "Incorrect API key provided",
                None,
            ),
            ProviderStreamFailureKind::Auth
        );
        assert_eq!(
            classify_provider_stream_failure_kind(
                Some("context_length_exceeded"),
                Some("invalid_request_error"),
                "This model's maximum context length is 128000 tokens",
                None,
            ),
            ProviderStreamFailureKind::ContextLength
        );
        assert_eq!(
            classify_provider_stream_failure_kind(
                Some("insufficient_quota"),
                Some("insufficient_quota"),
                "You exceeded your current quota",
                None,
            ),
            ProviderStreamFailureKind::InvalidRequest
        );
    }

    #[test]
    fn provider_stream_error_user_message_prefers_provider_text() {
        let error = ProviderConfigError::ProviderStream(Box::new(ProviderStreamErrorDetail {
            message: "The model is currently overloaded".to_string(),
            status_code: None,
            kind: ProviderStreamFailureKind::Capacity,
            code: Some("model_capacity".to_string()),
            error_type: None,
            param: None,
            event_type: Some("error".to_string()),
            diagnostic_kind: Some("provider_error_event".to_string()),
            model_id: Some("gpt-test".to_string()),
            adapter: Some("OpenAI Responses".to_string()),
        }));
        assert_eq!(
            error.user_message(),
            "The model is currently overloaded (model 'gpt-test')"
        );
        assert!(!error.user_message().contains("Failed to parse stream data"));
        assert_eq!(
            error.stream_failure_kind(),
            Some(ProviderStreamFailureKind::Capacity)
        );
        assert!(error.stream_detail().is_some());
    }

    #[test]
    fn sanitize_provider_stream_message_redacts_common_credential_shapes() {
        let redacted = sanitize_provider_stream_message(
            "auth failed for Bearer sk-live-secret-value and api_key=super-secret-token",
        );
        assert!(
            redacted.contains(REDACTED_CREDENTIAL_VALUE),
            "credential material must be redacted: {redacted}"
        );
        assert!(!redacted.contains("sk-live-secret-value"));
        assert!(!redacted.contains("super-secret-token"));
    }

    async fn collect_responses_stream_error(
        response_body: &'static str,
    ) -> (
        ProviderConfigError,
        Option<genai::OpenAIRespStreamDiagnostic>,
    ) {
        let (fixture_root, fixture) =
            spawn_raw_http_fixture("200 OK", "text/event-stream", response_body).await;
        let config = ProviderConnectionConfig {
            kind: openai_responses_kind(),
            base_url: Some(format!("{fixture_root}v1/")),
            api_key: Some("fixture-api-key".to_string()),
            proxy_url: None,
            request_overrides: Vec::new(),
            model_redirects: Vec::new(),
        };
        let mut request = neutral_request(vec![neutral_text_message(
            NeutralChatRole::User,
            "classify provider stream error",
        )]);
        request.model_id = "fixture-responses-model".to_string();

        let mut stream = stream_chat_with_capture(&config, request, true)
            .await
            .expect("open responses error fixture stream");
        let stream_error = loop {
            match stream.next_event().await {
                Some(Err(error)) => break error,
                Some(Ok(_)) => continue,
                None => panic!("expected a stream error"),
            }
        };
        let diagnostic = stream.stream_diagnostic();
        assert_eq!(fixture.await.expect("fixture task").len(), 1);
        (stream_error, diagnostic)
    }

    #[tokio::test]
    async fn openai_resp_flat_capacity_error_maps_to_provider_stream_not_parse() {
        // Grok-style flat envelope: top-level code/message without nested `error`.
        let (error, diagnostic) = collect_responses_stream_error(concat!(
            "event: error\n",
            "data: {\"type\":\"error\",\"code\":\"model_capacity\",\"message\":\"The model is currently overloaded\",\"api_key\":\"capacity-secret\"}\n\n"
        ))
        .await;

        assert_eq!(
            error.stream_failure_kind(),
            Some(ProviderStreamFailureKind::Capacity)
        );
        assert!(
            error.user_message().contains("currently overloaded"),
            "user message should keep the provider capacity text: {}",
            error.user_message()
        );
        assert!(
            !error.user_message().contains("Failed to parse stream data"),
            "legal provider capacity errors must not look like parse failures"
        );
        let detail = error.stream_detail().expect("structured stream detail");
        assert_eq!(detail.code.as_deref(), Some("model_capacity"));
        assert_eq!(detail.event_type.as_deref(), Some("error"));
        let diagnostic = diagnostic.expect("capacity diagnostic");
        assert_eq!(
            diagnostic.kind,
            genai::OpenAIRespStreamDiagnosticKind::ProviderErrorEvent
        );
        assert_eq!(
            diagnostic.provider_error.code.as_deref(),
            Some("model_capacity")
        );
        assert_eq!(
            diagnostic.provider_error.message.as_deref(),
            Some("The model is currently overloaded")
        );
        let diagnostic_json = serde_json::to_value(&diagnostic).expect("diagnostic json");
        assert!(
            !diagnostic_json.to_string().contains("capacity-secret"),
            "stream diagnostic must redact credentials from the wire frame"
        );
    }

    #[tokio::test]
    async fn openai_resp_nested_server_error_maps_to_provider_stream_not_parse() {
        // OpenAI/Krill nested envelope under `error`.
        let (error, diagnostic) = collect_responses_stream_error(concat!(
            "event: error\n",
            "data: {\"type\":\"error\",\"error\":{\"code\":\"server_error\",\"type\":\"server_error\",\"message\":\"The server had an error while processing your request\"}}\n\n"
        ))
        .await;

        assert_eq!(
            error.stream_failure_kind(),
            Some(ProviderStreamFailureKind::ServerError)
        );
        assert!(
            error.user_message().contains("server had an error"),
            "user message should keep nested provider text: {}",
            error.user_message()
        );
        assert!(!error.user_message().contains("Failed to parse stream data"));
        let detail = error.stream_detail().expect("structured stream detail");
        assert_eq!(detail.code.as_deref(), Some("server_error"));
        assert_eq!(detail.error_type.as_deref(), Some("server_error"));
        let diagnostic = diagnostic.expect("server_error diagnostic");
        assert_eq!(
            diagnostic.kind,
            genai::OpenAIRespStreamDiagnosticKind::ProviderErrorEvent
        );
        assert_eq!(
            diagnostic.provider_error.message.as_deref(),
            Some("The server had an error while processing your request")
        );
    }

    #[tokio::test]
    async fn openai_resp_response_failed_maps_to_provider_stream_not_parse() {
        let (error, diagnostic) = collect_responses_stream_error(concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{}}\n\n",
            "event: response.failed\n",
            "data: {\"type\":\"response.failed\",\"response\":{\"id\":\"resp-failed\",\"status\":\"failed\",\"model\":\"fixture-responses-model\",\"error\":{\"code\":\"server_error\",\"type\":\"server_error\",\"message\":\"upstream failed\"}}}\n\n"
        ))
        .await;

        assert_eq!(
            error.stream_failure_kind(),
            Some(ProviderStreamFailureKind::ServerError)
        );
        assert!(error.user_message().contains("upstream failed"));
        assert!(!error.user_message().contains("Failed to parse stream data"));
        let diagnostic = diagnostic.expect("response.failed diagnostic");
        assert_eq!(
            diagnostic.kind,
            genai::OpenAIRespStreamDiagnosticKind::ResponseFailed
        );
        assert_eq!(diagnostic.event_type.as_deref(), Some("response.failed"));
    }

    #[tokio::test]
    async fn openai_resp_malformed_json_maps_to_protocol_parse() {
        let (error, diagnostic) = collect_responses_stream_error(concat!(
            "event: response.output_text.delta\n",
            "data: {not-valid-json\n\n"
        ))
        .await;

        assert_eq!(
            error.stream_failure_kind(),
            Some(ProviderStreamFailureKind::ProtocolParse)
        );
        assert!(
            error.user_message().contains("Failed to parse stream data"),
            "true JSON parse failures keep the parse wording: {}",
            error.user_message()
        );
        let detail = error.stream_detail().expect("parse detail");
        assert_eq!(detail.diagnostic_kind.as_deref(), Some("invalid_json"));
        let diagnostic = diagnostic.expect("invalid_json diagnostic");
        assert_eq!(
            diagnostic.kind,
            genai::OpenAIRespStreamDiagnosticKind::InvalidJson
        );
    }

    #[test]
    fn parse_retry_after_seconds_accepts_integer_and_clamps() {
        assert_eq!(parse_retry_after_seconds("7"), Some(Duration::from_secs(7)));
        assert_eq!(
            parse_retry_after_seconds(" 12 "),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            parse_retry_after_seconds("120"),
            Some(Duration::from_secs(30))
        );
        assert_eq!(parse_retry_after_seconds(""), None);
        assert_eq!(
            parse_retry_after_seconds("Wed, 21 Oct 2015 07:28:00 GMT"),
            None
        );
        assert_eq!(
            retry_after_from_http_headers(&BTreeMap::from([(
                "Retry-After".to_string(),
                vec!["9".to_string()]
            )])),
            Some(Duration::from_secs(9))
        );
    }
}
