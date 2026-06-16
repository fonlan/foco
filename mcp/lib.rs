use std::{
    collections::{HashMap, HashSet},
    fmt,
    path::Path,
    process::Stdio,
    time::Duration,
};

#[cfg(windows)]
use process_wrap::tokio::{CommandWrap, CreationFlags, JobObject, KillOnDrop};
#[cfg(not(windows))]
use rmcp::transport::ConfigureCommandExt;
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientInfo, JsonObject},
    service::{RoleClient, RunningService},
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::time::timeout;
#[cfg(windows)]
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

const MCP_TOOL_PREFIX: &str = "mcp__";
const MCP_TOOL_SEPARATOR: &str = "__";
const CLOSE_TIMEOUT: Duration = Duration::from_secs(7);
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
                .keys()
                .filter(|server_id| !desired_ids.contains(server_id.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        };

        for server_id in stale_ids {
            self.stop_server(&server_id).await?;
        }

        for definition in definitions {
            self.sync_server(workspace_id, workspace_path, definition.clone())
                .await?;
        }

        Ok(())
    }

    pub async fn tool_definitions(&self, _workspace_id: &str) -> Vec<McpToolDefinition> {
        let servers = self.servers.lock().await;
        servers
            .values()
            .flat_map(|server| server.tools.clone())
            .collect()
    }

    pub async fn execute_tool(
        &self,
        _workspace_id: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolExecution, McpError> {
        let (server_id, original_name) = decode_mcp_tool_name(tool_name)?;
        let peer = {
            let servers = self.servers.lock().await;
            let server = servers
                .get(&server_id)
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

    pub async fn statuses(&self, _workspace_id: &str) -> Vec<McpServerStatus> {
        let servers = self.servers.lock().await;
        servers
            .values()
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
        _workspace_id: &str,
        workspace_path: &Path,
        definition: McpServerDefinition,
    ) -> Result<(), McpError> {
        let key = definition.id.clone();
        let should_restart = {
            let servers = self.servers.lock().await;
            servers
                .get(&key)
                .map(|runtime| runtime.definition != definition)
                .unwrap_or(true)
        };

        if should_restart {
            self.stop_server(&definition.id).await?;
        }

        if !definition.enabled {
            let mut servers = self.servers.lock().await;
            servers.insert(
                key,
                McpServerRuntime {
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
                        definition,
                        service: None,
                        tools: Vec::new(),
                        error: Some(error.to_string()),
                    },
                );
            }
        }
        validate_unique_tools(&self.tool_definitions("").await).map_err(|error| {
            McpError::InvalidConfig(format!(
                "MCP server '{}' produced conflicting tools: {error}",
                server_name
            ))
        })
    }

    async fn stop_server(&self, server_id: &str) -> Result<(), McpError> {
        self.stop_runtime_key(server_id).await
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

#[cfg(windows)]
fn start_stdio_transport(
    command: &str,
    args: Vec<String>,
    workspace_path: &Path,
) -> std::io::Result<TokioChildProcess> {
    // `npx`/`npm` resolve to `npx.cmd`/`npm.cmd` batch shims on Windows. Launching a
    // `.cmd`/`.bat` from a GUI-subsystem (console-less) parent always attaches a
    // `conhost.exe` console window to the outermost `cmd.exe`, regardless of
    // `CREATE_NO_WINDOW` — this is the visible window whose title shows "npm exec"
    // then the cmd.exe path. Resolve to the underlying `node.exe` + entry script so
    // no `cmd.exe` is spawned at all; fall back to the raw shim if that fails.
    let (program, args, direct_node) = resolve_consoleless_command(command, args)?;

    let mut command = CommandWrap::from(tokio::process::Command::new(program));
    command.wrap(CreationFlags(CREATE_NO_WINDOW));

    // The process-wrap `JobObject` wrapper spawns the child with `CREATE_SUSPENDED`
    // then resumes its threads. Under a Windows-Terminal-as-default-terminal host
    // that suspend/resume dance defeats `CREATE_NO_WINDOW` and surfaces a visible
    // console window. JobObject/KillOnDrop only matter for cleaning up child
    // process trees (the npx.exe → node → cmd → node chain); a resolved direct
    // node.exe entry has no child tree, so skip them and rely on tokio's
    // `kill_on_drop` instead. The shim fallback path still needs JobObject.
    if !direct_node {
        command.wrap(KillOnDrop).wrap(JobObject);
    }
    command.command_mut().args(args).current_dir(workspace_path);
    if direct_node {
        command.command_mut().kill_on_drop(true);
    }

    // rmcp's `TokioChildProcess::new` defaults `stderr` to `Stdio::inherit()`.
    // A GUI-subsystem parent (release build) has no console, so inheriting a null
    // stderr handle makes Windows allocate a fresh console for the child once it
    // writes to stderr. Piping stderr keeps `CREATE_NO_WINDOW` effective and lets
    // us surface the server's own diagnostics in the log.
    let (transport, stderr) =
        TokioChildProcess::builder(command).stderr(Stdio::piped()).spawn()?;
    if let Some(stderr) = stderr {
        drain_child_stderr(stderr);
    }

    Ok(transport)
}

/// Resolve a command so that launching it never spawns a `cmd.exe` wrapper or a
/// console-allocating `node.exe` (npx).
///
/// On Windows, `npx`/`npm` resolve to npm-generated `npx.cmd`/`npm.cmd` batch
/// shims. Launching a `.cmd`/`.bat` from a GUI-subsystem (console-less) parent
/// always attaches a visible console to its `cmd.exe`. Worse, even if the shim is
/// bypassed by running `node npx-cli.js` directly, npm's `npx-cli.js` itself
/// spawns the package via `cmd.exe /c <bin>` with `stdio: 'inherit'`, which
/// forces a console allocation on the node.exe process — a blank console window.
///
/// The only fully windowless path is to run the package's own bin entry directly
/// via `node.exe`, with no npm/npx/cmd.exe in between. This locates the package
/// in npm's npx cache (`<npm cache>/_npx/<hash>/node_modules/<pkg>`), reads its
/// `package.json` bin field, and returns `node.exe <entry> [package args...]`.
///
/// The third tuple element is `true` when the command was rewritten to a direct
/// `node.exe` invocation (no child process tree). Anything that cannot be
/// rewritten returns the resolved path unchanged with `direct_node = false`.
#[cfg(windows)]
fn resolve_consoleless_command(
    command: &str,
    args: Vec<String>,
) -> std::io::Result<(std::path::PathBuf, Vec<String>, bool)> {
    let resolved = which::which(command).map_err(|source| {
        std::io::Error::new(std::io::ErrorKind::NotFound, source.to_string())
    })?;

    let stem = resolved
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    let is_batch = resolved
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
        .unwrap_or(false);

    if is_batch && stem.as_deref() == Some("npx")
        && let Some((node, entry, pkg_args)) = resolve_npx_package_entry(&resolved, &args)?
    {
        let mut full_args = Vec::with_capacity(pkg_args.len() + 1);
        full_args.push(entry.to_string_lossy().into_owned());
        full_args.extend(pkg_args);
        return Ok((node, full_args, true));
    }

    Ok((resolved, args, false))
}

/// Resolve the `node` executable and the package bin entry that `npx <pkg>`
/// would execute, returning `(node, entry, package_args)`.
///
/// Searches npm's npx cache (`<npm cache>/_npx/*/node_modules`) for a directory
/// containing the package, reads its `package.json` `bin` field, and returns the
/// entry path plus the args destined for the package (everything after the
/// package spec, with npx-only flags like `-y` dropped).
#[cfg(windows)]
fn resolve_npx_package_entry(
    shim: &std::path::Path,
    args: &[String],
) -> std::io::Result<Option<(std::path::PathBuf, std::path::PathBuf, Vec<String>)>> {
    let Some(package_spec) = first_positional(args) else {
        return Ok(None);
    };
    let package_name = package_name(package_spec);

    let node = shim.parent().map(|dir| dir.join("node.exe"));
    let Some(node) = node.filter(|n| n.is_file()) else {
        return Ok(None);
    };

    let Some(npx_cache) = npm_npx_cache(&node)? else {
        return Ok(None);
    };

    // Find the npx cache directory that contains this package.
    let entries = match std::fs::read_dir(&npx_cache) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    for entry in entries.flatten() {
        let pkg_json = entry
            .path()
            .join("node_modules")
            .join(package_name)
            .join("package.json");
        if let Ok((dir, bin_field)) = read_package_bin(&pkg_json)
            && let Some(bin) = bin_field
            && let Some(bin_name) = bin_field_for_name(&bin, package_name)
            && {
                let bin_path = dir.join(&bin_name);
                bin_path.is_file()
            }
        {
            let bin_path = dir.join(bin_name);
            let pkg_args = npx_package_args(args);
            return Ok(Some((node, bin_path, pkg_args)));
        }
    }

    Ok(None)
}

/// Locate npm's `_npx` cache directory from `node.exe`. npm stores its cache at
/// `<cache>/_npx`. Prefer the `NPM_CONFIG_CACHE`/`npm_config_cache` env vars, then
/// the conventional Windows AppData location, then ask `npm` via a hidden node
/// subprocess.
#[cfg(windows)]
fn npm_npx_cache(node: &std::path::Path) -> std::io::Result<Option<std::path::PathBuf>> {
    use std::path::PathBuf;
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    // Prefer explicit npm cache config.
    for var in ["NPM_CONFIG_CACHE", "npm_config_cache"] {
        if let Some(cache_dir) = std::env::var_os(var) {
            let npx = PathBuf::from(cache_dir).join("_npx");
            if npx.is_dir() {
                return Ok(Some(npx));
            }
        }
    }

    // Conventional Windows location.
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let npx = PathBuf::from(local_app_data)
            .join("npm-cache")
            .join("_npx");
        if npx.is_dir() {
            return Ok(Some(npx));
        }
    }

    // Ask npm directly via a console-less node subprocess.
    let output = std::process::Command::new(node)
        .arg("-e")
        .arg("process.stdout.write(require('child_process').execSync('npm config get cache',{stdio:['ignore','pipe','ignore']}).toString().trim())")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    if output.status.success() {
        let cache = String::from_utf8_lossy(&output.stdout);
        let npx = PathBuf::from(cache.trim()).join("_npx");
        if npx.is_dir() {
            return Ok(Some(npx));
        }
    }

    Ok(None)
}

/// Read a `package.json` and return its directory and parsed `bin` field.
#[cfg(windows)]
fn read_package_bin(
    pkg_json: &std::path::Path,
) -> std::io::Result<(std::path::PathBuf, Option<serde_json::Value>)> {
    let contents = std::fs::read_to_string(pkg_json)?;
    let value: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    let dir = pkg_json
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    Ok((dir, value.get("bin").cloned()))
}

/// Given a `package.json` `bin` field (string or object), return the path for the
/// given package name. For string bins, returns the string; for object bins,
/// prefers the entry keyed by `package_name`, else the first entry.
#[cfg(windows)]
fn bin_field_for_name(bin: &serde_json::Value, package_name: &str) -> Option<String> {
    match bin {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => map
            .get(package_name)
            .or_else(|| map.values().next())
            .and_then(|v| v.as_str())
            .map(String::from),
        _ => None,
    }
}

/// The args meant for the target package: everything after the first positional
/// (the package spec), skipping `npx`-only flags that precede it (`-y`, `--yes`).
#[cfg(windows)]
fn npx_package_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut consumed_package = false;
    for arg in args {
        if !consumed_package {
            if arg.starts_with('-') {
                continue;
            }
            consumed_package = true;
            continue;
        }
        out.push(arg.clone());
    }
    out
}

#[cfg(windows)]
fn first_positional(args: &[String]) -> Option<&str> {
    args.iter().find(|a| !a.starts_with('-')).map(String::as_str)
}

/// Strip a version suffix from a package spec: `pkg@1.2.3` -> `pkg`,
/// `@scope/pkg@2` -> `@scope/pkg`.
#[cfg(windows)]
fn package_name(spec: &str) -> &str {
    if let Some(rest) = spec.strip_prefix('@') {
        match rest.find('@') {
            Some(idx) => &spec[..1 + idx],
            None => spec,
        }
    } else {
        spec.split('@').next().unwrap_or(spec)
    }
}

#[cfg(windows)]
fn drain_child_stderr(mut stderr: tokio::process::ChildStderr) {
    use tokio::io::AsyncBufReadExt;

    tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(&mut stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                tracing::debug!(target: "mcp::stdio", "stderr: {line}");
            }
        }
    });
}

