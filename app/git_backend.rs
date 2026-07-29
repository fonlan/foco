use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use gix::{
    bstr::ByteSlice,
    refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    },
};

use foco_agent::AgentInstanceId;

use super::{
    ApiError, GitBranchesResponse, GitDiffResponse, GitStatusFileSummary, GitStatusResponse,
    GitWorktreeSummary,
};

const AGENT_WORKTREE_ROOT_DIR: &str = "agent-worktrees";
pub(super) const AGENT_WORKTREE_SHARED_HEAD_MISMATCH_MESSAGE: &str =
    "does not match Agent worktree base revision";
pub(super) const AGENT_WORKTREE_SHARED_DIRTY_MESSAGE: &str =
    "cannot merge Agent worktree while shared workspace has uncommitted changes";
pub(super) const AGENT_WORKTREE_FOCO_RUNTIME_COMMITTED_MESSAGE: &str =
    "Agent worktree commits must not include Foco runtime path '.foco'";

/// Returns true when `repo_path` is the reserved Foco runtime directory or a path under it.
///
/// Repo paths use forward slashes. This is independent of `.gitignore`.
pub(crate) fn is_foco_reserved_repo_path(repo_path: &str) -> bool {
    let normalized = repo_path.trim().trim_start_matches("./").replace('\\', "/");
    normalized == ".foco" || normalized.starts_with(".foco/")
}

#[derive(Clone, Debug)]
pub(super) struct AgentWorktreeInfo {
    pub(super) root_path: PathBuf,
    pub(super) base_revision: String,
    pub(super) branch: String,
}

#[derive(Clone, Debug)]
pub(super) struct AgentWorktreeMergeResult {
    pub(super) base_revision: String,
    pub(super) changed_paths: Vec<String>,
    pub(super) diff_id: String,
}

#[derive(Clone, Debug)]
struct GitStatusEntry {
    workspace_path: String,
    repo_path: String,
    index_status: char,
    worktree_status: char,
}

pub(super) fn is_git_workspace(workspace_path: &Path) -> Result<bool, ApiError> {
    match gix::discover(workspace_path) {
        Ok(repo) => Ok(repo.workdir().is_some()),
        Err(_) => Ok(false),
    }
}

pub(super) fn git_status_response(workspace_path: &Path) -> Result<GitStatusResponse, ApiError> {
    let entries = status_entries(workspace_path)?;
    let status = status_text(&entries);

    Ok(GitStatusResponse {
        is_git_repository: true,
        files: entries.into_iter().map(status_summary).collect(),
        status,
    })
}

pub(super) fn git_diff_response(
    workspace_path: &Path,
    path: Option<String>,
) -> Result<GitDiffResponse, ApiError> {
    let repo = open_repo(workspace_path)?;
    let entries = status_entries_for_repo(workspace_path, &repo)?;
    let files = entries
        .iter()
        .filter(|entry| entry.worktree_status != ' ')
        .cloned()
        .map(status_summary)
        .collect::<Vec<_>>();
    let staged_files = entries
        .iter()
        .filter(|entry| entry.index_status != ' ' && entry.index_status != '?')
        .cloned()
        .map(status_summary)
        .collect::<Vec<_>>();
    let filtered_entries = entries
        .iter()
        .filter(|entry| {
            path.as_deref()
                .map_or(true, |path| entry.workspace_path == path)
        })
        .cloned()
        .collect::<Vec<_>>();
    let status = status_text(&entries);
    let staged_diff = staged_diff_text(&repo, &filtered_entries)?;
    let diff = unstaged_diff_text(&repo, &filtered_entries)?;

    Ok(GitDiffResponse {
        path,
        status,
        diff,
        staged_diff,
        files,
        staged_files,
    })
}

pub(super) fn git_head_text_for_workspace_path(
    workspace_path: &Path,
    workspace_relative_path: &str,
) -> Result<Option<String>, ApiError> {
    let repo = open_repo(workspace_path)?;
    let worktree_root = repo
        .workdir()
        .ok_or_else(|| ApiError::bad_request("git repository does not have a worktree"))?
        .canonicalize()
        .map_err(|source| {
            ApiError::internal(format!("failed to resolve git worktree: {source}"))
        })?;
    let workspace_root = workspace_path.canonicalize().map_err(|source| {
        ApiError::internal(format!("failed to resolve workspace path: {source}"))
    })?;
    let absolute_path = workspace_root.join(workspace_relative_path);
    let repo_path = absolute_path
        .strip_prefix(&worktree_root)
        .map_err(|_| ApiError::bad_request("path is outside git worktree"))?
        .display()
        .to_string()
        .replace('\\', "/");
    let head_tree_id = repo
        .head_tree_id_or_empty()
        .map_err(|source| ApiError::internal(format!("failed to read git HEAD tree: {source}")))?
        .detach();
    let head_tree = repo
        .find_tree(head_tree_id)
        .map_err(|source| ApiError::internal(format!("failed to read git HEAD tree: {source}")))?;
    let Some(bytes) = blob_from_tree(&repo, &head_tree, &repo_path)? else {
        return Ok(None);
    };
    Ok(String::from_utf8(bytes).ok())
}

pub(super) fn git_branches_response(
    workspace_path: &Path,
) -> Result<GitBranchesResponse, ApiError> {
    let repo = open_repo(workspace_path)?;
    let current_branch = repo
        .head_name()
        .map_err(|source| ApiError::internal(format!("failed to read git HEAD: {source}")))?
        .map(|name| name.shorten().to_string());
    let current_worktree_path = repo.workdir().map(canonical_path).transpose()?;
    let mut branches = repo
        .references()
        .map_err(|source| ApiError::internal(format!("failed to read git references: {source}")))?
        .local_branches()
        .map_err(|source| ApiError::internal(format!("failed to read git branches: {source}")))?
        .map(|reference| {
            reference
                .map(|reference| reference.name().shorten().to_string())
                .map_err(|source| {
                    ApiError::internal(format!("failed to read git branch: {source}"))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    branches.sort();

    Ok(GitBranchesResponse {
        is_git_repository: true,
        current_branch,
        branches,
        worktrees: git_worktrees(&repo, current_worktree_path.as_deref())?,
    })
}

pub(super) fn resolve_git_worktree_target(
    workspace_path: &Path,
    worktree_path: &str,
) -> Result<PathBuf, ApiError> {
    let requested = worktree_path.trim();
    if requested.is_empty() {
        return Ok(workspace_path.to_path_buf());
    }

    let requested_path = Path::new(requested);
    let requested_path = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        workspace_path.join(requested_path)
    }
    .canonicalize()
    .map_err(|source| {
        ApiError::bad_request(format!("failed to resolve git worktree target: {source}"))
    })?;
    let requested_path_text = path_to_slash_string(&requested_path);
    let workspace_root = workspace_path.canonicalize().map_err(|source| {
        ApiError::internal(format!("failed to resolve workspace path: {source}"))
    })?;

    if requested_path == workspace_root {
        return Ok(workspace_root);
    }

    let repo = open_repo(workspace_path)?;
    if git_worktrees(&repo, None)?
        .iter()
        .any(|worktree| worktree.path == requested_path_text)
    {
        return Ok(requested_path);
    }

    Err(ApiError::bad_request(format!(
        "unknown git worktree target: {}",
        requested_path.display()
    )))
}

fn git_worktrees(
    repo: &gix::Repository,
    current_worktree_path: Option<&Path>,
) -> Result<Vec<GitWorktreeSummary>, ApiError> {
    let main_path = repo.common_dir().parent().map(canonical_path).transpose()?;
    let mut worktrees = Vec::new();
    if let Some(path) = main_path {
        worktrees.push(GitWorktreeSummary {
            name: worktree_display_name(&path),
            path: path_to_slash_string(&path),
            branch: read_worktree_branch(&repo.common_dir().join("HEAD"))?,
            is_current: current_worktree_path == Some(path.as_path()),
        });
    }

    let linked_root = repo.common_dir().join("worktrees");
    match fs::read_dir(&linked_root) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|source| {
                    ApiError::internal(format!("failed to read git worktree metadata: {source}"))
                })?;
                let metadata_path = entry.path();
                if !entry
                    .file_type()
                    .map_err(|source| {
                        ApiError::internal(format!(
                            "failed to read git worktree metadata: {source}"
                        ))
                    })?
                    .is_dir()
                {
                    continue;
                }
                let Some(path) = read_linked_worktree_path(&metadata_path)? else {
                    continue;
                };
                worktrees.push(GitWorktreeSummary {
                    name: worktree_display_name(&path),
                    path: path_to_slash_string(&path),
                    branch: read_worktree_branch(&metadata_path.join("HEAD"))?,
                    is_current: current_worktree_path == Some(path.as_path()),
                });
            }
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(ApiError::internal(format!(
                "failed to read git worktrees: {source}"
            )));
        }
    }

    worktrees.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(worktrees)
}

fn read_linked_worktree_path(metadata_path: &Path) -> Result<Option<PathBuf>, ApiError> {
    let gitdir = metadata_path.join("gitdir");
    let text = match fs::read_to_string(&gitdir) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ApiError::internal(format!(
                "failed to read git worktree path: {source}"
            )));
        }
    };
    let dot_git = {
        let path = PathBuf::from(text.trim());
        if path.is_absolute() {
            path
        } else {
            metadata_path.join(path)
        }
    };
    let Some(worktree_path) = dot_git.parent() else {
        return Ok(None);
    };
    match worktree_path.canonicalize() {
        Ok(path) => Ok(Some(path)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ApiError::internal(format!(
            "failed to resolve git worktree path: {source}"
        ))),
    }
}

fn read_worktree_branch(head_path: &Path) -> Result<Option<String>, ApiError> {
    let text = fs::read_to_string(head_path).map_err(|source| {
        ApiError::internal(format!("failed to read git worktree HEAD: {source}"))
    })?;
    let head = text.trim();
    Ok(head
        .strip_prefix("ref: refs/heads/")
        .map(|branch| branch.to_string()))
}

fn canonical_path(path: &Path) -> Result<PathBuf, ApiError> {
    path.canonicalize().map_err(|source| {
        ApiError::internal(format!("failed to resolve git worktree path: {source}"))
    })
}

fn worktree_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

