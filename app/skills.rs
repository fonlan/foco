use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use foco_providers::{NeutralChatMessage, NeutralChatRole};
use foco_store::config::{
    GlobalConfig, SKILL_SCOPE_GLOBAL, SKILL_SCOPE_WORKSPACE, SkillSettings, WorkspaceConfig,
    WorkspaceLocation,
};
use foco_tools::output_budget::{SELECTED_SKILLS_MAX_TOTAL_BYTES, path_is_skill_md};
use serde::{Deserialize, Serialize};

const MAX_SKILL_MD_SOURCE_BYTES: u64 = 32 * 1024 * 1024;

use crate::{ApiError, markdown_code_block, neutral_text_message};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillDiscoveryErrorSummary {
    pub(crate) path: String,
    pub(crate) message: String,
}

pub(crate) struct SkillDiscovery {
    pub(crate) skills: Vec<SkillSettings>,
    pub(crate) errors: Vec<SkillDiscoveryErrorSummary>,
    pub(crate) required_disabled: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SkillSearchRoot {
    pub(crate) id: String,
    pub(crate) directory: PathBuf,
    scope: &'static str,
    workspace_id: Option<String>,
    workspace_name: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ParsedSkillFile {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) markdown: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SkillPromptEntry {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) scope: String,
    pub(crate) path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectedSkillPromptEntry {
    pub(crate) prompt: SkillPromptEntry,
    pub(crate) content_markdown: String,
}

pub(crate) fn skill_prompt_entry_from_settings(skill: &SkillSettings) -> SkillPromptEntry {
    SkillPromptEntry {
        key: skill.key.clone(),
        name: skill.name.clone(),
        description: skill.description.clone(),
        scope: skill_scope_prompt_label(skill),
        path: skill.path.display().to_string(),
    }
}

pub(crate) fn selected_skill_prompt_entry(
    prompt: SkillPromptEntry,
    content_markdown: impl Into<String>,
) -> SelectedSkillPromptEntry {
    SelectedSkillPromptEntry {
        prompt,
        content_markdown: content_markdown.into().trim().to_string(),
    }
}

pub(crate) fn format_selected_skills_message(
    entries: &[SelectedSkillPromptEntry],
    message: &str,
) -> String {
    let metadata = serde_json::to_string_pretty(
        &entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "name": entry.prompt.name,
                    "path": entry.prompt.path,
                })
            })
            .collect::<Vec<_>>(),
    )
    .expect("selected skill metadata is always JSON serializable");
    let instructions = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            format!(
                "## Skill {}: {}\n\nPath: `{}`\n\n### Instructions\n\n{}",
                index + 1,
                entry.prompt.name,
                entry.prompt.path,
                entry.content_markdown
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "# Selected Skills\n\n{}\n\n{}\n\n## End Selected Skills\n\n{}",
        markdown_code_block("json", &metadata),
        instructions,
        message,
    )
}

pub(crate) fn message_with_selected_skills(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
    requested_skill_keys: Option<Vec<String>>,
    message: &str,
) -> Result<String, ApiError> {
    let Some(requested_skill_keys) = requested_skill_keys else {
        return Ok(message.to_string());
    };
    let requested_skill_keys = normalize_skill_keys(requested_skill_keys)?;
    if requested_skill_keys.is_empty() {
        return Ok(message.to_string());
    }

    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, config);
    let required_disabled_ids = discovery
        .required_disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    let available_skills = discovery
        .skills
        .iter()
        .filter(|skill| skill_applies_to_workspace(skill, workspace_id))
        .collect::<Vec<_>>();
    let skills_by_key = available_skills
        .iter()
        .map(|skill| (skill.key.as_str(), *skill))
        .collect::<HashMap<_, _>>();
    let mut entries = Vec::with_capacity(requested_skill_keys.len());
    for skill_key in requested_skill_keys {
        let skill = match skills_by_key.get(skill_key.as_str()).copied() {
            Some(skill) => skill,
            None => unique_skill_by_legacy_id(&available_skills, &skill_key)?,
        };
        if skill_is_disabled(skill, &disabled_ids)
            || skill_is_required_disabled(skill, &required_disabled_ids)
        {
            return Err(ApiError::bad_request(format!(
                "selected skill '{}' is disabled",
                skill.key
            )));
        }

        let parsed = parse_skill_file(&skill.path).map_err(ApiError::bad_request)?;
        if parsed.id != skill.id {
            return Err(ApiError::bad_request(format!(
                "selected skill '{}' file now declares skill id '{}'",
                skill.key, parsed.id
            )));
        }

        let prompt = SkillPromptEntry {
            key: skill.key.clone(),
            name: parsed.name,
            description: parsed.description,
            scope: skill_scope_prompt_label(skill),
            path: skill.path.display().to_string(),
        };
        entries.push(selected_skill_prompt_entry(prompt, parsed.markdown));
    }

    validate_selected_skills_total_budget(&entries).map_err(ApiError::bad_request)?;
    Ok(format_selected_skills_message(&entries, message))
}

fn normalize_skill_keys(values: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut keys = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
        let key = value.trim();

        if key.is_empty() {
            continue;
        }

        validate_skill_key(key).map_err(ApiError::bad_request)?;
        if seen.insert(key.to_string()) {
            keys.push(key.to_string());
        }
    }

    Ok(keys)
}

pub(crate) fn normalize_manual_disabled_skill_ids(
    requested_disabled: Option<Vec<String>>,
    requested_enabled: Option<Vec<String>>,
    discovered_skills: &[SkillSettings],
) -> Result<Vec<String>, ApiError> {
    let discovered_keys = discovered_skills
        .iter()
        .map(|skill| skill.key.as_str())
        .collect::<HashSet<_>>();

    if let Some(values) = requested_disabled {
        let disabled = normalize_skill_keys(values)?;

        for key in &disabled {
            if !discovered_keys.contains(key.as_str()) {
                return Err(ApiError::bad_request(format!(
                    "disabled skill was not found: {key}"
                )));
            }
        }

        if let Some(enabled_values) = requested_enabled {
            let enabled = normalize_skill_keys(enabled_values)?;
            let enabled_keys = enabled.iter().map(String::as_str).collect::<HashSet<_>>();
            if let Some(key) = disabled
                .iter()
                .find(|key| enabled_keys.contains(key.as_str()))
            {
                return Err(ApiError::bad_request(format!(
                    "skill cannot be both enabled and disabled: {key}"
                )));
            }
        }

        return Ok(disabled);
    }

    if let Some(values) = requested_enabled {
        let enabled = normalize_skill_keys(values)?;
        let enabled_ids = enabled.iter().map(String::as_str).collect::<HashSet<_>>();
        for key in &enabled {
            if !discovered_keys.contains(key.as_str()) {
                return Err(ApiError::bad_request(format!(
                    "enabled skill was not found: {key}"
                )));
            }
        }

        return Ok(discovered_skills
            .iter()
            .filter(|skill| !enabled_ids.contains(skill.key.as_str()))
            .map(|skill| skill.key.clone())
            .collect());
    }

    Ok(Vec::new())
}

