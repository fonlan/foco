use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetadataCache {
    pub source_url: String,
    pub fetched_at: String,
    pub models: Vec<ModelMetadataRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetadataRecord {
    pub key: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model_id: String,
    pub name: String,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub pricing: ModelPricing,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub supports_tools: bool,
    pub supports_cache: bool,
    #[serde(default)]
    pub reasoning: bool,
    pub source_url: String,
    pub refreshed_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricing {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub reasoning: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

#[derive(Debug)]
pub enum ModelMetadataError {
    Invalid {
        message: String,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: Option<PathBuf>,
        source: serde_json::Error,
    },
}

impl fmt::Display for ModelMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { message } => write!(formatter, "invalid model metadata: {message}"),
            Self::Io { path, source } => write!(formatter, "{}: {}", path.display(), source),
            Self::Json {
                path: Some(path),
                source,
            } => write!(
                formatter,
                "{} contains invalid model metadata JSON: {}",
                path.display(),
                source
            ),
            Self::Json { path: None, source } => {
                write!(formatter, "models.dev returned invalid JSON: {source}")
            }
        }
    }
}

impl std::error::Error for ModelMetadataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Invalid { .. } => None,
        }
    }
}

pub fn parse_models_dev_metadata(
    content: &str,
    source_url: &str,
    fetched_at: &str,
) -> Result<ModelMetadataCache, ModelMetadataError> {
    let providers: BTreeMap<String, RawProvider> = serde_json::from_str(content)
        .map_err(|source| ModelMetadataError::Json { path: None, source })?;
    let mut models = Vec::new();

    for (provider_key, provider) in providers {
        let provider_id = required_source_id("provider", provider.id.as_deref(), &provider_key)?;
        let provider_name = provider
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| provider_id.clone());

        for (model_key, model) in provider.models {
            let model_id = required_source_id("model", model.id.as_deref(), &model_key)?;
            let name = model
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| model_id.clone());
            let pricing = ModelPricing {
                input: model.cost.as_ref().and_then(|cost| cost.input),
                output: model.cost.as_ref().and_then(|cost| cost.output),
                reasoning: model.cost.as_ref().and_then(|cost| cost.reasoning),
                cache_read: model.cost.as_ref().and_then(|cost| cost.cache_read),
                cache_write: model.cost.as_ref().and_then(|cost| cost.cache_write),
            };
            let supports_cache = pricing.cache_read.is_some() || pricing.cache_write.is_some();
            let context_window = model.limit.as_ref().and_then(|limit| limit.context);
            let max_output_tokens = model.limit.as_ref().and_then(|limit| limit.output);
            let input_modalities = model
                .modalities
                .as_ref()
                .map(|modalities| modalities.input.clone())
                .unwrap_or_default();
            let output_modalities = model
                .modalities
                .as_ref()
                .map(|modalities| modalities.output.clone())
                .unwrap_or_default();

            models.push(ModelMetadataRecord {
                key: model_metadata_key(&provider_id, &model_id),
                provider_id: provider_id.clone(),
                provider_name: provider_name.clone(),
                model_id,
                name,
                context_window,
                max_output_tokens,
                pricing,
                input_modalities,
                output_modalities,
                supports_tools: model.tool_call.unwrap_or(false),
                supports_cache,
                reasoning: model.reasoning.unwrap_or(false),
                source_url: source_url.to_string(),
                refreshed_at: fetched_at.to_string(),
            });
        }
    }

    models.sort_by(|left, right| {
        left.provider_name
            .cmp(&right.provider_name)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.key.cmp(&right.key))
    });

    Ok(ModelMetadataCache {
        source_url: source_url.to_string(),
        fetched_at: fetched_at.to_string(),
        models,
    })
}

pub fn read_model_metadata_cache(
    path: impl AsRef<Path>,
) -> Result<Option<ModelMetadataCache>, ModelMetadataError> {
    let path = path.as_ref();

    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path).map_err(|source| ModelMetadataError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let cache = serde_json::from_str(&content).map_err(|source| ModelMetadataError::Json {
        path: Some(path.to_path_buf()),
        source,
    })?;

    Ok(Some(cache))
}