pub(super) fn switch_git_branch(workspace_path: &Path, name: String) -> Result<(), ApiError> {
    let branch = validate_git_branch_name(name)?;
    let repo = open_repo(workspace_path)?;
    let entries = status_entries_for_repo(workspace_path, &repo)?;
    if !entries.is_empty() {
        return Err(ApiError::bad_request(
            "cannot switch git branch with uncommitted changes",
        ));
    }

    let ref_name = branch_ref_name(&branch);
    let mut reference = repo.find_reference(ref_name.as_str()).map_err(|source| {
        ApiError::bad_request(format!("git branch was not found: {branch} ({source})"))
    })?;
    let target_commit = reference.peel_to_commit().map_err(|source| {
        ApiError::bad_request(format!(
            "git branch does not point to a commit: {branch} ({source})"
        ))
    })?;
    let tree_id = target_commit
        .tree_id()
        .map_err(|source| ApiError::internal(format!("failed to read git branch tree: {source}")))?
        .detach();
    let mut target_index = repo.index_from_tree(&tree_id).map_err(|source| {
        ApiError::internal(format!("failed to build git index from branch: {source}"))
    })?;

    remove_tracked_files_missing_from_target(&repo, &target_index)?;
    checkout_index(&repo, &mut target_index)?;
    target_index
        .write(Default::default())
        .map_err(|source| ApiError::internal(format!("failed to write git index: {source}")))?;
    set_head_to_branch(&repo, &ref_name, "checkout: moving by Foco")?;

    Ok(())
}

pub(super) fn create_git_branch(workspace_path: &Path, name: String) -> Result<(), ApiError> {
    let branch = validate_git_branch_name(name)?;
    let repo = open_repo(workspace_path)?;
    let ref_name = branch_ref_name(&branch);
    let target = repo
        .head_id()
        .map_err(|source| {
            ApiError::bad_request(format!("cannot create branch from unborn HEAD: {source}"))
        })?
        .detach();

    repo.reference(
        ref_name.as_str(),
        target,
        PreviousValue::MustNotExist,
        "branch: Created by Foco",
    )
    .map_err(|source| {
        ApiError::bad_request(format!("failed to create git branch '{branch}': {source}"))
    })?;
    set_head_to_branch(&repo, &ref_name, "checkout: moving by Foco")?;

    Ok(())
}

pub(super) fn stage_git_file(
    workspace_path: &Path,
    workspace_relative_path: &str,
) -> Result<(), ApiError> {
    let repo = open_repo(workspace_path)?;
    let repo_path = repo_relative_path(workspace_path, &repo, workspace_relative_path)?;
    let mut index = mutable_index(&repo)?;

    replace_index_entry_from_worktree(&repo, &mut index, &repo_path)?;
    write_index(index)?;

    Ok(())
}

pub(super) fn unstage_git_file(
    workspace_path: &Path,
    workspace_relative_path: &str,
) -> Result<(), ApiError> {
    let repo = open_repo(workspace_path)?;
    let repo_path = repo_relative_path(workspace_path, &repo, workspace_relative_path)?;
    let mut index = mutable_index(&repo)?;
    let head_tree_id = repo
        .head_tree_id_or_empty()
        .map_err(|source| ApiError::internal(format!("failed to read git HEAD tree: {source}")))?
        .detach();
    let head_tree = repo
        .find_tree(head_tree_id)
        .map_err(|source| ApiError::internal(format!("failed to read git HEAD tree: {source}")))?;

    replace_index_entry_from_tree(&mut index, &head_tree, &repo_path)?;
    write_index(index)?;

    Ok(())
}

pub(super) fn discard_git_file(
    workspace_path: &Path,
    workspace_relative_path: &str,
) -> Result<(), ApiError> {
    let repo = open_repo(workspace_path)?;
    let repo_path = repo_relative_path(workspace_path, &repo, workspace_relative_path)?;
    let index = repo
        .index_or_load_from_head_or_empty()
        .map_err(|source| ApiError::internal(format!("failed to read git index: {source}")))?;
    let Some(worktree_path) = repo.workdir_path(repo_path.as_bytes().as_bstr()) else {
        return Err(ApiError::bad_request("path is outside git worktree"));
    };
    let Some(bytes) = blob_from_index(&repo, &index, &repo_path)? else {
        remove_worktree_path(&worktree_path)?;
        return Ok(());
    };

    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            ApiError::internal(format!("failed to create git worktree directory: {source}"))
        })?;
    }
    std::fs::write(&worktree_path, bytes).map_err(|source| {
        ApiError::internal(format!("failed to write git worktree file: {source}"))
    })?;

    Ok(())
}

pub(super) fn commit_staged_changes(
    workspace_path: &Path,
    message: String,
) -> Result<String, ApiError> {
    let message = validate_commit_message(message)?;
    let repo = open_repo(workspace_path)?;
    let entries = status_entries_for_repo(workspace_path, &repo)?;
    if entries
        .iter()
        .all(|entry| entry.index_status == ' ' || entry.index_status == '?')
    {
        return Err(ApiError::bad_request("no staged git changes to commit"));
    }

    let index = repo
        .index_or_load_from_head_or_empty()
        .map_err(|source| ApiError::internal(format!("failed to read git index: {source}")))?;
    let tree_id = write_tree_from_index(&repo, &index)?;
    let parents = repo
        .head_id()
        .map(|id| vec![id.detach()])
        .unwrap_or_else(|_| Vec::new());

    let commit_id = repo
        .commit("HEAD", message, tree_id, parents)
        .map_err(|source| ApiError::bad_request(format!("failed to create git commit: {source}")))?
        .detach()
        .to_string();

    Ok(commit_id)
}

pub(super) fn shared_workspace_head_commit_id(workspace_path: &Path) -> Result<String, ApiError> {
    let repo = open_repo(workspace_path)?;
    repo.head_id()
        .map(|id| id.detach().to_string())
        .map_err(|source| {
            ApiError::bad_request(format!("shared workspace has unborn HEAD: {source}"))
        })
}

pub(super) fn create_agent_worktree(
    workspace_path: &Path,
    instance_id: &str,
) -> Result<AgentWorktreeInfo, ApiError> {
    let repo = open_repo(workspace_path)?;
    let base_revision = repo
        .head_id()
        .map_err(|source| {
            ApiError::bad_request(format!(
                "cannot create Agent worktree from unborn HEAD: {source}"
            ))
        })?
        .detach();
    let branch = format!("foco/agent-worktrees/{instance_id}");
    let branch = validate_git_branch_name(branch)?;
    let ref_name = branch_ref_name(&branch);
    let root_path = agent_worktree_path(workspace_path, instance_id);
    if root_path.exists() {
        return Err(ApiError::bad_request(format!(
            "Agent worktree path already exists: {}",
            root_path.display()
        )));
    }

    let linked_git_dir = repo
        .common_dir()
        .join("worktrees")
        .join(agent_worktree_git_dir_name(instance_id));
    if linked_git_dir.exists() {
        return Err(ApiError::bad_request(format!(
            "Agent worktree git metadata already exists: {}",
            linked_git_dir.display()
        )));
    }

    repo.reference(
        ref_name.as_str(),
        base_revision,
        PreviousValue::MustNotExist,
        "branch: Created by Foco Agent worktree",
    )
    .map_err(|source| {
        ApiError::bad_request(format!(
            "failed to create Agent worktree branch '{branch}': {source}"
        ))
    })?;

    fs::create_dir_all(&root_path).map_err(|source| {
        ApiError::internal(format!(
            "failed to create Agent worktree directory: {source}"
        ))
    })?;
    fs::create_dir_all(&linked_git_dir).map_err(|source| {
        ApiError::internal(format!(
            "failed to create Agent worktree git directory: {source}"
        ))
    })?;
    let dot_git = root_path.join(".git");
    fs::write(&dot_git, format!("gitdir: {}\n", linked_git_dir.display())).map_err(|source| {
        ApiError::internal(format!(
            "failed to write Agent worktree .git file: {source}"
        ))
    })?;
    fs::write(
        linked_git_dir.join("gitdir"),
        format!("{}\n", dot_git.display()),
    )
    .map_err(|source| {
        ApiError::internal(format!(
            "failed to write Agent worktree gitdir file: {source}"
        ))
    })?;
    fs::write(
        linked_git_dir.join("commondir"),
        format!("{}\n", repo.common_dir().display()),
    )
    .map_err(|source| {
        ApiError::internal(format!(
            "failed to write Agent worktree commondir file: {source}"
        ))
    })?;
    fs::write(linked_git_dir.join("HEAD"), format!("ref: {ref_name}\n")).map_err(|source| {
        ApiError::internal(format!(
            "failed to write Agent worktree HEAD file: {source}"
        ))
    })?;

    let worktree_repo = open_repo(&root_path)?;
    let base_commit = repo.find_commit(base_revision).map_err(|source| {
        ApiError::internal(format!(
            "failed to read Agent worktree base commit: {source}"
        ))
    })?;
    let tree_id = base_commit.tree_id().map_err(|source| {
        ApiError::internal(format!("failed to read Agent worktree base tree: {source}"))
    })?;
    let mut index = worktree_repo
        .index_from_tree(&tree_id.detach())
        .map_err(|source| {
            ApiError::internal(format!("failed to build Agent worktree index: {source}"))
        })?;
    checkout_index(&worktree_repo, &mut index)?;
    write_index(index)?;

    // Skill directories are intentionally not materialised here (no
    // `.agents/skills` / `.claude/skills` symlinks or copies). Plan/isolated
    // worktrees must stay a pure Git checkout so status/diff/commit/merge stay
    // clean. Workspace/global Skills are exposed read-only via the run-scoped
    // `AvailableSkillsSnapshot` / `skill_read_root_dirs` grant in
    // `ensure_read_file_external_access`, aligned with the prompt routing table.
    Ok(AgentWorktreeInfo {
        root_path,
        base_revision: base_revision.to_string(),
        branch,
    })
}

pub(super) fn delete_agent_worktree(
    workspace_path: &Path,
    worktree_path: &Path,
    allow_changes: bool,
) -> Result<(), ApiError> {
    let worktree_path = validate_agent_worktree_path(workspace_path, worktree_path)?;
    let repo = open_repo(&worktree_path)?;
    if !allow_changes && !status_entries_for_repo(&worktree_path, &repo)?.is_empty() {
        return Err(ApiError::bad_request(
            "cannot delete Agent worktree with unmerged changes",
        ));
    }
    let git_dir = repo.git_dir().to_path_buf();
    let branch_ref = repo
        .head_name()
        .map_err(|source| {
            ApiError::internal(format!("failed to read Agent worktree HEAD: {source}"))
        })?
        .map(|name| name.to_string());
    drop(repo);

    remove_worktree_path(&worktree_path)?;
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir).map_err(|source| {
            ApiError::internal(format!(
                "failed to remove Agent worktree git metadata: {source}"
            ))
        })?;
    }
    if let Some(ref_name) = branch_ref
        .as_deref()
        .filter(|name| name.starts_with("refs/heads/foco/agent-worktrees/"))
    {
        delete_agent_worktree_branch(workspace_path, ref_name)?;
    }
    Ok(())
}