#[cfg(not(windows))]
fn start_stdio_transport(
    command: &str,
    args: Vec<String>,
    workspace_path: &Path,
) -> std::io::Result<TokioChildProcess> {
    TokioChildProcess::new(
        rmcp::transport::which_command(command)?.configure(move |cmd| {
            cmd.args(args);
            cmd.current_dir(workspace_path);
        }),
    )
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
            let transport =
                start_stdio_transport(command, definition.args.clone(), workspace_path)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn disabled_stdio_server(id: &str) -> McpServerDefinition {
        McpServerDefinition {
            id: id.to_string(),
            name: id.to_string(),
            enabled: false,
            transport: McpTransportKind::Stdio,
            command: Some("foco-test-mcp".to_string()),
            args: Vec::new(),
            url: None,
        }
    }

    #[tokio::test]
    async fn sync_workspace_servers_reuses_global_server_runtime() {
        let registry = McpRegistry::default();
        let definitions = vec![disabled_stdio_server("context7")];

        registry
            .sync_workspace_servers("workspace-a", Path::new("."), &definitions)
            .await
            .expect("first workspace sync should succeed");
        registry
            .sync_workspace_servers("workspace-b", Path::new("."), &definitions)
            .await
            .expect("second workspace sync should succeed");

        let servers = registry.servers.lock().await;

        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("context7"));
    }
}
