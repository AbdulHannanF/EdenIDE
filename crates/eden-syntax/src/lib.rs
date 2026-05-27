//! `eden-syntax` — tree-sitter syntax highlighting.
//!
//! [`Highlighter`] wraps tree-sitter + `tree-sitter-highlight` to turn source
//! text into a flat, sorted list of coloured [`Span`]s (byte ranges tagged with
//! a [`HighlightKind`]). The UI maps each kind to a theme colour. Only the kinds
//! Eden actually styles are recognised; everything else falls through to the
//! default text colour, so the output is compact.

use std::fmt;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter as TsHighlighter};

/// A syntactic category that a theme can colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightKind {
    /// Language keywords (`fn`, `let`, `match`, …).
    Keyword,
    /// Function and method names.
    Function,
    /// Types, traits, and type-like constructors.
    Type,
    /// Struct/enum constructors.
    Constructor,
    /// Field / property accesses.
    Property,
    /// Variables and parameters.
    Variable,
    /// Constants and builtins.
    Constant,
    /// String literals.
    String,
    /// Escape sequences inside strings.
    Escape,
    /// Comments.
    Comment,
    /// Operators.
    Operator,
    /// Brackets, delimiters, and other punctuation.
    Punctuation,
    /// Attributes / annotations (`#[...]`).
    Attribute,
    /// Loop/label identifiers.
    Label,
    /// Unhighlighted text (plain foreground).
    Default,
}

/// The recognised capture names, most-specific first, each mapped to a kind.
/// Order matters: `tree-sitter-highlight` resolves a capture to the best
/// matching recognised name.
const RECOGNIZED: &[(&str, HighlightKind)] = &[
    ("attribute", HighlightKind::Attribute),
    ("comment", HighlightKind::Comment),
    ("constant.builtin", HighlightKind::Constant),
    ("constant", HighlightKind::Constant),
    ("constructor", HighlightKind::Constructor),
    ("escape", HighlightKind::Escape),
    ("function.macro", HighlightKind::Function),
    ("function.method", HighlightKind::Function),
    ("function", HighlightKind::Function),
    ("keyword", HighlightKind::Keyword),
    ("label", HighlightKind::Label),
    ("operator", HighlightKind::Operator),
    ("property", HighlightKind::Property),
    ("punctuation.bracket", HighlightKind::Punctuation),
    ("punctuation.delimiter", HighlightKind::Punctuation),
    ("punctuation", HighlightKind::Punctuation),
    ("string", HighlightKind::String),
    ("type.builtin", HighlightKind::Type),
    ("type", HighlightKind::Type),
    ("variable.parameter", HighlightKind::Variable),
    ("variable.builtin", HighlightKind::Variable),
    ("variable", HighlightKind::Variable),
];

/// A coloured byte range, `start..end`, in the source text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// Inclusive start byte.
    pub start: usize,
    /// Exclusive end byte.
    pub end: usize,
    /// The highlight category.
    pub kind: HighlightKind,
}

/// Error building or running a [`Highlighter`].
#[derive(Debug)]
pub struct SyntaxError(String);

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SyntaxError {}

/// A reusable highlighter for a single language.
pub struct Highlighter {
    inner: TsHighlighter,
    config: HighlightConfiguration,
    kinds: Vec<HighlightKind>,
}

impl Highlighter {
    /// Dispatches to the appropriate language constructor by name.
    ///
    /// Returns `None` for unrecognised language IDs (the caller should fall
    /// back to plain-text rendering with no highlight spans).
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the grammar query fails to compile.
    pub fn for_language(lang: &str) -> Option<Result<Self, SyntaxError>> {
        match lang {
            "rust" => Some(Self::rust()),
            "javascript" | "js" => Some(Self::javascript()),
            "typescript" | "ts" => Some(Self::typescript()),
            "tsx" => Some(Self::tsx()),
            "python" | "py" => Some(Self::python()),
            "go" => Some(Self::go()),
            "c" => Some(Self::c()),
            "json" => Some(Self::json()),
            "bash" | "sh" => Some(Self::bash()),
            "html" => Some(Self::html()),
            "css" => Some(Self::css()),
            _ => None,
        }
    }

