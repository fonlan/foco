use std::{
    borrow::Cow,
    env, fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use flate2::read::GzDecoder;
use foco_providers::{NeutralChatRequest, NeutralChatRole, NeutralToolDefinition};
use foco_store::config::{
    GlobalConfig, ModelSettings, ProviderSettings, SKILL_SCOPE_GLOBAL, SKILL_SCOPE_WORKSPACE,
    SkillSettings,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    ApiError, AppState, api_audit_save_details, audited_provider_tool_request, config_snapshot,
    config_update_snapshot, discover_skills, markdown_code_block, merge_disabled_skill_keys,
    neutral_text_message, preserve_disabled_skill_keys_for_hidden_locations,
    provider_connection_config, refresh_derived_enabled_skills, save_config, settings_response,
    skills::parse_skill_markdown, unique_id, workspace_by_id,
};

const DEFAULT_SKILLS_SH_BASE_URL: &str = "https://skills.sh";
const DEFAULT_SKILLS_API_BASE_URL: &str = "https://skills-api.deeptoai.com";
const DEFAULT_GITHUB_API_BASE_URL: &str = "https://api.github.com";
const SKILL_FILE_NAME: &str = "SKILL.md";
const SKILL_STORE_METADATA_FILE_NAME: &str = ".foco-skill-store.json";
const SKILL_STORE_FILE_ENCODING_BASE64: &str = "base64";
pub(crate) const GITHUB_SKILL_ARCHIVE_MAX_COMPRESSED_BYTES: usize = 128 * 1024 * 1024;
pub(crate) const GITHUB_SKILL_ARCHIVE_MAX_EXTRACTED_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const GITHUB_SKILL_ARCHIVE_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const GITHUB_SKILL_ARCHIVE_MAX_FILES: usize = 4_096;
const SKILL_TRANSLATION_TOOL_NAME: &str = "submit_skill_translation";
const SKILL_TRANSLATION_TIMEOUT_MS: u64 = 120_000;
const SKILL_TRANSLATION_MAX_OUTPUT_TOKENS: u32 = 16_384;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreSearchQuery {
    pub(crate) query: String,
    pub(crate) page: Option<i64>,
    pub(crate) page_size: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreBrowseQuery {
    pub(crate) sort: Option<String>,
    pub(crate) page: Option<i64>,
    pub(crate) page_size: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SkillStorePageParams {
    page: u32,
    page_size: u32,
}

impl SkillStorePageParams {
    const DEFAULT_PAGE: u32 = 1;
    const DEFAULT_PAGE_SIZE: u32 = 20;
    const MAX_PAGE_SIZE: u32 = 100;

    pub(crate) fn from_query(page: Option<i64>, page_size: Option<i64>) -> Self {
        let page = page
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0)
            .unwrap_or(Self::DEFAULT_PAGE);
        let page_size = page_size
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0)
            .unwrap_or(Self::DEFAULT_PAGE_SIZE)
            .min(Self::MAX_PAGE_SIZE);

        Self { page, page_size }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SkillStoreBrowseSort {
    InstallsDesc,
    NameAsc,
    NameDesc,
}

impl SkillStoreBrowseSort {
    pub(crate) fn from_query(value: Option<&str>) -> Result<Self, ApiError> {
        match value.unwrap_or("installs_desc") {
            "installs_desc" => Ok(Self::InstallsDesc),
            "name_asc" => Ok(Self::NameAsc),
            "name_desc" => Ok(Self::NameDesc),
            other => Err(ApiError::bad_request(format!(
                "unsupported skill store sort '{other}'"
            ))),
        }
    }

    pub(crate) fn registry_params(self) -> (&'static str, &'static str) {
        match self {
            Self::InstallsDesc => ("installs", "desc"),
            Self::NameAsc => ("name", "asc"),
            Self::NameDesc => ("name", "desc"),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreDetailQuery {
    pub(crate) source: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreImportPreviewRequest {
    pub(crate) input: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SkillStoreImportTarget {
    pub(crate) source: String,
    pub(crate) skill_id: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreInstallRequest {
    pub(crate) skill_id: String,
    pub(crate) source: Option<String>,
    pub(crate) target: String,
    pub(crate) workspace_id: Option<String>,
    #[serde(default)]
    pub(crate) overwrite: bool,
    #[serde(default)]
    pub(crate) files: Vec<SkillStoreFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreUpdateRequest {
    pub(crate) key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreTranslateRequest {
    pub(crate) content: String,
    pub(crate) target_language: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreFile {
    pub(crate) path: String,
    pub(crate) content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) content_encoding: Option<String>,
}

impl SkillStoreFile {
    pub(crate) fn text(path: String, content: String) -> Self {
        Self {
            path,
            content,
            content_encoding: None,
        }
    }

    pub(crate) fn from_bytes(path: String, content: Vec<u8>) -> Self {
        match String::from_utf8(content) {
            Ok(content) => Self::text(path, content),
            Err(error) => Self {
                path,
                content: BASE64_STANDARD.encode(error.into_bytes()),
                content_encoding: Some(SKILL_STORE_FILE_ENCODING_BASE64.to_string()),
            },
        }
    }

    pub(crate) fn decoded_content(&self) -> Result<Cow<'_, [u8]>, ApiError> {
        match self.content_encoding.as_deref() {
            None => Ok(Cow::Borrowed(self.content.as_bytes())),
            Some(SKILL_STORE_FILE_ENCODING_BASE64) => BASE64_STANDARD
                .decode(&self.content)
                .map(Cow::Owned)
                .map_err(|source| {
                    ApiError::bad_request(format!(
                        "skill file '{}' has invalid base64 content: {source}",
                        self.path
                    ))
                }),
            Some(encoding) => Err(ApiError::bad_request(format!(
                "skill file '{}' uses unsupported content encoding '{encoding}'; expected text or base64",
                self.path
            ))),
        }
    }

    fn text_content(&self) -> Result<&str, ApiError> {
        if self.content_encoding.is_some() {
            return Err(ApiError::bad_request(format!(
                "skill file '{}' must contain text",
                self.path
            )));
        }
        Ok(&self.content)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreInstallMetadata {
    pub(crate) skill_id: String,
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) workspace_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreSkillSummary {
    id: String,
    name: String,
    description: String,
    source: Option<String>,
    installs: Option<u64>,
    installs_yesterday: Option<u64>,
    change: Option<i64>,
    official: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreListResponse {
    skills: Vec<SkillStoreSkillSummary>,
    total: usize,
    has_more: bool,
    source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreDetailResponse {
    id: String,
    name: String,
    description: String,
    source: Option<String>,
    files: Vec<SkillStoreFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreInstallResponse {
    pub(crate) target: String,
    pub(crate) workspace_id: Option<String>,
    pub(crate) path: String,
    pub(crate) detected: Vec<SkillSettings>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreUpdateResult {
    pub(crate) key: String,
    pub(crate) ok: bool,
    pub(crate) path: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreUpdateResponse {
    pub(crate) results: Vec<SkillStoreUpdateResult>,
    pub(crate) settings: crate::http::settings::SettingsResponse,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreTranslateResponse {
    translated_content: String,
}

#[derive(Clone)]
struct SkillStoreClient {
    http: reqwest::Client,
    skills_base_url: String,
    skills_api_base_url: String,
    github_api_base_url: String,
    token: Option<String>,
}

impl SkillStoreClient {
    fn from_env() -> Self {
        Self {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .expect("build skill store HTTP client"),
            skills_base_url: env::var("SKILLS_SH_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_SKILLS_SH_BASE_URL.to_string()),
            skills_api_base_url: env::var("SKILLS_API_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_SKILLS_API_BASE_URL.to_string()),
            github_api_base_url: env::var("SKILLS_STORE_GITHUB_API_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_GITHUB_API_BASE_URL.to_string()),
            token: env::var("SKILLS_SH_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    env::var("VERCEL_OIDC_TOKEN")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                }),
        }
    }
    async fn browse(
        &self,
        sort: SkillStoreBrowseSort,
        page_params: SkillStorePageParams,
    ) -> Result<SkillStoreListResponse, ApiError> {
        let (sort_by, sort_order) = sort.registry_params();
        let url = format!(
            "{}?sortBy={}&sortOrder={}&page={}&pageSize={}",
            self.skills_api_url("/api/skills"),
            url_query_value(sort_by),
            url_query_value(sort_order),
            page_params.page,
            page_params.page_size
        );
        let value = self
            .http
            .get(url)
            .send()
            .await
            .map_err(network_error)?
            .error_for_status()
            .map_err(network_error)?
            .json::<Value>()
            .await
            .map_err(network_error)?;
        Ok(list_response_from_value(value, "skills-api:browse"))
    }

    async fn hot(&self) -> Result<SkillStoreListResponse, ApiError> {
        if self.token.is_some() {
            let url = format!("{}?view=hot", self.skills_url("/api/v1/skills"));
            match self.skills_get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    let value = response.json::<Value>().await.map_err(network_error)?;
                    return Ok(list_response_from_value(value, "skills.sh:v1"));
                }
                Ok(response) => {
                    tracing::warn!(status = %response.status(), "skills.sh v1 hot request failed; falling back to public hot endpoint");
                }
                Err(error) => {
                    tracing::warn!(error = %error, "skills.sh v1 hot request failed; falling back to public hot endpoint");
                }
            }
        }

        let url = self.skills_url("/api/skills/hot/0");
        let value = self
            .http
            .get(url)
            .send()
            .await
            .map_err(network_error)?
            .error_for_status()
            .map_err(network_error)?
            .json::<Value>()
            .await
            .map_err(network_error)?;
        Ok(list_response_from_value(value, "skills.sh:public-hot"))
    }

    async fn search(
        &self,
        query: &str,
        page_params: SkillStorePageParams,
    ) -> Result<SkillStoreListResponse, ApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(ApiError::bad_request("search query must not be empty"));
        }
        let encoded_query = url_query_value(query);
        if self.token.is_some() {
            let url = format!(
                "{}?query={}&q={}&page={}&pageSize={}",
                self.skills_url("/api/v1/skills/search"),
                encoded_query,
                encoded_query,
                page_params.page,
                page_params.page_size
            );
            match self.skills_get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    let value = response.json::<Value>().await.map_err(network_error)?;
                    return Ok(list_response_from_value(value, "skills.sh:v1-search"));
                }
                Ok(response) => {
                    tracing::warn!(status = %response.status(), "skills.sh v1 search request failed; falling back to public registry search");
                }
                Err(error) => {
                    tracing::warn!(error = %error, "skills.sh v1 search request failed; falling back to public registry search");
                }
            }
        }

        self.skills_api_search(query, page_params).await
    }

    async fn skills_api_search(
        &self,
        query: &str,
        page_params: SkillStorePageParams,
    ) -> Result<SkillStoreListResponse, ApiError> {
        let encoded_query = url_query_value(query);
        let url = format!(
            "{}?query={}&q={}&page={}&pageSize={}",
            self.skills_api_url("/api/skills"),
            encoded_query,
            encoded_query,
            page_params.page,
            page_params.page_size
        );
        let value = self
            .http
            .get(url)
            .send()
            .await
            .map_err(network_error)?
            .error_for_status()
            .map_err(network_error)?
            .json::<Value>()
            .await
            .map_err(network_error)?;
        Ok(list_response_from_value(value, "skills-api:search"))
    }

    async fn canonical_github_source_for_skill(
        &self,
        skill_id: &str,
    ) -> Result<Option<String>, ApiError> {
        let skill_id = validate_skill_slug(skill_id)?;
        let response = self
            .skills_api_search(
                &skill_id,
                SkillStorePageParams::from_query(
                    None,
                    Some(SkillStorePageParams::MAX_PAGE_SIZE as i64),
                ),
            )
            .await?;
        Ok(response
            .skills
            .into_iter()
            .filter(|skill| skill.id == skill_id)
            .filter_map(|skill| skill.source)
            .find_map(|source| validate_github_source(&source).ok()))
    }

    async fn detail(
        &self,
        skill_id: &str,
        source: Option<&str>,
    ) -> Result<SkillStoreDetailResponse, ApiError> {
        let skill_id = validate_skill_slug(skill_id)?;
        let mut token_detail = None;
        if self.token.is_some() {
            let url = self.skills_url(&format!("/api/v1/skills/{skill_id}"));
            match self.skills_get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    let value = response.json::<Value>().await.map_err(network_error)?;
                    let detail = detail_response_from_value(value, &skill_id, source);
                    if skill_files_have_skill_md(&detail.files) {
                        match ensure_skill_files_valid(&detail.files) {
                            Ok(()) => return Ok(detail),
                            Err(error) => {
                                tracing::warn!(error = %error.message(), "skills.sh v1 detail files were invalid; trying registry detail");
                            }
                        }
                    }
                    token_detail = Some(detail);
                }
                Ok(response) => {
                    tracing::warn!(status = %response.status(), "skills.sh v1 detail request failed; trying registry detail");
                }
                Err(error) => {
                    tracing::warn!(error = %error, "skills.sh v1 detail request failed; trying registry detail");
                }
            }
        }

        match self.skills_api_detail(&skill_id).await {
            Ok(mut detail) => {
                if skill_files_have_skill_md(&detail.files) {
                    match ensure_skill_files_valid(&detail.files) {
                        Ok(()) => return Ok(detail),
                        Err(error) => {
                            tracing::warn!(error = %error.message(), "skills-api detail files were invalid; trying file endpoint");
                        }
                    }
                }

                if let Some(source) = detail.source.clone() {
                    match self.skills_api_skill_files(&source, &skill_id).await {
                        Ok(files) => {
                            detail.files = files;
                            return Ok(detail);
                        }
                        Err(error) => {
                            tracing::warn!(source, error = %error.message(), "skills-api file request failed; trying canonical registry source");
                        }
                    }

                    if let Ok(files) = self.github_skill_files(&source, &skill_id).await {
                        detail.files = files;
                        return Ok(detail);
                    }
                }

                match self.canonical_github_source_for_skill(&skill_id).await {
                    Ok(Some(source)) => match self.skills_api_skill_files(&source, &skill_id).await
                    {
                        Ok(files) => {
                            detail.source = Some(source);
                            detail.files = files;
                            return Ok(detail);
                        }
                        Err(error) => {
                            tracing::warn!(source, error = %error.message(), "canonical skills-api file request failed; trying GitHub fallback");
                            if let Ok(files) = self.github_skill_files(&source, &skill_id).await {
                                detail.source = Some(source);
                                detail.files = files;
                                return Ok(detail);
                            }
                        }
                    },
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(error = %error.message(), "canonical registry source lookup failed; trying GitHub fallback");
                    }
                }

                Err(ApiError::bad_request(format!(
                    "skill detail did not include {SKILL_FILE_NAME} and registry source was unavailable"
                )))
            }
            Err(registry_error) => {
                tracing::warn!(error = %registry_error.message(), "skills-api detail request failed; trying GitHub fallback");
                let fallback_source = token_detail
                    .as_ref()
                    .and_then(|detail| detail.source.as_deref())
                    .or(source)
                    .and_then(|source| validate_github_source(source).ok());
                if let Some(source) = fallback_source {
                    let files = self.github_skill_files(&source, &skill_id).await?;
                    let mut detail = token_detail.unwrap_or_else(|| SkillStoreDetailResponse {
                        id: skill_id.clone(),
                        name: skill_id.clone(),
                        description: String::new(),
                        source: Some(source.clone()),
                        files: Vec::new(),
                    });
                    detail.source = Some(source);
                    detail.files = files;
                    return Ok(detail);
                }
                Err(registry_error)
            }
        }
    }

    async fn import_preview(&self, input: &str) -> Result<SkillStoreDetailResponse, ApiError> {
        let target = parse_skill_store_import_target(input)?;
        let (skill_id, files) = self
            .github_skill_files_auto(&target.source, target.skill_id.as_deref())
            .await?;
        Ok(detail_response_from_files(skill_id, target.source, files))
    }

    async fn github_skill_files_auto(
        &self,
        source: &str,
        skill_id: Option<&str>,
    ) -> Result<(String, Vec<SkillStoreFile>), ApiError> {
        let source = validate_github_source(source)?;
        let requested_skill_id = skill_id.map(validate_skill_slug).transpose()?;

        if let Some(skill_id) = requested_skill_id {
            let files = self.github_skill_files(&source, &skill_id).await?;
            return Ok((skill_id, files));
        }

        for branch in ["main", "master"] {
            match self
                .github_skill_files_auto_for_branch(&source, branch)
                .await
            {
                Ok(result) => return Ok(result),
                Err(error) if error.message().contains("multiple") => return Err(error),
                Err(error) => {
                    tracing::debug!(branch, error = %error.message(), "GitHub skill auto lookup failed");
                }
            }
        }

        Err(ApiError::bad_request(format!(
            "could not find a unique {SKILL_FILE_NAME} in GitHub source '{source}'"
        )))
    }

    async fn github_skill_files_auto_for_branch(
        &self,
        source: &str,
        branch: &str,
    ) -> Result<(String, Vec<SkillStoreFile>), ApiError> {
        let tree_items = self.github_tree_items(source, branch).await?;
        let root = find_auto_github_skill_root(&tree_items, source)?;
        let fallback_id = source
            .rsplit('/')
            .next()
            .ok_or_else(|| ApiError::bad_request("GitHub source did not include repo name"))?;
        let files = self
            .github_skill_files_for_root(source, branch, &tree_items, &root)
            .await?;
        let skill_id = skill_id_from_files(&files).unwrap_or_else(|| fallback_id.to_string());
        let skill_id = validate_skill_slug(&skill_id)?;
        Ok((skill_id, files))
    }

    async fn github_tree_items(&self, source: &str, branch: &str) -> Result<Vec<Value>, ApiError> {
        let tree_url = format!(
            "{}/repos/{}/git/trees/{}?recursive=1",
            self.github_api_base_url.trim_end_matches('/'),
            source,
            branch
        );
        let tree = self
            .http
            .get(tree_url)
            .header(reqwest::header::USER_AGENT, "foco-skill-store")
            .send()
            .await
            .map_err(network_error)?
            .error_for_status()
            .map_err(network_error)?
            .json::<Value>()
            .await
            .map_err(network_error)?;

        tree.get("tree")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| ApiError::bad_request("GitHub tree response did not include files"))
    }

    async fn github_skill_files_for_root(
        &self,
        source: &str,
        branch: &str,
        tree_items: &[Value],
        skill_root: &str,
    ) -> Result<Vec<SkillStoreFile>, ApiError> {
        let expected_paths = github_skill_paths_for_root(tree_items, skill_root)?;
        let archive_url = format!(
            "{}/repos/{}/tarball/{}",
            self.github_api_base_url.trim_end_matches('/'),
            source,
            url_segment(branch)
        );
        let mut response = self
            .http
            .get(archive_url)
            .header(reqwest::header::USER_AGENT, "foco-skill-store")
            .send()
            .await
            .map_err(network_error)?
            .error_for_status()
            .map_err(network_error)?;
        if response
            .content_length()
            .is_some_and(|length| length > GITHUB_SKILL_ARCHIVE_MAX_COMPRESSED_BYTES as u64)
        {
            return Err(ApiError::bad_request(format!(
                "GitHub skill archive exceeds the compressed size limit of {} bytes",
                GITHUB_SKILL_ARCHIVE_MAX_COMPRESSED_BYTES
            )));
        }
        let mut archive = Vec::with_capacity(
            response
                .content_length()
                .unwrap_or_default()
                .min(GITHUB_SKILL_ARCHIVE_MAX_COMPRESSED_BYTES as u64) as usize,
        );
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            let next_len = archive.len().checked_add(chunk.len()).ok_or_else(|| {
                ApiError::bad_request("GitHub skill archive compressed size overflowed")
            })?;
            if next_len > GITHUB_SKILL_ARCHIVE_MAX_COMPRESSED_BYTES {
                return Err(ApiError::bad_request(format!(
                    "GitHub skill archive exceeds the compressed size limit of {} bytes",
                    GITHUB_SKILL_ARCHIVE_MAX_COMPRESSED_BYTES
                )));
            }
            archive.extend_from_slice(&chunk);
        }
        let skill_root = skill_root.to_string();
        let files = tokio::task::spawn_blocking(move || {
            extract_github_skill_archive(&archive, &skill_root, &expected_paths)
        })
        .await
        .map_err(|source| ApiError::internal(format!("GitHub archive task failed: {source}")))??;
        ensure_skill_files_valid(&files)?;
        Ok(files)
    }

    async fn skills_api_detail(
        &self,
        skill_id: &str,
    ) -> Result<SkillStoreDetailResponse, ApiError> {
        let skill_id = validate_skill_slug(skill_id)?;
        let url = self.skills_api_url(&format!("/api/skills/{}", url_segment(&skill_id)));
        let value = self
            .http
            .get(url)
            .send()
            .await
            .map_err(network_error)?
            .error_for_status()
            .map_err(network_error)?
            .json::<Value>()
            .await
            .map_err(network_error)?;
        Ok(detail_response_from_value(value, &skill_id, None))
    }

    async fn skills_api_skill_files(
        &self,
        source: &str,
        skill_id: &str,
    ) -> Result<Vec<SkillStoreFile>, ApiError> {
        let source = validate_github_source(source)?;
        let skill_id = validate_skill_slug(skill_id)?;
        let mut parts = source.split('/');
        let owner = parts.next().unwrap_or_default();
        let repo = parts.next().unwrap_or_default();
        let url = self.skills_api_url(&format!(
            "/api/skills/{}/{}/{}/files",
            url_segment(owner),
            url_segment(repo),
            url_segment(&skill_id)
        ));
        let value = self
            .http
            .get(url)
            .send()
            .await
            .map_err(network_error)?
            .error_for_status()
            .map_err(network_error)?
            .json::<Value>()
            .await
            .map_err(network_error)?;
        let files = files_from_value(&value);
        ensure_skill_files_valid(&files)?;
        Ok(files)
    }

    async fn github_skill_files(
        &self,
        source: &str,
        skill_id: &str,
    ) -> Result<Vec<SkillStoreFile>, ApiError> {
        let source = validate_github_source(source)?;
        let skill_id = validate_skill_slug(skill_id)?;

        for branch in ["main", "master"] {
            match self
                .github_skill_files_for_branch(&source, &skill_id, branch)
                .await
            {
                Ok(files) => return Ok(files),
                Err(error) => {
                    tracing::debug!(branch, error = %error.message(), "GitHub skill file lookup failed");
                }
            }
        }

        Err(ApiError::bad_request(format!(
            "could not find {SKILL_FILE_NAME} for skill '{skill_id}' in GitHub source '{source}'"
        )))
    }

    async fn github_skill_files_for_branch(
        &self,
        source: &str,
        skill_id: &str,
        branch: &str,
    ) -> Result<Vec<SkillStoreFile>, ApiError> {
        let tree_items = self.github_tree_items(source, branch).await?;
        let skill_root = find_github_skill_root(&tree_items, skill_id)?;
        self.github_skill_files_for_root(source, branch, &tree_items, &skill_root)
            .await
    }

    fn skills_url(&self, path: &str) -> String {
        format!("{}{}", self.skills_base_url.trim_end_matches('/'), path)
    }

    fn skills_api_url(&self, path: &str) -> String {
        format!("{}{}", self.skills_api_base_url.trim_end_matches('/'), path)
    }

    fn skills_get(&self, url: &str) -> reqwest::RequestBuilder {
        let builder = self.http.get(url);
        match self.token.as_deref() {
            Some(token) => builder.bearer_auth(token),
            None => builder,
        }
    }
}

pub(crate) async fn skill_store_hot() -> Result<Json<SkillStoreListResponse>, ApiError> {
    Ok(Json(SkillStoreClient::from_env().hot().await?))
}

pub(crate) async fn skill_store_browse(
    Query(query): Query<SkillStoreBrowseQuery>,
) -> Result<Json<SkillStoreListResponse>, ApiError> {
    Ok(Json(
        SkillStoreClient::from_env()
            .browse(
                SkillStoreBrowseSort::from_query(query.sort.as_deref())?,
                SkillStorePageParams::from_query(query.page, query.page_size),
            )
            .await?,
    ))
}

pub(crate) async fn skill_store_search(
    Query(query): Query<SkillStoreSearchQuery>,
) -> Result<Json<SkillStoreListResponse>, ApiError> {
    Ok(Json(
        SkillStoreClient::from_env()
            .search(
                &query.query,
                SkillStorePageParams::from_query(query.page, query.page_size),
            )
            .await?,
    ))
}

pub(crate) async fn skill_store_detail(
    AxumPath(skill_id): AxumPath<String>,
    Query(query): Query<SkillStoreDetailQuery>,
) -> Result<Json<SkillStoreDetailResponse>, ApiError> {
    Ok(Json(
        SkillStoreClient::from_env()
            .detail(&skill_id, query.source.as_deref())
            .await?,
    ))
}

pub(crate) async fn skill_store_import_preview(
    Json(request): Json<SkillStoreImportPreviewRequest>,
) -> Result<Json<SkillStoreDetailResponse>, ApiError> {
    Ok(Json(
        SkillStoreClient::from_env()
            .import_preview(&request.input)
            .await?,
    ))
}

pub(crate) async fn skill_store_translate(
    State(state): State<AppState>,
    Json(request): Json<SkillStoreTranslateRequest>,
) -> Result<Json<SkillStoreTranslateResponse>, ApiError> {
    let config = config_snapshot(&state)?;
    let model = resolve_skill_translation_model(&config)?;
    let provider = provider_for_model(&config, model)?;
    let workspace = workspace_by_id(&config, &config.app.active_workspace_id)?;
    let request = skill_translation_provider_request(
        &model.id,
        request.content.as_str(),
        request.target_language.as_str(),
    )?;
    let tool_arguments = audited_provider_tool_request(
        &workspace.path,
        &workspace.id,
        None,
        &provider.id,
        &provider_connection_config(provider)?,
        request,
        "skill store translation",
        SKILL_TRANSLATION_TOOL_NAME,
        "submit skill translation tool",
        SKILL_TRANSLATION_TIMEOUT_MS,
        config.app.llm_request_retry_count,
        api_audit_save_details(&config),
    )
    .await?;
    let translated_content = parse_skill_translation_output(tool_arguments)?;

    Ok(Json(SkillStoreTranslateResponse { translated_content }))
}

pub(crate) fn resolve_skill_translation_model(
    config: &GlobalConfig,
) -> Result<&ModelSettings, ApiError> {
    let model_id = config
        .skills
        .translation_model_id
        .as_deref()
        .map(str::trim)
        .filter(|model_id| !model_id.is_empty())
        .ok_or_else(|| ApiError::bad_request("skill translation model is not configured"))?;
    let model = config
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!("skill translation model was not found: {model_id}"))
        })?;
    if !model.enabled || !model_outputs_text(model) {
        return Err(ApiError::bad_request(format!(
            "skill translation model '{model_id}' is disabled or cannot output text"
        )));
    }
    if model
        .active_provider_id
        .as_deref()
        .map_or(true, |provider_id| provider_id.is_empty())
    {
        return Err(ApiError::bad_request(format!(
            "skill translation model '{model_id}' has no active provider selected"
        )));
    }

    Ok(model)
}

fn provider_for_model<'a>(
    config: &'a GlobalConfig,
    model: &ModelSettings,
) -> Result<&'a ProviderSettings, ApiError> {
    let provider_id = model.active_provider_id.as_deref().ok_or_else(|| {
        ApiError::bad_request(format!(
            "skill translation model '{}' has no active provider selected",
            model.id
        ))
    })?;
    if !model.provider_ids.iter().any(|id| id == provider_id) {
        return Err(ApiError::bad_request(format!(
            "active provider '{}' is not associated with skill translation model '{}'",
            provider_id, model.id
        )));
    }
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| ApiError::bad_request(format!("provider '{provider_id}' was not found")))?;
    if !provider.enabled {
        return Err(ApiError::bad_request(format!(
            "provider '{}' is disabled",
            provider.id
        )));
    }

    Ok(provider)
}

pub(crate) fn skill_translation_provider_request(
    model_id: &str,
    content: &str,
    target_language: &str,
) -> Result<NeutralChatRequest, ApiError> {
    let target_language = target_language.trim();
    if target_language.is_empty() {
        return Err(ApiError::bad_request("targetLanguage must not be empty"));
    }
    if content.trim().is_empty() {
        return Err(ApiError::bad_request("content must not be empty"));
    }

    Ok(NeutralChatRequest {
        model_id: model_id.to_string(),
        messages: vec![
            neutral_text_message(
                NeutralChatRole::System,
                "Translate SKILL.md display text only. Preserve Markdown structure, frontmatter keys, YAML syntax, code fences, inline code, commands, paths, URLs, package names, environment variables, placeholders, IDs, and product or proper names unless they are normal prose. Return the result by calling submit_skill_translation exactly once.".to_string(),
            ),
            neutral_text_message(
                NeutralChatRole::User,
                format!(
                    "Target language: {target_language}\n\nTranslate this SKILL.md content for display only. Do not add commentary.\n\n{}",
                    markdown_code_block("markdown", content)
                ),
            ),
        ],
        tools: vec![skill_translation_tool_definition()],
        thinking_level: None,
        max_output_tokens: Some(SKILL_TRANSLATION_MAX_OUTPUT_TOKENS),
        prompt_cache_key: None,
        prompt_cache_retention: None,
    agent_correlation: None,
    })
}

fn skill_translation_tool_definition() -> NeutralToolDefinition {
    NeutralToolDefinition {
        name: SKILL_TRANSLATION_TOOL_NAME.to_string(),
        description: "Submit the translated SKILL.md display Markdown.".to_string(),
        strict: true,
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "translatedContent": {
                    "type": "string",
                    "description": "The translated SKILL.md Markdown for display."
                }
            },
            "required": ["translatedContent"]
        }),
    }
}

fn parse_skill_translation_output(arguments: Value) -> Result<String, ApiError> {
    arguments
        .get("translatedContent")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| ApiError::internal("skill translation response was empty"))
}

fn model_outputs_text(model: &ModelSettings) -> bool {
    model.output_modalities.is_empty()
        || model
            .output_modalities
            .iter()
            .any(|modality| modality == "text")
}

pub(crate) async fn skill_store_install(
    State(state): State<AppState>,
    Json(request): Json<SkillStoreInstallRequest>,
) -> Result<Json<SkillStoreInstallResponse>, ApiError> {
    let skill_id = validate_skill_slug(&request.skill_id)?;
    let (files, source) = if request.files.is_empty() {
        let detail = SkillStoreClient::from_env()
            .detail(&skill_id, request.source.as_deref())
            .await?;
        (detail.files, detail.source)
    } else {
        (request.files, request.source.clone())
    };
    ensure_skill_files_valid(&files)?;
    validate_skill_markdown_matches_slug(&skill_id, &files)?;
    let source = source.as_deref().map(validate_github_source).transpose()?;

    let mut config = config_update_snapshot(&state).await?;
    let target = request.target.trim().to_string();
    if target == SKILL_SCOPE_WORKSPACE {
        let workspace_id = request.workspace_id.as_deref().ok_or_else(|| {
            ApiError::bad_request("workspaceId is required when target is workspace")
        })?;
        let workspace = workspace_by_id(&config, workspace_id)?;
        if workspace.is_remote() {
            return crate::remote_workspace::install_remote_workspace_skill(
                &state,
                workspace_id,
                SkillStoreInstallRequest {
                    skill_id: skill_id.clone(),
                    source: source.clone(),
                    target,
                    workspace_id: Some(workspace_id.to_string()),
                    overwrite: request.overwrite,
                    files,
                },
            )
            .await;
        }
    }
    let (target_root, workspace_id) = install_target_root(
        &state.user_profile_dir,
        &config,
        &target,
        request.workspace_id.as_deref(),
    )?;
    let metadata = source.map(|source| SkillStoreInstallMetadata {
        skill_id: skill_id.clone(),
        source,
        target: target.clone(),
        workspace_id: workspace_id.clone(),
    });
    let install_path =
        install_skill_files_to_target_dir(&target_root, &skill_id, &files, request.overwrite)?;
    if let Some(metadata) = metadata.as_ref() {
        write_skill_store_metadata(&install_path, metadata)?;
    }

    let discovery = refresh_skill_discovery(&state, &mut config)?;
    save_config(&state, &mut config)?;

    Ok(Json(SkillStoreInstallResponse {
        target,
        workspace_id,
        path: install_path.display().to_string(),
        detected: discovery,
    }))
}

pub(crate) async fn skill_store_update(
    State(state): State<AppState>,
    Json(request): Json<SkillStoreUpdateRequest>,
) -> Result<Json<SkillStoreUpdateResponse>, ApiError> {
    let mut config = config_update_snapshot(&state).await?;
    let discovery = discover_skills(&state.user_profile_dir, &config);
    let skill = discovery
        .skills
        .iter()
        .find(|skill| skill.key == request.key)
        .ok_or_else(|| ApiError::bad_request(format!("skill was not found: {}", request.key)))?;
    let metadata = read_skill_store_metadata_for_skill(skill)?.ok_or_else(|| {
        ApiError::bad_request(format!(
            "skill '{}' was not installed from the skill store with update metadata",
            request.key
        ))
    })?;
    let path =
        update_skill_from_store_metadata(&SkillStoreClient::from_env(), skill, &metadata).await?;
    refresh_skill_discovery(&state, &mut config)?;
    save_config(&state, &mut config)?;
    let Json(settings) = settings_response(&state, &config).await?;

    Ok(Json(SkillStoreUpdateResponse {
        results: vec![SkillStoreUpdateResult {
            key: request.key,
            ok: true,
            path: Some(path.display().to_string()),
            error: None,
        }],
        settings,
    }))
}

pub(crate) async fn skill_store_update_all(
    State(state): State<AppState>,
) -> Result<Json<SkillStoreUpdateResponse>, ApiError> {
    let mut config = config_update_snapshot(&state).await?;
    let discovery = discover_skills(&state.user_profile_dir, &config);
    let client = SkillStoreClient::from_env();
    let mut results = Vec::new();

    // ponytail: installed store skills are few; keep updates sequential to avoid config/dir write races. Add bounded concurrency only if this becomes slow.
    for skill in discovery.skills.iter() {
        let Some(metadata) = skill_store_metadata_for_skill(skill) else {
            continue;
        };
        match update_skill_from_store_metadata(&client, skill, &metadata).await {
            Ok(path) => results.push(SkillStoreUpdateResult {
                key: skill.key.clone(),
                ok: true,
                path: Some(path.display().to_string()),
                error: None,
            }),
            Err(error) => results.push(SkillStoreUpdateResult {
                key: skill.key.clone(),
                ok: false,
                path: None,
                error: Some(error.message().to_string()),
            }),
        }
    }

    refresh_skill_discovery(&state, &mut config)?;
    save_config(&state, &mut config)?;
    let Json(settings) = settings_response(&state, &config).await?;

    Ok(Json(SkillStoreUpdateResponse { results, settings }))
}

fn refresh_skill_discovery(
    state: &AppState,
    config: &mut GlobalConfig,
) -> Result<Vec<SkillSettings>, ApiError> {
    let discovery = discover_skills(&state.user_profile_dir, &config);
    config.skills.detected = discovery.skills.clone();
    let existing_disabled = std::mem::take(&mut config.skills.disabled);
    let disabled = merge_disabled_skill_keys(
        preserve_disabled_skill_keys_for_hidden_locations(existing_disabled, &discovery.skills),
        &discovery.required_disabled,
    );
    config.skills.disabled = disabled.clone();
    refresh_derived_enabled_skills(config, &state.user_profile_dir);
    config.skills.disabled = disabled;
    Ok(discovery.skills)
}

async fn update_skill_from_store_metadata(
    client: &SkillStoreClient,
    skill: &SkillSettings,
    metadata: &SkillStoreInstallMetadata,
) -> Result<PathBuf, ApiError> {
    let install_dir = installed_skill_dir(skill)?;
    let target_root = install_dir.parent().ok_or_else(|| {
        ApiError::bad_request(format!(
            "skill path has no install root: {}",
            skill.path.display()
        ))
    })?;
    let installed_dir_name = install_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ApiError::bad_request("skill install directory name is invalid"))?;
    let installed_dir_name = validate_skill_slug(installed_dir_name)?;
    if metadata.skill_id != installed_dir_name {
        return Err(ApiError::bad_request(format!(
            "skill metadata id '{}' does not match installed directory '{}'",
            metadata.skill_id, installed_dir_name
        )));
    }

    let detail = client
        .detail(&metadata.skill_id, Some(metadata.source.as_str()))
        .await?;
    let files = detail.files;
    ensure_skill_files_valid(&files)?;
    validate_skill_markdown_matches_slug(&installed_dir_name, &files)?;
    let install_path =
        install_skill_files_to_target_dir(target_root, &installed_dir_name, &files, true)?;
    let next_metadata = SkillStoreInstallMetadata {
        skill_id: installed_dir_name,
        source: detail.source.unwrap_or_else(|| metadata.source.clone()),
        target: metadata.target.clone(),
        workspace_id: metadata.workspace_id.clone(),
    };
    write_skill_store_metadata(&install_path, &next_metadata)?;
    Ok(install_path)
}

