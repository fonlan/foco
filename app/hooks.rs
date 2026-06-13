use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use foco_mcp::{McpRegistry, encode_mcp_tool_name};
use foco_providers::{
    NeutralChatMessage, NeutralChatRequest, NeutralChatRole, NeutralChatStream,
    NeutralChatStreamEvent, NeutralUsage, ProviderConnectionConfig,
    serialize_provider_request_body, stream_chat,
};
use foco_store::{
    config::{
        HOOK_HANDLER_COMMAND, HOOK_HANDLER_HTTP, HOOK_HANDLER_MCP_TOOL, HOOK_HANDLER_PROMPT,
        HookConfig, HookHandler, load_workspace_hook_config,
    },
    workspace::{
        NewHookRun, NewLlmRequest, NewLlmRequestEvent, UpdateLlmRequestOutcome, WorkspaceDatabase,
    },
};
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

const DEFAULT_HOOK_TIMEOUT_MS: u64 = 60_000;
const HOOK_OUTPUT_PREVIEW_CHARS: usize = 4000;

#[derive(Clone)]
pub struct HookRuntime {
    mcp_registry: Arc<McpRegistry>,
}

impl HookRuntime {
    pub fn new(mcp_registry: Arc<McpRegistry>) -> Self {
        Self { mcp_registry }
    }

    pub async fn run_hooks(&self, request: HookRunRequest<'_>) -> HookRunSummary {
        if HOOK_STACK_ACTIVE
            .try_with(|active| *active)
            .unwrap_or(false)
        {
            return HookRunSummary {
                errors: vec!["hook execution skipped to prevent hook recursion".to_string()],
                ..HookRunSummary::default()
            };
        }

        HOOK_STACK_ACTIVE
            .scope(true, async move { self.run_hooks_inner(request).await })
            .await
    }

    async fn run_hooks_inner(&self, request: HookRunRequest<'_>) -> HookRunSummary {
        let mut summary = HookRunSummary::default();
        let effective_hooks = match effective_hooks(request.global_config, request.workspace_path) {
            Ok(hooks) => hooks,
            Err(error) => {
                summary.errors.push(error.to_string());
                return summary;
            }
        };
        let mut executed_handlers = HashSet::new();

        for source in effective_hooks.sources {
            if source.config.disable_all_hooks {
                continue;
            }

            let Some(groups) = source.config.hooks.get(request.event) else {
                continue;
            };

            for group in groups {
                if !group.enabled {
                    continue;
                }
                if !matcher_matches(group.matcher.as_deref(), request.match_value.as_deref()) {
                    continue;
                }

                for handler in &group.hooks {
                    if !handler.enabled {
                        continue;
                    }
                    if !if_filter_matches(handler.if_filter.as_deref(), &request) {
                        continue;
                    }
                    let handler_key =
                        hook_handler_key(request.event, group.matcher.as_deref(), handler);
                    if !executed_handlers.insert(handler_key) {
                        continue;
                    }

                    if handler.async_hook {
                        self.spawn_async_handler(source.source.clone(), handler.clone(), &request);
                        if let Some(message) = handler.status_message.as_ref() {
                            summary.system_messages.push(message.clone());
                        }
                        continue;
                    }

                    let result = self.run_handler(&source.source, handler, &request).await;
                    let decision = result.decision.clone();
                    if let Err(error) =
                        persist_hook_result(&request, &source.source, handler, &result)
                    {
                        summary.errors.push(error);
                    }
                    if let Some(error) = result.error {
                        summary.errors.push(error);
                    }
                    if let Some(context) = result.additional_context {
                        summary.additional_context.push(context);
                    }
                    if let Some(message) = result.system_message {
                        summary.system_messages.push(message);
                    }
                    if let Some(decision) = decision {
                        summary.decisions.push(decision);
                    }
                    if let Some(hook_specific_output) = result.hook_specific_output {
                        summary.hook_specific_outputs.push(hook_specific_output);
                    }
                }
            }
        }

        summary
    }

    fn spawn_async_handler(
        &self,
        source: String,
        handler: HookHandler,
        request: &HookRunRequest<'_>,
    ) {
        let runtime = self.clone();
        let owned_request = request.to_owned_request();

        tokio::spawn(async move {
            HOOK_STACK_ACTIVE
                .scope(true, async move {
                    let request = owned_request.as_request();
                    let result = runtime.run_handler(&source, &handler, &request).await;
                    if let Err(error) = persist_hook_result(&request, &source, &handler, &result) {
                        tracing::warn!("{error}");
                    }
                })
                .await;
        });
    }

    async fn run_handler(
        &self,
        source: &str,
        handler: &HookHandler,
        request: &HookRunRequest<'_>,
    ) -> HookHandlerResult {
        let started_at = utc_timestamp();
        let input = hook_input_json(source, handler, request);
        let input_json = match serde_json::to_string(&input) {
            Ok(value) => value,
            Err(source) => {
                return HookHandlerResult::error(
                    started_at,
                    format!("failed to serialize hook input: {source}"),
                );
            }
        };
        let timeout_ms = handler.timeout.unwrap_or(DEFAULT_HOOK_TIMEOUT_MS);
        let execution = match handler.handler_type.as_str() {
            HOOK_HANDLER_COMMAND => {
                run_command_hook(handler, request.workspace_path, &input_json, timeout_ms).await
            }
            HOOK_HANDLER_HTTP => run_http_hook(handler, &input, timeout_ms).await,
            HOOK_HANDLER_MCP_TOOL => {
                run_mcp_hook(&self.mcp_registry, handler, request, input, timeout_ms).await
            }
            HOOK_HANDLER_PROMPT => run_prompt_hook(handler, request, input, timeout_ms).await,
            other => Err(format!("unsupported hook handler type: {other}")),
        };

        let completed_at = utc_timestamp();
        match execution {
            Ok(execution) => parse_hook_execution(started_at, completed_at, execution),
            Err(error) => HookHandlerResult::error_completed(started_at, completed_at, error),
        }
    }
}

