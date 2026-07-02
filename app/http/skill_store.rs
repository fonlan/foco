use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
};
use foco_store::config::{SKILL_SCOPE_GLOBAL, SKILL_SCOPE_WORKSPACE, SkillSettings};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ApiError, AppState, config_snapshot, discover_skills, merge_disabled_skill_keys,
    refresh_derived_enabled_skills, save_config, skills::parse_skill_markdown, unique_id,
    workspace_by_id,
};

const DEFAULT_SKILLS_SH_BASE_URL: &str = "https://skills.sh";
const DEFAULT_SKILLS_API_BASE_URL: &str = "https://skills-api.deeptoai.com";
const DEFAULT_GITHUB_API_BASE_URL: &str = "https://api.github.com";
const DEFAULT_GITHUB_RAW_BASE_URL: &str = "https://raw.githubusercontent.com";
const SKILL_FILE_NAME: &str = "SKILL.md";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreSearchQuery {
    query: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreDetailQuery {
    source: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreInstallRequest {
    skill_id: String,
    source: Option<String>,
    target: String,
    workspace_id: Option<String>,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    files: Vec<SkillStoreFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreFile {
    pub(crate) path: String,
    pub(crate) content: String,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillStoreInstallResponse {
    target: String,
    workspace_id: Option<String>,
    path: String,
    detected: Vec<SkillSettings>,
}

#[derive(Clone)]
struct SkillStoreClient {
    http: reqwest::Client,
    skills_base_url: String,
    skills_api_base_url: String,
    github_api_base_url: String,
    github_raw_base_url: String,
    token: Option<String>,
}

impl SkillStoreClient {
    fn from_env() -> Self {
        Self {
            http: reqwest::Client::new(),
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
            github_raw_base_url: env::var("SKILLS_STORE_GITHUB_RAW_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_GITHUB_RAW_BASE_URL.to_string()),
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

    async fn search(&self, query: &str) -> Result<SkillStoreListResponse, ApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(ApiError::bad_request("search query must not be empty"));
        }
        let encoded_query = url_query_value(query);
        if self.token.is_some() {
            let url = format!(
                "{}?query={}&q={}",
                self.skills_url("/api/v1/skills/search"),
                encoded_query,
                encoded_query
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

        self.skills_api_search(query).await
    }

    async fn skills_api_search(&self, query: &str) -> Result<SkillStoreListResponse, ApiError> {
        let encoded_query = url_query_value(query);
        let url = format!(
            "{}?query={}&q={}",
            self.skills_api_url("/api/skills"),
            encoded_query,
            encoded_query
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
                            tracing::warn!(source, error = %error.message(), "skills-api file request failed; trying GitHub fallback");
                        }
                    }

                    if let Ok(files) = self.github_skill_files(&source, &skill_id).await {
                        detail.files = files;
                        return Ok(detail);
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

        let tree_items = tree
            .get("tree")
            .and_then(Value::as_array)
            .ok_or_else(|| ApiError::bad_request("GitHub tree response did not include files"))?;
        let skill_root = find_github_skill_root(tree_items, skill_id)?;
        let mut paths = tree_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("blob"))
            .filter_map(|item| item.get("path").and_then(Value::as_str))
            .filter(|path| github_path_is_under_root(*path, &skill_root))
            .map(|path| path.to_string())
            .collect::<Vec<_>>();
        paths.sort();

        let mut files = Vec::with_capacity(paths.len());
        for github_path in paths {
            let relative = github_path
                .strip_prefix(&skill_root)
                .unwrap_or(&github_path)
                .trim_start_matches('/');
            let clean_path = sanitize_skill_file_path(if relative.is_empty() {
                SKILL_FILE_NAME
            } else {
                relative
            })?;
            let raw_url = format!(
                "{}/{}/{}/{}",
                self.github_raw_base_url.trim_end_matches('/'),
                source,
                branch,
                github_path
                    .split('/')
                    .map(url_segment)
                    .collect::<Vec<_>>()
                    .join("/")
            );
            let content = self
                .http
                .get(raw_url)
                .send()
                .await
                .map_err(network_error)?
                .error_for_status()
                .map_err(network_error)?
                .text()
                .await
                .map_err(network_error)?;
            files.push(SkillStoreFile {
                path: clean_path,
                content,
            });
        }
        ensure_skill_files_valid(&files)?;
        Ok(files)
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

pub(crate) async fn skill_store_search(
    Query(query): Query<SkillStoreSearchQuery>,
) -> Result<Json<SkillStoreListResponse>, ApiError> {
    Ok(Json(
        SkillStoreClient::from_env().search(&query.query).await?,
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

pub(crate) async fn skill_store_install(
    State(state): State<AppState>,
    Json(request): Json<SkillStoreInstallRequest>,
) -> Result<Json<SkillStoreInstallResponse>, ApiError> {
    let skill_id = validate_skill_slug(&request.skill_id)?;
    let files = if request.files.is_empty() {
        SkillStoreClient::from_env()
            .detail(&skill_id, request.source.as_deref())
            .await?
            .files
    } else {
        request.files
    };
    ensure_skill_files_valid(&files)?;
    validate_skill_markdown_matches_slug(&skill_id, &files)?;

    let mut config = config_snapshot(&state)?;
    let (target_root, workspace_id) = install_target_root(
        &state.user_profile_dir,
        &config,
        &request.target,
        request.workspace_id.as_deref(),
    )?;
    let install_path =
        install_skill_files_to_target_dir(&target_root, &skill_id, &files, request.overwrite)?;

    let discovery = discover_skills(&state.user_profile_dir, &config.workspaces);
    config.skills.detected = discovery.skills.clone();
    config.skills.disabled =
        merge_disabled_skill_keys(config.skills.disabled, &discovery.required_disabled);
    refresh_derived_enabled_skills(&mut config);
    save_config(&state, config)?;

    Ok(Json(SkillStoreInstallResponse {
        target: request.target,
        workspace_id,
        path: install_path.display().to_string(),
        detected: discovery.skills,
    }))
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
            Ok((
                workspace.path.join(".agents").join("skills"),
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
        fs::write(&path, &file.content).map_err(|source| {
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
        if path == SKILL_FILE_NAME {
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
    let parsed = parse_skill_markdown(Path::new(SKILL_FILE_NAME), &skill_file.content)
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
        "git@github.com:",
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let source = rest.trim_end_matches('/');
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
                Some(SkillStoreFile {
                    path: path.clone(),
                    content,
                })
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
            Some(SkillStoreFile { path, content })
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
    let has_more = value
        .get("hasMore")
        .or_else(|| value.get("has_more"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

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