fn installed_skill_dir(skill: &SkillSettings) -> Result<&Path, ApiError> {
    if skill.path.file_name().and_then(|name| name.to_str()) != Some(SKILL_FILE_NAME) {
        return Err(ApiError::bad_request(format!(
            "skill path must end with {SKILL_FILE_NAME}: {}",
            skill.path.display()
        )));
    }
    skill.path.parent().ok_or_else(|| {
        ApiError::bad_request(format!(
            "skill path has no parent: {}",
            skill.path.display()
        ))
    })
}

pub(crate) fn skill_store_metadata_for_skill(
    skill: &SkillSettings,
) -> Option<SkillStoreInstallMetadata> {
    match read_skill_store_metadata_for_skill(skill) {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::warn!(skill_key = %skill.key, error = %error.message(), "failed to read skill store metadata");
            None
        }
    }
}

fn read_skill_store_metadata_for_skill(
    skill: &SkillSettings,
) -> Result<Option<SkillStoreInstallMetadata>, ApiError> {
    let install_dir = installed_skill_dir(skill)?;
    let path = skill_store_metadata_path(install_dir);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|source| {
        ApiError::internal(format!(
            "failed to read skill store metadata {}: {}",
            path.display(),
            source
        ))
    })?;
    let metadata: SkillStoreInstallMetadata = serde_json::from_str(&content).map_err(|source| {
        ApiError::bad_request(format!(
            "skill store metadata is invalid at {}: {}",
            path.display(),
            source
        ))
    })?;
    validate_skill_slug(&metadata.skill_id)?;
    validate_github_source(&metadata.source)?;
    if metadata.target != SKILL_SCOPE_GLOBAL && metadata.target != SKILL_SCOPE_WORKSPACE {
        return Err(ApiError::bad_request(format!(
            "invalid skill store metadata target: {}",
            metadata.target
        )));
    }
    Ok(Some(metadata))
}