pub(crate) fn merge_disabled_skill_keys(
    existing_disabled: Vec<String>,
    required_disabled: &[String],
) -> Vec<String> {
    let mut disabled = Vec::new();
    let mut seen = HashSet::new();

    for key in existing_disabled
        .into_iter()
        .chain(required_disabled.iter().cloned())
    {
        if seen.insert(key.clone()) {
            disabled.push(key);
        }
    }

    disabled
}

pub(crate) fn preserve_disabled_skill_keys_for_hidden_locations(
    existing_disabled: Vec<String>,
    discovered_skills: &[SkillSettings],
) -> Vec<String> {
    let visible_keys = discovered_skills
        .iter()
        .map(|skill| skill.key.as_str())
        .collect::<HashSet<_>>();

    existing_disabled
        .into_iter()
        .filter(|key| !visible_keys.contains(key.as_str()))
        .collect()
}

pub(crate) fn merge_manual_disabled_skill_keys(
    existing_disabled: Vec<String>,
    requested_disabled: Vec<String>,
    discovered_skills: &[SkillSettings],
) -> Vec<String> {
    let mut disabled =
        preserve_disabled_skill_keys_for_hidden_locations(existing_disabled, discovered_skills);
    disabled.extend(requested_disabled);
    disabled.sort();
    disabled.dedup();
    disabled
}

pub(crate) fn refresh_derived_enabled_skills(config: &mut GlobalConfig, user_profile_dir: &Path) {
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, config);

    config.skills.enabled = discovery
        .skills
        .iter()
        .filter(|skill| !skill_is_disabled(skill, &disabled_ids))
        .map(|skill| skill.key.clone())
        .collect();
}

pub(crate) fn discover_skills(user_profile_dir: &Path, config: &GlobalConfig) -> SkillDiscovery {
    let disabled_location_ids = config
        .skills
        .disabled_locations
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    discover_skills_in_roots(
        skill_search_roots(user_profile_dir, &config.workspaces)
            .into_iter()
            .filter(|root| !disabled_location_ids.contains(root.id.as_str())),
    )
}

#[cfg(test)]
pub(crate) fn discover_skills_in_all_locations(
    user_profile_dir: &Path,
    workspaces: &[WorkspaceConfig],
) -> SkillDiscovery {
    discover_skills_in_roots(skill_search_roots(user_profile_dir, workspaces))
}

pub(crate) fn discover_workspace_skills_for_path(
    workspace_id: &str,
    workspace_name: &str,
    workspace_path: &Path,
) -> SkillDiscovery {
    let roots = [
        SkillSearchRoot {
            id: workspace_skill_location_id(workspace_id, "agents"),
            directory: workspace_path.join(".agents").join("skills"),
            scope: SKILL_SCOPE_WORKSPACE,
            workspace_id: Some(workspace_id.to_string()),
            workspace_name: Some(workspace_name.to_string()),
        },
        SkillSearchRoot {
            id: workspace_skill_location_id(workspace_id, "claude"),
            directory: workspace_path.join(".claude").join("skills"),
            scope: SKILL_SCOPE_WORKSPACE,
            workspace_id: Some(workspace_id.to_string()),
            workspace_name: Some(workspace_name.to_string()),
        },
    ];
    discover_skills_in_roots(roots)
}

/// Discover only global skills under `user_profile_dir/.agents/skills`.
///
/// Produces the same `global:<id>` keys, `scope=global`, absolute paths, and
/// frontmatter/duplicate/invalid/required-disabled handling as full discovery.
pub(crate) fn discover_global_skills_for_profile(user_profile_dir: &Path) -> SkillDiscovery {
    discover_skills_in_roots([global_skill_search_root(user_profile_dir)])
}

pub(crate) fn global_skill_search_root(user_profile_dir: &Path) -> SkillSearchRoot {
    SkillSearchRoot {
        id: "global:agents".to_string(),
        directory: user_profile_dir.join(".agents").join("skills"),
        scope: SKILL_SCOPE_GLOBAL,
        workspace_id: None,
        workspace_name: None,
    }
}

fn discover_skills_in_roots(roots: impl IntoIterator<Item = SkillSearchRoot>) -> SkillDiscovery {
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    let mut invalid_skills = Vec::new();
    let mut required_disabled = Vec::new();
    let mut seen_keys = HashSet::new();

    for root in roots {
        let candidates = match skill_file_candidates(&root.directory) {
            Ok(candidates) => candidates,
            Err(message) => {
                errors.push(SkillDiscoveryErrorSummary {
                    path: root.directory.display().to_string(),
                    message,
                });
                continue;
            }
        };

        for path in candidates {
            match parse_skill_file_frontmatter(&path) {
                Ok(parsed) => {
                    let key = skill_key(&root, &parsed.id);
                    if !seen_keys.insert(key.clone()) {
                        errors.push(SkillDiscoveryErrorSummary {
                            path: path.display().to_string(),
                            message: format!(
                                "duplicate skill id '{}' in {} skill scope",
                                parsed.id,
                                skill_scope_label(&root)
                            ),
                        });
                        continue;
                    }

                    skills.push(skill_settings_from_parsed(&root, path, parsed));
                }
                Err(message) => {
                    if let Some(skill) = disabled_skill_settings_from_invalid_file(&root, &path) {
                        invalid_skills.push(skill);
                    }
                    errors.push(SkillDiscoveryErrorSummary {
                        path: path.display().to_string(),
                        message,
                    });
                }
            }
        }
    }

    let mut seen_invalid_keys = HashSet::new();
    for skill in invalid_skills {
        if seen_keys.contains(skill.key.as_str()) || !seen_invalid_keys.insert(skill.key.clone()) {
            continue;
        }

        required_disabled.push(skill.key.clone());
        skills.push(skill);
    }

    skills.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then_with(|| left.workspace_name.cmp(&right.workspace_name))
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.path.cmp(&right.path))
    });

    required_disabled.sort();
    required_disabled.dedup();

    SkillDiscovery {
        skills,
        errors,
        required_disabled,
    }
}

fn skill_settings_from_parsed(
    root: &SkillSearchRoot,
    path: PathBuf,
    parsed: ParsedSkillFile,
) -> SkillSettings {
    let key = skill_key(root, &parsed.id);

    SkillSettings {
        key,
        id: parsed.id,
        name: parsed.name,
        description: parsed.description,
        path,
        scope: root.scope.to_string(),
        workspace_id: root.workspace_id.clone(),
        workspace_name: root.workspace_name.clone(),
    }
}

