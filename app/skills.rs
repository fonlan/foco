use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use foco_providers::{NeutralChatMessage, NeutralChatRole};
use foco_store::config::{
    GlobalConfig, SKILL_SCOPE_GLOBAL, SKILL_SCOPE_WORKSPACE, SkillSettings, WorkspaceConfig,
};
use serde::Serialize;

use crate::{ApiError, neutral_text_message, xml_cdata_section, xml_text_escape};

// Prefix used to identify injected enabled skill front matter messages.
pub(crate) const ENABLED_SKILLS_MESSAGE_PREFIX: &str =
    "Enabled skill front matter loaded from configured skills";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillDiscoveryErrorSummary {
    path: String,
    pub(crate) message: String,
}

pub(crate) struct SkillDiscovery {
    pub(crate) skills: Vec<SkillSettings>,
    pub(crate) errors: Vec<SkillDiscoveryErrorSummary>,
    pub(crate) required_disabled: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SkillSearchRoot {
    pub(crate) directory: PathBuf,
    scope: &'static str,
    workspace_id: Option<String>,
    workspace_name: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ParsedSkillFile {
    id: String,
    name: String,
    description: String,
    frontmatter: String,
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
    let discovery = discover_skills(user_profile_dir, &config.workspaces);
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
    let mut links = Vec::with_capacity(requested_skill_keys.len());
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

        links.push(format!("[${}]({})", skill.name, skill.path.display()));
    }

    Ok(format!("{} {}", links.join(" "), message))
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

pub(crate) fn refresh_derived_enabled_skills(config: &mut GlobalConfig) {
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    config.skills.enabled = config
        .skills
        .detected
        .iter()
        .filter(|skill| !skill_is_disabled(skill, &disabled_ids))
        .map(|skill| skill.key.clone())
        .collect();
}

pub(crate) fn discover_skills(
    user_profile_dir: &Path,
    workspaces: &[WorkspaceConfig],
) -> SkillDiscovery {
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    let mut invalid_skills = Vec::new();
    let mut required_disabled = Vec::new();
    let mut seen_keys = HashSet::new();

    for root in skill_search_roots(user_profile_dir, workspaces) {
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
            match parse_skill_file(&path) {
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
    let mut roots = Vec::new();

    roots.push(SkillSearchRoot {
        directory: user_profile_dir.join(".agents").join("skills"),
        scope: SKILL_SCOPE_GLOBAL,
        workspace_id: None,
        workspace_name: None,
    });

    for workspace in workspaces {
        for directory in [
            workspace.path.join(".agents").join("skills"),
            workspace.path.join(".claude").join("skills"),
        ] {
            roots.push(SkillSearchRoot {
                directory,
                scope: SKILL_SCOPE_WORKSPACE,
                workspace_id: Some(workspace.id.clone()),
                workspace_name: Some(workspace.name.clone()),
            });
        }
    }

    roots
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

fn skill_applies_to_workspace(skill: &SkillSettings, workspace_id: &str) -> bool {
    skill.scope == SKILL_SCOPE_GLOBAL
        || (skill.scope == SKILL_SCOPE_WORKSPACE
            && skill.workspace_id.as_deref() == Some(workspace_id))
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
    let content = fs::read_to_string(path)
        .map_err(|source| format!("failed to read skill file {}: {}", path.display(), source))?;

    parse_skill_markdown(path, &content)
}

fn parse_skill_file_id(path: &Path) -> Result<String, String> {
    let content = fs::read_to_string(path)
        .map_err(|source| format!("failed to read skill file {}: {}", path.display(), source))?;

    parse_skill_markdown_id(path, &content)
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
    let description = skill_frontmatter_field(path, &frontmatter, "description")?;

    Ok(ParsedSkillFile {
        id: id.clone(),
        name: id,
        description,
        frontmatter: frontmatter.join("\n"),
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

fn skill_frontmatter_field(
    path: &Path,
    frontmatter: &[&str],
    field: &str,
) -> Result<String, String> {
    for line in frontmatter {
        let trimmed = line.trim();

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

pub(crate) fn enabled_skill_frontmatter_messages(
    user_profile_dir: &Path,
    config: &GlobalConfig,
    workspace_id: &str,
) -> Result<Vec<NeutralChatMessage>, ApiError> {
    let disabled_ids = config
        .skills
        .disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let discovery = discover_skills(user_profile_dir, &config.workspaces);
    let required_disabled_ids = discovery
        .required_disabled
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    let mut entries = Vec::new();
    for skill in discovery.skills.iter().filter(|skill| {
        skill_applies_to_workspace(skill, workspace_id)
            && !skill_is_disabled(skill, &disabled_ids)
            && !skill_is_required_disabled(skill, &required_disabled_ids)
    }) {
        let parsed = parse_skill_file(&skill.path).map_err(ApiError::bad_request)?;

        if parsed.id != skill.id {
            return Err(ApiError::bad_request(format!(
                "enabled skill '{}' file now declares skill id '{}'",
                skill.key, parsed.id
            )));
        }

        entries.push(skill_frontmatter_entry(&skill.path, parsed));
    }

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![neutral_text_message(
        NeutralChatRole::Developer,
        format!(
            "<skills_instructions>\n<source>{}</source>\n{}\n</skills_instructions>",
            xml_text_escape(ENABLED_SKILLS_MESSAGE_PREFIX),
            entries.join("\n")
        ),
    )])
}

fn skill_frontmatter_entry(path: &Path, skill: ParsedSkillFile) -> String {
    format!(
        "<skill path=\"{}\">\n{}\n</skill>",
        xml_text_escape(&path.display().to_string()),
        xml_cdata_section("frontmatter", skill.frontmatter.trim())
    )
}