fn write_skill_store_metadata(
    install_path: &Path,
    metadata: &SkillStoreInstallMetadata,
) -> Result<(), ApiError> {
    validate_skill_slug(&metadata.skill_id)?;
    validate_github_source(&metadata.source)?;
    // ponytail: this hidden file is the only update marker; updates re-fetch latest files and do not scan remote versions.
    let path = skill_store_metadata_path(install_path);
    let content = serde_json::to_string_pretty(metadata).map_err(|source| {
        ApiError::internal(format!(
            "failed to serialize skill store metadata: {source}"
        ))
    })?;
    fs::write(&path, content).map_err(|source| {
        ApiError::internal(format!(
            "failed to write skill store metadata {}: {}",
            path.display(),
            source
        ))
    })
}

fn skill_store_metadata_path(install_path: &Path) -> PathBuf {
    install_path.join(SKILL_STORE_METADATA_FILE_NAME)
}
pub(crate) fn install_target_root(
    user_profile_dir: &Path,
    config: &foco_store::config::GlobalConfig,
    target: &str,
    workspace_id: Option<&str>,
) -> Result<(PathBuf, Option<String>), ApiError> {
    match target.trim() {
        SKILL_SCOPE_GLOBAL => Ok((user_profile_dir.join(".agents").join("skills"), None)),
        SKILL_SCOPE_WORKSPACE => {
            let workspace_id = workspace_id.ok_or_else(|| {
                ApiError::bad_request("workspaceId is required when target is workspace")
            })?;
            let workspace = workspace_by_id(config, workspace_id)?;
            let workspace_path = workspace.local_path().ok_or_else(|| {
                ApiError::bad_request(
                    "installing skills into a remote workspace must be routed through its sidecar",
                )
            })?;
            Ok((
                workspace_path.join(".agents").join("skills"),
                Some(workspace.id.clone()),
            ))
        }
        other => Err(ApiError::bad_request(format!(
            "skill install target must be global or workspace: {other}"
        ))),
    }
}