pub fn write_model_metadata_cache(
    path: impl AsRef<Path>,
    cache: &ModelMetadataCache,
) -> Result<(), ModelMetadataError> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| ModelMetadataError::Invalid {
        message: format!("cache path has no parent: {}", path.display()),
    })?;

    if !parent.is_dir() {
        return Err(ModelMetadataError::Invalid {
            message: format!(
                "model metadata cache directory does not exist: {}",
                parent.display()
            ),
        });
    }

    let content =
        serde_json::to_string_pretty(cache).map_err(|source| ModelMetadataError::Json {
            path: Some(path.to_path_buf()),
            source,
        })?;
    let temp_file = path.with_extension("json.tmp");

    fs::write(&temp_file, content).map_err(|source| ModelMetadataError::Io {
        path: temp_file.clone(),
        source,
    })?;
    if path.exists() {
        fs::remove_file(path).map_err(|source| ModelMetadataError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    fs::rename(&temp_file, path).map_err(|source| ModelMetadataError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(())
}

pub fn model_metadata_key(provider_id: &str, model_id: &str) -> String {
    format!("{provider_id}/{model_id}")
}

fn required_source_id(
    label: &str,
    field_id: Option<&str>,
    object_key: &str,
) -> Result<String, ModelMetadataError> {
    let id = field_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or(object_key);

    if id.trim().is_empty() {
        return Err(ModelMetadataError::Invalid {
            message: format!("{label} entry has an empty object key and no id"),
        });
    }

    Ok(id.to_string())
}

#[derive(Deserialize)]
struct RawProvider {
    id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    models: BTreeMap<String, RawModel>,
}

#[derive(Deserialize)]
struct RawModel {
    id: Option<String>,
    name: Option<String>,
    reasoning: Option<bool>,
    tool_call: Option<bool>,
    limit: Option<RawLimit>,
    cost: Option<RawCost>,
    modalities: Option<RawModalities>,
}

#[derive(Deserialize)]
struct RawLimit {
    context: Option<u64>,
    output: Option<u64>,
}

#[derive(Deserialize)]
struct RawCost {
    input: Option<f64>,
    output: Option<f64>,
    reasoning: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

#[derive(Clone, Deserialize)]
struct RawModalities {
    #[serde(default)]
    input: Vec<String>,
    #[serde(default)]
    output: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_models_dev_limits_pricing_modalities_and_capabilities() {
        let cache = parse_models_dev_metadata(
            r#"{
  "openai": {
    "id": "openai",
    "name": "OpenAI",
    "models": {
      "gpt-test": {
        "id": "gpt-test",
        "name": "GPT Test",
        "reasoning": true,
        "tool_call": true,
        "limit": { "context": 128000, "output": 16384 },
        "cost": { "input": 1.25, "output": 10.0, "cache_read": 0.125 },
        "modalities": { "input": ["text", "image"], "output": ["text"] }
      }
    }
  }
}"#,
            MODELS_DEV_API_URL,
            "2026-06-03T10:00:00Z",
        )
        .expect("metadata should parse");

        assert_eq!(cache.models.len(), 1);
        let model = &cache.models[0];

        assert_eq!(model.key, "openai/gpt-test");
        assert_eq!(model.context_window, Some(128000));
        assert_eq!(model.max_output_tokens, Some(16384));
        assert_eq!(model.pricing.input, Some(1.25));
        assert_eq!(model.pricing.output, Some(10.0));
        assert!(model.supports_tools);
        assert!(model.supports_cache);
        assert!(model.reasoning);
        assert_eq!(model.input_modalities, ["text", "image"]);
        assert_eq!(model.output_modalities, ["text"]);
        assert_eq!(model.source_url, MODELS_DEV_API_URL);
        assert_eq!(model.refreshed_at, "2026-06-03T10:00:00Z");
    }

    #[test]
    fn parses_models_with_missing_optional_limits() {
        let cache = parse_models_dev_metadata(
            r#"{
  "local": {
    "models": {
      "unknown": {
        "id": "unknown",
        "name": "Unknown",
        "modalities": { "input": ["text"], "output": ["text"] }
      }
    }
  }
}"#,
            MODELS_DEV_API_URL,
            "2026-06-03T10:00:00Z",
        )
        .expect("metadata should parse");
        let model = &cache.models[0];

        assert_eq!(model.provider_id, "local");
        assert_eq!(model.context_window, None);
        assert_eq!(model.max_output_tokens, None);
        assert!(!model.supports_tools);
        assert!(!model.supports_cache);
    }
}
