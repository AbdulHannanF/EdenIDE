//! `eden-workspace` — the project model and file enumeration.
//!
//! [`Project`] is a root directory plus gitignore-aware file walking (via the
//! `ignore` crate, the same engine ripgrep uses). The file list feeds the Cmd-P
//! fuzzy finder and the sidebar tree.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// An upper bound on enumerated files, to keep Cmd-P responsive in huge trees.
const MAX_FILES: usize = 50_000;

/// A project rooted at a directory.
#[derive(Clone, Debug)]
pub struct Project {
    root: PathBuf,
}

impl Project {
    /// Creates a project rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The project root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// All non-ignored files under the root, as paths relative to it.
    ///
    /// Honors `.gitignore`, `.ignore`, and global git excludes, and skips hidden
    /// files. Capped at 50 000 entries to keep Cmd-P responsive in huge trees.
    #[must_use]
    pub fn files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let walker = WalkBuilder::new(&self.root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .parents(true)
            .build();
        for entry in walker.flatten() {
            if entry.file_type().is_some_and(|t| t.is_file()) {
                if let Ok(rel) = entry.path().strip_prefix(&self.root) {
                    files.push(rel.to_path_buf());
                }
                if files.len() >= MAX_FILES {
                    break;
                }
            }
        }
        files.sort();
        files
    }

    /// The relative file paths as forward-slashed strings, for fuzzy matching
    /// and display.
    #[must_use]
    pub fn file_strings(&self) -> Vec<String> {
        self.files()
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect()
    }
}

/// One row in the flattened, visible file tree.
#[derive(Clone, Debug)]
pub struct TreeEntry {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Display name (file or directory name).
    pub name: String,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether an expanded directory.
    pub expanded: bool,
    /// Nesting depth from the root (root children are depth 0).
    pub depth: usize,
}

/// A lazily-expanded, gitignore-aware file tree.
///
/// Only expanded directories have their children materialised, so opening a
/// huge repo doesn't walk everything up front. The entries are kept as a flat,
/// depth-tagged list — the order they render in.
pub struct FileTree {
    root: PathBuf,
    entries: Vec<TreeEntry>,
}

impl FileTree {
    /// Builds a tree rooted at `root`, with the top level loaded.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let entries = read_dir_entries(&root, 0);
        Self { root, entries }
    }

    /// The project root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The flattened visible rows, top to bottom.
    #[must_use]
    pub fn entries(&self) -> &[TreeEntry] {
        &self.entries
    }

    /// Toggles a directory row open/closed. No-op for files or bad indices.
    pub fn toggle(&mut self, index: usize) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        if !entry.is_dir {
            return;
        }
        if entry.expanded {
            self.collapse(index);
        } else {
            self.expand(index);
        }
    }

    fn expand(&mut self, index: usize) {
        let (path, depth) = {
            let entry = &self.entries[index];
            (entry.path.clone(), entry.depth)
        };
        let children = read_dir_entries(&path, depth + 1);
        self.entries[index].expanded = true;
        self.entries.splice(index + 1..index + 1, children);
    }

    fn collapse(&mut self, index: usize) {
        let depth = self.entries[index].depth;
        let mut end = index + 1;
        while end < self.entries.len() && self.entries[end].depth > depth {
            end += 1;
        }
        self.entries.drain(index + 1..end);
        self.entries[index].expanded = false;
    }
}

/// Reads the immediate children of `dir` (gitignore-aware), directories first,
/// then alphabetically.
fn read_dir_entries(dir: &Path, depth: usize) -> Vec<TreeEntry> {
    let mut entries: Vec<TreeEntry> = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .parents(true)
        .build()
        .flatten()
        .filter(|e| e.path() != dir)
        .map(|e| {
            let path = e.path().to_path_buf();
            let is_dir = e.file_type().is_some_and(|t| t.is_dir());
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            TreeEntry { path, name, is_dir, expanded: false, depth }
        })
        .collect();
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_own_crate_files() {
        let project = Project::new(env!("CARGO_MANIFEST_DIR"));
        let files = project.file_strings();
        assert!(files.iter().any(|f| f == "Cargo.toml"), "Cargo.toml missing");
        assert!(files.iter().any(|f| f == "src/lib.rs"), "src/lib.rs missing");
    }

    #[test]
    fn tree_expands_and_collapses() {
        let mut tree = FileTree::new(env!("CARGO_MANIFEST_DIR"));
        let src = tree
            .entries()
            .iter()
            .position(|e| e.is_dir && e.name == "src")
            .expect("src directory at top level");
        let before = tree.entries().len();
        tree.toggle(src);
        assert!(tree.entries().len() > before, "expand added children");
        assert!(tree.entries()[src].expanded);
        // A child sits right after, one level deeper.
        assert_eq!(tree.entries()[src + 1].depth, tree.entries()[src].depth + 1);
        tree.toggle(src);
        assert_eq!(tree.entries().len(), before, "collapse removed children");
        assert!(!tree.entries()[src].expanded);
    }
}