pub(crate) fn install_skill_files_to_target_dir(
    target_root: &Path,
    skill_id: &str,
    files: &[SkillStoreFile],
    overwrite: bool,
) -> Result<PathBuf, ApiError> {
    let skill_id = validate_skill_slug(skill_id)?;
    ensure_skill_files_valid(files)?;
    validate_skill_markdown_matches_slug(&skill_id, files)?;

    fs::create_dir_all(target_root).map_err(|source| {
        ApiError::internal(format!(
            "failed to create skill install root {}: {}",
            target_root.display(),
            source
        ))
    })?;
    let destination = target_root.join(&skill_id);
    if destination.exists() && !overwrite {
        return Err(ApiError::conflict(format!(
            "skill already exists at {}",
            destination.display()
        )));
    }

    let temp_dir = target_root.join(format!(".{skill_id}.install-{}", unique_id("tmp")));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|source| {
            ApiError::internal(format!(
                "failed to clear temporary skill directory {}: {}",
                temp_dir.display(),
                source
            ))
        })?;
    }
    fs::create_dir_all(&temp_dir).map_err(|source| {
        ApiError::internal(format!(
            "failed to create temporary skill directory {}: {}",
            temp_dir.display(),
            source
        ))
    })?;

    let write_result = write_skill_files(&temp_dir, files);
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(error);
    }

    if destination.exists() {
        let backup_dir = target_root.join(format!(".{skill_id}.backup-{}", unique_id("tmp")));
        // ponytail: std has atomic directory rename but no cross-platform atomic non-empty directory replace; rollback is enough for local installs.
        fs::rename(&destination, &backup_dir).map_err(|source| {
            let _ = fs::remove_dir_all(&temp_dir);
            ApiError::internal(format!(
                "failed to prepare existing skill directory {} for replacement: {}",
                destination.display(),
                source
            ))
        })?;
        if let Err(source) = fs::rename(&temp_dir, &destination) {
            let _ = fs::rename(&backup_dir, &destination);
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(ApiError::internal(format!(
                "failed to replace skill directory {}: {}",
                destination.display(),
                source
            )));
        }
        let _ = fs::remove_dir_all(backup_dir);
    } else {
        fs::rename(&temp_dir, &destination).map_err(|source| {
            let _ = fs::remove_dir_all(&temp_dir);
            ApiError::internal(format!(
                "failed to install skill directory {}: {}",
                destination.display(),
                source
            ))
        })?;
    }

    Ok(destination)
}

