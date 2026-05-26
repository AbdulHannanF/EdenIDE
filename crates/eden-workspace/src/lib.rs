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
    /// files. Capped at [`MAX_FILES`].
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
}