pub(super) fn agent_worktree_head_commit(worktree_path: &Path) -> Result<String, ApiError> {
    let repo = open_repo(worktree_path)?;
    let head = repo
        .head_id()
        .map_err(|source| {
            ApiError::bad_request(format!("Agent worktree has unborn HEAD: {source}"))
        })?
        .detach();
    Ok(head.to_string())
}

fn delete_agent_worktree_branch(workspace_path: &Path, ref_name: &str) -> Result<(), ApiError> {
    let repo = open_repo(workspace_path)?;
    match repo.find_reference(ref_name) {
        Ok(reference) => reference.delete().map_err(|source| {
            ApiError::internal(format!(
                "failed to delete Agent worktree branch '{ref_name}': {source}"
            ))
        }),
        Err(_) => Ok(()),
    }
}

pub(super) fn merge_agent_worktree(
    workspace_path: &Path,
    worktree_path: &Path,
    base_revision: &str,
) -> Result<AgentWorktreeMergeResult, ApiError> {
    let worktree_path = validate_agent_worktree_path(workspace_path, worktree_path)?;
    let shared_repo = open_repo(workspace_path)?;
    let worktree_repo = open_repo(&worktree_path)?;
    let shared_head = shared_repo
        .head_id()
        .map_err(|source| {
            ApiError::bad_request(format!("shared workspace has unborn HEAD: {source}"))
        })?
        .detach()
        .to_string();
    if shared_head != base_revision {
        return Err(ApiError::bad_request(format!(
            "shared workspace HEAD '{shared_head}' does not match Agent worktree base revision '{base_revision}'"
        )));
    }
    let worktree_head = worktree_repo
        .head_id()
        .map_err(|source| {
            ApiError::bad_request(format!("Agent worktree has unborn HEAD: {source}"))
        })?
        .detach()
        .to_string();
    if worktree_head != base_revision {
        return Err(ApiError::bad_request(format!(
            "Agent worktree HEAD '{worktree_head}' does not match recorded base revision '{base_revision}'"
        )));
    }
    if !status_entries_for_repo(workspace_path, &shared_repo)?.is_empty() {
        return Err(ApiError::bad_request(AGENT_WORKTREE_SHARED_DIRTY_MESSAGE));
    }

    let entries = status_entries_for_repo(&worktree_path, &worktree_repo)?;
    let diff = git_diff_response(&worktree_path, None)?;
    for entry in &entries {
        if is_foco_reserved_repo_path(&entry.repo_path) {
            return Err(ApiError::bad_request(format!(
                "{AGENT_WORKTREE_FOCO_RUNTIME_COMMITTED_MESSAGE}: {}",
                entry.repo_path
            )));
        }
        let Some(target_path) = shared_repo.workdir_path(entry.repo_path.as_bytes().as_bstr())
        else {
            return Err(ApiError::bad_request(
                "Agent worktree path is outside shared git worktree",
            ));
        };
        match blob_from_worktree(&worktree_repo, &entry.repo_path)? {
            Some(bytes) => {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent).map_err(|source| {
                        ApiError::internal(format!(
                            "failed to create merge target directory: {source}"
                        ))
                    })?;
                }
                fs::write(&target_path, bytes).map_err(|source| {
                    ApiError::internal(format!(
                        "failed to write merged Agent worktree file: {source}"
                    ))
                })?;
            }
            None => {
                // Missing worktree blobs mean deletions. Directory / symlink-to-dir
                // noise must not wipe the shared path (blob_from_worktree returns
                // None for those too).
                if let Some(source_path) =
                    worktree_repo.workdir_path(entry.repo_path.as_bytes().as_bstr())
                {
                    if worktree_path_is_directory(&source_path) {
                        continue;
                    }
                }
                remove_worktree_path(&target_path)?;
            }
        }
    }

    Ok(AgentWorktreeMergeResult {
        base_revision: base_revision.to_string(),
        changed_paths: entries
            .into_iter()
            .map(|entry| entry.workspace_path)
            .collect(),
        diff_id: agent_worktree_diff_id(&diff),
    })
}

pub(super) fn fast_forward_shared_workspace_to_agent_worktree(
    workspace_path: &Path,
    worktree_path: &Path,
    base_revision: &str,
) -> Result<Option<String>, ApiError> {
    let worktree_path = validate_agent_worktree_path(workspace_path, worktree_path)?;
    let shared_repo = open_repo(workspace_path)?;
    let worktree_repo = open_repo(&worktree_path)?;
    let worktree_head = worktree_repo
        .head_id()
        .map_err(|source| {
            ApiError::bad_request(format!("Agent worktree has unborn HEAD: {source}"))
        })?
        .detach();
    // Reject committed `.foco` runtime paths before shared HEAD / dirty checks so
    // SharedHeadMismatch cannot route into automatic merge with a poisoned diff.
    if worktree_head.to_string() != base_revision {
        reject_committed_foco_runtime_paths(
            &worktree_repo,
            base_revision,
            &worktree_head.to_string(),
        )?;
    }
    let shared_head = shared_repo
        .head_id()
        .map_err(|source| {
            ApiError::bad_request(format!("shared workspace has unborn HEAD: {source}"))
        })?
        .detach();
    if shared_head.to_string() != base_revision {
        return Err(ApiError::bad_request(format!(
            "shared workspace HEAD '{shared_head}' does not match Agent worktree base revision '{base_revision}'"
        )));
    }
    if !status_entries_for_repo(workspace_path, &shared_repo)?.is_empty() {
        return Err(ApiError::bad_request(AGENT_WORKTREE_SHARED_DIRTY_MESSAGE));
    }
    if !status_entries_for_repo(&worktree_path, &worktree_repo)?.is_empty() {
        return Err(ApiError::bad_request(
            "cannot merge Agent worktree with uncommitted changes",
        ));
    }

    if worktree_head.to_string() == base_revision {
        return Ok(None);
    }
    let current_branch = shared_repo
        .head_name()
        .map_err(|source| ApiError::internal(format!("failed to read git HEAD: {source}")))?
        .map(|name| name.shorten().to_string())
        .ok_or_else(|| ApiError::bad_request("cannot fast-forward detached shared workspace"))?;
    let target_commit = worktree_repo.find_commit(worktree_head).map_err(|source| {
        ApiError::bad_request(format!(
            "failed to read Agent worktree HEAD commit: {source}"
        ))
    })?;
    let tree_id = target_commit.tree_id().map_err(|source| {
        ApiError::internal(format!("failed to read Agent worktree HEAD tree: {source}"))
    })?;
    let mut target_index = shared_repo
        .index_from_tree(&tree_id.detach())
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to build Agent worktree merge index: {source}"
            ))
        })?;

    remove_tracked_files_missing_from_target(&shared_repo, &target_index)?;
    checkout_index(&shared_repo, &mut target_index)?;
    write_index(target_index)?;
    shared_repo
        .reference(
            branch_ref_name(&current_branch).as_str(),
            worktree_head,
            PreviousValue::Any,
            "merge: Fast-forward Foco Agent worktree",
        )
        .map_err(|source| {
            ApiError::bad_request(format!("failed to fast-forward shared workspace: {source}"))
        })?;

    Ok(Some(worktree_head.to_string()))
}

pub(super) fn agent_worktree_committed_diff(
    workspace_path: &Path,
    worktree_path: &Path,
    base_revision: &str,
) -> Result<String, ApiError> {
    let worktree_path = validate_agent_worktree_path(workspace_path, worktree_path)?;
    let worktree_repo = open_repo(&worktree_path)?;
    let base_id = gix::ObjectId::from_hex(base_revision.as_bytes()).map_err(|source| {
        ApiError::bad_request(format!("invalid Agent worktree base revision: {source}"))
    })?;
    let base_commit = worktree_repo.find_commit(base_id).map_err(|source| {
        ApiError::bad_request(format!(
            "failed to read Agent worktree base commit: {source}"
        ))
    })?;
    let head_id = worktree_repo
        .head_id()
        .map_err(|source| {
            ApiError::bad_request(format!("Agent worktree has unborn HEAD: {source}"))
        })?
        .detach();
    reject_committed_foco_runtime_paths(&worktree_repo, base_revision, &head_id.to_string())?;
    let head_commit = worktree_repo.find_commit(head_id).map_err(|source| {
        ApiError::bad_request(format!(
            "failed to read Agent worktree HEAD commit: {source}"
        ))
    })?;
    let base_tree_id = base_commit.tree_id().map_err(|source| {
        ApiError::internal(format!("failed to read Agent worktree base tree: {source}"))
    })?;
    let head_tree_id = head_commit.tree_id().map_err(|source| {
        ApiError::internal(format!("failed to read Agent worktree HEAD tree: {source}"))
    })?;
    let base_tree = worktree_repo
        .find_tree(base_tree_id.detach())
        .map_err(|source| ApiError::internal(format!("failed to read base tree: {source}")))?;
    let head_tree = worktree_repo
        .find_tree(head_tree_id.detach())
        .map_err(|source| ApiError::internal(format!("failed to read HEAD tree: {source}")))?;
    diff_trees_text(&worktree_repo, &base_tree, &head_tree)
}

pub(super) fn validate_agent_worktree_path(
    workspace_path: &Path,
    worktree_path: &Path,
) -> Result<PathBuf, ApiError> {
    let managed_root = agent_worktree_root(workspace_path)
        .canonicalize()
        .map_err(|source| {
            ApiError::bad_request(format!("failed to resolve Agent worktree root: {source}"))
        })?;
    let worktree_path = worktree_path.canonicalize().map_err(|source| {
        ApiError::bad_request(format!("failed to resolve Agent worktree path: {source}"))
    })?;
    if worktree_path == managed_root || !worktree_path.starts_with(&managed_root) {
        return Err(ApiError::bad_request(format!(
            "Agent worktree path is outside Foco managed directory: {}",
            worktree_path.display()
        )));
    }
    Ok(worktree_path)
}