fn write_skill_files(root: &Path, files: &[SkillStoreFile]) -> Result<(), ApiError> {
    for file in files {
        let relative = sanitize_skill_file_path(&file.path)?;
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                ApiError::internal(format!(
                    "failed to create skill file directory {}: {}",
                    parent.display(),
                    source
                ))
            })?;
        }
        fs::write(&path, file.decoded_content()?.as_ref()).map_err(|source| {
            ApiError::internal(format!(
                "failed to write skill file {}: {}",
                path.display(),
                source
            ))
        })?;
    }
    Ok(())
}

pub(crate) fn ensure_skill_files_valid(files: &[SkillStoreFile]) -> Result<(), ApiError> {
    if files.is_empty() {
        return Err(ApiError::bad_request("skill detail must include files"));
    }

    let mut has_skill_md = false;
    for file in files {
        let path = sanitize_skill_file_path(&file.path)?;
        file.decoded_content()?;
        if path == SKILL_FILE_NAME {
            file.text_content()?;
            has_skill_md = true;
        }
    }
    if !has_skill_md {
        return Err(ApiError::bad_request(format!(
            "skill files must include {SKILL_FILE_NAME}"
        )));
    }
    Ok(())
}

fn validate_skill_markdown_matches_slug(
    skill_id: &str,
    files: &[SkillStoreFile],
) -> Result<(), ApiError> {
    let skill_file = files
        .iter()
        .find(|file| sanitize_skill_file_path(&file.path).ok().as_deref() == Some(SKILL_FILE_NAME))
        .ok_or_else(|| {
            ApiError::bad_request(format!("skill files must include {SKILL_FILE_NAME}"))
        })?;
    let parsed = parse_skill_markdown(Path::new(SKILL_FILE_NAME), skill_file.text_content()?)
        .map_err(ApiError::bad_request)?;
    if parsed.id != skill_id {
        return Err(ApiError::bad_request(format!(
            "skill id '{}' does not match {SKILL_FILE_NAME} name '{}'",
            skill_id, parsed.id
        )));
    }
    Ok(())
}

