//! Eden-domain types that the rest of the app sees from the LSP layer.
//!
//! These are intentionally thin wrappers over the LSP wire format so the UI
//! has no dependency on `serde_json` or any LSP crate.

/// A zero-indexed position in a document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    /// Zero-indexed line number.
    pub line: u32,
    /// Zero-indexed UTF-16 code-unit offset (LSP convention).
    pub character: u32,
}

/// Diagnostic severity, matching the LSP `DiagnosticSeverity` enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// A hard error.
    Error,
    /// A compiler or linter warning.
    Warning,
    /// Informational note.
    Information,
    /// A hint or suggestion.
    Hint,
}

/// A diagnostic annotation on a source range.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// Start of the affected range.
    pub start: Position,
    /// End of the affected range (exclusive).
    pub end: Position,
    /// Severity of the diagnostic.
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
}

/// A hover tooltip produced by the language server.
#[derive(Clone, Debug)]
pub struct HoverCard {
    /// Markdown or plain-text content. Rendered as plain text in Phase 4;
    /// a proper Markdown renderer is a Phase 6 follow-up.
    pub contents: String,
}

/// A single completion candidate.
#[derive(Clone, Debug)]
pub struct CompletionItem {
    /// Display label shown in the popup list.
    pub label: String,
    /// Text to insert on commit (falls back to `label` when absent).
    pub insert_text: String,
    /// Optional detail line (type signature, module path, …).
    pub detail: Option<String>,
}
