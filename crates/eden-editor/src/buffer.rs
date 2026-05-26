//! The text buffer: a thin, intention-revealing wrapper over a ropey [`Rope`].
//!
//! A rope handles multi-megabyte files without copying on every edit, and
//! cloning one is O(1) (structural sharing) — which is what makes the
//! snapshot-based undo in [`crate::Editor`] cheap.

use std::ops::Range;

use ropey::Rope;

/// A text buffer backed by a rope.
#[derive(Clone, Debug, Default)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    /// An empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    /// A buffer initialised with `text`.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    /// The underlying rope, for read access (e.g. rendering).
    #[must_use]
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// Replaces the rope wholesale (used when restoring an undo snapshot).
    pub(crate) fn set_rope(&mut self, rope: Rope) {
        self.rope = rope;
    }

    /// Total number of chars (Unicode scalar values).
    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Number of lines. A trailing newline produces a final empty line, matching
    /// ropey's convention.
    #[must_use]
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// The line index containing char `char_idx`.
    #[must_use]
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx.min(self.len_chars()))
    }

    /// The char index at the start of `line`.
    #[must_use]
    pub fn line_to_char(&self, line: usize) -> usize {
        self.rope.line_to_char(line.min(self.len_lines().saturating_sub(1)))
    }

    /// `(line, column)` for a char index, where column is chars from line start.
    #[must_use]
    pub fn line_col(&self, char_idx: usize) -> (usize, usize) {
        let char_idx = char_idx.min(self.len_chars());
        let line = self.rope.char_to_line(char_idx);
        (line, char_idx - self.rope.line_to_char(line))
    }

    /// The number of visible chars on `line`, excluding the trailing line break.
    #[must_use]
    pub fn line_len(&self, line: usize) -> usize {
        if line >= self.len_lines() {
            return 0;
        }
        let slice = self.rope.line(line);
        let mut n = slice.len_chars();
        if n > 0 && slice.char(n - 1) == '\n' {
            n -= 1;
            if n > 0 && slice.char(n - 1) == '\r' {
                n -= 1;
            }
        }
        n
    }

    /// The char index of the end of `line`'s text (before any line break).
    #[must_use]
    pub fn line_end(&self, line: usize) -> usize {
        self.line_to_char(line) + self.line_len(line)
    }

    /// Collects `range` into a `String`.
    #[must_use]
    pub fn slice_to_string(&self, range: Range<usize>) -> String {
        self.rope.slice(range).to_string()
    }

    /// Inserts `text` at char index `at`.
    pub fn insert(&mut self, at: usize, text: &str) {
        self.rope.insert(at.min(self.len_chars()), text);
    }

    /// Removes the chars in `range`.
    pub fn remove(&mut self, range: Range<usize>) {
        let end = range.end.min(self.len_chars());
        let start = range.start.min(end);
        if start < end {
            self.rope.remove(start..end);
        }
    }
}

impl std::fmt::Display for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.rope)
    }
}