pub(crate) fn validate_skill_slug(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::bad_request("skill id must not be empty"));
    }
    if value == "." || value == ".." || value.starts_with('.') {
        return Err(ApiError::bad_request(format!("invalid skill id: {value}")));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return Err(ApiError::bad_request(format!("invalid skill id: {value}")));
    }
    Ok(value.to_string())
}

pub(crate) fn sanitize_skill_file_path(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("skill file path must not be empty"));
    }
    let path = Path::new(&trimmed);
    if path.is_absolute() {
        return Err(ApiError::bad_request(format!(
            "skill file path must be relative: {trimmed}"
        )));
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                if part.is_empty()
                    || part == "."
                    || part == ".."
                    || part.starts_with('.')
                    || part.contains('\0')
                {
                    return Err(ApiError::bad_request(format!(
                        "unsafe skill file path: {trimmed}"
                    )));
                }
                parts.push(part.to_string());
            }
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(ApiError::bad_request(format!(
                    "unsafe skill file path: {trimmed}"
                )));
            }
        }
    }

    if parts.is_empty() {
        return Err(ApiError::bad_request("skill file path must not be empty"));
    }
    Ok(parts.join("/"))
}

fn validate_github_source(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    let parts = value.split('/').collect::<Vec<_>>();
    if parts.len() != 2
        || parts
            .iter()
            .any(|part| validate_repo_segment(part).is_err())
    {
        return Err(ApiError::bad_request(format!(
            "skill source must be a GitHub owner/repo pair: {value}"
        )));
    }
    Ok(value.to_string())
}

fn validate_repo_segment(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return Err(());
    }
    Ok(())
}

pub(crate) fn parse_skill_store_import_target(
    input: &str,
) -> Result<SkillStoreImportTarget, ApiError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ApiError::bad_request("import input must not be empty"));
    }

    if let Some(target) = parse_npx_skills_add_command(input)? {
        return Ok(target);
    }
    if let Some(target) = parse_skills_sh_skill_url(input)? {
        return Ok(target);
    }
    if let Some(source) = github_source_from_string(input) {
        return Ok(SkillStoreImportTarget {
            source,
            skill_id: None,
        });
    }

    Err(ApiError::bad_request(
        "paste a skills.sh skill URL, GitHub repository URL, or npx skills add command",
    ))
}

fn parse_npx_skills_add_command(input: &str) -> Result<Option<SkillStoreImportTarget>, ApiError> {
    let tokens = shell_words(input)?;
    let Some(add_index) = tokens
        .windows(2)
        .position(|window| window[0] == "skills" && window[1] == "add")
    else {
        return Ok(None);
    };
    if !tokens[..=add_index].iter().any(|token| token == "npx") {
        return Ok(None);
    }

    let mut source = None;
    let mut skill_id = None;
    let mut index = add_index + 2;
    while index < tokens.len() {
        let token = tokens[index].trim();
        if let Some(value) = token.strip_prefix("--skill=") {
            skill_id = Some(validate_skill_slug(value)?);
        } else if token == "--skill" {
            let value = tokens
                .get(index + 1)
                .ok_or_else(|| ApiError::bad_request("npx skills add --skill requires a value"))?;
            skill_id = Some(validate_skill_slug(value)?);
            index += 1;
        } else if source.is_none() {
            source = github_source_from_string(token);
        }
        index += 1;
    }

    let source = source.ok_or_else(|| {
        ApiError::bad_request("npx skills add command must include a GitHub repository URL")
    })?;
    Ok(Some(SkillStoreImportTarget { source, skill_id }))
}

fn shell_words(input: &str) -> Result<Vec<String>, ApiError> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (None, ch) if ch.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            (None, '\'' | '"') => quote = Some(ch),
            (Some(active), ch) if ch == active => quote = None,
            (_, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            _ => current.push(ch),
        }
    }
    if let Some(active) = quote {
        return Err(ApiError::bad_request(format!(
            "import command has an unterminated {active} quote"
        )));
    }
    if !current.is_empty() {
        words.push(current);
    }
    Ok(words)
}

fn parse_skills_sh_skill_url(input: &str) -> Result<Option<SkillStoreImportTarget>, ApiError> {
    let Some((host, mut segments)) = http_host_and_path(input) else {
        return Ok(None);
    };
    if host != "skills.sh" && host != "www.skills.sh" {
        return Ok(None);
    }
    if segments.first().map(String::as_str) == Some("skills") {
        segments.remove(0);
    }
    if segments.len() < 3 || segments[0] == "b" || segments[0] == "api" || segments[0] == "docs" {
        return Err(ApiError::bad_request(
            "skills.sh URL must point to a skill page like /owner/repo/skill",
        ));
    }

    let owner = &segments[0];
    let repo = segments[1].trim_end_matches(".git");
    let skill_id = &segments[2];
    let source = validate_github_source(&format!("{owner}/{repo}"))?;
    let skill_id = validate_skill_slug(skill_id)?;
    Ok(Some(SkillStoreImportTarget {
        source,
        skill_id: Some(skill_id),
    }))
}