pub(super) fn agent_worktree_root(workspace_path: &Path) -> PathBuf {
    workspace_path.join(".foco").join(AGENT_WORKTREE_ROOT_DIR)
}

pub(super) fn agent_instance_worktree_relative_path(instance_id: &AgentInstanceId) -> String {
    format!(".foco/{AGENT_WORKTREE_ROOT_DIR}/{instance_id}")
}

pub(super) fn agent_worktree_relative_path(
    workspace_path: &Path,
    worktree_path: &Path,
) -> Result<String, ApiError> {
    if worktree_path.is_absolute() {
        let relative = worktree_path.strip_prefix(workspace_path).map_err(|_| {
            ApiError::bad_request(format!(
                "Agent worktree path is outside workspace: {}",
                worktree_path.display()
            ))
        })?;
        Ok(relative.to_string_lossy().replace('\\', "/"))
    } else {
        Ok(worktree_path.to_string_lossy().replace('\\', "/"))
    }
}

pub(super) fn resolve_agent_worktree_path(workspace_path: &Path, root_path: &str) -> PathBuf {
    let root_path = Path::new(root_path);
    if root_path.is_absolute() {
        root_path.to_path_buf()
    } else {
        workspace_path.join(root_path)
    }
}

pub(super) fn agent_instance_worktree_path(
    workspace_path: &Path,
    instance_id: &AgentInstanceId,
) -> PathBuf {
    // ponytail: Foco-managed worktrees always live under the current workspace;
    // if that layout changes, make the relative location explicit in the DB.
    agent_worktree_path(workspace_path, instance_id.as_str())
}

fn agent_worktree_path(workspace_path: &Path, instance_id: &str) -> PathBuf {
    agent_worktree_root(workspace_path).join(instance_id)
}

fn agent_worktree_git_dir_name(instance_id: &str) -> String {
    format!("foco-{instance_id}")
}

pub(super) fn agent_worktree_diff_id(diff: &GitDiffResponse) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in diff
        .status
        .bytes()
        .chain(diff.diff.bytes())
        .chain(diff.staged_diff.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("agent-diff-{hash:016x}")
}

fn open_repo(workspace_path: &Path) -> Result<gix::Repository, ApiError> {
    let repo = gix::discover(workspace_path).map_err(|source| {
        ApiError::bad_request(format!(
            "workspace is not a git repository: {} ({source})",
            workspace_path.display()
        ))
    })?;

    if repo.workdir().is_none() {
        return Err(ApiError::bad_request(format!(
            "workspace is not a git worktree: {}",
            workspace_path.display()
        )));
    }

    Ok(repo)
}

fn status_entries(workspace_path: &Path) -> Result<Vec<GitStatusEntry>, ApiError> {
    let repo = open_repo(workspace_path)?;
    status_entries_for_repo(workspace_path, &repo)
}

fn status_entries_for_repo(
    workspace_path: &Path,
    repo: &gix::Repository,
) -> Result<Vec<GitStatusEntry>, ApiError> {
    let mut entries = BTreeMap::<String, GitStatusEntry>::new();
    let workspace_root = workspace_path.canonicalize().map_err(|source| {
        ApiError::internal(format!("failed to resolve workspace path: {source}"))
    })?;
    let worktree_root = repo
        .workdir()
        .ok_or_else(|| ApiError::bad_request("git repository does not have a worktree"))?
        .canonicalize()
        .map_err(|source| {
            ApiError::internal(format!("failed to resolve git worktree: {source}"))
        })?;
    let status_iter = repo
        .status(gix::progress::Discard)
        .map_err(|source| ApiError::internal(format!("failed to prepare git status: {source}")))?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(Vec::new())
        .map_err(|source| ApiError::internal(format!("failed to read git status: {source}")))?;

    for item in status_iter {
        let item = item.map_err(|source| {
            ApiError::internal(format!("failed to read git status item: {source}"))
        })?;
        match item {
            gix::status::Item::TreeIndex(change) => {
                let repo_path = bstr_to_path_string(change.location());
                if is_foco_reserved_repo_path(&repo_path) {
                    continue;
                }
                if let Some(workspace_path_string) =
                    workspace_relative_path(&workspace_root, &worktree_root, &repo_path)
                {
                    let entry = entries.entry(repo_path.clone()).or_insert(GitStatusEntry {
                        workspace_path: workspace_path_string,
                        repo_path,
                        index_status: ' ',
                        worktree_status: ' ',
                    });
                    entry.index_status = tree_index_status(&change);
                }
            }
            gix::status::Item::IndexWorktree(item) => {
                let repo_path = bstr_to_path_string(item.rela_path());
                if is_foco_reserved_repo_path(&repo_path) {
                    continue;
                }
                // Directories (and symlinks-to-directories) are not blob paths.
                // gix may still surface them as untracked when ignore rules use
                // trailing-slash directory patterns (e.g. `web/dist/`). Diff,
                // stage, merge, and plan phase commit all assume file blobs.
                if let Some(absolute_path) = repo.workdir_path(repo_path.as_bytes().as_bstr()) {
                    if worktree_path_is_directory(&absolute_path) {
                        continue;
                    }
                }
                if let Some(workspace_path_string) =
                    workspace_relative_path(&workspace_root, &worktree_root, &repo_path)
                {
                    let entry = entries.entry(repo_path.clone()).or_insert(GitStatusEntry {
                        workspace_path: workspace_path_string,
                        repo_path,
                        index_status: ' ',
                        worktree_status: ' ',
                    });
                    match item.summary() {
                        Some(gix::status::index_worktree::iter::Summary::Added) => {
                            entry.index_status = '?';
                            entry.worktree_status = '?';
                        }
                        Some(summary) => {
                            entry.worktree_status = worktree_status(summary);
                        }
                        None => {}
                    }
                }
            }
        }
    }

    Ok(entries.into_values().collect())
}

fn staged_diff_text(
    repo: &gix::Repository,
    entries: &[GitStatusEntry],
) -> Result<String, ApiError> {
    let head_tree_id = repo
        .head_tree_id_or_empty()
        .map_err(|source| ApiError::internal(format!("failed to read git HEAD tree: {source}")))?
        .detach();
    let head_tree = repo
        .find_tree(head_tree_id)
        .map_err(|source| ApiError::internal(format!("failed to read git HEAD tree: {source}")))?;
    let index = repo
        .index_or_load_from_head_or_empty()
        .map_err(|source| ApiError::internal(format!("failed to read git index: {source}")))?;
    let mut diff = String::new();

    for entry in entries
        .iter()
        .filter(|entry| entry.index_status != ' ' && entry.index_status != '?')
    {
        let old = blob_from_tree(repo, &head_tree, &entry.repo_path)?;
        let new = blob_from_index(repo, &index, &entry.repo_path)?;
        append_file_diff(
            &mut diff,
            &entry.workspace_path,
            old.as_deref(),
            new.as_deref(),
        )?;
    }

    Ok(diff)
}

fn unstaged_diff_text(
    repo: &gix::Repository,
    entries: &[GitStatusEntry],
) -> Result<String, ApiError> {
    let index = repo
        .index_or_load_from_head_or_empty()
        .map_err(|source| ApiError::internal(format!("failed to read git index: {source}")))?;
    let mut diff = String::new();

    for entry in entries.iter().filter(|entry| entry.worktree_status != ' ') {
        let old = blob_from_index(repo, &index, &entry.repo_path)?;
        let new = blob_from_worktree(repo, &entry.repo_path)?;
        append_file_diff(
            &mut diff,
            &entry.workspace_path,
            old.as_deref(),
            new.as_deref(),
        )?;
    }

    Ok(diff)
}

fn diff_trees_text(
    repo: &gix::Repository,
    old_tree: &gix::Tree<'_>,
    new_tree: &gix::Tree<'_>,
) -> Result<String, ApiError> {
    let old_index = repo.index_from_tree(&old_tree.id).map_err(|source| {
        ApiError::internal(format!(
            "failed to build git index from base tree: {source}"
        ))
    })?;
    let new_index = repo.index_from_tree(&new_tree.id).map_err(|source| {
        ApiError::internal(format!(
            "failed to build git index from HEAD tree: {source}"
        ))
    })?;
    let paths = index_paths(&old_index)
        .into_iter()
        .chain(index_paths(&new_index))
        .collect::<BTreeSet<_>>();
    let mut diff = String::new();

    for path in paths {
        let old = blob_from_tree(repo, old_tree, &path)?;
        let new = blob_from_tree(repo, new_tree, &path)?;
        append_file_diff(&mut diff, &path, old.as_deref(), new.as_deref())?;
    }

    Ok(diff)
}

fn index_paths(index: &gix::index::File) -> BTreeSet<String> {
    index
        .entries()
        .iter()
        .map(|entry| entry.path(index).to_str_lossy().into_owned())
        .collect()
}

/// Rejects base..HEAD tree changes that introduce or modify reserved `.foco` runtime paths.
fn reject_committed_foco_runtime_paths(
    repo: &gix::Repository,
    base_revision: &str,
    head_revision: &str,
) -> Result<(), ApiError> {
    let base_id = gix::ObjectId::from_hex(base_revision.as_bytes()).map_err(|source| {
        ApiError::bad_request(format!("invalid Agent worktree base revision: {source}"))
    })?;
    let head_id = gix::ObjectId::from_hex(head_revision.as_bytes()).map_err(|source| {
        ApiError::bad_request(format!("invalid Agent worktree HEAD revision: {source}"))
    })?;
    let base_commit = repo.find_commit(base_id).map_err(|source| {
        ApiError::bad_request(format!(
            "failed to read Agent worktree base commit: {source}"
        ))
    })?;
    let head_commit = repo.find_commit(head_id).map_err(|source| {
        ApiError::bad_request(format!(
            "failed to read Agent worktree HEAD commit: {source}"
        ))
    })?;
    let base_tree_id = base_commit.tree_id().map_err(|source| {
        ApiError::internal(format!("failed to read Agent worktree base tree: {source}"))
    })?;
    let head_tree_id = head_commit.tree_id().map_err(|source| {
        ApiError::internal(format!("failed to read Agent worktree HEAD tree: {source}"))
    })?;
    let base_index = repo
        .index_from_tree(&base_tree_id.detach())
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to build git index from base tree: {source}"
            ))
        })?;
    let head_index = repo
        .index_from_tree(&head_tree_id.detach())
        .map_err(|source| {
            ApiError::internal(format!(
                "failed to build git index from HEAD tree: {source}"
            ))
        })?;
    let base_paths = index_paths(&base_index);
    let head_paths = index_paths(&head_index);
    let mut offending = BTreeSet::new();
    for path in base_paths.iter().chain(head_paths.iter()) {
        if !is_foco_reserved_repo_path(path) {
            continue;
        }
        let in_base = base_paths.contains(path);
        let in_head = head_paths.contains(path);
        if in_base != in_head {
            offending.insert(path.clone());
            continue;
        }
        if in_base && in_head {
            let base_blob = blob_from_index_file(repo, &base_index, path)?;
            let head_blob = blob_from_index_file(repo, &head_index, path)?;
            if base_blob != head_blob {
                offending.insert(path.clone());
            }
        }
    }
    if let Some(path) = offending.into_iter().next() {
        return Err(ApiError::bad_request(format!(
            "{AGENT_WORKTREE_FOCO_RUNTIME_COMMITTED_MESSAGE}: {path}"
        )));
    }
    Ok(())
}

