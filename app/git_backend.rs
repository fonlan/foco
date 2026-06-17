use std::{
    collections::BTreeMap,
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

use super::{
    ApiError, GitBranchesResponse, GitDiffResponse, GitStatusFileSummary, GitStatusResponse,
};

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
    })
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
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ApiError::internal(format!(
            "failed to read git worktree file: {source}"
        ))),
    }
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
}