fn disabled_skill_settings_from_invalid_file(
    root: &SkillSearchRoot,
    path: &Path,
) -> Option<SkillSettings> {
    let id = parse_skill_file_id(path).ok()?;
    let key = skill_key(root, &id);

    Some(SkillSettings {
        key,
        id: id.clone(),
        name: id,
        description: "Invalid skill frontmatter.".to_string(),
        path: path.to_path_buf(),
        scope: root.scope.to_string(),
        workspace_id: root.workspace_id.clone(),
        workspace_name: root.workspace_name.clone(),
    })
}

pub(crate) fn skill_search_roots(
    user_profile_dir: &Path,
    workspaces: &[WorkspaceConfig],
) -> Vec<SkillSearchRoot> {
    let mut roots = vec![global_skill_search_root(user_profile_dir)];

    for workspace in workspaces {
        let Some(workspace_path) = workspace.local_path() else {
            continue;
        };
        for (location, directory) in [
            ("agents", workspace_path.join(".agents").join("skills")),
            ("claude", workspace_path.join(".claude").join("skills")),
        ] {
            roots.push(SkillSearchRoot {
                id: workspace_skill_location_id(&workspace.id, location),
                directory,
                scope: SKILL_SCOPE_WORKSPACE,
                workspace_id: Some(workspace.id.clone()),
                workspace_name: Some(workspace.name.clone()),
            });
        }
    }

    roots
}

fn workspace_skill_location_id(workspace_id: &str, location: &str) -> String {
    format!("workspace:{workspace_id}:{location}")
}

pub(crate) fn deletable_skill_directory_for_path(
    skill_path: &Path,
    roots: &[SkillSearchRoot],
) -> Result<PathBuf, String> {
    if skill_path.file_name().and_then(|name| name.to_str()) != Some("SKILL.md") {
        return Err(format!(
            "skill path is not a SKILL.md file: {}",
            skill_path.display()
        ));
    }

    let skill_dir = skill_path.parent().ok_or_else(|| {
        format!(
            "skill path has no parent directory: {}",
            skill_path.display()
        )
    })?;
    if !skill_dir.join("SKILL.md").is_file() {
        return Err(format!(
            "skill directory does not contain SKILL.md: {}",
            skill_dir.display()
        ));
    }

    for root in roots {
        if skill_dir == root.directory {
            return Err(format!(
                "skill '{}' is defined directly in a skills root; delete it manually",
                skill_path.display()
            ));
        }

        if skill_dir.parent() == Some(root.directory.as_path()) {
            reject_symlink(&root.directory, "skills root")?;
            reject_symlink(skill_dir, "skill directory")?;
            return Ok(skill_dir.to_path_buf());
        }
    }

    Err(format!(
        "skill directory is not a direct child of a configured skills root: {}",
        skill_dir.display()
    ))
}

fn reject_symlink(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| format!("failed to inspect {label} {}: {source}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{label} is a symlink and must be deleted manually: {}",
            path.display()
        ));
    }

    Ok(())
}

fn skill_key(root: &SkillSearchRoot, skill_id: &str) -> String {
    match root.scope {
        SKILL_SCOPE_GLOBAL => format!("global:{skill_id}"),
        SKILL_SCOPE_WORKSPACE => {
            let workspace_id = root.workspace_id.as_deref().unwrap_or_default();
            format!("workspace:{workspace_id}:{skill_id}")
        }
        scope => format!("{scope}:{skill_id}"),
    }
}

fn skill_scope_label(root: &SkillSearchRoot) -> String {
    match root.scope {
        SKILL_SCOPE_GLOBAL => "global".to_string(),
        SKILL_SCOPE_WORKSPACE => format!(
            "workspace '{}'",
            root.workspace_name
                .as_deref()
                .or(root.workspace_id.as_deref())
                .unwrap_or("")
        ),
        scope => scope.to_string(),
    }
}

pub(crate) fn skill_is_disabled(skill: &SkillSettings, disabled_ids: &HashSet<&str>) -> bool {
    disabled_ids.contains(skill.key.as_str()) || disabled_ids.contains(skill.id.as_str())
}

pub(crate) fn skill_is_required_disabled(
    skill: &SkillSettings,
    required_disabled_ids: &HashSet<&str>,
) -> bool {
    required_disabled_ids.contains(skill.key.as_str())
}

pub(crate) fn skill_applies_to_workspace(skill: &SkillSettings, workspace_id: &str) -> bool {
    skill.scope == SKILL_SCOPE_GLOBAL
        || (skill.scope == SKILL_SCOPE_WORKSPACE
            && skill.workspace_id.as_deref() == Some(workspace_id))
}

/// Discovery errors that declare a duplicate skill id (same scoped key collision).
pub(crate) fn skill_has_duplicate_declaration(
    errors: &[SkillDiscoveryErrorSummary],
    skill_id: &str,
) -> bool {
    let needle = format!("duplicate skill id '{skill_id}'");
    errors.iter().any(|error| error.message.contains(&needle))
}

/// Live-discover Skills for one workspace: host global + that workspace only.
/// Applies the same disabled-key / required-disabled rules used by settings and
/// routing (disabled locations are already excluded by [`discover_skills`]).
pub(crate) fn discover_skills_for_workspace(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
) -> SkillDiscovery {
    let mut discovery = discover_skills(user_profile_dir, config);
    discovery
        .skills
        .retain(|skill| skill_applies_to_workspace(skill, workspace_id));
    discovery
}

fn unique_skill_by_legacy_id<'a>(
    skills: &[&'a SkillSettings],
    legacy_id: &str,
) -> Result<&'a SkillSettings, ApiError> {
    let matches = skills
        .iter()
        .copied()
        .filter(|skill| skill.id == legacy_id)
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [skill] => Ok(*skill),
        [] => Err(ApiError::bad_request(format!(
            "selected skill was not found: {legacy_id}"
        ))),
        _ => Err(ApiError::bad_request(format!(
            "selected skill id '{legacy_id}' is ambiguous; use a scoped skill key"
        ))),
    }
}

fn skill_file_candidates(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let metadata = match fs::metadata(directory) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(format!(
                "failed to inspect skill directory {}: {}",
                directory.display(),
                source
            ));
        }
    };
    if !metadata.is_dir() {
        return Err(format!(
            "skill path is not a directory: {}",
            directory.display()
        ));
    }

    let mut candidates = Vec::new();
    let direct_skill = directory.join("SKILL.md");
    if direct_skill.is_file() {
        candidates.push(direct_skill);
    }

    let entries = fs::read_dir(directory).map_err(|source| {
        format!(
            "failed to read skill directory {}: {}",
            directory.display(),
            source
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| {
            format!(
                "failed to read skill directory entry under {}: {}",
                directory.display(),
                source
            )
        })?;
        let file_type = entry.file_type().map_err(|source| {
            format!(
                "failed to read skill directory entry type under {}: {}",
                directory.display(),
                source
            )
        })?;

        if file_type.is_dir() {
            let nested_skill = entry.path().join("SKILL.md");
            if nested_skill.is_file() {
                candidates.push(nested_skill);
            }
        }
    }

    candidates.sort();

    Ok(candidates)
}