fn blob_from_index_file(
    repo: &gix::Repository,
    index: &gix::index::File,
    repo_path: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    let Some(entry) = index.entry_by_path(repo_path.as_bytes().as_bstr()) else {
        return Ok(None);
    };
    let Some(mode) = entry.mode.to_tree_entry_mode() else {
        return Ok(None);
    };
    if !mode.is_blob() {
        return Ok(None);
    }
    let blob = repo
        .find_blob(entry.id)
        .map_err(|source| ApiError::internal(format!("failed to read git index blob: {source}")))?;
    Ok(Some(blob.data.clone()))
}

fn append_file_diff(
    output: &mut String,
    repo_path: &str,
    old: Option<&[u8]>,
    new: Option<&[u8]>,
) -> Result<(), ApiError> {
    let old_exists = old.is_some();
    let new_exists = new.is_some();
    let old = old.unwrap_or_default();
    let new = new.unwrap_or_default();
    if old_exists == new_exists && old == new {
        return Ok(());
    }

    output.push_str(&format!("diff --git a/{repo_path} b/{repo_path}\n"));
    if is_binary_or_non_text(old) || is_binary_or_non_text(new) {
        output.push_str(&format!(
            "Binary files a/{repo_path} and b/{repo_path} differ\n"
        ));
        return Ok(());
    }

    if old_exists {
        output.push_str(&format!("--- a/{repo_path}\n"));
    } else {
        output.push_str("--- /dev/null\n");
    }
    if new_exists {
        output.push_str(&format!("+++ b/{repo_path}\n"));
    } else {
        output.push_str("+++ /dev/null\n");
    }

    let input = gix::diff::blob::InternedInput::new(old, new);
    let diff =
        gix::diff::blob::diff_with_slider_heuristics(gix::diff::blob::Algorithm::Histogram, &input);
    let hunks = gix::diff::blob::UnifiedDiff::new(
        &diff,
        &input,
        gix::diff::blob::unified_diff::ConsumeBinaryHunk::new(Vec::new(), "\n"),
        gix::diff::blob::unified_diff::ContextSize::default(),
    )
    .consume()
    .map_err(|source| ApiError::internal(format!("failed to format git diff: {source}")))?;
    output.push_str(&String::from_utf8_lossy(&hunks));

    Ok(())
}

fn is_binary_or_non_text(bytes: &[u8]) -> bool {
    bytes.contains(&0) || std::str::from_utf8(bytes).is_err()
}

fn blob_from_tree(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    repo_path: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    let Some(entry) = tree
        .lookup_entry_by_path(repo_path)
        .map_err(|source| ApiError::internal(format!("failed to read git tree entry: {source}")))?
    else {
        return Ok(None);
    };
    if !entry.mode().is_blob() {
        return Ok(None);
    }

    let blob = repo
        .find_blob(entry.object_id())
        .map_err(|source| ApiError::internal(format!("failed to read git blob: {source}")))?;
    Ok(Some(blob.data.clone()))
}

fn blob_from_index(
    repo: &gix::Repository,
    index: &gix::worktree::IndexPersistedOrInMemory,
    repo_path: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    let Some(entry) = index.entry_by_path(repo_path.as_bytes().as_bstr()) else {
        return Ok(None);
    };
    let Some(mode) = entry.mode.to_tree_entry_mode() else {
        return Ok(None);
    };
    if !mode.is_blob() {
        return Ok(None);
    }

    let blob = repo
        .find_blob(entry.id)
        .map_err(|source| ApiError::internal(format!("failed to read git index blob: {source}")))?;
    Ok(Some(blob.data.clone()))
}

fn blob_from_worktree(
    repo: &gix::Repository,
    repo_path: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    let Some(path) = repo.workdir_path(repo_path.as_bytes().as_bstr()) else {
        return Ok(None);
    };
    // Safety net for callers that did not filter status first: directories and
    // symlinks-to-directories are not file blobs. Returning None keeps diff /
    // stage / merge from hard-failing plan phase completion.
    if worktree_path_is_directory(&path) {
        return Ok(None);
    }
    match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) if source.kind() == std::io::ErrorKind::IsADirectory => Ok(None),
        Err(source) => Err(ApiError::internal(format!(
            "failed to read git worktree file: {source}"
        ))),
    }
}

/// True when `path` exists and resolves to a directory (symlink targets included).
fn worktree_path_is_directory(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
}

fn mutable_index(repo: &gix::Repository) -> Result<gix::index::File, ApiError> {
    repo.open_index()
        .map_err(|source| ApiError::internal(format!("failed to read git index: {source}")))
}

fn write_index(mut index: gix::index::File) -> Result<(), ApiError> {
    // ponytail: gix preserves cache-tree extensions on write; drop it after our
    // manual entry edits so native Git and gix don't disagree about staged files.
    index.remove_tree();
    index
        .write(Default::default())
        .map_err(|source| ApiError::internal(format!("failed to write git index: {source}")))
}

fn repo_relative_path(
    workspace_path: &Path,
    repo: &gix::Repository,
    workspace_relative_path: &str,
) -> Result<String, ApiError> {
    let workspace_root = workspace_path.canonicalize().map_err(|source| {
        ApiError::internal(format!("failed to resolve workspace path: {source}"))
    })?;
    let worktree_root = repo
        .workdir()
        .ok_or_else(|| ApiError::bad_request("git repository does not have a worktree"))?
        .canonicalize()
        .map_err(|source| {
            ApiError::internal(format!("failed to resolve git worktree: {source}"))
        })?;
    let absolute_path = workspace_root.join(workspace_relative_path);
    absolute_path
        .strip_prefix(&worktree_root)
        .map_err(|_| ApiError::bad_request("path is outside git worktree"))
        .map(|path| path.display().to_string().replace('\\', "/"))
}

fn index_entry_mode_from_worktree(
    repo: &gix::Repository,
    repo_path: &str,
) -> gix::index::entry::Mode {
    #[cfg(not(unix))]
    let _ = (repo, repo_path);

    #[cfg(unix)]
    {
        let Some(path) = repo.workdir_path(repo_path.as_bytes().as_bstr()) else {
            return gix::index::entry::Mode::FILE;
        };
        let Ok(metadata) = gix::index::fs::Metadata::from_path_no_follow(&path) else {
            return gix::index::entry::Mode::FILE;
        };
        if metadata.is_file() && metadata.is_executable() {
            return gix::index::entry::Mode::FILE_EXECUTABLE;
        }
    }

    gix::index::entry::Mode::FILE
}

fn replace_index_entry_from_worktree(
    repo: &gix::Repository,
    index: &mut gix::index::File,
    repo_path: &str,
) -> Result<(), ApiError> {
    remove_index_entry(index, repo_path);

    let Some(bytes) = blob_from_worktree(repo, repo_path)? else {
        index.sort_entries();
        return Ok(());
    };
    let mode = index_entry_mode_from_worktree(repo, repo_path);
    let id = repo
        .write_blob(bytes)
        .map_err(|source| ApiError::internal(format!("failed to write git blob: {source}")))?
        .detach();
    index.dangerously_push_entry(
        Default::default(),
        id,
        gix::index::entry::Flags::from_stage(gix::index::entry::Stage::Unconflicted),
        mode,
        repo_path.as_bytes().as_bstr(),
    );
    index.sort_entries();

    Ok(())
}
fn replace_index_entry_from_tree(
    index: &mut gix::index::File,
    tree: &gix::Tree<'_>,
    repo_path: &str,
) -> Result<(), ApiError> {
    remove_index_entry(index, repo_path);

    let Some(entry) = tree
        .lookup_entry_by_path(repo_path)
        .map_err(|source| ApiError::internal(format!("failed to read git tree entry: {source}")))?
    else {
        index.sort_entries();
        return Ok(());
    };
    let mode = gix::index::entry::Mode::from(entry.mode());
    index.dangerously_push_entry(
        Default::default(),
        entry.object_id(),
        gix::index::entry::Flags::from_stage(gix::index::entry::Stage::Unconflicted),
        mode,
        repo_path.as_bytes().as_bstr(),
    );
    index.sort_entries();

    Ok(())
}

fn remove_index_entry(index: &mut gix::index::File, repo_path: &str) {
    index.remove_entries(|_, path, _| path == repo_path.as_bytes().as_bstr());
}

fn validate_commit_message(message: String) -> Result<String, ApiError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(ApiError::bad_request(
            "git commit message must not be empty",
        ));
    }

    Ok(message.to_string())
}

fn write_tree_from_index(
    repo: &gix::Repository,
    index: &gix::index::State,
) -> Result<gix::ObjectId, ApiError> {
    let mut root = PendingTree::default();
    for entry in index.entries() {
        if entry.stage() != gix::index::entry::Stage::Unconflicted {
            return Err(ApiError::bad_request(
                "cannot commit git index with conflicted entries",
            ));
        }
        let Some(mode) = entry.mode.to_tree_entry_mode() else {
            return Err(ApiError::bad_request(
                "git index contains non-tree entry mode",
            ));
        };
        let path = entry.path(index);
        let parts = path
            .split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return Err(ApiError::bad_request("git index contains empty path"));
        }
        root.insert(&parts, mode.kind(), entry.id)?;
    }

    root.write(repo)
}

#[derive(Default)]
struct PendingTree {
    files: Vec<PendingTreeFile>,
    directories: BTreeMap<Vec<u8>, PendingTree>,
}