fn http_host_and_path(input: &str) -> Option<(String, Vec<String>)> {
    let trimmed = input
        .trim()
        .trim_matches(|ch| matches!(ch, '`' | '\'' | '"' | ','));
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let (host, path) = without_scheme
        .split_once('/')
        .unwrap_or((without_scheme, ""));
    if host.is_empty() || host.contains('@') || host.contains(':') {
        return None;
    }
    let path = path
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.trim_end_matches(".git").to_string())
        .collect::<Vec<_>>();
    Some((host.to_ascii_lowercase(), path))
}

fn find_auto_github_skill_root(tree_items: &[Value], source: &str) -> Result<String, ApiError> {
    let skill_file_suffix = format!("/{SKILL_FILE_NAME}");
    let mut candidates = tree_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("blob"))
        .filter_map(|item| item.get("path").and_then(Value::as_str))
        .filter(|path| path.ends_with(&skill_file_suffix) || *path == SKILL_FILE_NAME)
        .map(|path| {
            path.strip_suffix(&skill_file_suffix)
                .unwrap_or("")
                .to_string()
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();

    if candidates.len() == 1 {
        return Ok(candidates.remove(0));
    }
    if candidates.is_empty() {
        return Err(ApiError::bad_request(format!(
            "GitHub tree did not contain {SKILL_FILE_NAME}"
        )));
    }

    let repo_name = source.rsplit('/').next().unwrap_or_default();
    let mut repo_matches = candidates
        .iter()
        .filter(|root| root.rsplit('/').next().unwrap_or_default() == repo_name)
        .cloned()
        .collect::<Vec<_>>();
    if repo_matches.len() == 1 {
        return Ok(repo_matches.remove(0));
    }

    Err(ApiError::bad_request(format!(
        "GitHub source '{source}' contains multiple {SKILL_FILE_NAME} files; pass --skill to choose one"
    )))
}

fn skill_id_from_files(files: &[SkillStoreFile]) -> Option<String> {
    files
        .iter()
        .find(|file| file.path == SKILL_FILE_NAME)
        .and_then(|file| file.text_content().ok())
        .and_then(|content| parse_skill_markdown(Path::new(SKILL_FILE_NAME), content).ok())
        .map(|parsed| parsed.id)
}

fn detail_response_from_files(
    fallback_id: String,
    source: String,
    files: Vec<SkillStoreFile>,
) -> SkillStoreDetailResponse {
    let id = skill_id_from_files(&files).unwrap_or(fallback_id);
    SkillStoreDetailResponse {
        name: id.clone(),
        description: String::new(),
        source: Some(source),
        id,
        files,
    }
}

fn find_github_skill_root(tree_items: &[Value], skill_id: &str) -> Result<String, ApiError> {
    let skill_file_suffix = format!("/{SKILL_FILE_NAME}");
    let mut candidates = tree_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("blob"))
        .filter_map(|item| item.get("path").and_then(Value::as_str))
        .filter(|path| path.ends_with(&skill_file_suffix) || *path == SKILL_FILE_NAME)
        .filter_map(|path| {
            let root = path.strip_suffix(&skill_file_suffix).unwrap_or("");
            if path == SKILL_FILE_NAME
                || root
                    .rsplit('/')
                    .next()
                    .map(|segment| segment == skill_id)
                    .unwrap_or(false)
            {
                Some(root.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| candidate.matches('/').count());
    candidates.into_iter().next().ok_or_else(|| {
        ApiError::bad_request(format!(
            "GitHub tree did not contain {SKILL_FILE_NAME} for skill '{skill_id}'"
        ))
    })
}

pub(crate) fn github_skill_paths_for_root(
    tree_items: &[Value],
    skill_root: &str,
) -> Result<std::collections::BTreeSet<String>, ApiError> {
    let mut paths = std::collections::BTreeSet::new();
    for github_path in tree_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("blob"))
        .filter_map(|item| item.get("path").and_then(Value::as_str))
        .filter(|path| github_path_is_under_root(path, skill_root))
    {
        let relative = github_relative_path(github_path, skill_root)?;
        let Ok(relative) = sanitize_skill_file_path(relative) else {
            continue;
        };
        paths.insert(relative);
    }
    if paths.len() > GITHUB_SKILL_ARCHIVE_MAX_FILES {
        return Err(ApiError::bad_request(format!(
            "GitHub skill contains too many files ({}; max {})",
            paths.len(),
            GITHUB_SKILL_ARCHIVE_MAX_FILES
        )));
    }
    Ok(paths)
}

fn github_relative_path<'a>(github_path: &'a str, skill_root: &str) -> Result<&'a str, ApiError> {
    if skill_root.is_empty() {
        Ok(github_path)
    } else {
        github_path
            .strip_prefix(skill_root)
            .and_then(|path| path.strip_prefix('/'))
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "GitHub archive path is outside the selected skill root: {github_path}"
                ))
            })
    }
}