pub struct HookRunRequest<'a> {
    pub global_config: &'a HookConfig,
    pub workspace_id: &'a str,
    pub workspace_path: &'a Path,
    pub event: &'a str,
    pub match_value: Option<String>,
    pub chat_id: Option<&'a str>,
    pub run_id: Option<&'a str>,
    pub session_id: Option<&'a str>,
    pub tool_call_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub provider_id: Option<&'a str>,
    pub provider_config: Option<&'a ProviderConnectionConfig>,
    pub permission_mode: Option<&'a str>,
    pub payload: Value,
}

struct OwnedHookRunRequest {
    global_config: HookConfig,
    workspace_id: String,
    workspace_path: PathBuf,
    event: String,
    match_value: Option<String>,
    chat_id: Option<String>,
    run_id: Option<String>,
    session_id: Option<String>,
    tool_call_id: Option<String>,
    model_id: Option<String>,
    provider_id: Option<String>,
    provider_config: Option<ProviderConnectionConfig>,
    permission_mode: Option<String>,
    payload: Value,
}

impl<'a> HookRunRequest<'a> {
    fn to_owned_request(&self) -> OwnedHookRunRequest {
        OwnedHookRunRequest {
            global_config: (*self.global_config).clone(),
            workspace_id: self.workspace_id.to_string(),
            workspace_path: self.workspace_path.to_path_buf(),
            event: self.event.to_string(),
            match_value: self.match_value.clone(),
            chat_id: self.chat_id.map(str::to_string),
            run_id: self.run_id.map(str::to_string),
            session_id: self.session_id.map(str::to_string),
            tool_call_id: self.tool_call_id.map(str::to_string),
            model_id: self.model_id.map(str::to_string),
            provider_id: self.provider_id.map(str::to_string),
            provider_config: self.provider_config.cloned(),
            permission_mode: self.permission_mode.map(str::to_string),
            payload: self.payload.clone(),
        }
    }
}