pub(crate) fn parse_skill_file(path: &Path) -> Result<ParsedSkillFile, String> {
    let content = load_skill_md_document(path)?;
    parse_skill_markdown(path, &content)
}

fn parse_skill_file_frontmatter(path: &Path) -> Result<ParsedSkillFile, String> {
    // Discovery parses the complete document before retaining only its frontmatter.
    let mut parsed = parse_skill_file(path)?;
    parsed.markdown.clear();
    Ok(parsed)
}

fn parse_skill_file_id(path: &Path) -> Result<String, String> {
    let content = load_skill_md_document(path)?;
    parse_skill_markdown_id(path, &content)
}

/// Strict full-document UTF-8 load for `SKILL.md`. Never returns a partial body.
pub(crate) fn load_skill_md_document(path: &Path) -> Result<String, String> {
    if !path_is_skill_md(path) {
        return Err(format!(
            "skill file {} must be named SKILL.md",
            path.display()
        ));
    }

    let metadata = fs::metadata(path).map_err(|source| {
        format!(
            "failed to inspect skill file {}: {}",
            path.display(),
            source
        )
    })?;
    if !metadata.is_file() {
        return Err(format!("skill path is not a file: {}", path.display()));
    }

    // Parsing and selected-skill validation need the complete document, but discovery must not
    // allocate unbounded memory from a workspace-controlled skill file.
    if metadata.len() > MAX_SKILL_MD_SOURCE_BYTES {
        return Err(skill_md_source_size_error(path, metadata.len()));
    }

    let bytes = fs::read(path)
        .map_err(|source| format!("failed to read skill file {}: {}", path.display(), source))?;
    if bytes.len() > MAX_SKILL_MD_SOURCE_BYTES as usize {
        return Err(skill_md_source_size_error(path, bytes.len() as u64));
    }

    String::from_utf8(bytes).map_err(|source| {
        format!(
            "skill file {} is not valid UTF-8: {}",
            path.display(),
            source
        )
    })
}

fn skill_md_source_size_error(path: &Path, actual_bytes: u64) -> String {
    format!(
        "skill file {} exceeds the source file safety limit ({} bytes; max {} bytes)",
        path.display(),
        actual_bytes,
        MAX_SKILL_MD_SOURCE_BYTES
    )
}

/// Validate final selected skill bodies for one provider turn.
///
/// Call after dedupe / disabled / precedence resolution. Counts each entry's
/// `content_markdown` UTF-8 byte length once (callers must already dedupe by key).
pub(crate) fn validate_selected_skills_total_budget(
    entries: &[SelectedSkillPromptEntry],
) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut sizes = Vec::with_capacity(entries.len());
    let mut total: usize = 0;
    for entry in entries {
        let size = entry.content_markdown.len();
        total = total.saturating_add(size);
        sizes.push((entry.prompt.key.as_str(), size));
    }

    if total <= SELECTED_SKILLS_MAX_TOTAL_BYTES {
        return Ok(());
    }

    let details = sizes
        .into_iter()
        .map(|(key, size)| format!("{key}={size}"))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "selected skills exceed the total content budget ({} bytes; max {} bytes). Sizes: [{details}]",
        total, SELECTED_SKILLS_MAX_TOTAL_BYTES
    ))
}

pub(crate) fn parse_skill_markdown(path: &Path, content: &str) -> Result<ParsedSkillFile, String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.lines();

    if lines.next().map(str::trim) != Some("---") {
        return Err(format!(
            "skill file {} must start with YAML frontmatter delimiter '---'",
            path.display()
        ));
    }

    let mut frontmatter = Vec::new();
    let mut has_closing_delimiter = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            has_closing_delimiter = true;
            break;
        }

        frontmatter.push(line);
    }

    if !has_closing_delimiter {
        return Err(format!(
            "skill file {} is missing closing YAML frontmatter delimiter '---'",
            path.display()
        ));
    }

    let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    if body.is_empty() {
        return Err(format!(
            "skill file {} must contain instructions after frontmatter",
            path.display()
        ));
    }

    let id = skill_frontmatter_field(path, &frontmatter, "name")?;
    validate_skill_id(&id).map_err(|error| format!("skill file {}: {}", path.display(), error))?;
    let description = skill_frontmatter_description(path, &frontmatter)?;

    Ok(ParsedSkillFile {
        id: id.clone(),
        name: id,
        description,
        markdown: content.to_string(),
    })
}

fn parse_skill_markdown_id(path: &Path, content: &str) -> Result<String, String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.lines();

    if lines.next().map(str::trim) != Some("---") {
        return Err(format!(
            "skill file {} must start with YAML frontmatter delimiter '---'",
            path.display()
        ));
    }

    let mut frontmatter = Vec::new();
    for line in lines.by_ref() {
        if line.trim() == "---" {
            let id = skill_frontmatter_field(path, &frontmatter, "name")?;
            validate_skill_id(&id)
                .map_err(|error| format!("skill file {}: {}", path.display(), error))?;
            return Ok(id);
        }

        frontmatter.push(line);
    }

    Err(format!(
        "skill file {} is missing closing YAML frontmatter delimiter '---'",
        path.display()
    ))
}

fn skill_frontmatter_field<T: AsRef<str>>(
    path: &Path,
    frontmatter: &[T],
    field: &str,
) -> Result<String, String> {
    for line in frontmatter {
        let trimmed = line.as_ref().trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };

        if key.trim() != field {
            continue;
        }

        let value = unquote_frontmatter_value(value.trim());
        if value.trim().is_empty() {
            return Err(format!(
                "skill file {} frontmatter field '{}' must not be empty",
                path.display(),
                field
            ));
        }

        return Ok(value.trim().to_string());
    }

    Err(format!(
        "skill file {} frontmatter is missing required field '{}'",
        path.display(),
        field
    ))
}

/// Parse frontmatter `description`, supporting:
/// - inline scalar: `description: value`
/// - YAML block scalars: `description: >` / `description: |` (+ optional chomping/indent)
/// - ecosystem-compatible unindented multi-line text after `description:`
///
/// Multi-line forms are folded into a single display string (whitespace-joined).
/// Stops at the next top-level mapping key (e.g. `license:`, `metadata:`) so sibling
/// fields are never swallowed into the description.
fn skill_frontmatter_description<T: AsRef<str>>(
    path: &Path,
    frontmatter: &[T],
) -> Result<String, String> {
    let mut lines = frontmatter.iter().map(|line| line.as_ref()).peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((key, rest)) = trimmed.split_once(':') else {
            continue;
        };

        if key.trim() != "description" {
            continue;
        }

        let rest = rest.trim();
        if !rest.is_empty() {
            // Standard YAML folded/literal block scalar indicators. Headers may include a
            // trailing YAML comment (`description: > # folded summary`).
            let block_header = frontmatter_value_without_inline_comment(rest);
            if is_yaml_block_scalar_indicator(block_header) {
                return collect_description_continuation(path, &mut lines, true);
            }

            let value = unquote_frontmatter_value(rest);
            if value.trim().is_empty() {
                return Err(format!(
                    "skill file {} frontmatter field 'description' must not be empty",
                    path.display()
                ));
            }
            return Ok(value.trim().to_string());
        }

        // Empty inline value: accept indented/unindented continuation until the next
        // top-level key (vercel-composition-patterns and similar agent-skill frontmatter).
        return collect_description_continuation(path, &mut lines, false);
    }

    Err(format!(
        "skill file {} frontmatter is missing required field 'description'",
        path.display()
    ))
}