struct PendingTreeFile {
    name: Vec<u8>,
    mode: gix::objs::tree::EntryKind,
    oid: gix::ObjectId,
}

impl PendingTree {
    fn insert(
        &mut self,
        parts: &[&[u8]],
        mode: gix::objs::tree::EntryKind,
        oid: gix::ObjectId,
    ) -> Result<(), ApiError> {
        let [name] = parts else {
            let Some((directory, rest)) = parts.split_first() else {
                return Err(ApiError::bad_request("git index contains empty path"));
            };
            return self
                .directories
                .entry(directory.to_vec())
                .or_default()
                .insert(rest, mode, oid);
        };
        self.files.push(PendingTreeFile {
            name: name.to_vec(),
            mode,
            oid,
        });
        Ok(())
    }

    fn write(self, repo: &gix::Repository) -> Result<gix::ObjectId, ApiError> {
        let mut entries = Vec::new();
        for (name, directory) in self.directories {
            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Tree.into(),
                filename: name.into(),
                oid: directory.write(repo)?,
            });
        }
        for file in self.files {
            entries.push(gix::objs::tree::Entry {
                mode: file.mode.into(),
                filename: file.name.into(),
                oid: file.oid,
            });
        }
        entries.sort();
        let tree = gix::objs::Tree { entries };
        repo.write_object(tree)
            .map(|id| id.detach())
            .map_err(|source| ApiError::internal(format!("failed to write git tree: {source}")))
    }
}

#[cfg(test)]
#[test]
fn agent_instance_worktree_path_is_derived_from_current_workspace() {
    let first_workspace = Path::new("/old/windows/migration/source");
    let second_workspace = Path::new("/Users/fonla/Repos/Foco");
    let instance_id = AgentInstanceId::new("agent-instance-migrated".to_string()).expect("id");

    assert_eq!(
        agent_instance_worktree_relative_path(&instance_id),
        ".foco/agent-worktrees/agent-instance-migrated"
    );
    assert_eq!(
        agent_instance_worktree_path(second_workspace, &instance_id),
        second_workspace
            .join(".foco")
            .join("agent-worktrees")
            .join("agent-instance-migrated")
    );
    assert_ne!(
        agent_instance_worktree_path(first_workspace, &instance_id),
        agent_instance_worktree_path(second_workspace, &instance_id),
        "worktree execution paths must follow the current workspace instead of a persisted absolute path"
    );
}

#[cfg(test)]
#[test]
fn write_index_drops_stale_cache_tree_extension() {
    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");
    fs::write(
        workspace_path.join(".git").join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n\tlogallrefupdates = true\n[user]\n\tname = Foco Test\n\temail = foco@example.invalid\n",
    )
    .expect("test git config");
    fs::write(workspace_path.join("tracked.txt"), "base\n").expect("tracked file");
    stage_git_file(workspace_path, "tracked.txt").expect("stage tracked file");
    commit_staged_changes(workspace_path, "initial".to_string()).expect("initial commit");

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .arg("write-tree")
        .output()
        .expect("run git write-tree");
    assert!(
        output.status.success(),
        "git write-tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        repo.open_index()
            .expect("index with cache tree")
            .tree()
            .is_some(),
        "native git write-tree should populate the cache-tree extension"
    );

    let mut index = repo.open_index().expect("cached index");
    write_index(index).expect("rewrite index");
    index = repo.open_index().expect("rewritten index");
    assert!(
        index.tree().is_none(),
        "Foco index writes must drop stale cache-tree data before mutating entries"
    );
}

#[cfg(all(test, unix))]
#[test]
fn stage_git_file_preserves_regular_file_modes() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");

    fs::write(workspace_path.join("a-data.txt"), "data\n").expect("non-executable file");
    let executable_path = workspace_path.join("b-script.sh");
    fs::write(&executable_path, "#!/bin/sh\n").expect("executable file");
    fs::set_permissions(&executable_path, fs::Permissions::from_mode(0o755))
        .expect("set executable permission");

    stage_git_file(workspace_path, "a-data.txt").expect("stage non-executable file");
    stage_git_file(workspace_path, "b-script.sh").expect("stage executable file");

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .args(["ls-files", "--stage"])
        .output()
        .expect("read index modes");
    assert!(
        output.status.success(),
        "git ls-files --stage failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let index_output = String::from_utf8(output.stdout).expect("utf-8 index output");
    let modes = index_output
        .lines()
        .map(|line| line.split_whitespace().next().expect("index mode"))
        .collect::<Vec<_>>();

    assert_eq!(modes, ["100644", "100755"]);
}