pub(crate) fn extract_github_skill_archive(
    archive: &[u8],
    skill_root: &str,
    expected_paths: &std::collections::BTreeSet<String>,
) -> Result<Vec<SkillStoreFile>, ApiError> {
    if archive.len() > GITHUB_SKILL_ARCHIVE_MAX_COMPRESSED_BYTES {
        return Err(ApiError::bad_request(format!(
            "GitHub skill archive exceeds the compressed size limit of {} bytes",
            GITHUB_SKILL_ARCHIVE_MAX_COMPRESSED_BYTES
        )));
    }
    let decoder = GzDecoder::new(Cursor::new(archive));
    let mut tar_archive = tar::Archive::new(decoder);
    let entries = tar_archive.entries().map_err(|source| {
        ApiError::bad_request(format!("GitHub skill archive is invalid: {source}"))
    })?;
    let mut files = Vec::with_capacity(expected_paths.len());
    let mut seen_paths = std::collections::BTreeSet::new();
    let mut extracted_bytes = 0_u64;

    for entry in entries {
        let mut entry = entry.map_err(|source| {
            ApiError::bad_request(format!("GitHub skill archive entry is invalid: {source}"))
        })?;
        let path = entry.path().map_err(|source| {
            ApiError::bad_request(format!("GitHub skill archive path is invalid: {source}"))
        })?;
        validate_archive_entry_path(&path)?;
        let entry_type = entry.header().entry_type();
        let mut components = path.components();
        let Some(Component::Normal(_archive_root)) = components.next() else {
            return Err(ApiError::bad_request(
                "GitHub skill archive entry is missing its top-level directory",
            ));
        };
        let repository_path = components.as_path();
        if repository_path.as_os_str().is_empty() {
            continue;
        }
        let repository_path = repository_path
            .to_str()
            .ok_or_else(|| ApiError::bad_request("GitHub skill archive path is not valid UTF-8"))?;
        if !github_path_is_under_root(repository_path, skill_root) {
            continue;
        }
        if !(entry_type.is_file() || entry_type.is_dir()) {
            return Err(ApiError::bad_request(
                "GitHub skill archive contains an unsupported link or special entry inside the selected skill root",
            ));
        }
        if entry_type.is_dir() {
            continue;
        }
        let raw_relative = github_relative_path(repository_path, skill_root)?;
        let Ok(relative) = sanitize_skill_file_path(raw_relative) else {
            continue;
        };
        if !expected_paths.contains(&relative) {
            return Err(ApiError::bad_request(format!(
                "GitHub skill archive contained an unexpected file inside the selected skill root: {relative}"
            )));
        }
        if !seen_paths.insert(relative.clone()) {
            return Err(ApiError::bad_request(format!(
                "GitHub skill archive contains a duplicate file: {relative}"
            )));
        }
        if seen_paths.len() > GITHUB_SKILL_ARCHIVE_MAX_FILES {
            return Err(ApiError::bad_request(format!(
                "GitHub skill archive contains too many files (max {})",
                GITHUB_SKILL_ARCHIVE_MAX_FILES
            )));
        }
        let size = entry.header().size().map_err(|source| {
            ApiError::bad_request(format!(
                "GitHub skill archive file size is invalid: {source}"
            ))
        })?;
        if size > GITHUB_SKILL_ARCHIVE_MAX_FILE_BYTES {
            return Err(ApiError::bad_request(format!(
                "GitHub skill file '{relative}' exceeds the per-file size limit of {} bytes",
                GITHUB_SKILL_ARCHIVE_MAX_FILE_BYTES
            )));
        }
        extracted_bytes = extracted_bytes.checked_add(size).ok_or_else(|| {
            ApiError::bad_request("GitHub skill archive extracted size overflowed")
        })?;
        if extracted_bytes > GITHUB_SKILL_ARCHIVE_MAX_EXTRACTED_BYTES {
            return Err(ApiError::bad_request(format!(
                "GitHub skill archive exceeds the extracted size limit of {} bytes",
                GITHUB_SKILL_ARCHIVE_MAX_EXTRACTED_BYTES
            )));
        }
        let mut content = Vec::with_capacity(size as usize);
        entry.read_to_end(&mut content).map_err(|source| {
            ApiError::bad_request(format!(
                "failed to read GitHub skill archive file '{relative}': {source}"
            ))
        })?;
        if content.len() as u64 != size {
            return Err(ApiError::bad_request(format!(
                "GitHub skill archive file '{relative}' size did not match its header"
            )));
        }
        files.push(SkillStoreFile::from_bytes(relative, content));
    }

    let missing = expected_paths.difference(&seen_paths).next().cloned();
    if let Some(path) = missing {
        return Err(ApiError::bad_request(format!(
            "GitHub skill archive did not contain expected file '{path}'"
        )));
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn validate_archive_entry_path(path: &Path) -> Result<(), ApiError> {
    if path.is_absolute() {
        return Err(ApiError::bad_request(
            "GitHub skill archive contains an absolute path",
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(part) if !part.is_empty() && part != "." && part != ".." => {}
            _ => {
                return Err(ApiError::bad_request(
                    "GitHub skill archive contains an unsafe path",
                ));
            }
        }
    }
    Ok(())
}

fn github_path_is_under_root(path: &str, root: &str) -> bool {
    if root.is_empty() {
        true
    } else {
        path == root || path.starts_with(&format!("{root}/"))
    }
}

fn detail_response_from_value(
    value: Value,
    fallback_id: &str,
    fallback_source: Option<&str>,
) -> SkillStoreDetailResponse {
    let id = string_field(&value, &["skillId", "id", "slug", "name"])
        .unwrap_or_else(|| fallback_id.to_string());
    let files = files_from_value(&value);
    SkillStoreDetailResponse {
        name: string_field(&value, &["name", "title"]).unwrap_or_else(|| id.clone()),
        description: string_field(&value, &["description", "summary"]).unwrap_or_default(),
        source: registry_source_from_value(&value).or_else(|| fallback_source.map(str::to_string)),
        id,
        files,
    }
}

pub(crate) fn registry_source_from_value(value: &Value) -> Option<String> {
    registry_value_candidates(value)
        .into_iter()
        .find_map(github_source_from_registry_value)
}

fn github_source_from_registry_value(value: &Value) -> Option<String> {
    let owner = string_field(
        value,
        &[
            "owner",
            "githubOwner",
            "repoOwner",
            "repositoryOwner",
            "sourceOwner",
        ],
    );
    let repo = string_field(
        value,
        &[
            "repo",
            "repository",
            "githubRepo",
            "repoName",
            "repositoryName",
            "sourceRepo",
        ],
    );
    if let (Some(owner), Some(repo)) = (owner, repo) {
        let source = format!("{owner}/{repo}");
        if validate_github_source(&source).is_ok() {
            return Some(source);
        }
    }

    value
        .get("repository")
        .and_then(Value::as_object)
        .and_then(|repository| {
            let owner = repository
                .get("owner")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            let repo = repository
                .get("repo")
                .or_else(|| repository.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            github_source_from_string(&format!("{owner}/{repo}"))
        })
        .or_else(|| {
            string_field(
                value,
                &[
                    "source",
                    "repository",
                    "repo",
                    "github",
                    "githubUrl",
                    "repositoryUrl",
                ],
            )
            .and_then(|source| github_source_from_string(&source))
        })
}

fn github_source_from_string(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches(".git");
    if validate_github_source(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    for prefix in [
        "https://github.com/",
        "http://github.com/",
        "https://www.github.com/",
        "http://www.github.com/",
        "github.com/",
        "www.github.com/",
        "git@github.com:",
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let source = rest
                .split(['?', '#'])
                .next()
                .unwrap_or(rest)
                .trim_end_matches('/');
            let source = source.split('/').take(2).collect::<Vec<_>>().join("/");
            if validate_github_source(&source).is_ok() {
                return Some(source);
            }
        }
    }
    None
}

fn files_from_value(value: &Value) -> Vec<SkillStoreFile> {
    registry_files_from_value(value)
}

pub(crate) fn registry_files_from_value(value: &Value) -> Vec<SkillStoreFile> {
    registry_value_candidates(value)
        .into_iter()
        .find_map(files_from_registry_candidate)
        .unwrap_or_default()
}

fn files_from_registry_candidate(value: &Value) -> Option<Vec<SkillStoreFile>> {
    if let Some(files) = value.as_array().map(|files| files_from_array(files)) {
        if !files.is_empty() {
            return Some(files);
        }
    }

    let files_value = value.get("files")?;
    if let Some(files) = files_value.as_array().map(|files| files_from_array(files)) {
        return Some(files);
    }
    files_value.as_object().map(|files| {
        files
            .iter()
            .filter_map(|(path, content)| {
                let content = content.as_str()?.to_string();
                Some(SkillStoreFile::text(path.clone(), content))
            })
            .collect()
    })
}

fn files_from_array(files: &[Value]) -> Vec<SkillStoreFile> {
    files
        .iter()
        .filter_map(|file| {
            let path = string_field(file, &["path", "name", "filename"])?;
            let content = raw_string_field(file, &["content", "text", "body"])?;
            let content_encoding = file
                .get("contentEncoding")
                .or_else(|| file.get("content_encoding"))
                .and_then(Value::as_str)
                .map(str::to_string);
            Some(SkillStoreFile {
                path,
                content,
                content_encoding,
            })
        })
        .collect()
}

fn registry_value_candidates(value: &Value) -> Vec<&Value> {
    let mut candidates = Vec::new();
    candidates.push(value);
    for key in ["data", "result", "skill"] {
        if let Some(candidate) = value.get(key) {
            candidates.push(candidate);
            if let Some(skill) = candidate.get("skill") {
                candidates.push(skill);
            }
        }
    }
    candidates
}

fn skill_files_have_skill_md(files: &[SkillStoreFile]) -> bool {
    files
        .iter()
        .any(|file| sanitize_skill_file_path(&file.path).ok().as_deref() == Some(SKILL_FILE_NAME))
}

fn list_response_from_value(value: Value, source: &str) -> SkillStoreListResponse {
    let skills_value = value
        .get("skills")
        .or_else(|| value.get("data"))
        .or_else(|| value.get("items"))
        .unwrap_or(&value);
    let skills = skills_value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(skill_summary_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let total = value
        .get("total")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(skills.len());
    let has_more = bool_field(&value, &["hasMore", "has_more"]).unwrap_or_else(|| {
        match (
            u64_field(&value, &["page"]),
            u64_field(&value, &["totalPages", "total_pages"]),
        ) {
            (Some(page), Some(total_pages)) => page < total_pages,
            _ => false,
        }
    });

    SkillStoreListResponse {
        skills,
        total,
        has_more,
        source: source.to_string(),
    }
}

fn skill_summary_from_value(value: &Value) -> Option<SkillStoreSkillSummary> {
    let id = string_field(value, &["skillId", "id", "slug", "name"])?;
    let name = string_field(value, &["name", "title"]).unwrap_or_else(|| id.clone());
    let description = string_field(value, &["description", "summary"]).unwrap_or_default();
    Some(SkillStoreSkillSummary {
        id,
        name,
        description,
        source: string_field(value, &["source", "repository", "repo"]),
        installs: u64_field(value, &["installs", "installCount"]),
        installs_yesterday: u64_field(value, &["installsYesterday", "installs_yesterday"]),
        change: i64_field(value, &["change"]),
        official: bool_field(value, &["isOfficial", "official"]).unwrap_or(false),
    })
}

fn string_field(value: &Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        value
            .get(*name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn raw_string_field(value: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str).map(str::to_string))
}

fn u64_field(value: &Value, names: &[&str]) -> Option<u64> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_u64))
}

fn i64_field(value: &Value, names: &[&str]) -> Option<i64> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_i64))
}

fn bool_field(value: &Value, names: &[&str]) -> Option<bool> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_bool))
}

fn network_error(error: reqwest::Error) -> ApiError {
    ApiError::bad_request(format!("skill store request failed: {error}"))
}

fn url_query_value(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn url_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
