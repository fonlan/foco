use std::{
    collections::{HashMap, HashSet},
    fmt,
    path::Path,
    time::Duration,
};

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientInfo, JsonObject},
    service::{RoleClient, RunningService},
    transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::time::timeout;

const MCP_TOOL_PREFIX: &str = "mcp__";
const MCP_TOOL_SEPARATOR: &str = "__";
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_TOOL_CALL_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpTransportKind {
    Stdio,
    StreamableHttp,
}

impl McpTransportKind {
    pub fn parse(value: &str) -> Result<Self, McpError> {
        match value.trim() {
            "stdio" => Ok(Self::Stdio),
            "streamable-http" => Ok(Self::StreamableHttp),
            other => Err(McpError::InvalidConfig(format!(
                "unsupported MCP transport '{other}'"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp => "streamable-http",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDefinition {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub transport: McpTransportKind,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    pub name: String,
    pub server_id: String,
    pub server_name: String,
    pub original_name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolExecution {
    pub output: Value,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatus {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub transport: McpTransportKind,
    pub state: McpServerState,
    pub error: Option<String>,
    pub tool_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpServerState {
    Disabled,
    Connected,
    Error,
    Stopped,
}

#[derive(Default)]
pub struct McpRegistry {
    servers: Mutex<HashMap<String, McpServerRuntime>>,
}

struct McpServerRuntime {
    workspace_id: String,
    definition: McpServerDefinition,
    service: Option<RunningService<RoleClient, ClientInfo>>,
    tools: Vec<McpToolDefinition>,
    error: Option<String>,
}

impl McpRegistry {
    pub async fn sync_workspace_servers(
        &self,
        workspace_id: &str,
        workspace_path: &Path,
        definitions: &[McpServerDefinition],
    ) -> Result<(), McpError> {
        validate_server_definitions(definitions)?;
        let desired_ids = definitions
            .iter()
            .map(|definition| definition.id.clone())
            .collect::<HashSet<_>>();
        let stale_ids = {
            let servers = self.servers.lock().await;
            servers
                .values()
                .filter(|runtime| {
                    runtime.workspace_id == workspace_id
                        && !desired_ids.contains(runtime.definition.id.as_str())
                })
                .map(|runtime| runtime.definition.id.clone())
                .collect::<Vec<_>>()
        };

        for server_id in stale_ids {
            self.stop_server(workspace_id, &server_id).await?;
        }

        for definition in definitions {
            self.sync_server(workspace_id, workspace_path, definition.clone())
                .await?;
        }

        Ok(())
    }

    pub async fn tool_definitions(&self, workspace_id: &str) -> Vec<McpToolDefinition> {
        let servers = self.servers.lock().await;
        servers
            .values()
            .filter(|server| server.workspace_id == workspace_id)
            .flat_map(|server| server.tools.clone())
            .collect()
    }

    pub async fn execute_tool(
        &self,
        workspace_id: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolExecution, McpError> {
        let (server_id, original_name) = decode_mcp_tool_name(tool_name)?;
        let peer = {
            let servers = self.servers.lock().await;
            let runtime_key = runtime_key(workspace_id, &server_id);
            let server = servers
                .get(&runtime_key)
                .ok_or_else(|| McpError::ServerNotFound(server_id.clone()))?;
            let service = server
                .service
                .as_ref()
                .ok_or_else(|| McpError::ServerNotRunning(server_id.clone()))?;

            service.peer().clone()
        };
        let request =
            CallToolRequestParams::new(original_name).with_arguments(json_object(arguments)?);
        let result = timeout(DEFAULT_TOOL_CALL_TIMEOUT, peer.call_tool(request))
            .await
            .map_err(|_| McpError::ToolTimedOut {
                tool_name: tool_name.to_string(),
                timeout_ms: duration_millis(DEFAULT_TOOL_CALL_TIMEOUT),
            })?
            .map_err(|source| McpError::Runtime(source.to_string()))?;
        let is_error = result.is_error.unwrap_or(false);
        let output = serde_json::to_value(result).map_err(|source| {
            McpError::Runtime(format!("failed to serialize MCP result: {source}"))
        })?;

        Ok(McpToolExecution { output, is_error })
    }

    pub async fn statuses(&self, workspace_id: &str) -> Vec<McpServerStatus> {
        let servers = self.servers.lock().await;
        servers
            .values()
            .filter(|server| server.workspace_id == workspace_id)
            .map(|server| {
                let state = if !server.definition.enabled {
                    McpServerState::Disabled
                } else if server.error.is_some() {
                    McpServerState::Error
                } else if server.service.is_some() {
                    McpServerState::Connected
                } else {
                    McpServerState::Stopped
                };

                McpServerStatus {
                    id: server.definition.id.clone(),
                    name: server.definition.name.clone(),
                    enabled: server.definition.enabled,
                    transport: server.definition.transport.clone(),
                    state,
                    error: server.error.clone(),
                    tool_count: server.tools.len(),
                }
            })
            .collect()
    }

    pub async fn stop_all(&self) -> Result<(), McpError> {
        let keys = {
            let servers = self.servers.lock().await;
            servers.keys().cloned().collect::<Vec<_>>()
        };

        for key in keys {
            self.stop_runtime_key(&key).await?;
        }

        Ok(())
    }

    async fn sync_server(
        &self,
        workspace_id: &str,
        workspace_path: &Path,
        definition: McpServerDefinition,
    ) -> Result<(), McpError> {
        let key = runtime_key(workspace_id, &definition.id);
        let should_restart = {
            let servers = self.servers.lock().await;
            servers
                .get(&key)
                .map(|runtime| {
                    runtime.definition != definition || runtime.workspace_id != workspace_id
                })
                .unwrap_or(true)
        };

        if should_restart {
            self.stop_server(workspace_id, &definition.id).await?;
        }

        if !definition.enabled {
            let mut servers = self.servers.lock().await;
            servers.insert(
                key,
                McpServerRuntime {
                    workspace_id: workspace_id.to_string(),
                    definition,
                    service: None,
                    tools: Vec::new(),
                    error: None,
                },
            );
            return Ok(());
        }

        let already_running = {
            let servers = self.servers.lock().await;
            servers
                .get(&key)
                .map(|runtime| runtime.service.is_some() && runtime.error.is_none())
                .unwrap_or(false)
        };

        if already_running {
            return Ok(());
        }

        let server_id = definition.id.clone();
        let server_name = definition.name.clone();
        match start_runtime(&definition, workspace_path).await {
            Ok((service, tools)) => {
                let mut servers = self.servers.lock().await;
                servers.insert(
                    key,
                    McpServerRuntime {
                        workspace_id: workspace_id.to_string(),
                        definition,
                        service: Some(service),
                        tools,
                        error: None,
                    },
                );
            }
            Err(error) => {
                tracing::warn!(server_id = %server_id, error = %error, "failed to start MCP server");
                let mut servers = self.servers.lock().await;
                servers.insert(
                    key,
                    McpServerRuntime {
                        workspace_id: workspace_id.to_string(),
                        definition,
                        service: None,
                        tools: Vec::new(),
                        error: Some(error.to_string()),
                    },
                );
            }
        }
        validate_unique_tools(&self.tool_definitions(workspace_id).await).map_err(|error| {
            McpError::InvalidConfig(format!(
                "MCP server '{}' produced conflicting tools: {error}",
                server_name
            ))
        })
    }

    async fn stop_server(&self, workspace_id: &str, server_id: &str) -> Result<(), McpError> {
        let key = runtime_key(workspace_id, server_id);
        self.stop_runtime_key(&key).await
    }

    async fn stop_runtime_key(&self, key: &str) -> Result<(), McpError> {
        let runtime = {
            let mut servers = self.servers.lock().await;
            servers.remove(key)
        };

        if let Some(mut runtime) = runtime {
            if let Some(mut service) = runtime.service.take() {
                service
                    .close_with_timeout(CLOSE_TIMEOUT)
                    .await
                    .map_err(|source| McpError::Runtime(source.to_string()))?;
            }
        }

        Ok(())
    }
}

pub fn encode_mcp_tool_name(server_id: &str, tool_name: &str) -> Result<String, McpError> {
    validate_tool_name_part("server id", server_id)?;
    validate_tool_name_part("tool name", tool_name)?;
    Ok(format!(
        "{MCP_TOOL_PREFIX}{server_id}{MCP_TOOL_SEPARATOR}{tool_name}"
    ))
}

pub fn decode_mcp_tool_name(tool_name: &str) -> Result<(String, String), McpError> {
    let rest = tool_name
        .strip_prefix(MCP_TOOL_PREFIX)
        .ok_or_else(|| McpError::InvalidToolName(tool_name.to_string()))?;
    let (server_id, original_name) = rest
        .split_once(MCP_TOOL_SEPARATOR)
        .ok_or_else(|| McpError::InvalidToolName(tool_name.to_string()))?;

    validate_tool_name_part("server id", server_id)?;
    validate_tool_name_part("tool name", original_name)?;

    Ok((server_id.to_string(), original_name.to_string()))
}

pub fn is_mcp_tool_name(tool_name: &str) -> bool {
    tool_name.starts_with(MCP_TOOL_PREFIX)
}

pub fn validate_server_definitions(definitions: &[McpServerDefinition]) -> Result<(), McpError> {
    let mut seen_ids = HashSet::new();

    for definition in definitions {
        validate_id("MCP server id", &definition.id)?;
        if !seen_ids.insert(definition.id.as_str()) {
            return Err(McpError::InvalidConfig(format!(
                "duplicate MCP server id '{}'",
                definition.id
            )));
        }

        if definition.name.trim().is_empty() {
            return Err(McpError::InvalidConfig(format!(
                "MCP server '{}' name must not be empty",
                definition.id
            )));
        }

        match definition.transport {
            McpTransportKind::Stdio => {
                require_non_empty(
                    definition.command.as_deref(),
                    format!("MCP stdio server '{}' command", definition.id),
                )?;
                if definition
                    .url
                    .as_deref()
                    .is_some_and(|url| !url.trim().is_empty())
                {
                    return Err(McpError::InvalidConfig(format!(
                        "MCP stdio server '{}' must not set url",
                        definition.id
                    )));
                }
            }
            McpTransportKind::StreamableHttp => {
                let url = require_non_empty(
                    definition.url.as_deref(),
                    format!("MCP Streamable HTTP server '{}' url", definition.id),
                )?;
                validate_http_url(url)?;
                if definition
                    .command
                    .as_deref()
                    .is_some_and(|command| !command.trim().is_empty())
                    || !definition.args.is_empty()
                {
                    return Err(McpError::InvalidConfig(format!(
                        "MCP Streamable HTTP server '{}' must not set command or args",
                        definition.id
                    )));
                }
            }
        }
    }

    Ok(())
}

async fn start_runtime(
    definition: &McpServerDefinition,
    workspace_path: &Path,
) -> Result<
    (
        RunningService<RoleClient, ClientInfo>,
        Vec<McpToolDefinition>,
    ),
    McpError,
> {
    let client_info = ClientInfo::default();
    let service = match definition.transport {
        McpTransportKind::Stdio => {
            let command = definition.command.as_deref().ok_or_else(|| {
                McpError::InvalidConfig(format!(
                    "MCP server '{}' command is missing",
                    definition.id
                ))
            })?;
            let args = definition.args.clone();
            let workspace_path = workspace_path.to_path_buf();
            let transport = TokioChildProcess::new(
                rmcp::transport::which_command(command)?.configure(move |cmd| {
                    cmd.args(args);
                    cmd.current_dir(workspace_path);
                }),
            )?;

            client_info.clone().serve(transport).await
        }
        McpTransportKind::StreamableHttp => {
            let url = definition.url.as_deref().ok_or_else(|| {
                McpError::InvalidConfig(format!("MCP server '{}' url is missing", definition.id))
            })?;
            let transport = StreamableHttpClientTransport::from_uri(url.to_string());

            client_info.clone().serve(transport).await
        }
    }
    .map_err(|source| McpError::Runtime(source.to_string()))?;
    let remote_tools = service
        .peer()
        .list_all_tools()
        .await
        .map_err(|source| McpError::Runtime(source.to_string()))?;
    let mut tools = Vec::with_capacity(remote_tools.len());

    for tool in remote_tools {
        let original_name = tool.name.to_string();
        let name = encode_mcp_tool_name(&definition.id, &original_name)?;
        let input_schema = Value::Object((*tool.input_schema).clone());
        let description = tool.description.unwrap_or_default().to_string();

        tools.push(McpToolDefinition {
            name,
            server_id: definition.id.clone(),
            server_name: definition.name.clone(),
            original_name,
            description: if description.trim().is_empty() {
                format!("MCP tool from server '{}'.", definition.name)
            } else {
                description
            },
            input_schema,
        });
    }

    validate_unique_tools(&tools)?;

    Ok((service, tools))
}

fn runtime_key(workspace_id: &str, server_id: &str) -> String {
    format!("{workspace_id}{MCP_TOOL_SEPARATOR}{server_id}")
}

fn validate_unique_tools(tools: &[McpToolDefinition]) -> Result<(), McpError> {
    let mut seen_names = HashSet::new();

    for tool in tools {
        if !seen_names.insert(tool.name.as_str()) {
            return Err(McpError::InvalidConfig(format!(
                "duplicate MCP tool name '{}'",
                tool.name
            )));
        }
    }

    Ok(())
}

fn json_object(value: Value) -> Result<JsonObject, McpError> {
    match value {
        Value::Object(object) => Ok(object),
        Value::Null => Ok(JsonObject::new()),
        other => Err(McpError::InvalidArguments(format!(
            "MCP tool arguments must be a JSON object, got {}",
            value_kind(&other)
        ))),
    }
}

fn require_non_empty<'a>(
    value: Option<&'a str>,
    field: impl Into<String>,
) -> Result<&'a str, McpError> {
    let field = field.into();
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| McpError::InvalidConfig(format!("{field} must not be empty")))?;

    Ok(value)
}

fn validate_id(field: &str, value: &str) -> Result<(), McpError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(McpError::InvalidConfig(format!(
            "{field} must not be empty"
        )));
    }

    if trimmed != value || value.chars().any(char::is_whitespace) {
        return Err(McpError::InvalidConfig(format!(
            "{field} '{value}' must not contain whitespace"
        )));
    }

    if value.contains(MCP_TOOL_SEPARATOR) {
        return Err(McpError::InvalidConfig(format!(
            "{field} '{value}' must not contain '{MCP_TOOL_SEPARATOR}'"
        )));
    }

    Ok(())
}

fn validate_tool_name_part(field: &str, value: &str) -> Result<(), McpError> {
    validate_id(field, value)?;

    if value.starts_with(MCP_TOOL_PREFIX) {
        return Err(McpError::InvalidToolName(value.to_string()));
    }

    Ok(())
}

fn validate_http_url(value: &str) -> Result<(), McpError> {
    let url = reqwest::Url::parse(value).map_err(|source| {
        McpError::InvalidConfig(format!(
            "invalid MCP Streamable HTTP url '{value}': {source}"
        ))
    })?;

    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(McpError::InvalidConfig(format!(
            "invalid MCP Streamable HTTP url '{value}': scheme must be http or https"
        )));
    }

    Ok(())
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[derive(Debug)]
pub enum McpError {
    InvalidArguments(String),
    InvalidConfig(String),
    InvalidToolName(String),
    Runtime(String),
    ServerNotFound(String),
    ServerNotRunning(String),
    ToolTimedOut { tool_name: String, timeout_ms: u64 },
    Io(std::io::Error),
}

impl fmt::Display for McpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments(message)
            | Self::InvalidConfig(message)
            | Self::Runtime(message) => formatter.write_str(message),
            Self::InvalidToolName(name) => write!(formatter, "invalid MCP tool name '{name}'"),
            Self::ServerNotFound(server_id) => {
                write!(formatter, "MCP server was not found: {server_id}")
            }
            Self::ServerNotRunning(server_id) => {
                write!(formatter, "MCP server is not running: {server_id}")
            }
            Self::ToolTimedOut {
                tool_name,
                timeout_ms,
            } => write!(
                formatter,
                "MCP tool '{tool_name}' timed out after {timeout_ms} ms"
            ),
            Self::Io(source) => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for McpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::InvalidArguments(_)
            | Self::InvalidConfig(_)
            | Self::InvalidToolName(_)
            | Self::Runtime(_)
            | Self::ServerNotFound(_)
            | Self::ServerNotRunning(_)
            | Self::ToolTimedOut { .. } => None,
        }
    }
}

impl From<std::io::Error> for McpError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source)
    }
}

impl From<reqwest::Error> for McpError {
    fn from(source: reqwest::Error) -> Self {
        Self::Runtime(source.to_string())
    }
}

pub fn crate_name() -> &'static str {
    "foco-mcp"
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}
