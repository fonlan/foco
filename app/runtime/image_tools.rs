use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use foco_providers::{
    DEFAULT_OPENAI_BASE_URL, OPENAI_CHAT_KIND, OPENAI_RESPONSES_KIND, normalized_base_url,
};
use foco_store::config::{GlobalConfig, ModelSettings, ProviderSettings};
use foco_tools::IMAGE_GEN_TOOL;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const DEFAULT_IMAGE_GEN_MODEL_ID: &str = "gpt-image-2";
const DEFAULT_IMAGE_GEN_TIMEOUT_MS: u64 = 300_000;
const MAX_IMAGE_GEN_TIMEOUT_MS: u64 = 600_000;
const MAX_IMAGE_GEN_COUNT: u8 = 4;
const IMAGE_OUTPUT_MODALITY: &str = "image";
const DEFAULT_IMAGE_OUTPUT_FORMAT: &str = "png";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageGenToolInput {
    prompt: String,
    mode: Option<String>,
    model: Option<String>,
    input_images: Option<Vec<ImageGenInputImage>>,
    mask_path: Option<String>,
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
    output_format: Option<String>,
    compression: Option<u8>,
    count: Option<u8>,
    output_dir: Option<String>,
    output_name: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageGenInputImage {
    path: String,
    description: Option<String>,
}

#[derive(Debug)]
struct SelectedImageModel<'a> {
    model: &'a ModelSettings,
    provider: &'a ProviderSettings,
}

