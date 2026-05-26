//! `eden-vcs` — Git integration: status, blame, diff models, branches.
//!
//! [`GitRepo`] wraps a `git2::Repository` and exposes a non-blocking,
//! snapshot-style API: `file_statuses()`, `diff_hunks()`, and `blame_line()`.
//! All returned values are owned, so callers never hold a borrow into git2
//! internals across frames.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

// ── public types ──────────────────────────────────────────────────────────────

/// Combined VCS state for a single file.
#[derive(Clone, Debug, Default)]
pub struct VcsState {
    /// File has staged changes.
    pub staged: bool,
    /// File has unstaged changes in the working tree.
    pub unstaged: bool,
    /// File is new and untracked.
    pub untracked: bool,
    /// File has unresolved merge conflicts.
    pub conflicted: bool,
}

/// One file's path and VCS state.
#[derive(Clone, Debug)]
pub struct FileStatus {
    /// Path relative to the repo root.
    pub path: PathBuf,
    /// The file's combined VCS state.
    pub state: VcsState,
}

/// What kind of change a diff hunk represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    /// Lines were added.
    Added,
    /// Lines were modified.
    Modified,
    /// Lines were deleted (marker sits at the line *before* the deletion).
    Deleted,
}

/// A contiguous run of changed lines in the working tree.
#[derive(Clone, Debug)]
pub struct DiffHunk {
    /// Zero-indexed first line of the hunk in the working tree.
    pub start_line: u32,
    /// Zero-indexed last line (inclusive) of the hunk.
    pub end_line: u32,
    /// What kind of change this hunk is.
    pub kind: DiffKind,
}

/// Blame information for a single line.
#[derive(Clone, Debug)]
pub struct BlameEntry {
    /// Abbreviated commit hash (7 hex chars).
    pub commit_short: String,
    /// Author name.
    pub author: String,
    /// Commit summary (first line of the message).
    pub summary: String,
}

// ── GitRepo ───────────────────────────────────────────────────────────────────

/// A handle to a discovered git repository.
pub struct GitRepo {
    repo: git2::Repository,
}

impl std::fmt::Debug for GitRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitRepo").field("root", &self.root()).finish()
    }
}

impl GitRepo {
    /// Discovers and opens the repository containing `path` (walks up the tree
    /// just like git does).
    ///
    /// # Errors
    ///
    /// Returns an error if no repository is found.
    pub fn discover(path: &Path) -> Result<Self> {
        let repo =
            git2::Repository::discover(path).with_context(|| format!("no git repo at {path:?}"))?;
        Ok(Self { repo })
    }

    /// The repository root (the `.git` parent directory).
    #[must_use]
    pub fn root(&self) -> &Path {
        self.repo.workdir().unwrap_or_else(|| self.repo.path())
    }

    /// The current branch name, or a short commit hash when HEAD is detached.
    /// Returns `None` only if the repository has no commits.
    #[must_use]
    pub fn branch_name(&self) -> Option<String> {
        let head = self.repo.head().ok()?;
        if head.is_branch() {
            head.shorthand().map(|s| s.to_owned())
        } else {
            head.target().map(|oid| oid.to_string()[..7].to_owned())
        }
    }

    // ── status ─────────────────────────────────────────────────────────────

    /// Returns VCS status for all modified/untracked files in the working tree.
    pub fn file_statuses(&self) -> Result<Vec<FileStatus>> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);
        let statuses = self.repo.statuses(Some(&mut opts)).context("git statuses")?;
        let out = statuses
            .iter()
            .filter_map(|entry| {
                let path = PathBuf::from(entry.path()?);
                let flags = entry.status();
                let state = VcsState {
                    staged: flags.intersects(
                        git2::Status::INDEX_NEW
                            | git2::Status::INDEX_MODIFIED
                            | git2::Status::INDEX_DELETED
                            | git2::Status::INDEX_RENAMED,
                    ),
                    unstaged: flags.intersects(
                        git2::Status::WT_MODIFIED
                            | git2::Status::WT_DELETED
                            | git2::Status::WT_RENAMED,
                    ),
                    untracked: flags.contains(git2::Status::WT_NEW),
                    conflicted: flags.contains(git2::Status::CONFLICTED),
                };
                Some(FileStatus { path, state })
            })
            .collect();
        Ok(out)
    }

    // ── diff hunks ─────────────────────────────────────────────────────────

    /// Returns diff hunks for `path` relative to HEAD.
    ///
    /// Compares the working-tree file against the HEAD commit tree so the
    /// gutter can show added / modified / deleted markers.
    pub fn diff_hunks(&self, path: &Path) -> Result<Vec<DiffHunk>> {
        let rel = path
            .strip_prefix(self.root())
            .unwrap_or(path)
            .to_str()
            .unwrap_or_default()
            .replace('\\', "/");

        let head = match self.repo.head() {
            Ok(h) => h,
            Err(_) => return Ok(vec![]),
        };
        let tree = head.peel_to_tree().context("HEAD tree")?;

        let mut opts = git2::DiffOptions::new();
        opts.context_lines(0).pathspec(&rel);

        let diff = self
            .repo
            .diff_tree_to_workdir(Some(&tree), Some(&mut opts))
            .context("diff_tree_to_workdir")?;

        let mut hunks: Vec<DiffHunk> = Vec::new();
        diff.foreach(
            &mut |_, _| true,
            None,
            Some(&mut |_delta, hunk| {
                let old_lines = hunk.old_lines();
                let new_lines = hunk.new_lines();
                let new_start = hunk.new_start().saturating_sub(1);
                let kind = if old_lines == 0 {
                    DiffKind::Added
                } else if new_lines == 0 {
                    DiffKind::Deleted
                } else {
                    DiffKind::Modified
                };
                let end_line = if kind == DiffKind::Deleted {
                    new_start
                } else {
                    new_start + new_lines.saturating_sub(1)
                };
                hunks.push(DiffHunk { start_line: new_start, end_line, kind });
                true
            }),
            None,
        )
        .context("diff foreach")?;

        Ok(hunks)
    }

    // ── blame ──────────────────────────────────────────────────────────────

    /// Returns blame information for a specific 0-indexed line in `path`.
    pub fn blame_line(&self, path: &Path, line: u32) -> Result<Option<BlameEntry>> {
        let blame = self
            .repo
            .blame_file(path, None)
            .with_context(|| format!("blame {path:?}"))?;
        let hunk = blame
            .get_line(line as usize + 1)
            .ok_or_else(|| anyhow::anyhow!("line {line} out of blame range"))?;
        let commit = self.repo.find_commit(hunk.final_commit_id()).ok();
        let (commit_short, author, summary) = commit
            .map(|c| {
                let short = format!("{:.7}", c.id());
                let author =
                    c.author().name().unwrap_or("?").to_owned();
                let summary = c.summary().unwrap_or("").to_owned();
                (short, author, summary)
            })
            .unwrap_or_else(|| ("0000000".into(), "?".into(), String::new()));
        Ok(Some(BlameEntry { commit_short, author, summary }))
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_own_repo() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo = GitRepo::discover(&manifest);
        assert!(repo.is_ok(), "should find eden repo: {repo:?}");
    }

    #[test]
    fn file_statuses_returns_vec() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if let Ok(repo) = GitRepo::discover(&manifest) {
            let statuses = repo.file_statuses();
            assert!(statuses.is_ok());
        }
    }
}