fn is_yaml_block_scalar_indicator(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    let style = bytes[0];
    if style != b'>' && style != b'|' {
        return false;
    }

    // YAML block headers allow chomping (`-`/`+`) and indent digit (1-9) in either order.
    let mut idx = 1;
    let mut saw_chomp = false;
    let mut saw_indent = false;
    while idx < bytes.len() {
        let b = bytes[idx];
        if !saw_chomp && (b == b'-' || b == b'+') {
            saw_chomp = true;
            idx += 1;
            continue;
        }
        if !saw_indent && b.is_ascii_digit() && b != b'0' {
            saw_indent = true;
            idx += 1;
            continue;
        }
        return false;
    }
    true
}

/// Strip a YAML end-of-line comment (` # ...`) from an unquoted frontmatter value.
/// `#` only starts a comment when it is at the start of the value or preceded by whitespace.
fn frontmatter_value_without_inline_comment(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'#' && (idx == 0 || bytes[idx - 1].is_ascii_whitespace()) {
            return value[..idx].trim_end();
        }
        idx += 1;
    }
    value
}

fn is_top_level_frontmatter_key_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }

    // Top-level mapping keys in skill frontmatter are unindented `key: ...` lines.
    // Indented nested content (e.g. under `metadata:`) is not a top-level key boundary.
    if line.starts_with(' ') || line.starts_with('\t') {
        return false;
    }

    let Some((key, rest)) = trimmed.split_once(':') else {
        return false;
    };

    let key = key.trim();
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        return false;
    }

    // Require YAML mapping separation: after `:` the value is empty or starts with
    // whitespace. This keeps `license: MIT` as a boundary while rejecting bare URLs
    // (`https://example.com`) and prose colons (`See docs: more`) as false keys.
    rest.is_empty() || rest.starts_with(char::is_whitespace)
}

fn collect_description_continuation<'a, I>(
    path: &Path,
    lines: &mut std::iter::Peekable<I>,
    require_indented_block: bool,
) -> Result<String, String>
where
    I: Iterator<Item = &'a str>,
{
    let mut parts = Vec::new();
    let mut saw_indented_line = false;

    while let Some(&next) = lines.peek() {
        if is_top_level_frontmatter_key_line(next) {
            break;
        }

        let line = lines.next().expect("peeked line must exist");
        let trimmed = line.trim();
        let is_indented = line.starts_with(' ') || line.starts_with('\t');

        // Only unindented `# ...` lines are YAML comments. Indented `# Heading` inside a
        // block scalar is description content and must be preserved.
        if trimmed.is_empty() || (!is_indented && trimmed.starts_with('#')) {
            // Preserve paragraph breaks only after content has started.
            if !parts.is_empty() {
                parts.push(String::new());
            }
            continue;
        }

        if require_indented_block {
            if is_indented {
                saw_indented_line = true;
                parts.push(trimmed.to_string());
            } else if !saw_indented_line {
                // Non-indented text immediately after `>`/`|` with no block body yet is
                // treated as empty (block scalar without content).
                break;
            } else {
                // After an indented block body, a non-indented non-key line is unusual;
                // stop so we do not absorb sibling content.
                break;
            }
        } else {
            parts.push(trimmed.to_string());
        }
    }

    // Descriptions are used as flat skill metadata text; fold block content to a single
    // whitespace-joined line (display-oriented, not full YAML literal fidelity).
    let description = parts
        .into_iter()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if description.is_empty() {
        return Err(format!(
            "skill file {} frontmatter field 'description' must not be empty",
            path.display()
        ));
    }

    Ok(description)
}

fn unquote_frontmatter_value(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let quote = bytes[0];

        if (quote == b'"' || quote == b'\'') && bytes[value.len() - 1] == quote {
            return value[1..value.len() - 1].to_string();
        }
    }

    value.to_string()
}

fn validate_skill_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("skill id must not be empty".to_string());
    }

    if id.chars().any(char::is_whitespace) {
        return Err(format!("skill id '{}' must not contain whitespace", id));
    }

    Ok(())
}

fn validate_skill_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("skill key must not be empty".to_string());
    }

    if key.chars().any(char::is_whitespace) {
        return Err(format!("skill key '{}' must not contain whitespace", key));
    }

    Ok(())
}

/// One Skill directory the model may `read_file` without an external-access
/// question. Roots are canonicalized and limited to that Skill folder only
/// (SKILL.md, references/, scripts/, assets/, …), not the whole workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AvailableSkillsSnapshot {
    pub(crate) prompt_entries: Vec<SkillPromptEntry>,
    pub(crate) read_root_dirs: Vec<PathBuf>,
}

/// Live-discover Skills for this workspace and apply the same filters used by
/// the `## Skills` routing table: workspace scope, disabled locations (via
/// discovery), disabled keys, and required-disabled keys.
pub(crate) fn available_skills_snapshot_for_workspace(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
) -> AvailableSkillsSnapshot {
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, config);
    let required_disabled_ids = discovery
        .required_disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    let skills = discovery
        .skills
        .into_iter()
        .filter(|skill| {
            skill_applies_to_workspace(skill, workspace_id)
                && !skill_is_disabled(skill, &disabled_ids)
                && !skill_is_required_disabled(skill, &required_disabled_ids)
        })
        .collect::<Vec<_>>();

    AvailableSkillsSnapshot {
        prompt_entries: skills
            .iter()
            .map(skill_prompt_entry_from_settings)
            .collect(),
        read_root_dirs: skill_read_root_dirs_from_settings(&skills),
    }
}

pub(crate) fn skill_read_root_dirs_from_settings(skills: &[SkillSettings]) -> Vec<PathBuf> {
    let mut roots = skills
        .iter()
        .filter_map(skill_read_root_dir)
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

pub(crate) fn skill_read_root_dir(skill: &SkillSettings) -> Option<PathBuf> {
    skill
        .path
        .parent()
        .and_then(|skill_dir| fs::canonicalize(skill_dir).ok())
}

pub(crate) fn path_is_within_skill_read_roots(target_path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| target_path.starts_with(root))
}