#[derive(Debug, Serialize)]
struct OpenAiImageRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    n: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    background: Option<&'a str>,
    #[serde(rename = "output_format", skip_serializing_if = "Option::is_none")]
    output_format: Option<&'a str>,
    #[serde(rename = "output_compression", skip_serializing_if = "Option::is_none")]
    output_compression: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageResponse {
    data: Vec<OpenAiImageData>,
    usage: Option<OpenAiImageUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageData {
    b64_json: Option<String>,
    revised_prompt: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct OpenAiImageUsage {
    #[serde(default)]
    total_tokens: Option<u64>,
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}

pub(crate) fn is_image_tool_name(tool_name: &str) -> bool {
    tool_name == IMAGE_GEN_TOOL
}

pub(crate) fn image_tool_timeout_ms(arguments: &Value) -> Result<u64, String> {
    match arguments.get("timeoutMs") {
        Some(Value::Null) | None => Ok(DEFAULT_IMAGE_GEN_TIMEOUT_MS),
        Some(Value::Number(timeout_ms)) => {
            let timeout_ms = timeout_ms
                .as_u64()
                .ok_or_else(|| "timeoutMs must be an integer or null".to_string())?;
            if timeout_ms == 0 || timeout_ms > MAX_IMAGE_GEN_TIMEOUT_MS {
                Err(format!(
                    "timeoutMs must be between 1 and {MAX_IMAGE_GEN_TIMEOUT_MS} milliseconds"
                ))
            } else {
                Ok(timeout_ms)
            }
        }
        Some(_) => Err("timeoutMs must be an integer or null".to_string()),
    }
}

pub(crate) async fn execute_image_tool(
    config: &GlobalConfig,
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    tool_name: &str,
    arguments: Value,
    timeout: Duration,
) -> Result<Value, String> {
    match tool_name {
        IMAGE_GEN_TOOL => {
            let input = serde_json::from_value::<ImageGenToolInput>(arguments)
                .map_err(|source| format!("image_gen arguments do not match schema: {source}"))?;
            execute_image_gen(config, workspace_path, chat_id, run_id, input, timeout).await
        }
        _ => Err(format!("unknown image tool '{tool_name}'")),
    }
}

async fn execute_image_gen(
    config: &GlobalConfig,
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    input: ImageGenToolInput,
    timeout: Duration,
) -> Result<Value, String> {
    image_tool_timeout_ms_from_input(input.timeout_ms)?;
    let prompt = input.prompt.trim();
    if prompt.is_empty() {
        return Err("prompt must not be empty".to_string());
    }

    let mode = input.mode.as_deref().unwrap_or("generate").trim();
    if let Some(images) = &input.input_images {
        for image in images {
            if image.path.trim().is_empty() {
                return Err("inputImages[].path must not be empty".to_string());
            }
            let _ = image.description.as_deref().map(str::trim);
        }
    }
    if mode != "generate" {
        return Err("image_gen edit mode is not implemented yet".to_string());
    }
    if input
        .input_images
        .as_ref()
        .is_some_and(|images| !images.is_empty())
        || input
            .mask_path
            .as_ref()
            .is_some_and(|path| !path.trim().is_empty())
    {
        return Err(
            "image_gen inputImages and maskPath require edit mode, which is not implemented yet"
                .to_string(),
        );
    }

    let count = input.count.unwrap_or(1);
    if count == 0 || count > MAX_IMAGE_GEN_COUNT {
        return Err(format!("count must be between 1 and {MAX_IMAGE_GEN_COUNT}"));
    }

    let output_format = normalized_output_format(input.output_format.as_deref())?;
    let selected = select_image_model(config, input.model.as_deref())?;
    let api_key = selected
        .provider
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("provider '{}' is missing an API key", selected.provider.id))?;
    let base_url = openai_image_base_url(selected.provider)?;
    let endpoint = format!("{}/images/generations", base_url.trim_end_matches('/'));
    let client = image_http_client(selected.provider, timeout)?;
    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&OpenAiImageRequest {
            model: &selected.model.id,
            prompt,
            n: count,
            size: input
                .size
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            quality: input
                .quality
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            background: input
                .background
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            output_format: Some(output_format),
            output_compression: input.compression,
        })
        .send()
        .await
        .map_err(|source| format!("OpenAI image request failed: {source}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|source| format!("failed to read OpenAI image response: {source}"))?;
    if !status.is_success() {
        return Err(format_openai_status_error(status, &body));
    }

    let response = serde_json::from_str::<OpenAiImageResponse>(&body)
        .map_err(|source| format!("failed to parse OpenAI image response JSON: {source}"))?;
    if response.data.is_empty() {
        return Err("OpenAI image response did not include image data".to_string());
    }

    let output_dir =
        image_output_dir(workspace_path, chat_id, run_id, input.output_dir.as_deref())?;
    fs::create_dir_all(&output_dir)
        .map_err(|source| format!("failed to create image output directory: {source}"))?;
    ensure_workspace_child_dir(workspace_path, &output_dir)?;

    let output_name = normalized_output_name(input.output_name.as_deref());
    let mut files = Vec::new();
    let mut revised_prompt = None::<String>;
    let mut actual_output_format = None::<String>;

    for (index, image) in response.data.iter().enumerate() {
        let b64_json = image
            .b64_json
            .as_deref()
            .ok_or_else(|| "OpenAI image response did not include b64_json".to_string())?;
        let bytes = BASE64_STANDARD
            .decode(b64_json)
            .map_err(|source| format!("failed to decode generated image: {source}"))?;
        let detected_format = detect_image_format(&bytes).unwrap_or(output_format);
        actual_output_format.get_or_insert_with(|| detected_format.to_string());
        let file_name = image_file_name(
            &output_name,
            output_extension(detected_format),
            index,
            response.data.len(),
        );
        let path = output_dir.join(file_name);
        fs::write(&path, &bytes)
            .map_err(|source| format!("failed to write generated image: {source}"))?;
        let sha256 = Sha256::digest(&bytes);
        if revised_prompt.is_none() {
            revised_prompt = image.revised_prompt.clone();
        }
        files.push(json!({
            "path": workspace_relative_path(workspace_path, &path)?,
            "mimeType": output_mime_type(detected_format),
            "bytes": bytes.len(),
            "sha256": format!("{sha256:x}"),
        }));
    }

    Ok(json!({
        "provider": selected.provider.id,
        "providerKind": selected.provider.kind,
        "model": selected.model.id,
        "mode": mode,
        "prompt": prompt,
        "revisedPrompt": revised_prompt,
        "files": files,
        "size": input.size,
        "quality": input.quality.unwrap_or_else(|| "auto".to_string()),
        "background": input.background.unwrap_or_else(|| "auto".to_string()),
        "outputFormat": actual_output_format.unwrap_or_else(|| output_format.to_string()),
        "usage": response.usage,
        "warnings": Vec::<String>::new(),
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
    }))
}

fn image_tool_timeout_ms_from_input(timeout_ms: Option<u64>) -> Result<u64, String> {
    match timeout_ms {
        Some(timeout_ms) if timeout_ms > 0 && timeout_ms <= MAX_IMAGE_GEN_TIMEOUT_MS => {
            Ok(timeout_ms)
        }
        Some(_) => Err(format!(
            "timeoutMs must be between 1 and {MAX_IMAGE_GEN_TIMEOUT_MS} milliseconds"
        )),
        None => Ok(DEFAULT_IMAGE_GEN_TIMEOUT_MS),
    }
}

fn select_image_model<'a>(
    config: &'a GlobalConfig,
    requested_model: Option<&str>,
) -> Result<SelectedImageModel<'a>, String> {
    let model = match requested_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(model_id) => config
            .models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| format!("image model '{model_id}' is not configured"))?,
        None => config
            .models
            .iter()
            .find(|model| {
                model.id == DEFAULT_IMAGE_GEN_MODEL_ID && image_model_available(config, model)
            })
            .or_else(|| {
                config
                    .models
                    .iter()
                    .find(|model| image_model_available(config, model))
            })
            .ok_or_else(|| "no enabled image-output model is configured".to_string())?,
    };

    if !image_model_available(config, model) {
        return Err(format!(
            "model '{}' must be enabled, have an active enabled provider, and include image in outputModalities",
            model.id
        ));
    }

    let provider_id = model
        .active_provider_id
        .as_deref()
        .ok_or_else(|| format!("model '{}' is missing an active provider", model.id))?;
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id && provider.enabled)
        .ok_or_else(|| format!("active provider '{provider_id}' is unavailable"))?;

    if !openai_image_provider_supported(provider) {
        return Err(format!(
            "image_gen currently supports OpenAI-compatible providers only; provider '{}' uses kind '{}'",
            provider.id, provider.kind
        ));
    }

    Ok(SelectedImageModel { model, provider })
}