    /// Builds a highlighter for Rust.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn rust() -> Result<Self, SyntaxError> {
        Self::build(tree_sitter_rust::LANGUAGE.into(), "rust", tree_sitter_rust::HIGHLIGHTS_QUERY)
    }

    /// Builds a highlighter for JavaScript.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn javascript() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
        )
    }

    /// Builds a highlighter for TypeScript.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn typescript() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        )
    }

    /// Builds a highlighter for TSX (TypeScript + JSX).
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn tsx() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tsx",
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        )
    }

    /// Builds a highlighter for Python.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn python() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
        )
    }

    /// Builds a highlighter for Go.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn go() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_go::LANGUAGE.into(),
            "go",
            tree_sitter_go::HIGHLIGHTS_QUERY,
        )
    }

    /// Builds a highlighter for C.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn c() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_c::LANGUAGE.into(),
            "c",
            tree_sitter_c::HIGHLIGHT_QUERY,
        )
    }

    /// Builds a highlighter for JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn json() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
        )
    }

    /// Builds a highlighter for Bash.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn bash() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
        )
    }

    /// Builds a highlighter for HTML.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn html() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_html::LANGUAGE.into(),
            "html",
            tree_sitter_html::HIGHLIGHTS_QUERY,
        )
    }

    /// Builds a highlighter for CSS.
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] if the bundled highlight query fails to compile.
    pub fn css() -> Result<Self, SyntaxError> {
        Self::build(
            tree_sitter_css::LANGUAGE.into(),
            "css",
            tree_sitter_css::HIGHLIGHTS_QUERY,
        )
    }

    fn build(
        language: tree_sitter::Language,
        name: &str,
        highlights_query: &str,
    ) -> Result<Self, SyntaxError> {
        let mut config = HighlightConfiguration::new(language, name, highlights_query, "", "")
            .map_err(|e| SyntaxError(e.to_string()))?;
        let names: Vec<&str> = RECOGNIZED.iter().map(|(n, _)| *n).collect();
        config.configure(&names);
        Ok(Self {
            inner: TsHighlighter::new(),
            config,
            kinds: RECOGNIZED.iter().map(|(_, kind)| *kind).collect(),
        })
    }

    /// Produces a flat, sorted, non-overlapping list of highlight spans for
    /// `source`. Ranges that map to no recognised kind are omitted (the caller
    /// paints them with the default foreground).
    pub fn highlight(&mut self, source: &str) -> Vec<Span> {
        let mut spans = Vec::new();
        let mut stack: Vec<HighlightKind> = Vec::new();
        let events = match self
            .inner
            .highlight(&self.config, source.as_bytes(), None, |_| None)
        {
            Ok(events) => events,
            Err(_) => return spans,
        };
        for event in events {
            match event {
                Ok(HighlightEvent::HighlightStart(h)) => {
                    stack.push(self.kinds.get(h.0).copied().unwrap_or(HighlightKind::Default));
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    stack.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    let kind = stack.last().copied().unwrap_or(HighlightKind::Default);
                    if end > start && kind != HighlightKind::Default {
                        if let Some(last) = spans.last_mut()
                            && last.kind == kind
                            && last.end == start
                        {
                            last.end = end;
                            continue;
                        }
                        spans.push(Span { start, end, kind });
                    }
                }
                Err(_) => break,
            }
        }
        spans
    }
}

/// A queryable set of highlight spans.
#[derive(Clone, Debug, Default)]
pub struct Highlights {
    spans: Vec<Span>,
}

impl Highlights {
    /// Wraps a sorted, non-overlapping span list.
    #[must_use]
    pub fn new(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    /// The highlight kind covering `byte`, or [`HighlightKind::Default`].
    #[must_use]
    pub fn kind_at(&self, byte: usize) -> HighlightKind {
        let idx = self.spans.partition_point(|s| s.end <= byte);
        match self.spans.get(idx) {
            Some(span) if span.start <= byte => span.kind,
            _ => HighlightKind::Default,
        }
    }

    /// The number of spans.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// Whether there are no spans.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_keywords_and_strings() {
        let mut hl = Highlighter::rust().expect("build rust highlighter");
        let src = "fn main() {\n    let s = \"hi\";\n}\n";
        let spans = hl.highlight(src);
        assert!(!spans.is_empty(), "expected some highlight spans");

        let highlights = Highlights::new(spans);
        // `fn` is a keyword.
        assert_eq!(highlights.kind_at(0), HighlightKind::Keyword);
        // The string literal "hi" lies after the `=`.
        let string_byte = src.find('"').unwrap() + 1;
        assert_eq!(highlights.kind_at(string_byte), HighlightKind::String);
    }

    #[test]
    fn kind_at_returns_default_outside_spans() {
        let mut hl = Highlighter::rust().expect("build rust highlighter");
        let spans = hl.highlight("fn f() {}");
        let highlights = Highlights::new(spans);
        // A space between tokens has no highlight.
        assert_eq!(highlights.kind_at(2), HighlightKind::Default);
    }
}