pub(crate) fn available_skills_routing_message(
    entries: &[SkillPromptEntry],
) -> Option<NeutralChatMessage> {
    if entries.is_empty() {
        return None;
    }

    let entries = entries
        .iter()
        .map(skill_frontmatter_entry)
        .collect::<Vec<_>>()
        .join("\n");
    Some(neutral_text_message(
        NeutralChatRole::Developer,
        format!(
            "## Skills\n\nA skill is a set of instructions provided through a `SKILL.md` source. Below is the list of skills that can be used in this session. Each entry includes a name, description, skill key, scope, and source locator. Treat this list as a routing table for the current user turn. Foco currently exposes filesystem-backed skills; `file` locators are paths on the host filesystem.\n\n### Available Skills\n\n{}\n\n### How to Use Skills\n\n- Discovery: The list above is the skills available in this session (name + description + skill key + scope + source locator). Empty selected `skillIds` or empty Agent task skill ids mean no skill was explicitly preselected for the task; they do not mean the available-skill list is empty. `file` entries live on the host filesystem and must be opened with `read_file` when the skill is selected. Workspace skill paths are usually workspace-relative in practice; global skill paths are usually absolute paths outside the workspace and `read_file` will request explicit user authorization before reading them.\n- Trigger rules: Before starting task work, compare the user's latest request with the available skill names and descriptions. If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list, its `SKILL.md` file can't be read, or the user denies external read access for a global skill, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1. After deciding to use a skill, the main agent must read its `SKILL.md` completely with `read_file` before taking task actions. `startLine` and `endLine` must both be null for `SKILL.md`; it cannot be reconstructed from partial ranges. Referenced resources under `references/`, scripts, and assets stay ordinary files and may use ranged reads.\n  2. When `SKILL.md` references another resource, resolve relative paths against that skill's directory and read only the resources required for the current task.\n  3. If `SKILL.md` points to extra folders such as `references/`, use its routing instructions to identify the relevant files. The main agent must read each required instruction or reference file itself before acting on it. Do not delegate reading, summarizing, or interpreting skill instructions to a subagent.\n  4. Prefer running or patching provided scripts, templates, or assets from the skill directory instead of retyping large code blocks or recreating assets.\n  5. Reuse provided assets or templates from the skill source whenever they fit the task.\n- Coordination and sequencing: If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them. Announce which skill(s) you're using and why in one short line. If you skip an obvious skill, say why.\n- Context hygiene: Progressive disclosure applies to selecting relevant files, not partially reading a selected instruction file. Do not load unrelated references, scripts, or assets. Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked. When variants exist, pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly, state the issue, pick the next-best approach, and continue.",
            entries
        ),
    ))
}

pub(crate) fn enabled_skill_frontmatter_messages(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let snapshot = available_skills_snapshot_for_workspace(user_profile_dir, config, workspace_id);
    Ok(available_skills_routing_message(&snapshot.prompt_entries)
        .into_iter()
        .collect())
}

fn skill_frontmatter_entry(skill: &SkillPromptEntry) -> String {
    format!(
        "- Name: {}; description: {}; key: {}; scope: {}; file: {}",
        serde_json::to_string(&skill.name).expect("skill name is always JSON serializable"),
        serde_json::to_string(&skill.description)
            .expect("skill description is always JSON serializable"),
        serde_json::to_string(&skill.key).expect("skill key is always JSON serializable"),
        serde_json::to_string(&skill.scope).expect("skill scope is always JSON serializable"),
        serde_json::to_string(&skill.path).expect("skill path is always JSON serializable")
    )
}