impl OwnedHookRunRequest {
    fn as_request(&self) -> HookRunRequest<'_> {
        HookRunRequest {
            global_config: &self.global_config,
            workspace_id: &self.workspace_id,
            workspace_path: &self.workspace_path,
            event: &self.event,
            match_value: self.match_value.clone(),
            chat_id: self.chat_id.as_deref(),
            run_id: self.run_id.as_deref(),
            session_id: self.session_id.as_deref(),
            tool_call_id: self.tool_call_id.as_deref(),
            model_id: self.model_id.as_deref(),
            provider_id: self.provider_id.as_deref(),
            provider_config: self.provider_config.as_ref(),
            permission_mode: self.permission_mode.as_deref(),
            payload: self.payload.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookRunSummary {
    pub decisions: Vec<HookDecision>,
    pub hook_specific_outputs: Vec<Value>,
    pub additional_context: Vec<String>,
    pub system_messages: Vec<String>,
    pub errors: Vec<String>,
}

impl HookRunSummary {
    pub fn first_block_reason(&self) -> Option<String> {
        self.decisions.iter().find_map(|decision| match decision {
            HookDecision::Block { reason } | HookDecision::Deny { reason } => Some(reason.clone()),
            HookDecision::Ask { reason } => Some(reason.clone()),
            HookDecision::Allow => None,
        })
    }

    pub fn hook_messages(&self, event: &str) -> Vec<HookNotification> {
        let mut messages = Vec::new();

        for decision in &self.decisions {
            match decision {
                HookDecision::Allow => {}
                HookDecision::Ask { reason } => messages.push(HookNotification {
                    event: event.to_string(),
                    level: "warning".to_string(),
                    message: format!("Hook asked for permission: {reason}"),
                }),
                HookDecision::Block { reason } | HookDecision::Deny { reason } => {
                    messages.push(HookNotification {
                        event: event.to_string(),
                        level: "error".to_string(),
                        message: format!("Hook blocked {event}: {reason}"),
                    });
                }
            }
        }

        for message in &self.system_messages {
            messages.push(HookNotification {
                event: event.to_string(),
                level: "info".to_string(),
                message: message.clone(),
            });
        }

        for error in &self.errors {
            messages.push(HookNotification {
                event: event.to_string(),
                level: "warning".to_string(),
                message: error.clone(),
            });
        }

        messages
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum HookDecision {
    Allow,
    Ask { reason: String },
    Block { reason: String },
    Deny { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookNotification {
    pub event: String,
    pub level: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveHookSummary {
    pub source: String,
    pub event: String,
    pub matcher: Option<String>,
    pub handler_type: String,
    pub command: Option<String>,
    pub url: Option<String>,
    pub server_id: Option<String>,
    pub tool_name: Option<String>,
    pub async_hook: bool,
    pub status_message: Option<String>,
}

pub fn effective_hook_summaries(
    global_config: &HookConfig,
    workspace_path: &Path,
) -> Result<Vec<EffectiveHookSummary>, foco_store::config::ConfigError> {
    let effective = effective_hooks(global_config, workspace_path)?;
    let mut summaries = Vec::new();

    for source in effective.sources {
        if source.config.disable_all_hooks {
            continue;
        }

        for (event, groups) in &source.config.hooks {
            for group in groups {
                if !group.enabled {
                    continue;
                }
                for handler in &group.hooks {
                    if !handler.enabled {
                        continue;
                    }
                    summaries.push(EffectiveHookSummary {
                        source: source.source.clone(),
                        event: event.clone(),
                        matcher: group.matcher.clone(),
                        handler_type: handler.handler_type.clone(),
                        command: handler.command.clone(),
                        url: handler.url.clone(),
                        server_id: handler.server_id.clone(),
                        tool_name: handler.tool_name.clone(),
                        async_hook: handler.async_hook,
                        status_message: handler.status_message.clone(),
                    });
                }
            }
        }
    }

    Ok(summaries)
}

struct EffectiveHooks {
    sources: Vec<HookConfigSource>,
}

struct HookConfigSource {
    source: String,
    config: HookConfig,
}

fn effective_hooks(
    global_config: &HookConfig,
    workspace_path: &Path,
) -> Result<EffectiveHooks, foco_store::config::ConfigError> {
    Ok(EffectiveHooks {
        sources: vec![
            HookConfigSource {
                source: "global".to_string(),
                config: global_config.clone(),
            },
            HookConfigSource {
                source: "workspace".to_string(),
                config: load_workspace_hook_config(workspace_path)?,
            },
        ],
    })
}

fn matcher_matches(matcher: Option<&str>, value: Option<&str>) -> bool {
    let matcher = matcher.map(str::trim).filter(|value| !value.is_empty());
    let Some(matcher) = matcher else {
        return true;
    };
    if matcher == "*" {
        return true;
    }

    let value = value.unwrap_or_default();
    if matcher.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character == '_'
            || character == '-'
            || character == '|'
    }) {
        return matcher.split('|').any(|part| part == value);
    }

    Regex::new(matcher)
        .map(|regex| regex.is_match(value))
        .unwrap_or(false)
}

fn if_filter_matches(filter: Option<&str>, request: &HookRunRequest<'_>) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let Some((tool_name, pattern)) = filter
        .strip_suffix(')')
        .and_then(|value| value.split_once('('))
    else {
        return false;
    };
    let Some(match_value) = request.match_value.as_deref() else {
        return false;
    };
    if tool_name != match_value {
        return false;
    }

    let command_text = request
        .payload
        .get("toolInput")
        .or_else(|| request.payload.get("tool_input"))
        .and_then(|input| input.get("command").or_else(|| input.get("path")))
        .and_then(Value::as_str)
        .unwrap_or_default();

    wildcard_matches(pattern.trim(), command_text)
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix.trim_end());
    }
    pattern == value
}

async fn run_command_hook(
    handler: &HookHandler,
    workspace_path: &Path,
    input_json: &str,
    timeout_ms: u64,
) -> Result<HookExecution, String> {
    let command = handler
        .command
        .as_deref()
        .ok_or_else(|| "command hook is missing command".to_string())?;
    let mut child = if handler.args.is_empty() {
        let shell = handler
            .shell
            .as_deref()
            .unwrap_or(if cfg!(windows) { "cmd" } else { "sh" });
        let mut command_process = Command::new(shell);
        if cfg!(windows) && shell.eq_ignore_ascii_case("cmd") {
            command_process.arg("/C").arg(command);
        } else {
            command_process.arg("-c").arg(command);
        }
        command_process
    } else {
        let mut command_process = Command::new(command);
        command_process.args(&handler.args);
        command_process
    };

    child
        .current_dir(workspace_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child
        .spawn()
        .map_err(|source| format!("failed to start hook command '{command}': {source}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .await
            .map_err(|source| format!("failed to write hook stdin: {source}"))?;
    }

    let output = timeout(Duration::from_millis(timeout_ms), child.wait_with_output())
        .await
        .map_err(|_| format!("hook command timed out after {timeout_ms} ms"))?
        .map_err(|source| format!("failed to wait for hook command: {source}"))?;

    Ok(HookExecution {
        exit_code: output.status.code().map(i64::from),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        output_json: None,
        is_error: !output.status.success(),
    })
}

async fn run_http_hook(
    handler: &HookHandler,
    input: &Value,
    timeout_ms: u64,
) -> Result<HookExecution, String> {
    let url = handler
        .url
        .as_deref()
        .ok_or_else(|| "http hook is missing url".to_string())?;
    let client = reqwest::Client::new();
    let response = timeout(
        Duration::from_millis(timeout_ms),
        client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(input.to_string())
            .send(),
    )
    .await
    .map_err(|_| format!("http hook timed out after {timeout_ms} ms"))?
    .map_err(|source| format!("http hook request failed: {source}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|source| format!("failed to read http hook response: {source}"))?;

    Ok(HookExecution {
        exit_code: Some(i64::from(status.as_u16())),
        stdout: text.clone(),
        stderr: String::new(),
        output_json: if status.is_success() {
            serde_json::from_str(&text).ok()
        } else {
            None
        },
        is_error: !status.is_success(),
    })
}

async fn run_mcp_hook(
    registry: &Arc<McpRegistry>,
    handler: &HookHandler,
    request: &HookRunRequest<'_>,
    input: Value,
    timeout_ms: u64,
) -> Result<HookExecution, String> {
    let server_id = handler
        .server_id
        .as_deref()
        .ok_or_else(|| "mcp_tool hook is missing serverId".to_string())?;
    let tool_name = handler
        .tool_name
        .as_deref()
        .ok_or_else(|| "mcp_tool hook is missing toolName".to_string())?;
    let encoded_tool_name =
        encode_mcp_tool_name(server_id, tool_name).map_err(|source| source.to_string())?;
    let arguments = handler.input.clone().unwrap_or(input);
    let execution = timeout(
        Duration::from_millis(timeout_ms),
        registry.execute_tool(request.workspace_id, &encoded_tool_name, arguments),
    )
    .await
    .map_err(|_| format!("mcp_tool hook timed out after {timeout_ms} ms"))?
    .map_err(|source| source.to_string())?;

    Ok(HookExecution {
        exit_code: None,
        stdout: serde_json::to_string(&execution.output).unwrap_or_default(),
        stderr: String::new(),
        output_json: Some(execution.output),
        is_error: execution.is_error,
    })
}

async fn run_prompt_hook(
    handler: &HookHandler,
    request: &HookRunRequest<'_>,
    input: Value,
    timeout_ms: u64,
) -> Result<HookExecution, String> {
    let prompt = handler
        .prompt
        .as_deref()
        .ok_or_else(|| "prompt hook is missing prompt".to_string())?;
    let provider_config = request
        .provider_config
        .ok_or_else(|| "prompt hook requires an active provider".to_string())?;
    let model_id = request
        .model_id
        .ok_or_else(|| "prompt hook requires an active model".to_string())?;
    let input_json = serde_json::to_string_pretty(&input)
        .map_err(|source| format!("failed to serialize prompt hook input: {source}"))?;
    let hook_request = NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            NeutralChatMessage {
                role: NeutralChatRole::System,
                content: format!("{prompt}\n\nReturn only a JSON hook result. Do not call tools."),
                attachments: Vec::new(),
                reasoning: None,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            },
            NeutralChatMessage {
                role: NeutralChatRole::User,
                content: input_json,
                attachments: Vec::new(),
                reasoning: None,
                tool_calls: Vec::new(),
                tool_call_id: None,
                tool_name: None,
            },
        ],
        tools: Vec::new(),
        thinking_level: None,
        max_output_tokens: Some(1024),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    };
    let mut audited_stream =
        audited_prompt_hook_stream(provider_config, hook_request, request, timeout_ms).await?;
    let mut first_token_at = None;
    let mut first_token_latency_ms = None;
    let mut usage = None;
    let mut output = String::new();

    loop {
        let Some(event) = timeout(
            Duration::from_millis(timeout_ms),
            audited_stream.stream.next_event(),
        )
        .await
        .map_err(|_| format!("prompt hook timed out after {timeout_ms} ms"))
        .map_err(|message| {
            let _ = audited_stream.fail(&message);
            message
        })?
        else {
            break;
        };
        let event = event
            .map_err(|source| format!("prompt hook stream failed: {source}"))
            .map_err(|message| {
                let _ = audited_stream.fail(&message);
                message
            })?;
        audited_stream.events.push(event.clone());
        match event {
            NeutralChatStreamEvent::Start => {}
            NeutralChatStreamEvent::ReasoningDelta { .. }
            | NeutralChatStreamEvent::ThoughtSignatureDelta { .. } => {
                capture_prompt_hook_first_token(
                    audited_stream.started_at,
                    &mut first_token_at,
                    &mut first_token_latency_ms,
                );
            }
            NeutralChatStreamEvent::Usage { usage: event_usage } => {
                usage = Some(event_usage);
            }
            NeutralChatStreamEvent::TextDelta { delta } => {
                capture_prompt_hook_first_token(
                    audited_stream.started_at,
                    &mut first_token_at,
                    &mut first_token_latency_ms,
                );
                output.push_str(&delta);
            }
            NeutralChatStreamEvent::ToolCall { tool_call } => {
                let message = format!(
                    "prompt hook attempted unsupported tool call '{}'",
                    tool_call.name
                );
                let _ = audited_stream.fail(&message);
                return Err(message);
            }
            NeutralChatStreamEvent::Complete {
                text,
                tool_calls,
                usage: complete_usage,
                ..
            } => {
                if !tool_calls.is_empty() {
                    let message = "prompt hook completed with unsupported tool calls".to_string();
                    let _ = audited_stream.fail(&message);
                    return Err(message);
                }
                if output.trim().is_empty() {
                    output = text;
                }
                if let Some(complete_usage) = complete_usage {
                    usage = Some(complete_usage);
                }
                break;
            }
            NeutralChatStreamEvent::Error { message } => {
                let message = format!("prompt hook stream error: {message}");
                let _ = audited_stream.fail(&message);
                return Err(message);
            }
        }
    }
    audited_stream.complete(first_token_at, first_token_latency_ms, usage, &output)?;

    Ok(HookExecution {
        exit_code: Some(0),
        stdout: output.clone(),
        stderr: String::new(),
        output_json: serde_json::from_str(&output).ok(),
        is_error: false,
    })
}

struct AuditedPromptHookStream {
    stream: NeutralChatStream,
    workspace_path: PathBuf,
    request_id: String,
    started_at: Instant,
    events: Vec<NeutralChatStreamEvent>,
}

impl AuditedPromptHookStream {
    fn complete(
        &self,
        first_token_at: Option<String>,
        first_token_latency_ms: Option<i64>,
        usage: Option<NeutralUsage>,
        output: &str,
    ) -> Result<(), String> {
        let mut database =
            WorkspaceDatabase::open_or_create(&self.workspace_path).map_err(|source| {
                format!("failed to open workspace database for prompt hook audit: {source}")
            })?;
        database
            .update_llm_request_outcome(
                &self.request_id,
                UpdateLlmRequestOutcome {
                    first_token_at: first_token_at.as_deref(),
                    completed_at: Some(&utc_timestamp()),
                    input_tokens: usage.as_ref().and_then(|usage| usage.input_tokens),
                    output_tokens: usage.as_ref().and_then(|usage| usage.output_tokens),
                    cache_read_tokens: usage.as_ref().and_then(|usage| usage.cache_read_tokens),
                    cache_write_tokens: usage.as_ref().and_then(|usage| usage.cache_write_tokens),
                    first_token_latency_ms,
                    total_latency_ms: Some(elapsed_millis(self.started_at)),
                    status_code: Some(200),
                    final_state: "succeeded",
                    response_body_json: Some(
                        &json!({
                            "requestKind": "prompt hook",
                            "text": output,
                            "usage": usage,
                        })
                        .to_string(),
                    ),
                },
            )
            .map_err(|source| format!("failed to update prompt hook LLM audit: {source}"))?;
        self.persist_events(&mut database)?;

        Ok(())
    }

    fn fail(&self, message: &str) -> Result<(), String> {
        fail_prompt_hook_audit(
            &self.workspace_path,
            &self.request_id,
            self.started_at,
            None,
            message,
        )
    }

    fn persist_events(&self, database: &mut WorkspaceDatabase) -> Result<(), String> {
        for (index, event) in self.events.iter().enumerate() {
            let sequence = i64::try_from(index + 1)
                .map_err(|_| "too many prompt hook LLM audit events".to_string())?;
            let event_type = hook_provider_event_type(event);
            database
                .insert_llm_request_event(NewLlmRequestEvent {
                    id: &format!("{}-event-{sequence}", self.request_id),
                    llm_request_id: &self.request_id,
                    sequence,
                    event_at: &utc_timestamp(),
                    event_type,
                    raw_chunk_json: None,
                    normalized_event_json: &serde_json::to_string(event).map_err(|source| {
                        format!("failed to serialize prompt hook LLM audit event: {source}")
                    })?,
                })
                .map_err(|source| {
                    format!("failed to insert prompt hook LLM audit event: {source}")
                })?;
        }

        Ok(())
    }
}

async fn audited_prompt_hook_stream(
    provider_config: &ProviderConnectionConfig,
    hook_request: NeutralChatRequest,
    request: &HookRunRequest<'_>,
    timeout_ms: u64,
) -> Result<AuditedPromptHookStream, String> {
    let workspace_id = request.workspace_id;
    let provider_id = request
        .provider_id
        .ok_or_else(|| "prompt hook requires an active provider".to_string())?;
    let request_id = format!("llm-hook-{}", uuid_suffix());
    let request_started_at = utc_timestamp();
    let request_body_json = serialize_provider_request_body(provider_config.kind, &hook_request)
        .map_err(|source| format!("failed to serialize prompt hook provider request: {source}"))?;
    let mut database =
        WorkspaceDatabase::open_or_create(request.workspace_path).map_err(|source| {
            format!("failed to open workspace database for prompt hook audit: {source}")
        })?;
    database
        .insert_llm_request(NewLlmRequest {
            id: &request_id,
            workspace_id,
            chat_id: request.chat_id,
            provider_id,
            model_id: &hook_request.model_id,
            request_started_at: &request_started_at,
            first_token_at: None,
            completed_at: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            first_token_latency_ms: None,
            total_latency_ms: None,
            status_code: None,
            final_state: "running",
            request_body_json: Some(&request_body_json),
            response_body_json: None,
        })
        .map_err(|source| format!("failed to insert prompt hook LLM audit: {source}"))?;
    database
        .insert_llm_request_event(NewLlmRequestEvent {
            id: &format!("{request_id}-event-0"),
            llm_request_id: &request_id,
            sequence: 0,
            event_at: &request_started_at,
            event_type: "start",
            raw_chunk_json: None,
            normalized_event_json: &json!({
                "type": "start",
                "requestKind": "prompt hook",
                "llmRequestId": &request_id,
                "workspaceId": workspace_id,
                "chatId": request.chat_id,
                "runId": request.run_id,
                "event": request.event,
            })
            .to_string(),
        })
        .map_err(|source| format!("failed to insert prompt hook LLM audit event: {source}"))?;
    drop(database);

    let started_at = std::time::Instant::now();
    match timeout(
        Duration::from_millis(timeout_ms),
        stream_chat(provider_config, hook_request),
    )
    .await
    {
        Ok(Ok(stream)) => Ok(AuditedPromptHookStream {
            stream,
            workspace_path: request.workspace_path.to_path_buf(),
            request_id,
            started_at,
            events: Vec::new(),
        }),
        Ok(Err(source)) => {
            fail_prompt_hook_audit(
                request.workspace_path,
                &request_id,
                started_at,
                source.status_code().map(i64::from),
                &format!("prompt hook provider call failed: {source}"),
            )?;
            Err(format!("prompt hook provider call failed: {source}"))
        }
        Err(_) => {
            let message = format!("prompt hook timed out after {timeout_ms} ms");
            fail_prompt_hook_audit(
                request.workspace_path,
                &request_id,
                started_at,
                None,
                &message,
            )?;
            Err(message)
        }
    }
}

fn hook_provider_event_type(event: &NeutralChatStreamEvent) -> &'static str {
    match event {
        NeutralChatStreamEvent::Start => "start",
        NeutralChatStreamEvent::TextDelta { .. } => "text_delta",
        NeutralChatStreamEvent::ReasoningDelta { .. } => "reasoning_delta",
        NeutralChatStreamEvent::ThoughtSignatureDelta { .. } => "thought_signature_delta",
        NeutralChatStreamEvent::ToolCall { .. } => "tool_call",
        NeutralChatStreamEvent::Usage { .. } => "usage",
        NeutralChatStreamEvent::Complete { .. } => "completion",
        NeutralChatStreamEvent::Error { .. } => "error",
    }
}

fn capture_prompt_hook_first_token(
    started_at: Instant,
    first_token_at: &mut Option<String>,
    first_token_latency_ms: &mut Option<i64>,
) {
    if first_token_at.is_none() {
        *first_token_at = Some(utc_timestamp());
        *first_token_latency_ms = Some(elapsed_millis(started_at));
    }
}

fn elapsed_millis(started_at: Instant) -> i64 {
    i64::try_from(started_at.elapsed().as_millis())
        .expect("request latency should fit in i64 milliseconds")
}

fn fail_prompt_hook_audit(
    workspace_path: &Path,
    request_id: &str,
    started_at: std::time::Instant,
    status_code: Option<i64>,
    message: &str,
) -> Result<(), String> {
    let mut database = WorkspaceDatabase::open_or_create(workspace_path).map_err(|source| {
        format!("failed to open workspace database for prompt hook audit: {source}")
    })?;
    database
        .update_llm_request_outcome(
            request_id,
            UpdateLlmRequestOutcome {
                first_token_at: None,
                completed_at: Some(&utc_timestamp()),
                input_tokens: None,
                output_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                first_token_latency_ms: None,
                total_latency_ms: Some(elapsed_millis(started_at)),
                status_code: status_code.filter(|code| *code > 0),
                final_state: "failed",
                response_body_json: Some(&json!({ "error": message }).to_string()),
            },
        )
        .map_err(|source| format!("failed to update prompt hook LLM audit: {source}"))?;

    Ok(())
}

fn hook_input_json(source: &str, handler: &HookHandler, request: &HookRunRequest<'_>) -> Value {
    json!({
        "session_id": request.session_id,
        "cwd": request.workspace_path.display().to_string(),
        "hook_event_name": request.event,
        "workspaceId": request.workspace_id,
        "chatId": request.chat_id,
        "runId": request.run_id,
        "modelId": request.model_id,
        "providerId": request.provider_id,
        "permissionMode": request.permission_mode,
        "toolCallId": request.tool_call_id,
        "source": source,
        "handlerType": handler.handler_type,
        "payload": request.payload,
    })
}

tokio::task_local! {
    static HOOK_STACK_ACTIVE: bool;
}

fn hook_handler_key(event: &str, matcher: Option<&str>, handler: &HookHandler) -> String {
    let handler_json = serde_json::to_string(handler).unwrap_or_default();
    format!("{event}\n{}\n{handler_json}", matcher.unwrap_or_default())
}

#[derive(Debug)]
struct HookExecution {
    exit_code: Option<i64>,
    stdout: String,
    stderr: String,
    output_json: Option<Value>,
    is_error: bool,
}

struct HookHandlerResult {
    started_at: String,
    completed_at: String,
    status: String,
    exit_code: Option<i64>,
    stdout: String,
    stderr: String,
    output_json: Option<Value>,
    decision: Option<HookDecision>,
    hook_specific_output: Option<Value>,
    additional_context: Option<String>,
    system_message: Option<String>,
    error: Option<String>,
}

impl HookHandlerResult {
    fn error(started_at: String, error: String) -> Self {
        Self::error_completed(started_at, utc_timestamp(), error)
    }

    fn error_completed(started_at: String, completed_at: String, error: String) -> Self {
        Self {
            started_at,
            completed_at,
            status: "error".to_string(),
            exit_code: None,
            stdout: String::new(),
            stderr: error.clone(),
            output_json: Some(json!({ "error": error })),
            decision: None,
            hook_specific_output: None,
            additional_context: None,
            system_message: None,
            error: Some(error),
        }
    }
}

fn parse_hook_execution(
    started_at: String,
    completed_at: String,
    execution: HookExecution,
) -> HookHandlerResult {
    let parsed_json = execution
        .output_json
        .clone()
        .or_else(|| serde_json::from_str::<Value>(&execution.stdout).ok());
    let mut error = None;
    let decision = parsed_json
        .as_ref()
        .and_then(|value| match parse_hook_decision(value) {
            Ok(decision) => decision,
            Err(parse_error) => {
                error = Some(parse_error);
                None
            }
        });
    let additional_context = parsed_json
        .as_ref()
        .and_then(|value| {
            value.get("additionalContext").or_else(|| {
                value
                    .get("hookSpecificOutput")
                    .and_then(|hook_output| hook_output.get("additionalContext"))
            })
        })
        .and_then(Value::as_str)
        .map(str::to_string);
    let hook_specific_output = parsed_json
        .as_ref()
        .and_then(|value| value.get("hookSpecificOutput"))
        .cloned();
    let system_message = parsed_json
        .as_ref()
        .and_then(|value| value.get("systemMessage"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let exit_blocks = execution.exit_code == Some(2);
    let decision = decision.or_else(|| {
        if exit_blocks {
            Some(HookDecision::Block {
                reason: non_empty_string(&execution.stderr)
                    .or_else(|| non_empty_string(&execution.stdout))
                    .unwrap_or_else(|| "hook blocked this action".to_string()),
            })
        } else {
            None
        }
    });
    let status = if decision.is_some() {
        "blocked"
    } else if execution.is_error {
        "error"
    } else {
        "succeeded"
    };
    let error = error.or_else(|| {
        if execution.is_error && decision.is_none() {
            Some(
                non_empty_string(&execution.stderr)
                    .or_else(|| non_empty_string(&execution.stdout))
                    .unwrap_or_else(|| "hook failed".to_string()),
            )
        } else {
            None
        }
    });

    HookHandlerResult {
        started_at,
        completed_at,
        status: status.to_string(),
        exit_code: execution.exit_code,
        stdout: execution.stdout,
        stderr: execution.stderr,
        output_json: parsed_json,
        decision,
        hook_specific_output,
        additional_context,
        system_message,
        error,
    }
}

fn parse_hook_decision(value: &Value) -> Result<Option<HookDecision>, String> {
    if let Some(decision) = value.get("decision").and_then(Value::as_str)
        && decision == "block"
    {
        return Ok(Some(HookDecision::Block {
            reason: decision_reason(value),
        }));
    }

    let Some(hook_output) = value.get("hookSpecificOutput") else {
        return Ok(None);
    };
    let permission_decision = hook_output
        .get("permissionDecision")
        .and_then(Value::as_str)
        .or_else(|| hook_output.get("decision").and_then(Value::as_str))
        .or_else(|| {
            hook_output
                .get("decision")
                .and_then(|decision| decision.get("behavior"))
                .and_then(Value::as_str)
        });
    let Some(permission_decision) = permission_decision else {
        return Ok(None);
    };
    let reason = hook_output
        .get("permissionDecisionReason")
        .or_else(|| hook_output.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or("hook returned a permission decision")
        .to_string();

    match permission_decision {
        "allow" | "approve" | "approved" | "continue" => Ok(Some(HookDecision::Allow)),
        "ask" | "prompt" => Ok(Some(HookDecision::Ask { reason })),
        "deny" | "denied" => Ok(Some(HookDecision::Deny { reason })),
        "block" | "blocked" => Ok(Some(HookDecision::Block { reason })),
        other => Err(format!(
            "unsupported hookSpecificOutput permission decision '{other}'"
        )),
    }
}

fn decision_reason(value: &Value) -> String {
    value
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("hook blocked this action")
        .to_string()
}

fn persist_hook_result(
    request: &HookRunRequest<'_>,
    source: &str,
    handler: &HookHandler,
    result: &HookHandlerResult,
) -> Result<(), String> {
    if !request.global_config.audit_enabled {
        return Ok(());
    }

    let mut database = WorkspaceDatabase::open_or_create(request.workspace_path)
        .map_err(|source| source.to_string())?;
    let output_json = result
        .output_json
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|source| format!("failed to serialize hook output: {source}"))?;
    let input_json = serde_json::to_string(&hook_input_json(source, handler, request))
        .map_err(|source| format!("failed to serialize hook input: {source}"))?;
    let hook_run_id = format!("hook-{}", uuid_suffix());
    let stdout_preview = non_empty_string(&redacted_preview(&result.stdout));
    let stderr_preview = non_empty_string(&redacted_preview(&result.stderr));
    database
        .insert_hook_run(NewHookRun {
            id: &hook_run_id,
            workspace_id: request.workspace_id,
            chat_id: request.chat_id,
            run_id: request.run_id,
            tool_call_id: request.tool_call_id,
            event: request.event,
            hook_source: source,
            handler_type: &handler.handler_type,
            input_json: &input_json,
            output_json: output_json.as_deref(),
            status: &result.status,
            exit_code: result.exit_code,
            stdout_preview: stdout_preview.as_deref(),
            stderr_preview: stderr_preview.as_deref(),
            started_at: &result.started_at,
            completed_at: &result.completed_at,
        })
        .map_err(|source| source.to_string())
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn redacted_preview(value: &str) -> String {
    value
        .lines()
        .map(redact_secret_text_line)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(HOOK_OUTPUT_PREVIEW_CHARS)
        .collect()
}

fn redact_secret_text_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("authorization")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("api-key")
        || lower.contains("cookie")
        || lower.contains("password")
    {
        "[REDACTED]".to_string()
    } else {
        line.to_string()
    }
}

fn utc_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn uuid_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_HOOK_ID: AtomicU64 = AtomicU64::new(1);
    format!(
        "{}-{}",
        chrono::Utc::now().timestamp_millis(),
        NEXT_HOOK_ID.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_store::config::{HOOK_HANDLER_COMMAND, HOOK_HANDLER_HTTP, HookMatcherGroup};
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    fn hook_request<'a>(
        global_config: &'a HookConfig,
        workspace_path: &'a Path,
        event: &'a str,
        payload: Value,
    ) -> HookRunRequest<'a> {
        HookRunRequest {
            global_config,
            workspace_id: "workspace-1",
            workspace_path,
            event,
            match_value: Some("run_command".to_string()),
            chat_id: Some("chat-1"),
            run_id: Some("run-1"),
            session_id: Some("chat-1"),
            tool_call_id: Some("tool-1"),
            model_id: Some("model-1"),
            provider_id: Some("provider-1"),
            provider_config: None,
            permission_mode: None,
            payload,
        }
    }

    fn command_handler(command: impl Into<String>) -> HookHandler {
        HookHandler {
            enabled: true,
            handler_type: HOOK_HANDLER_COMMAND.to_string(),
            if_filter: None,
            command: Some(command.into()),
            args: Vec::new(),
            shell: None,
            url: None,
            server_id: None,
            tool_name: None,
            prompt: None,
            timeout: None,
            async_hook: false,
            async_rewake: false,
            status_message: None,
            input: None,
        }
    }

    #[test]
    fn matcher_and_if_filter_follow_hook_rules() {
        assert!(matcher_matches(None, None));
        assert!(matcher_matches(Some("*"), Some("write_file")));
        assert!(matcher_matches(
            Some("read_file|write_file"),
            Some("write_file")
        ));
        assert!(!matcher_matches(
            Some("read_file|write_file"),
            Some("run_command")
        ));
        assert!(matcher_matches(Some("run_.*"), Some("run_command")));

        let config = HookConfig::default();
        let workspace = Path::new(".");
        let request = hook_request(
            &config,
            workspace,
            "PreToolUse",
            json!({ "toolInput": { "command": "git status --short" } }),
        );

        assert!(if_filter_matches(Some("run_command(git *)"), &request));
        assert!(!if_filter_matches(Some("run_command(cargo *)"), &request));
        assert!(!if_filter_matches(Some("malformed"), &request));
    }

    #[test]
    fn hook_output_parses_specific_fields_and_nested_context() {
        let execution = HookExecution {
            exit_code: Some(0),
            stdout: json!({
                "hookSpecificOutput": {
                    "permissionDecision": "ask",
                    "permissionDecisionReason": "needs approval",
                    "additionalContext": "temporary context",
                    "updatedInput": { "command": "git status" }
                }
            })
            .to_string(),
            stderr: String::new(),
            output_json: None,
            is_error: false,
        };
        let result = parse_hook_execution("start".to_string(), "done".to_string(), execution);

        assert_eq!(
            result.decision,
            Some(HookDecision::Ask {
                reason: "needs approval".to_string()
            })
        );
        assert_eq!(
            result.additional_context.as_deref(),
            Some("temporary context")
        );
        assert_eq!(
            result.hook_specific_output,
            Some(json!({
                "permissionDecision": "ask",
                "permissionDecisionReason": "needs approval",
                "additionalContext": "temporary context",
                "updatedInput": { "command": "git status" }
            }))
        );

        let nested = parse_hook_decision(&json!({
            "hookSpecificOutput": {
                "decision": { "behavior": "deny" },
                "reason": "policy"
            }
        }))
        .expect("nested decision parses");
        assert_eq!(
            nested,
            Some(HookDecision::Deny {
                reason: "policy".to_string()
            })
        );
    }

    #[tokio::test]
    async fn command_hook_uses_shell_form_direct_args_and_timeout() {
        let workspace = TempDir::new().expect("workspace");
        let shell_handler = command_handler(if cfg!(windows) {
            "more > nul && echo {\"systemMessage\":\"ok\"}"
        } else {
            "cat >/dev/null && printf '{\"systemMessage\":\"ok\"}'"
        });
        let shell_result = run_command_hook(
            &shell_handler,
            workspace.path(),
            "{\"hello\":\"world\"}",
            5_000,
        )
        .await
        .expect("shell hook");
        assert_eq!(shell_result.exit_code, Some(0));
        assert!(shell_result.stdout.contains("systemMessage"));
        assert!(shell_result.stdout.contains("ok"));

        let mut direct_handler = command_handler(if cfg!(windows) { "cmd" } else { "sh" });
        direct_handler.args = if cfg!(windows) {
            vec!["/C".to_string(), "echo direct".to_string()]
        } else {
            vec!["-c".to_string(), "printf direct".to_string()]
        };
        let direct_result = run_command_hook(&direct_handler, workspace.path(), "{}", 5_000)
            .await
            .expect("direct hook");
        assert_eq!(direct_result.exit_code, Some(0));
        assert!(direct_result.stdout.contains("direct"));

        let slow_handler = command_handler(if cfg!(windows) {
            "ping -n 3 127.0.0.1 > nul"
        } else {
            "sleep 1"
        });
        let timeout_error = match run_command_hook(&slow_handler, workspace.path(), "{}", 10).await
        {
            Ok(_) => panic!("slow hook should time out"),
            Err(error) => error,
        };
        assert!(timeout_error.contains("timed out"));
    }

    #[tokio::test]
    async fn http_hook_requires_successful_json_output() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let addr = listener.local_addr().expect("listener address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("request");
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer).await.expect("read request");
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: 16\r\n\r\n{\"error\":\"down\"}",
                )
                .await
                .expect("write response");
        });

        let handler = HookHandler {
            enabled: true,
            handler_type: HOOK_HANDLER_HTTP.to_string(),
            if_filter: None,
            command: None,
            args: Vec::new(),
            shell: None,
            url: Some(format!("http://{addr}/hook")),
            server_id: None,
            tool_name: None,
            prompt: None,
            timeout: Some(5_000),
            async_hook: false,
            async_rewake: false,
            status_message: None,
            input: None,
        };
        let result = run_http_hook(&handler, &json!({ "secret": "value" }), 5_000)
            .await
            .expect("http hook");
        server.await.expect("server task");

        assert_eq!(result.exit_code, Some(503));
        assert!(result.output_json.is_none());
        assert!(result.is_error);
    }

    #[test]
    fn previews_redact_secrets() {
        let redacted = redacted_preview(
            "Authorization: Bearer sk-test\npassword=secret\nsafe line\ncookie: abc",
        );

        assert!(redacted.contains("safe line"));
        assert_eq!(redacted.matches("[REDACTED]").count(), 3);
        assert!(!redacted.contains("sk-test"));
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("abc"));
    }

