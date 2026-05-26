//! `eden-lsp` — Language Server Protocol client pool.
//!
//! [`LspPool`] manages language-server child processes (one per language) and
//! exposes a non-blocking API safe to call from the render thread. Results
//! (diagnostics, hover cards, completions) arrive asynchronously and are
//! cached in shared state; the render thread simply reads whatever is current
//! on each frame.
//!
//! **Phase 4 status**: Rust (`rust-analyzer`) is the only wired language.
//! Additional servers follow the exact same pattern once Phase 3's syntax
//! highlighter is extended to more grammars.

mod client;
mod rpc;
mod types;

pub use client::LspClient;
pub use types::{CompletionItem, DefinitionResult, Diagnostic, HoverCard, Position, Severity};

use std::path::Path;
use std::sync::OnceLock;

/// Derives a `file://` URI from an absolute path. Handles Windows drive
/// letters and percent-encodes spaces — good enough for LSP.
pub fn path_to_uri(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let forward_slash = raw.replace('\\', "/");
    // Percent-encode characters that are invalid in a URI path.
    let encoded: String = forward_slash
        .chars()
        .flat_map(|c| match c {
            ' ' => vec!['%', '2', '0'],
            '%' => vec!['%', '2', '5'],
            _ => vec![c],
        })
        .collect();
    // On Windows `C:/...` → prepend an extra slash so it's `file:///C:/...`.
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

/// Returns the LSP language identifier for a file, or `None` if unknown.
pub fn language_id(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "py" => Some("python"),
        "go" => Some("go"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("cpp"),
        "json" => Some("json"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "md" => Some("markdown"),
        "html" => Some("html"),
        "css" => Some("css"),
        _ => None,
    }
}

/// Manages language-server child processes, one per language.
pub struct LspPool {
    runtime: tokio::runtime::Runtime,
    rust: OnceLock<Option<LspClient>>,
    root_uri: String,
}

impl LspPool {
    /// Creates the pool for the given workspace root.
    ///
    /// Language servers are spawned lazily the first time a file of that
    /// language is opened.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        // Tokio runtime creation can only fail if OS resource limits are hit
        // (e.g. too many threads). We log and fall back to a current-thread
        // runtime, which can never fail, to avoid a panic in library code.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("eden-lsp")
            .build()
            .unwrap_or_else(|err| {
                tracing::error!("failed to build multi-thread LSP runtime ({err:#}); falling back to current-thread");
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .thread_name("eden-lsp")
                    .build()
                    // current_thread build is infallible in practice
                    .unwrap_or_else(|e| panic!("cannot create any tokio runtime: {e}"))
            });
        Self {
            runtime,
            rust: OnceLock::new(),
            root_uri: path_to_uri(root),
        }
    }

    // ── document lifecycle ─────────────────────────────────────────────────

    /// Notifies the relevant language server that `path` was opened with `text`.
    pub fn open_document(&self, path: &Path, text: &str) {
        let Some(lang) = language_id(path) else { return };
        let Some(client) = self.client_for(lang) else { return };
        client.open_document(&path_to_uri(path), lang, text);
    }

    /// Notifies the relevant language server that `path` changed to `text`.
    pub fn change_document(&self, path: &Path, version: i32, text: &str) {
        let Some(lang) = language_id(path) else { return };
        let Some(client) = self.client_for(lang) else { return };
        client.change_document(&path_to_uri(path), version, text);
    }

    // ── queries ────────────────────────────────────────────────────────────

    /// The current diagnostics for `path`. Returns an empty vec if the server
    /// isn't running or hasn't published diagnostics yet.
    #[must_use]
    pub fn diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let Some(lang) = language_id(path) else { return Vec::new() };
        let Some(client) = self.client_for(lang) else { return Vec::new() };
        client.diagnostics(&path_to_uri(path))
    }

    /// Fires a hover request for the given position. Non-blocking; the result
    /// becomes available via [`hover`][Self::hover] on a future frame.
    pub fn request_hover(&self, path: &Path, pos: Position) {
        let Some(lang) = language_id(path) else { return };
        let Some(client) = self.client_for(lang) else { return };
        client.request_hover(&path_to_uri(path), pos);
    }

    /// The most recently received hover card, if any.
    #[must_use]
    pub fn hover(&self, path: &Path) -> Option<HoverCard> {
        let lang = language_id(path)?;
        self.client_for(lang)?.hover()
    }

    /// Discards the cached hover card (call when the cursor moves).
    pub fn clear_hover(&self, path: &Path) {
        let Some(lang) = language_id(path) else { return };
        if let Some(client) = self.client_for(lang) {
            client.clear_hover();
        }
    }

    /// Fires a completion request. Results available via
    /// [`completions`][Self::completions] on a future frame.
    pub fn request_completions(&self, path: &Path, pos: Position) {
        let Some(lang) = language_id(path) else { return };
        let Some(client) = self.client_for(lang) else { return };
        client.request_completions(&path_to_uri(path), pos);
    }

    /// The most recently received completion items.
    #[must_use]
    pub fn completions(&self, path: &Path) -> Vec<CompletionItem> {
        let Some(lang) = language_id(path) else { return Vec::new() };
        self.client_for(lang)
            .map(LspClient::completions)
            .unwrap_or_default()
    }

    // ── go-to-definition ──────────────────────────────────────────────────

    /// Fires a `textDocument/definition` request. The result is available via
    /// [`definition_result`][Self::definition_result] on a subsequent frame.
    pub fn request_definition(&self, path: &Path, pos: Position) {
        let Some(lang) = language_id(path) else { return };
        let Some(client) = self.client_for(lang) else { return };
        let id = client.request_definition(&path_to_uri(path), pos);
        tracing::debug!(id, "go-to-definition requested");
    }

    /// The most recently received definition result, if any.
    #[must_use]
    pub fn definition_result(&self, path: &Path) -> Option<DefinitionResult> {
        let lang = language_id(path)?;
        self.client_for(lang)?.definition_result()
    }

    /// Discards the cached definition result.
    pub fn clear_definition(&self, path: &Path) {
        let Some(lang) = language_id(path) else { return };
        if let Some(client) = self.client_for(lang) {
            client.clear_definition();
        }
    }

    // ── internals ─────────────────────────────────────────────────────────

    fn client_for(&self, lang: &str) -> Option<&LspClient> {
        match lang {
            "rust" => self
                .rust
                .get_or_init(|| {
                    let handle = self.runtime.handle();
                    match LspClient::spawn(handle, "rust-analyzer", &[], &self.root_uri) {
                        Ok(c) => {
                            tracing::info!("spawned rust-analyzer");
                            Some(c)
                        }
                        Err(err) => {
                            tracing::warn!("rust-analyzer not available: {err:#}");
                            None
                        }
                    }
                })
                .as_ref(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_uri_windows() {
        let path = Path::new(r"C:\Users\test\foo bar.rs");
        let uri = path_to_uri(path);
        assert!(uri.starts_with("file:///C:/"), "got: {uri}");
        assert!(uri.contains("%20"), "spaces not encoded: {uri}");
    }

    #[test]
    fn language_id_rust() {
        assert_eq!(language_id(Path::new("src/main.rs")), Some("rust"));
        assert_eq!(language_id(Path::new("foo.txt")), None);
    }
}