#[cfg(all(test, unix))]
#[test]
fn stage_git_file_commits_chmod_only_changes() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");

    fs::write(workspace_path.join("a-data.txt"), "data\n").expect("non-executable file");
    let executable_path = workspace_path.join("b-script.sh");
    fs::write(&executable_path, "#!/bin/sh\n").expect("executable file");
    stage_git_file(workspace_path, "a-data.txt").expect("stage non-executable file");
    stage_git_file(workspace_path, "b-script.sh").expect("stage executable file");
    commit_staged_changes(workspace_path, "initial commit".to_owned()).expect("initial commit");

    fs::set_permissions(&executable_path, fs::Permissions::from_mode(0o755))
        .expect("set executable permission");
    stage_git_file(workspace_path, "b-script.sh").expect("stage chmod-only change");

    let index_output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .args(["ls-files", "--stage"])
        .output()
        .expect("read index modes");
    assert!(
        index_output.status.success(),
        "git ls-files --stage failed: {}",
        String::from_utf8_lossy(&index_output.stderr)
    );
    let index_modes = String::from_utf8(index_output.stdout)
        .expect("utf-8 index output")
        .lines()
        .map(|line| {
            line.split_whitespace()
                .next()
                .expect("index mode")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(index_modes, vec!["100644".to_owned(), "100755".to_owned()]);

    let staged_diff = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .args(["diff", "--cached", "--raw", "--", "b-script.sh"])
        .output()
        .expect("read staged chmod-only diff");
    assert!(
        staged_diff.status.success(),
        "git diff --cached --raw failed: {}",
        String::from_utf8_lossy(&staged_diff.stderr)
    );
    let staged_diff = String::from_utf8(staged_diff.stdout).expect("utf-8 staged diff");
    let (header, path) = staged_diff
        .trim_end()
        .split_once('\t')
        .expect("raw diff header and path");
    let fields = header.split_whitespace().collect::<Vec<_>>();
    let [old_mode, new_mode, old_blob, new_blob, status] = fields.as_slice() else {
        panic!("unexpected raw diff header: {header}");
    };
    assert_eq!(
        (*old_mode, *new_mode, old_blob == new_blob, *status, path),
        (":100644", "100755", true, "M", "b-script.sh"),
        "chmod-only staging must change only the regular-file mode"
    );

    commit_staged_changes(workspace_path, "chmod-only commit".to_owned())
        .expect("commit chmod-only change");

    let tree_output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .args(["ls-tree", "-r", "HEAD"])
        .output()
        .expect("read committed tree modes");
    assert!(
        tree_output.status.success(),
        "git ls-tree -r HEAD failed: {}",
        String::from_utf8_lossy(&tree_output.stderr)
    );
    let tree_modes = String::from_utf8(tree_output.stdout)
        .expect("utf-8 tree output")
        .lines()
        .map(|line| {
            line.split_whitespace()
                .next()
                .expect("tree mode")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(tree_modes, vec!["100644".to_owned(), "100755".to_owned()]);

    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .args(["status", "--porcelain"])
        .output()
        .expect("read worktree status");
    assert!(
        status.status.success(),
        "git status --porcelain failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(
        status.stdout.is_empty(),
        "worktree should be clean after committing chmod-only change: {}",
        String::from_utf8_lossy(&status.stdout)
    );
}

#[cfg(test)]
#[test]
fn phase12_agent_worktrees_isolate_delete_and_merge_changes() {
    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");
    fs::write(
        workspace_path.join(".git").join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n\tlogallrefupdates = true\n[user]\n\tname = Foco Test\n\temail = foco@example.invalid\n",
    )
    .expect("test git config");
    fs::write(workspace_path.join("README.md"), "base\n").expect("base file");
    fs::write(workspace_path.join(".gitignore"), ".foco/\n").expect("ignore Foco internals");
    stage_git_file(workspace_path, "README.md").expect("stage base file");
    stage_git_file(workspace_path, ".gitignore").expect("stage ignore file");
    commit_staged_changes(workspace_path, "initial".to_string()).expect("initial commit");

    let first =
        create_agent_worktree(workspace_path, "agent-instance-test-a").expect("first worktree");
    let second =
        create_agent_worktree(workspace_path, "agent-instance-test-b").expect("second worktree");
    let branch_response = git_branches_response(workspace_path).expect("branch response");
    assert!(branch_response.branches.contains(&first.branch));
    assert!(branch_response.branches.contains(&second.branch));
    assert!(
        branch_response.worktrees.iter().any(
            |worktree| worktree.is_current && worktree.branch == branch_response.current_branch
        ),
        "main worktree should be marked current"
    );
    assert!(
        branch_response.worktrees.iter().any(|worktree| {
            worktree.path
                == path_to_slash_string(&first.root_path.canonicalize().expect("first path"))
                && worktree.branch.as_deref() == Some(first.branch.as_str())
                && !worktree.is_current
        }),
        "linked Agent worktree should be listed"
    );
    assert_ne!(first.root_path, second.root_path);
    fs::write(first.root_path.join("first.txt"), "first\n").expect("first change");
    fs::write(second.root_path.join("second.txt"), "second\n").expect("second change");

    let delete_error = delete_agent_worktree(workspace_path, &first.root_path, false)
        .expect_err("dirty worktree delete must fail");
    assert!(delete_error.message.contains("unmerged changes"));

    let merge = merge_agent_worktree(workspace_path, &first.root_path, &first.base_revision)
        .expect("merge first worktree");
    assert_eq!(merge.base_revision, first.base_revision);
    assert_eq!(merge.changed_paths, vec!["first.txt".to_string()]);
    assert!(merge.diff_id.starts_with("agent-diff-"));
    assert_eq!(
        fs::read_to_string(workspace_path.join("first.txt")).expect("merged first file"),
        "first\n"
    );
    let merge_error =
        merge_agent_worktree(workspace_path, &second.root_path, &second.base_revision)
            .expect_err("merge must reject dirty shared workspace");
    assert!(merge_error.message.contains("uncommitted changes"));
    assert!(
        validate_agent_worktree_path(workspace_path, workspace_path).is_err(),
        "managed deletion must reject paths outside Foco agent worktrees"
    );
}

#[cfg(test)]
#[test]
fn git_worktree_target_resolves_only_registered_worktrees() {
    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");
    fs::write(
        workspace_path.join(".git").join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n\tlogallrefupdates = true\n[user]\n\tname = Foco Test\n\temail = foco@example.invalid\n",
    )
    .expect("test git config");
    fs::write(workspace_path.join("README.md"), "base\n").expect("base file");
    stage_git_file(workspace_path, "README.md").expect("stage base file");
    commit_staged_changes(workspace_path, "initial".to_string()).expect("initial commit");

    let worktree =
        create_agent_worktree(workspace_path, "agent-instance-target-test").expect("worktree");
    assert_eq!(
        resolve_git_worktree_target(workspace_path, worktree.root_path.to_str().expect("path"))
            .expect("known worktree"),
        worktree
            .root_path
            .canonicalize()
            .expect("canonical worktree")
    );
    assert_eq!(
        resolve_git_worktree_target(workspace_path, ".").expect("main workspace"),
        workspace_path.canonicalize().expect("canonical workspace")
    );

    let unknown = workspace_path.join("unregistered-worktree");
    fs::create_dir(&unknown).expect("unknown dir");
    let error = resolve_git_worktree_target(workspace_path, unknown.to_str().expect("path"))
        .expect_err("unknown target must fail");
    assert!(error.message.contains("unknown git worktree target"));
}

#[cfg(test)]
#[test]
fn plan_worktree_fast_forward_merges_committed_phase_history() {
    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");
    fs::write(
        workspace_path.join(".git").join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n\tlogallrefupdates = true\n[user]\n\tname = Foco Test\n\temail = foco@example.invalid\n",
    )
    .expect("test git config");
    fs::write(workspace_path.join("README.md"), "base\n").expect("base file");
    fs::write(workspace_path.join(".gitignore"), ".foco/\n").expect("ignore Foco internals");
    stage_git_file(workspace_path, "README.md").expect("stage base file");
    stage_git_file(workspace_path, ".gitignore").expect("stage ignore file");
    commit_staged_changes(workspace_path, "initial".to_string()).expect("initial commit");

    let worktree =
        create_agent_worktree(workspace_path, "agent-instance-plan-test").expect("worktree");
    fs::write(worktree.root_path.join("phase-1.txt"), "phase 1\n").expect("phase 1 file");
    stage_git_file(&worktree.root_path, "phase-1.txt").expect("stage phase 1");
    let first_commit =
        commit_staged_changes(&worktree.root_path, "phase 1".to_string()).expect("phase 1 commit");
    fs::write(worktree.root_path.join("phase-2.txt"), "phase 2\n").expect("phase 2 file");
    stage_git_file(&worktree.root_path, "phase-2.txt").expect("stage phase 2");
    let second_commit =
        commit_staged_changes(&worktree.root_path, "phase 2".to_string()).expect("phase 2 commit");
    assert_ne!(first_commit, second_commit);
    let source_diff =
        agent_worktree_committed_diff(workspace_path, &worktree.root_path, &worktree.base_revision)
            .expect("committed diff");
    assert!(source_diff.contains("phase-1.txt"));
    assert!(source_diff.contains("phase-2.txt"));

    let merged = fast_forward_shared_workspace_to_agent_worktree(
        workspace_path,
        &worktree.root_path,
        &worktree.base_revision,
    )
    .expect("fast-forward merge");
    assert_eq!(merged.as_deref(), Some(second_commit.as_str()));
    assert_eq!(
        fs::read_to_string(workspace_path.join("phase-1.txt")).expect("phase 1 merged"),
        "phase 1\n"
    );
    assert_eq!(
        fs::read_to_string(workspace_path.join("phase-2.txt")).expect("phase 2 merged"),
        "phase 2\n"
    );
    assert!(
        git_diff_response(workspace_path, None)
            .expect("shared diff")
            .status
            .trim()
            .is_empty()
    );
    delete_agent_worktree(workspace_path, &worktree.root_path, false).expect("delete worktree");
    assert!(!worktree.root_path.exists());
}

#[cfg(test)]
#[test]
fn is_foco_reserved_repo_path_matches_runtime_tree() {
    assert!(is_foco_reserved_repo_path(".foco"));
    assert!(is_foco_reserved_repo_path(".foco/foco.sqlite"));
    assert!(is_foco_reserved_repo_path(".foco/foco.sqlite-wal"));
    assert!(is_foco_reserved_repo_path("./.foco/agent-worktrees/x"));
    assert!(!is_foco_reserved_repo_path("src/foco.rs"));
    assert!(!is_foco_reserved_repo_path("foco/config.toml"));
}

#[cfg(test)]
#[test]
fn untracked_foco_runtime_does_not_block_worktree_status_or_delete() {
    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");
    fs::write(
        workspace_path.join(".git").join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n\tlogallrefupdates = true\n[user]\n\tname = Foco Test\n\temail = foco@example.invalid\n",
    )
    .expect("test git config");
    // Intentionally no .gitignore for .foco — protection must not depend on it.
    fs::write(workspace_path.join("README.md"), "base\n").expect("base file");
    stage_git_file(workspace_path, "README.md").expect("stage base file");
    commit_staged_changes(workspace_path, "initial".to_string()).expect("initial commit");

    let worktree =
        create_agent_worktree(workspace_path, "agent-instance-foco-runtime").expect("worktree");
    fs::create_dir_all(worktree.root_path.join(".foco")).expect("runtime dir");
    fs::write(
        worktree.root_path.join(".foco").join("foco.sqlite"),
        "runtime-db",
    )
    .expect("runtime db");
    fs::write(
        worktree.root_path.join(".foco").join("foco.sqlite-wal"),
        "wal",
    )
    .expect("runtime wal");

    let status = git_status_response(&worktree.root_path).expect("status");
    assert!(
        status.files.is_empty(),
        "untracked .foco runtime must not appear in status: {:?}",
        status.files
    );
    delete_agent_worktree(workspace_path, &worktree.root_path, false)
        .expect("delete must ignore untracked .foco runtime");
    assert!(!worktree.root_path.exists());
}

#[cfg(test)]
#[test]
fn fast_forward_rejects_committed_foco_runtime_paths() {
    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");
    fs::write(
        workspace_path.join(".git").join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n\tlogallrefupdates = true\n[user]\n\tname = Foco Test\n\temail = foco@example.invalid\n",
    )
    .expect("test git config");
    fs::write(workspace_path.join("README.md"), "base\n").expect("base file");
    stage_git_file(workspace_path, "README.md").expect("stage base file");
    commit_staged_changes(workspace_path, "initial".to_string()).expect("initial commit");
    let shared_head_before = shared_workspace_head_commit_id(workspace_path).expect("shared head");

    let worktree =
        create_agent_worktree(workspace_path, "agent-instance-foco-commit").expect("worktree");
    fs::create_dir_all(worktree.root_path.join(".foco")).expect("runtime dir");
    fs::write(
        worktree.root_path.join(".foco").join("foco.sqlite"),
        "should-not-merge",
    )
    .expect("runtime db");
    // Force-commit reserved path the way a misbehaving agent shell command might
    // (`git add -f` / raw git). Foco stage/commit APIs intentionally hide `.foco`.
    let force_add = std::process::Command::new("git")
        .arg("-C")
        .arg(&worktree.root_path)
        .args(["add", "-f", ".foco/foco.sqlite"])
        .output()
        .expect("force-add reserved path");
    assert!(
        force_add.status.success(),
        "force-add failed: {}",
        String::from_utf8_lossy(&force_add.stderr)
    );
    let force_commit = std::process::Command::new("git")
        .arg("-C")
        .arg(&worktree.root_path)
        .args([
            "-c",
            "user.name=Foco Test",
            "-c",
            "user.email=foco@example.invalid",
            "commit",
            "-m",
            "bad runtime commit",
        ])
        .output()
        .expect("force-commit reserved path");
    assert!(
        force_commit.status.success(),
        "force-commit failed: {}",
        String::from_utf8_lossy(&force_commit.stderr)
    );

    let error = fast_forward_shared_workspace_to_agent_worktree(
        workspace_path,
        &worktree.root_path,
        &worktree.base_revision,
    )
    .expect_err("fast-forward must reject committed .foco");
    assert!(
        error
            .message
            .contains(AGENT_WORKTREE_FOCO_RUNTIME_COMMITTED_MESSAGE),
        "unexpected error: {}",
        error.message
    );
    assert_eq!(
        shared_workspace_head_commit_id(workspace_path).expect("shared head after reject"),
        shared_head_before,
        "shared HEAD must remain unchanged"
    );
    assert!(
        !workspace_path.join(".foco").join("foco.sqlite").exists(),
        "shared workspace must not receive worktree .foco runtime files"
    );
}

#[cfg(test)]
#[test]
fn committed_foco_is_rejected_before_shared_head_mismatch() {
    let workspace = tempfile::tempdir().expect("workspace");
    let workspace_path = workspace.path();
    let repo = gix::init(workspace_path).expect("init repository");
    let mut index = gix::index::File::from_state(
        gix::index::State::new(repo.object_hash()),
        repo.index_path(),
    );
    index.write(Default::default()).expect("empty index");
    fs::write(
        workspace_path.join(".git").join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n\tlogallrefupdates = true\n[user]\n\tname = Foco Test\n\temail = foco@example.invalid\n",
    )
    .expect("test git config");
    fs::write(workspace_path.join("README.md"), "base\n").expect("base file");
    stage_git_file(workspace_path, "README.md").expect("stage base file");
    commit_staged_changes(workspace_path, "initial".to_string()).expect("initial commit");

    let worktree = create_agent_worktree(workspace_path, "agent-instance-foco-before-mismatch")
        .expect("worktree");
    fs::create_dir_all(worktree.root_path.join(".foco")).expect("runtime dir");
    fs::write(
        worktree.root_path.join(".foco").join("foco.sqlite"),
        "should-not-merge",
    )
    .expect("runtime db");
    let force_add = std::process::Command::new("git")
        .arg("-C")
        .arg(&worktree.root_path)
        .args(["add", "-f", ".foco/foco.sqlite"])
        .output()
        .expect("force-add reserved path");
    assert!(
        force_add.status.success(),
        "force-add failed: {}",
        String::from_utf8_lossy(&force_add.stderr)
    );
    let force_commit = std::process::Command::new("git")
        .arg("-C")
        .arg(&worktree.root_path)
        .args([
            "-c",
            "user.name=Foco Test",
            "-c",
            "user.email=foco@example.invalid",
            "commit",
            "-m",
            "bad runtime commit",
        ])
        .output()
        .expect("force-commit reserved path");
    assert!(
        force_commit.status.success(),
        "force-commit failed: {}",
        String::from_utf8_lossy(&force_commit.stderr)
    );

    // Advance shared HEAD so a late foco check would be skipped by SharedHeadMismatch.
    fs::write(workspace_path.join("README.md"), "shared moved\n").expect("shared edit");
    stage_git_file(workspace_path, "README.md").expect("stage shared edit");
    commit_staged_changes(workspace_path, "shared advance".to_string()).expect("shared commit");
    let shared_head_before = shared_workspace_head_commit_id(workspace_path).expect("shared head");

    let ff_error = fast_forward_shared_workspace_to_agent_worktree(
        workspace_path,
        &worktree.root_path,
        &worktree.base_revision,
    )
    .expect_err("must reject committed .foco before head mismatch");
    assert!(
        ff_error
            .message
            .contains(AGENT_WORKTREE_FOCO_RUNTIME_COMMITTED_MESSAGE),
        "expected foco rejection, got: {}",
        ff_error.message
    );
    assert!(
        !ff_error
            .message
            .contains("does not match Agent worktree base"),
        "must not surface SharedHeadMismatch when .foco is committed: {}",
        ff_error.message
    );

    let diff_error =
        agent_worktree_committed_diff(workspace_path, &worktree.root_path, &worktree.base_revision)
            .expect_err("committed diff must reject .foco");
    assert!(
        diff_error
            .message
            .contains(AGENT_WORKTREE_FOCO_RUNTIME_COMMITTED_MESSAGE),
        "unexpected committed-diff error: {}",
        diff_error.message
    );

    assert_eq!(
        shared_workspace_head_commit_id(workspace_path).expect("shared head after reject"),
        shared_head_before,
        "shared HEAD must remain unchanged"
    );
}

fn remove_tracked_files_missing_from_target(
    repo: &gix::Repository,
    target_index: &gix::index::File,
) -> Result<(), ApiError> {
    let current_index = repo
        .index_or_load_from_head_or_empty()
        .map_err(|source| ApiError::internal(format!("failed to read git index: {source}")))?;

    for entry in current_index.entries() {
        let repo_path = entry.path(&current_index);
        if target_index.entry_by_path(repo_path).is_none() {
            let Some(path) = repo.workdir_path(repo_path) else {
                continue;
            };
            remove_worktree_path(&path)?;
        }
    }

    Ok(())
}

fn remove_worktree_path(path: &Path) -> Result<(), ApiError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(path).map_err(|source| {
            ApiError::internal(format!("failed to remove git worktree directory: {source}"))
        }),
        Ok(_) => std::fs::remove_file(path).map_err(|source| {
            ApiError::internal(format!("failed to remove git worktree file: {source}"))
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ApiError::internal(format!(
            "failed to inspect git worktree path: {source}"
        ))),
    }
}

fn checkout_index(repo: &gix::Repository, index: &mut gix::index::File) -> Result<(), ApiError> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| ApiError::bad_request("git repository does not have a worktree"))?;
    let options = repo
        .checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)
        .map_err(|source| {
            ApiError::internal(format!("failed to prepare git checkout: {source}"))
        })?;
    let files = gix::progress::Discard;
    let bytes = gix::progress::Discard;
    let should_interrupt = AtomicBool::new(false);
    let outcome = gix::worktree::state::checkout(
        index,
        workdir,
        repo.objects.clone().into_arc().map_err(|source| {
            ApiError::internal(format!("failed to prepare git object database: {source}"))
        })?,
        &files,
        &bytes,
        &should_interrupt,
        options,
    )
    .map_err(|source| ApiError::bad_request(format!("failed to checkout git branch: {source}")))?;

    if !outcome.collisions.is_empty() || !outcome.errors.is_empty() {
        return Err(ApiError::bad_request(format!(
            "failed to checkout git branch: {} collision(s), {} error(s)",
            outcome.collisions.len(),
            outcome.errors.len()
        )));
    }

    Ok(())
}