    #[tokio::test]
    async fn runtime_deduplicates_and_audits_async_handlers() {
        let workspace = TempDir::new().expect("workspace");
        let mut config = HookConfig {
            audit_enabled: true,
            ..HookConfig::default()
        };
        let mut handler = command_handler(if cfg!(windows) {
            "more > nul && echo {\"systemMessage\":\"async done\"}"
        } else {
            "cat >/dev/null && printf '{\"systemMessage\":\"async done\"}'"
        });
        handler.async_hook = true;
        handler.status_message = Some("async started".to_string());
        config.hooks.insert(
            "PreToolUse".to_string(),
            vec![HookMatcherGroup {
                enabled: true,
                matcher: Some("run_command".to_string()),
                hooks: vec![handler.clone(), handler],
            }],
        );

        let runtime = HookRuntime::new(Arc::new(McpRegistry::default()));
        let mut request = hook_request(
            &config,
            workspace.path(),
            "PreToolUse",
            json!({ "toolInput": { "command": "git status" } }),
        );
        request.chat_id = None;
        request.tool_call_id = None;
        let summary = runtime.run_hooks(request).await;

        assert_eq!(summary.system_messages, vec!["async started".to_string()]);
        for _ in 0..100 {
            let runs = WorkspaceDatabase::open_or_create(workspace.path())
                .expect("database")
                .hook_runs(10)
                .expect("runs");
            if runs.len() == 1 {
                assert!(
                    runs[0]
                        .stdout_preview
                        .as_deref()
                        .unwrap_or_default()
                        .contains("async done")
                );
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        panic!("async hook audit row was not persisted");
    }
}