fn skill_scope_prompt_label(skill: &SkillSettings) -> String {
    match skill.scope.as_str() {
        SKILL_SCOPE_GLOBAL => "global".to_string(),
        SKILL_SCOPE_WORKSPACE => format!(
            "workspace:{}",
            skill
                .workspace_name
                .as_deref()
                .or(skill.workspace_id.as_deref())
                .unwrap_or("")
        ),
        scope => scope.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_global_skills_for_profile_scans_agents_skills_only() {
        let profile = tempfile::tempdir().expect("profile");
        let skill_dir = profile
            .path()
            .join(".agents")
            .join("skills")
            .join("global-demo");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: global-demo\ndescription: Global demo.\n---\n\nBody.\n",
        )
        .expect("skill");
        // Workspace-style path under profile must not be discovered by global-only helper.
        let fake_ws = profile.path().join(".claude").join("skills").join("ws");
        fs::create_dir_all(&fake_ws).expect("ws skill dir");
        fs::write(
            fake_ws.join("SKILL.md"),
            "---\nname: ws\ndescription: Workspace-like.\n---\n\nBody.\n",
        )
        .expect("ws skill");

        let discovery = discover_global_skills_for_profile(profile.path());
        assert_eq!(discovery.skills.len(), 1);
        assert_eq!(discovery.skills[0].key, "global:global-demo");
        assert_eq!(discovery.skills[0].scope, SKILL_SCOPE_GLOBAL);
        assert!(discovery.skills[0].path.is_absolute());
        assert!(discovery.skills[0].path.ends_with("SKILL.md"));
    }

    #[test]
    fn available_skills_snapshot_filters_disabled_and_workspace() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");
        let skill_dir = workspace
            .path()
            .join(".agents")
            .join("skills")
            .join("build");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---
name: build
description: build helpers
---

Body.",
        )
        .expect("skill");

        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.workspaces[0].id = "workspace-a".to_string();
        config.workspaces[0].name = "A".to_string();
        config.workspaces[0].path = workspace.path().to_path_buf();

        let snapshot =
            available_skills_snapshot_for_workspace(profile.path(), &config, "workspace-a");
        assert_eq!(snapshot.prompt_entries.len(), 1);
        assert_eq!(snapshot.prompt_entries[0].name, "build");
        assert_eq!(snapshot.read_root_dirs.len(), 1);
        assert!(snapshot.read_root_dirs[0].ends_with("build"));

        config
            .skills
            .disabled
            .push(snapshot.prompt_entries[0].key.clone());
        let disabled =
            available_skills_snapshot_for_workspace(profile.path(), &config, "workspace-a");
        assert!(disabled.prompt_entries.is_empty());
        assert!(disabled.read_root_dirs.is_empty());

        let other =
            available_skills_snapshot_for_workspace(profile.path(), &config, "workspace-other");
        assert!(other.prompt_entries.is_empty());
    }

    #[test]
    fn discover_skills_for_workspace_keeps_global_and_current_workspace_only() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace_a = tempfile::tempdir().expect("workspace a");
        let workspace_b = tempfile::tempdir().expect("workspace b");
        let global_dir = profile.path().join(".agents").join("skills").join("g1");
        fs::create_dir_all(&global_dir).expect("global skill dir");
        fs::write(
            global_dir.join("SKILL.md"),
            "---\nname: g1\ndescription: global skill\n---\n\nBody.",
        )
        .expect("global skill");
        for (workspace, id) in [(&workspace_a, "wa"), (&workspace_b, "wb")] {
            let skill_dir = workspace.path().join(".agents").join("skills").join(id);
            fs::create_dir_all(&skill_dir).expect("workspace skill dir");
            fs::write(
                skill_dir.join("SKILL.md"),
                format!("---\nname: {id}\ndescription: workspace {id}\n---\n\nBody."),
            )
            .expect("workspace skill");
        }

        let mut config = GlobalConfig::first_run(workspace_a.path().to_path_buf());
        config.workspaces[0].id = "workspace-a".to_string();
        config.workspaces[0].name = "A".to_string();
        config.workspaces[0].path = workspace_a.path().to_path_buf();
        config.workspaces.push(WorkspaceConfig {
            id: "workspace-b".to_string(),
            name: "B".to_string(),
            path: workspace_b.path().to_path_buf(),
            location: WorkspaceLocation::Local,
            pinned: false,
            code_graph_enabled: false,
            terminal_shell: config.workspaces[0].terminal_shell.clone(),
            common_commands: Vec::new(),
        });

        let discovery = discover_skills_for_workspace(profile.path(), &config, "workspace-a");
        let keys = discovery
            .skills
            .iter()
            .map(|skill| skill.key.as_str())
            .collect::<Vec<_>>();
        assert!(keys.contains(&"global:g1"));
        assert!(keys.iter().any(|key| key.contains("wa")));
        assert!(!keys.iter().any(|key| key.contains("wb")));
    }

    #[test]
    fn available_skills_snapshot_includes_claude_and_honors_disabled_location() {
        let profile = tempfile::tempdir().expect("profile");
        let workspace = tempfile::tempdir().expect("workspace");

        let agents_dir = workspace
            .path()
            .join(".agents")
            .join("skills")
            .join("build");
        fs::create_dir_all(&agents_dir).expect("agents skill dir");
        fs::write(
            agents_dir.join("SKILL.md"),
            "---
name: build
description: build helpers
---

Body.",
        )
        .expect("agents skill");

        let claude_dir = workspace
            .path()
            .join(".claude")
            .join("skills")
            .join("deploy");
        let claude_ref = claude_dir.join("references");
        fs::create_dir_all(&claude_ref).expect("claude skill dir");
        fs::write(
            claude_dir.join("SKILL.md"),
            "---
name: deploy
description: deploy helpers
---

Body.",
        )
        .expect("claude skill");
        fs::write(claude_ref.join("details.md"), "details").expect("claude ref");

        let mut config = GlobalConfig::first_run(workspace.path().to_path_buf());
        config.workspaces[0].id = "workspace-a".to_string();
        config.workspaces[0].name = "A".to_string();
        config.workspaces[0].path = workspace.path().to_path_buf();

        let snapshot =
            available_skills_snapshot_for_workspace(profile.path(), &config, "workspace-a");
        assert_eq!(snapshot.prompt_entries.len(), 2);
        assert_eq!(snapshot.read_root_dirs.len(), 2);
        assert!(
            snapshot
                .prompt_entries
                .iter()
                .any(|entry| entry.path.contains(".claude"))
        );
        assert!(path_is_within_skill_read_roots(
            &fs::canonicalize(claude_ref.join("details.md")).expect("canon ref"),
            &snapshot.read_root_dirs,
        ));

        config
            .skills
            .disabled_locations
            .push("workspace:workspace-a:claude".to_string());
        let filtered =
            available_skills_snapshot_for_workspace(profile.path(), &config, "workspace-a");
        assert_eq!(filtered.prompt_entries.len(), 1);
        assert_eq!(filtered.prompt_entries[0].name, "build");
        assert!(!path_is_within_skill_read_roots(
            &fs::canonicalize(claude_ref.join("details.md")).expect("canon ref"),
            &filtered.read_root_dirs,
        ));
    }

    #[test]
    fn deletable_skill_directory_allows_nested_skill_directory() {
        let profile = tempfile::tempdir().expect("profile");
        let roots = skill_search_roots(profile.path(), &[]);
        let root = &roots[0].directory;
        let skill_dir = root.join("demo");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        let skill_file = skill_dir.join("SKILL.md");
        fs::write(
            &skill_file,
            "---\nname: demo\ndescription: demo\n---\nUse demo.",
        )
        .expect("skill file");

        let deletable = deletable_skill_directory_for_path(&skill_file, &roots).expect("deletable");
        assert_eq!(deletable, skill_dir);

        fs::remove_dir_all(&deletable).expect("remove skill dir");
        assert!(!skill_dir.exists());
        assert!(root.exists());
    }

    #[test]
    fn deletable_skill_directory_rejects_root_level_skill_file() {
        let profile = tempfile::tempdir().expect("profile");
        let roots = skill_search_roots(profile.path(), &[]);
        let root = &roots[0].directory;
        fs::create_dir_all(root).expect("skills root");
        let skill_file = root.join("SKILL.md");
        fs::write(
            &skill_file,
            "---\nname: root\ndescription: root\n---\nUse root.",
        )
        .expect("skill file");

        let error = deletable_skill_directory_for_path(&skill_file, &roots).expect_err("rejected");
        assert!(error.contains("defined directly in a skills root"));
        assert!(root.exists());
        assert!(skill_file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn deletable_skill_directory_rejects_symlinked_skills_root() {
        let profile = tempfile::tempdir().expect("profile");
        let external = tempfile::tempdir().expect("external");
        let roots = skill_search_roots(profile.path(), &[]);
        let root = &roots[0].directory;
        let parent = root.parent().expect("root parent");
        fs::create_dir_all(parent).expect("skills parent");
        std::os::unix::fs::symlink(external.path(), root).expect("root symlink");

        let skill_dir = root.join("demo");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        let skill_file = skill_dir.join("SKILL.md");
        fs::write(
            &skill_file,
            "---\nname: demo\ndescription: demo\n---\nUse demo.",
        )
        .expect("skill file");

        let error = deletable_skill_directory_for_path(&skill_file, &roots).expect_err("rejected");
        assert!(error.contains("skills root is a symlink"));
        assert!(external.path().join("demo").exists());
    }

    fn skill_document_with_body_len(id: &str, description: &str, body_len: usize) -> String {
        let header = format!("---\nname: {id}\ndescription: {description}\n---\n\n");
        format!("{header}{}", "x".repeat(body_len))
    }

    #[test]
    fn load_skill_md_document_accepts_document_larger_than_former_64kib_limit() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("SKILL.md");
        let header = "---\nname: edge\ndescription: boundary\n---\n\n";
        let former_limit = 64 * 1024;
        let body_len = former_limit - header.len() + 1;
        let content = format!("{header}{}", "a".repeat(body_len));
        assert_eq!(content.len(), former_limit + 1);
        fs::write(&path, &content).expect("write");

        let loaded = load_skill_md_document(&path).expect("load document");
        assert_eq!(loaded.len(), former_limit + 1);
        let parsed = parse_skill_file(&path).expect("parse document");
        assert_eq!(parsed.id, "edge");
        assert_eq!(parsed.markdown.len(), former_limit + 1);
    }

    #[test]
    fn load_skill_md_document_rejects_document_larger_than_source_safety_limit() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("SKILL.md");
        let file = fs::File::create(&path).expect("create");
        file.set_len(MAX_SKILL_MD_SOURCE_BYTES + 1)
            .expect("set sparse size");

        let error = load_skill_md_document(&path).expect_err("source too large");
        assert!(error.contains("exceeds the source file safety limit"));
        assert!(error.contains(&format!("max {MAX_SKILL_MD_SOURCE_BYTES}")));
    }

    #[test]
    fn load_skill_md_document_accepts_multibyte_utf8_larger_than_former_64kib_limit() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("SKILL.md");
        let content = format!(
            "---\nname: huge\ndescription: large\n---\n\n{}",
            "你".repeat(64 * 1024)
        );
        assert!(content.len() > 64 * 1024);
        fs::write(&path, &content).expect("write");

        let parsed = parse_skill_file(&path).expect("parse document");
        assert_eq!(parsed.id, "huge");
        assert!(parsed.markdown.contains("你"));
    }

    #[test]
    fn load_skill_md_document_accepts_utf8_bom_larger_than_former_64kib_limit() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("SKILL.md");
        let header = "---\nname: bom\ndescription: bom test\n---\n\n";
        let body_len = 64 * 1024 - header.len() + 1;
        let mut content = Vec::from([0xEF, 0xBB, 0xBF]);
        content.extend_from_slice(format!("{header}{}", "c".repeat(body_len)).as_bytes());
        assert!(content.len() > 64 * 1024);
        fs::write(&path, &content).expect("write");

        let parsed = parse_skill_file(&path).expect("parse document");
        assert_eq!(parsed.id, "bom");
    }

    #[test]
    fn load_skill_md_document_accepts_multibyte_utf8_within_budget() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("SKILL.md");
        let body = "你".repeat(100);
        let content = format!("---\nname: multi\ndescription: multibyte\n---\n\n{body}");
        fs::write(&path, &content).expect("write");

        let parsed = parse_skill_file(&path).expect("parse multi");
        assert_eq!(parsed.id, "multi");
        assert!(parsed.markdown.contains("你"));
    }

    #[test]
    fn discovery_includes_skill_larger_than_former_64kib_limit() {
        let profile = tempfile::tempdir().expect("profile");
        let skill_dir = profile
            .path()
            .join(".agents")
            .join("skills")
            .join("oversized");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        let path = skill_dir.join("SKILL.md");
        let header = "---\nname: oversized\ndescription: large skill\n---\n\n";
        let body_len = 64 * 1024 - header.len() + 1;
        fs::write(&path, format!("{header}{}", "z".repeat(body_len))).expect("write");

        let discovery = discover_global_skills_for_profile(profile.path());
        assert!(
            discovery
                .skills
                .iter()
                .any(|skill| skill.id == "oversized" && skill.description.contains("large skill")),
            "large skill must enter routing: {:?}",
            discovery.skills
        );
    }

    #[test]
    fn validate_selected_skills_total_budget_accepts_exact_128kib() {
        let half = SELECTED_SKILLS_MAX_TOTAL_BYTES / 2;
        let entries = vec![
            selected_skill_prompt_entry(
                SkillPromptEntry {
                    key: "global:a".to_string(),
                    name: "a".to_string(),
                    description: "a".to_string(),
                    scope: "global".to_string(),
                    path: "/tmp/a/SKILL.md".to_string(),
                },
                "x".repeat(half),
            ),
            selected_skill_prompt_entry(
                SkillPromptEntry {
                    key: "global:b".to_string(),
                    name: "b".to_string(),
                    description: "b".to_string(),
                    scope: "global".to_string(),
                    path: "/tmp/b/SKILL.md".to_string(),
                },
                "y".repeat(half),
            ),
        ];
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.content_markdown.len())
                .sum::<usize>(),
            SELECTED_SKILLS_MAX_TOTAL_BYTES
        );
        validate_selected_skills_total_budget(&entries).expect("exact total");
    }

    #[test]
    fn validate_selected_skills_total_budget_rejects_one_byte_over() {
        // Three skills whose combined size is exactly 128KiB+1.
        let sizes = [
            64 * 1024,
            64 * 1024,
            SELECTED_SKILLS_MAX_TOTAL_BYTES - 2 * 64 * 1024 + 1,
        ];
        assert_eq!(
            sizes.iter().sum::<usize>(),
            SELECTED_SKILLS_MAX_TOTAL_BYTES + 1
        );
        let keys = ["global:a", "global:b", "global:c"];
        let entries = keys
            .iter()
            .zip(sizes)
            .map(|(key, size)| {
                selected_skill_prompt_entry(
                    SkillPromptEntry {
                        key: (*key).to_string(),
                        name: key.to_string(),
                        description: key.to_string(),
                        scope: "global".to_string(),
                        path: format!("/tmp/{key}/SKILL.md"),
                    },
                    "x".repeat(size),
                )
            })
            .collect::<Vec<_>>();
        let error = validate_selected_skills_total_budget(&entries).expect_err("over total");
        assert!(error.contains("total content budget"), "{error}");
        assert!(error.contains("global:a="), "{error}");
        assert!(error.contains("global:b="), "{error}");
        assert!(error.contains("global:c="), "{error}");
        assert!(!error.contains(&"x".repeat(16)));
    }

    #[test]
    fn validate_selected_skills_total_budget_accepts_single_skill_over_former_64kib_limit() {
        let entries = vec![selected_skill_prompt_entry(
            SkillPromptEntry {
                key: "global:huge".to_string(),
                name: "huge".to_string(),
                description: "huge".to_string(),
                scope: "global".to_string(),
                path: "/tmp/huge/SKILL.md".to_string(),
            },
            "x".repeat(64 * 1024 + 1),
        )];
        validate_selected_skills_total_budget(&entries).expect("single skill within total budget");
    }

    #[test]
    fn message_with_selected_skills_accepts_skill_over_former_64kib_limit() {
        let profile = tempfile::tempdir().expect("profile");
        let skill_dir = profile
            .path()
            .join(".agents")
            .join("skills")
            .join("oversized");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        let header = "---\nname: oversized\ndescription: large\n---\n\n";
        let body_len = 64 * 1024 - header.len() + 1;
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("{header}{}", "q".repeat(body_len)),
        )
        .expect("write");

        let workspace = tempfile::tempdir().expect("workspace");
        let config = GlobalConfig::first_run(workspace.path().to_path_buf());
        let message = message_with_selected_skills(
            profile.path(),
            &config,
            "default",
            Some(vec!["global:oversized".to_string()]),
            "user message",
        )
        .expect("must format selected skill");
        assert!(message.contains("Skill 1: oversized"));
        assert!(message.contains(&"q".repeat(16)));
    }

    #[test]
    fn skill_document_with_body_len_helper_is_byte_precise() {
        let doc = skill_document_with_body_len("demo", "desc", 10);
        assert!(doc.ends_with("xxxxxxxxxx"));
        assert_eq!(doc.matches('x').count(), 10);
    }
}
