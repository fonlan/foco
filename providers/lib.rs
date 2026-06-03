use std::fmt;

use genai::{
    Client,
    adapter::AdapterKind,
    resolver::{AuthData, Endpoint, ProviderConfig},
};

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

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderConfigError {
    Connection(String),
    EmptyBaseUrl,
    InvalidBaseUrl { value: String, source: String },
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