fn set_head_to_branch(
    repo: &gix::Repository,
    ref_name: &str,
    message: &str,
) -> Result<(), ApiError> {
    repo.edit_reference(RefEdit {
        name: "HEAD".try_into().map_err(|source| {
            ApiError::internal(format!("failed to prepare git HEAD update: {source}"))
        })?,
        deref: false,
        change: Change::Update {
            expected: PreviousValue::Any,
            new: Target::Symbolic(ref_name.try_into().map_err(|source| {
                ApiError::internal(format!("failed to prepare git branch target: {source}"))
            })?),
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: message.into(),
            },
        },
    })
    .map_err(|source| ApiError::internal(format!("failed to update git HEAD: {source}")))?;

    Ok(())
}

fn validate_git_branch_name(name: String) -> Result<String, ApiError> {
    let branch = name.trim();
    if branch.is_empty() {
        return Err(ApiError::bad_request("git branch name must not be empty"));
    }

    let ref_name = branch_ref_name(branch);
    gix::validate::reference::branch_name(ref_name.as_bytes().as_bstr()).map_err(|source| {
        ApiError::bad_request(format!("invalid git branch name '{branch}': {source}"))
    })?;

    Ok(branch.to_string())
}

fn branch_ref_name(branch: &str) -> String {
    format!("refs/heads/{branch}")
}

fn status_summary(entry: GitStatusEntry) -> GitStatusFileSummary {
    GitStatusFileSummary {
        path: entry.workspace_path,
        index_status: entry.index_status.to_string(),
        worktree_status: entry.worktree_status.to_string(),
    }
}

fn status_text(entries: &[GitStatusEntry]) -> String {
    let mut status = String::new();
    for entry in entries {
        status.push(entry.index_status);
        status.push(entry.worktree_status);
        status.push(' ');
        status.push_str(&entry.workspace_path);
        status.push('\n');
    }
    status
}

fn tree_index_status(change: &gix::diff::index::Change) -> char {
    match change {
        gix::diff::index::Change::Addition { .. } => 'A',
        gix::diff::index::Change::Deletion { .. } => 'D',
        gix::diff::index::Change::Modification { .. } => 'M',
        gix::diff::index::Change::Rewrite { .. } => 'R',
    }
}

fn worktree_status(summary: gix::status::index_worktree::iter::Summary) -> char {
    match summary {
        gix::status::index_worktree::iter::Summary::Removed => 'D',
        gix::status::index_worktree::iter::Summary::Added => '?',
        gix::status::index_worktree::iter::Summary::Modified => 'M',
        gix::status::index_worktree::iter::Summary::TypeChange => 'T',
        gix::status::index_worktree::iter::Summary::Renamed => 'R',
        gix::status::index_worktree::iter::Summary::Copied => 'C',
        gix::status::index_worktree::iter::Summary::IntentToAdd => 'A',
        gix::status::index_worktree::iter::Summary::Conflict => 'U',
    }
}

fn workspace_relative_path(
    workspace_root: &Path,
    worktree_root: &Path,
    repo_path: &str,
) -> Option<String> {
    let absolute = worktree_root.join(repo_path_to_path_buf(repo_path));
    let relative = absolute.strip_prefix(workspace_root).ok()?;
    Some(path_to_slash_string(relative))
}

fn bstr_to_path_string(path: &gix::bstr::BStr) -> String {
    path.to_str_lossy().replace('\\', "/")
}

fn repo_path_to_path_buf(repo_path: &str) -> PathBuf {
    repo_path.split('/').collect()
}

fn path_to_slash_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_includes_untracked_text_files() {
        let temp = tempfile::tempdir().expect("temp repo");
        gix::init(temp.path()).expect("init git repo");
        std::fs::write(temp.path().join("note.txt"), "new note\n").expect("write untracked file");

        let response =
            git_diff_response(temp.path(), Some("note.txt".to_string())).expect("read git diff");

        assert_eq!(response.files.len(), 1);
        assert_eq!(response.files[0].index_status, "?");
        assert_eq!(response.files[0].worktree_status, "?");
        assert!(response.diff.contains("diff --git a/note.txt b/note.txt"));
        assert!(response.diff.contains("--- /dev/null"));
        assert!(response.diff.contains("+++ b/note.txt"));
        assert!(response.diff.contains("+new note"));
    }

    #[test]
    fn diff_skips_untracked_directory_and_symlink_to_directory() {
        let temp = tempfile::tempdir().expect("temp repo");
        let workspace = temp.path();
        gix::init(workspace).expect("init git repo");
        // Trailing-slash ignore matches real directories but often not a symlink
        // that points at a directory — the production failure mode for plan phase
        // worktree commit / agent task worktree snapshot.
        fs::write(workspace.join(".gitignore"), "web/dist/\n").expect("gitignore");
        fs::write(workspace.join("changed.txt"), "work\n").expect("changed file");

        let plain_dir = workspace.join("plain-dir");
        fs::create_dir_all(&plain_dir).expect("plain directory");
        fs::write(plain_dir.join("nested.txt"), "nested\n").expect("nested file");

        let web = workspace.join("web");
        fs::create_dir_all(&web).expect("web dir");
        let dist_target = workspace.join("real-dist-target");
        fs::create_dir_all(&dist_target).expect("real dist dir");
        fs::write(dist_target.join("index.html"), "<html></html>\n").expect("dist file");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&dist_target, web.join("dist"))
                .expect("symlink web/dist -> directory");
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(web.join("dist")).expect("web/dist directory");
        }

        let response = git_diff_response(workspace, None)
            .expect("git diff must not fail on directory / symlink-to-directory noise");

        let paths = response
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert!(
            !paths
                .iter()
                .any(|path| *path == "web/dist" || *path == "plain-dir"),
            "directory / symlink-to-directory paths must not appear as file status: {paths:?}"
        );
        assert!(
            paths.iter().any(|path| *path == "changed.txt"),
            "expected changed.txt in files: {paths:?}"
        );
        assert!(
            response.diff.contains("changed.txt"),
            "diff should include real file changes"
        );
    }
}
