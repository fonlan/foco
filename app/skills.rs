use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use foco_providers::{NeutralChatMessage, NeutralChatRole};
use foco_store::config::{
    GlobalConfig, SKILL_SCOPE_GLOBAL, SKILL_SCOPE_WORKSPACE, SkillSettings, WorkspaceConfig,
};
use serde::Serialize;

use crate::{ApiError, neutral_text_message, xml_cdata_section, xml_text_escape};

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
    pub(crate) id: String,
    pub(crate) name: String,
    description: String,
    pub(crate) markdown: String,
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

        entries.push(selected_skill_entry(&skill.path, parsed));
    }

    Ok(format!(
        "<selected_skills>\n{}\n</selected_skills>\n\n{}",
        entries.join("\n"),
        message
    ))
}

fn selected_skill_entry(path: &Path, skill: ParsedSkillFile) -> String {
    format!(
        "<skill name=\"{}\" path=\"{}\">\n{}\n</skill>",
        xml_text_escape(&skill.name),
        xml_text_escape(&path.display().to_string()),
        xml_cdata_section("content_markdown", skill.markdown.trim())
    )
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
    discover_skills_in_roots(skill_search_roots(user_profile_dir, workspaces))
}

pub(crate) fn discover_workspace_skills_for_path(
    workspace_id: &str,
    workspace_name: &str,
    workspace_path: &Path,
) -> SkillDiscovery {
    let roots = [
        SkillSearchRoot {
            directory: workspace_path.join(".agents").join("skills"),
            scope: SKILL_SCOPE_WORKSPACE,
            workspace_id: Some(workspace_id.to_string()),
            workspace_name: Some(workspace_name.to_string()),
        },
        SkillSearchRoot {
            directory: workspace_path.join(".claude").join("skills"),
            scope: SKILL_SCOPE_WORKSPACE,
            workspace_id: Some(workspace_id.to_string()),
            workspace_name: Some(workspace_name.to_string()),
        },
    ];
    discover_skills_in_roots(roots)
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
    let mut roots = Vec::new();

    roots.push(SkillSearchRoot {
        directory: user_profile_dir.join(".agents").join("skills"),
        scope: SKILL_SCOPE_GLOBAL,
        workspace_id: None,
        workspace_name: None,
    });

    for workspace in workspaces {
        let Some(workspace_path) = workspace.local_path() else {
            continue;
        };
        for directory in [
            workspace_path.join(".agents").join("skills"),
            workspace_path.join(".claude").join("skills"),
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

fn parse_skill_file_frontmatter(path: &Path) -> Result<ParsedSkillFile, String> {
    let file = fs::File::open(path)
        .map_err(|source| format!("failed to read skill file {}: {}", path.display(), source))?;
    let mut lines = BufReader::new(file).lines();
    let first_line = lines
        .next()
        .transpose()
        .map_err(|source| format!("failed to read skill file {}: {}", path.display(), source))?;

    if first_line.as_deref().map(str::trim) != Some("---") {
        return Err(format!(
            "skill file {} must start with YAML frontmatter delimiter '---'",
            path.display()
        ));
    }

    let mut frontmatter = Vec::new();
    for line in lines {
        let line = line.map_err(|source| {
            format!("failed to read skill file {}: {}", path.display(), source)
        })?;
        if line.trim() == "---" {
            let id = skill_frontmatter_field(path, &frontmatter, "name")?;
            validate_skill_id(&id)
                .map_err(|error| format!("skill file {}: {}", path.display(), error))?;
            let description = skill_frontmatter_field(path, &frontmatter, "description")?;

            return Ok(ParsedSkillFile {
                id: id.clone(),
                name: id,
                description,
                markdown: String::new(),
            });
        }

        frontmatter.push(line);
    }

    Err(format!(
        "skill file {} is missing closing YAML frontmatter delimiter '---'",
        path.display()
    ))
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

    let entries = discovery
        .skills
        .iter()
        .filter(|skill| {
            skill_applies_to_workspace(skill, workspace_id)
                && !skill_is_disabled(skill, &disabled_ids)
                && !skill_is_required_disabled(skill, &required_disabled_ids)
        })
        .map(skill_frontmatter_entry)
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![neutral_text_message(
        NeutralChatRole::Developer,
        format!(
            "<skills_instructions>\n## Skills\nA skill is a set of instructions provided through a `SKILL.md` source. Below is the list of skills that can be used in this session. Each entry includes a name, description, skill key, scope, and source locator. Treat this list as a routing table for the current user turn. Foco currently exposes filesystem-backed skills; `file` locators are paths on the host filesystem.\n\n### Available skills\n{}\n\n### How to use skills\n- Discovery: The list above is the skills available in this session (name + description + skill key + scope + source locator). Empty selected `skillIds` or empty Agent task skill ids mean no skill was explicitly preselected for the task; they do not mean the available-skill list is empty. `file` entries live on the host filesystem and must be opened with `read_file` when the skill is selected. Workspace skill paths are usually workspace-relative in practice; global skill paths are usually absolute paths outside the workspace and `read_file` will request explicit user authorization before reading them.\n- Trigger rules: Before starting task work, compare the user's latest request with the available skill names and descriptions. If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list, its `SKILL.md` file can't be read, or the user denies external read access for a global skill, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1. After deciding to use a skill, the main agent must read its `SKILL.md` completely with `read_file` before taking task actions. If a read is truncated or line-ranged, continue until the full file is loaded.\n  2. When `SKILL.md` references another resource, resolve relative paths against that skill's directory and read only the resources required for the current task.\n  3. If `SKILL.md` points to extra folders such as `references/`, use its routing instructions to identify the relevant files. The main agent must read each required instruction or reference file itself before acting on it. Do not delegate reading, summarizing, or interpreting skill instructions to a subagent.\n  4. Prefer running or patching provided scripts, templates, or assets from the skill directory instead of retyping large code blocks or recreating assets.\n  5. Reuse provided assets or templates from the skill source whenever they fit the task.\n- Coordination and sequencing: If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them. Announce which skill(s) you're using and why in one short line. If you skip an obvious skill, say why.\n- Context hygiene: Progressive disclosure applies to selecting relevant files, not partially reading a selected instruction file. Do not load unrelated references, scripts, or assets. Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked. When variants exist, pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly, state the issue, pick the next-best approach, and continue.\n</skills_instructions>",
            entries.join("\n")
        ),
    )])
}

fn skill_frontmatter_entry(skill: &SkillSettings) -> String {
    format!(
        "- {}: {} (key: {}, scope: {}, file: {})",
        xml_text_escape(&skill.name),
        xml_text_escape(&skill.description),
        xml_text_escape(&skill.key),
        xml_text_escape(&skill_scope_prompt_label(skill)),
        xml_text_escape(&skill.path.display().to_string())
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
}