pub(crate) fn image_model_available(config: &GlobalConfig, model: &ModelSettings) -> bool {
    model.enabled
        && model
            .output_modalities
            .iter()
            .any(|modality| modality == IMAGE_OUTPUT_MODALITY)
        && model
            .active_provider_id
            .as_ref()
            .is_some_and(|provider_id| {
                model.provider_ids.iter().any(|id| id == provider_id)
                    && config.providers.iter().any(|provider| {
                        provider.id == *provider_id
                            && provider.enabled
                            && openai_image_provider_supported(provider)
                    })
            })
}

fn detect_image_format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpeg");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    None
}

fn output_extension(output_format: &str) -> &'static str {
    match output_format {
        "jpeg" => "jpg",
        "webp" => "webp",
        _ => "png",
    }
}

fn openai_image_provider_supported(provider: &ProviderSettings) -> bool {
    provider.kind == OPENAI_CHAT_KIND || provider.kind == OPENAI_RESPONSES_KIND
}

fn openai_image_base_url(provider: &ProviderSettings) -> Result<String, String> {
    let base_url = provider
        .base_url
        .as_deref()
        .unwrap_or(DEFAULT_OPENAI_BASE_URL);
    normalized_base_url(base_url).map_err(|source| source.to_string())
}

fn image_http_client(
    provider: &ProviderSettings,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent("Foco/0.1");
    if provider.api_proxy.enabled {
        let proxy = reqwest::Proxy::all(provider.api_proxy.url.trim())
            .map_err(|source| format!("failed to configure image_gen proxy: {source}"))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|source| format!("failed to create image_gen HTTP client: {source}"))
}

fn normalized_output_format(value: Option<&str>) -> Result<&'static str, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(DEFAULT_IMAGE_OUTPUT_FORMAT),
        Some("png") => Ok("png"),
        Some("jpeg") => Ok("jpeg"),
        Some("webp") => Ok("webp"),
        Some(other) => Err(format!("outputFormat '{other}' is unsupported")),
    }
}

fn output_mime_type(output_format: &str) -> &'static str {
    match output_format {
        "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn image_output_dir(
    workspace_path: &Path,
    chat_id: &str,
    run_id: &str,
    requested_output_dir: Option<&str>,
) -> Result<PathBuf, String> {
    match requested_output_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(path) => safe_workspace_path(workspace_path, path),
        None => Ok(workspace_path
            .join(".foco")
            .join("sessions")
            .join(safe_path_component(chat_id))
            .join("image_gen")
            .join(safe_path_component(run_id))),
    }
}

fn safe_workspace_path(workspace_path: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err("outputDir must be workspace-relative".to_string());
    }

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => safe.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("outputDir must stay inside the workspace".to_string());
            }
        }
    }
    if safe.as_os_str().is_empty() {
        return Err("outputDir must not be empty".to_string());
    }
    Ok(workspace_path.join(safe))
}

fn ensure_workspace_child_dir(workspace_path: &Path, path: &Path) -> Result<(), String> {
    let workspace = fs::canonicalize(workspace_path)
        .map_err(|source| format!("failed to resolve workspace path: {source}"))?;
    let child = fs::canonicalize(path)
        .map_err(|source| format!("failed to resolve image output directory: {source}"))?;
    if !child.starts_with(&workspace) {
        return Err("outputDir must stay inside the workspace".to_string());
    }
    Ok(())
}

fn normalized_output_name(value: Option<&str>) -> String {
    value
        .map(safe_path_component)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "image".to_string())
}

fn image_file_name(base_name: &str, extension: &str, index: usize, total: usize) -> String {
    if total == 1 {
        format!("{base_name}.{extension}")
    } else {
        format!("{base_name}-{:03}.{extension}", index + 1)
    }
}

fn safe_path_component(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '.'])
        .to_string()
}

fn workspace_relative_path(workspace_path: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(workspace_path)
        .map_err(|_| "generated image path escaped workspace".to_string())?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn format_openai_status_error(status: StatusCode, body: &str) -> String {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(500).collect());
    format!(
        "OpenAI image request failed with status {}: {message}",
        status.as_u16()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_image_output_dir_is_session_scoped() {
        let dir =
            image_output_dir(Path::new("/workspace"), "chat-123", "run/456", None).expect("dir");

        assert_eq!(
            dir,
            Path::new("/workspace")
                .join(".foco")
                .join("sessions")
                .join("chat-123")
                .join("image_gen")
                .join("run-456")
        );
    }

    #[test]
    fn rejects_output_dir_escape() {
        let error = image_output_dir(Path::new("/workspace"), "chat", "run", Some("../out"))
            .expect_err("escape should fail");
        assert!(error.contains("inside the workspace"));
    }
}
