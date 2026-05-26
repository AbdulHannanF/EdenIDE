//! Cursors and selections.
//!
//! A [`Selection`] is an `anchor` (the fixed end, where selection began) and a
//! `head` (the moving end, where the caret is). A caret is just an empty
//! selection. All positions are char indices into the buffer's rope (Unicode
//! scalar values), which is the unit ropey edits in.

use std::ops::Range;

/// A single selection / caret, as a pair of char indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The fixed end of the selection.
    pub anchor: usize,
    /// The moving end — where the caret is drawn.
    pub head: usize,
}

impl Selection {
    /// A zero-width caret at `at`.
    #[must_use]
    pub fn caret(at: usize) -> Self {
        Self { anchor: at, head: at }
    }

    /// A selection spanning `anchor..head` (in either order).
    #[must_use]
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    /// The lower bound of the selection.
    #[must_use]
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// The upper bound of the selection.
    #[must_use]
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    /// Whether the selection is an empty caret.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The selected char range, `start..end`.
    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.start()..self.end()
    }

    /// Clamps both ends into `0..=max`.
    #[must_use]
    pub fn clamped(self, max: usize) -> Self {
        Self {
            anchor: self.anchor.min(max),
            head: self.head.min(max),
        }
    }
}
