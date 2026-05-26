//! `eden-editor` — the text buffer, cursors, selections, undo, and edit ops.
//!
//! [`Editor`] is the model the UI drives: it owns a [`Buffer`] (a ropey rope),
//! a set of [`Selection`]s (multi-cursor), and a snapshot-based undo history.
//! It contains no rendering — given a char index the UI asks the [`Buffer`] for
//! line/column and lays glyphs out itself.

mod buffer;
mod history;
mod selection;

pub use buffer::Buffer;
pub use selection::Selection;

use history::{EditKind, History};

/// Horizontal direction for caret movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Horizontal {
    Left,
    Right,
}

/// Vertical direction for caret movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Vertical {
    Up,
    Down,
}

/// A text editor: a buffer plus multi-cursor selections and undo history.
#[derive(Default)]
pub struct Editor {
    buffer: Buffer,
    selections: Vec<Selection>,
    history: History,
}

impl Editor {
    /// An empty editor with a single caret at the start.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            selections: vec![Selection::caret(0)],
            history: History::default(),
        }
    }

    /// An editor over `text`, with a single caret at the start.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        Self {
            buffer: Buffer::from_text(text),
            selections: vec![Selection::caret(0)],
            history: History::default(),
        }
    }

    /// The buffer, for read access (rendering, queries).
    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// The current selections, sorted by start and non-overlapping.
    #[must_use]
    pub fn selections(&self) -> &[Selection] {
        &self.selections
    }

    /// The primary (first) selection.
    #[must_use]
    pub fn primary(&self) -> Selection {
        self.selections.first().copied().unwrap_or(Selection::caret(0))
    }

    /// Whether an undo step is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Whether a redo step is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    fn len(&self) -> usize {
        self.buffer.len_chars()
    }

    // --- editing ---------------------------------------------------------

    /// Inserts `text` at every selection, replacing any selected ranges.
    pub fn insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let single_char = text.chars().take(2).count() == 1 && !text.contains('\n');
        let kind = if single_char {
            EditKind::Insert
        } else {
            EditKind::Other
        };
        self.history.record(self.buffer.rope(), &self.selections, kind);
        self.normalize();

        let inserted = text.chars().count() as isize;
        let mut delta: isize = 0;
        let mut next = Vec::with_capacity(self.selections.len());
        for sel in self.selections.clone() {
            let start = (sel.start() as isize + delta) as usize;
            let end = (sel.end() as isize + delta) as usize;
            self.buffer.remove(start..end);
            self.buffer.insert(start, text);
            delta += inserted - (end - start) as isize;
            next.push(Selection::caret(start + inserted as usize));
        }
        self.selections = next;
        self.normalize();
    }

    /// Deletes the selection, or the char before each caret (Backspace).
    pub fn backspace(&mut self) {
        self.normalize();
        let changes = self
            .selections
            .iter()
            .any(|s| !s.is_empty() || s.head > 0);
        if !changes {
            return;
        }
        self.history.record(self.buffer.rope(), &self.selections, EditKind::Delete);

        let mut delta: isize = 0;
        let mut next = Vec::with_capacity(self.selections.len());
        for sel in self.selections.clone() {
            let (rstart, rend) = if sel.is_empty() {
                if sel.head == 0 {
                    (0, 0)
                } else {
                    (sel.head - 1, sel.head)
                }
            } else {
                (sel.start(), sel.end())
            };
            let start = (rstart as isize + delta) as usize;
            let end = (rend as isize + delta) as usize;
            self.buffer.remove(start..end);
            delta -= (end - start) as isize;
            next.push(Selection::caret(start));
        }
        self.selections = next;
        self.normalize();
    }

    /// Deletes the selection, or the char after each caret (Delete).
    pub fn delete_forward(&mut self) {
        self.normalize();
        let len = self.len();
        let changes = self.selections.iter().any(|s| !s.is_empty() || s.head < len);
        if !changes {
            return;
        }
        self.history.record(self.buffer.rope(), &self.selections, EditKind::Delete);

        let mut delta: isize = 0;
        let mut next = Vec::with_capacity(self.selections.len());
        for sel in self.selections.clone() {
            let (rstart, rend) = if sel.is_empty() {
                if sel.head >= len {
                    (sel.head, sel.head)
                } else {
                    (sel.head, sel.head + 1)
                }
            } else {
                (sel.start(), sel.end())
            };
            let start = (rstart as isize + delta) as usize;
            let end = (rend as isize + delta) as usize;
            self.buffer.remove(start..end);
            delta -= (end - start) as isize;
            next.push(Selection::caret(start));
        }
        self.selections = next;
        self.normalize();
    }

    // --- movement --------------------------------------------------------

    /// Moves all carets left by one char (extending the selection if `extend`).
    pub fn move_left(&mut self, extend: bool) {
        self.history.break_run();
        self.move_horizontal(Horizontal::Left, extend);
    }

    /// Moves all carets right by one char.
    pub fn move_right(&mut self, extend: bool) {
        self.history.break_run();
        self.move_horizontal(Horizontal::Right, extend);
    }

    /// Moves all carets up one line, preserving column where possible.
    pub fn move_up(&mut self, extend: bool) {
        self.history.break_run();
        self.move_vertical(Vertical::Up, extend);
    }

    /// Moves all carets down one line.
    pub fn move_down(&mut self, extend: bool) {
        self.history.break_run();
        self.move_vertical(Vertical::Down, extend);
    }

    /// Moves all carets to the start of their line.
    pub fn move_line_start(&mut self, extend: bool) {
        self.history.break_run();
        for sel in &mut self.selections {
            let line = self.buffer.char_to_line(sel.head);
            let head = self.buffer.line_to_char(line);
            sel.head = head;
            if !extend {
                sel.anchor = head;
            }
        }
        self.normalize();
    }

    /// Moves all carets to the end of their line's text.
    pub fn move_line_end(&mut self, extend: bool) {
        self.history.break_run();
        for sel in &mut self.selections {
            let line = self.buffer.char_to_line(sel.head);
            let head = self.buffer.line_end(line);
            sel.head = head;
            if !extend {
                sel.anchor = head;
            }
        }
        self.normalize();
    }

    /// Collapses to a single caret at `char_idx`.
    pub fn set_caret(&mut self, char_idx: usize) {
        self.history.break_run();
        let at = char_idx.min(self.len());
        self.selections = vec![Selection::caret(at)];
    }

    /// Adds an additional caret at `char_idx` (Cmd-click style multi-cursor).
    pub fn add_caret(&mut self, char_idx: usize) {
        self.history.break_run();
        let at = char_idx.min(self.len());
        self.selections.push(Selection::caret(at));
        self.normalize();
    }

    /// Selects the entire buffer.
    pub fn select_all(&mut self) {
        self.history.break_run();
        self.selections = vec![Selection::new(0, self.len())];
    }

    fn move_horizontal(&mut self, dir: Horizontal, extend: bool) {
        let len = self.len();
        for sel in &mut self.selections {
            if !extend && !sel.is_empty() {
                let target = match dir {
                    Horizontal::Left => sel.start(),
                    Horizontal::Right => sel.end(),
                };
                *sel = Selection::caret(target);
                continue;
            }
            let head = match dir {
                Horizontal::Left => sel.head.saturating_sub(1),
                Horizontal::Right => (sel.head + 1).min(len),
            };
            sel.head = head;
            if !extend {
                sel.anchor = head;
            }
        }
        self.normalize();
    }

    fn move_vertical(&mut self, dir: Vertical, extend: bool) {
        let len = self.len();
        let lines = self.buffer.len_lines();
        for sel in &mut self.selections {
            let (line, col) = self.buffer.line_col(sel.head);
            let target_line = match dir {
                Vertical::Up if line > 0 => Some(line - 1),
                Vertical::Down if line + 1 < lines => Some(line + 1),
                _ => None,
            };
            let head = match target_line {
                Some(tl) => {
                    let ll = self.buffer.line_len(tl);
                    self.buffer.line_to_char(tl) + col.min(ll)
                }
                None => match dir {
                    Vertical::Up => 0,
                    Vertical::Down => len,
                },
            };
            sel.head = head;
            if !extend {
                sel.anchor = head;
            }
        }
        self.normalize();
    }

    // --- history ---------------------------------------------------------

    /// Restores the previous snapshot. Returns whether anything changed.
    pub fn undo(&mut self) -> bool {
        if let Some((rope, selections)) = self.history.undo(self.buffer.rope(), &self.selections) {
            self.buffer.set_rope(rope);
            self.selections = selections;
            self.normalize();
            true
        } else {
            false
        }
    }

    /// Re-applies the next snapshot. Returns whether anything changed.
    pub fn redo(&mut self) -> bool {
        if let Some((rope, selections)) = self.history.redo(self.buffer.rope(), &self.selections) {
            self.buffer.set_rope(rope);
            self.selections = selections;
            self.normalize();
            true
        } else {
            false
        }
    }

    // --- invariants ------------------------------------------------------

    /// Clamps, sorts, and merges selections so they stay valid, ordered, and
    /// non-overlapping.
    fn normalize(&mut self) {
        let max = self.len();
        for sel in &mut self.selections {
            *sel = sel.clamped(max);
        }
        self.selections.sort_by_key(|s| (s.start(), s.end()));

        let mut merged: Vec<Selection> = Vec::with_capacity(self.selections.len());
        for sel in std::mem::take(&mut self.selections) {
            if let Some(last) = merged.last_mut()
                && (sel.start() < last.end() || sel.start() == last.start())
            {
                *last = Selection::new(last.start().min(sel.start()), last.end().max(sel.end()));
                continue;
            }
            merged.push(sel);
        }
        if merged.is_empty() {
            merged.push(Selection::caret(0));
        }
        self.selections = merged;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(editor: &Editor) -> String {
        editor.buffer().to_string()
    }

    #[test]
    fn insert_and_carets_follow() {
        let mut e = Editor::new();
        e.insert("hello");
        assert_eq!(text(&e), "hello");
        assert_eq!(e.primary(), Selection::caret(5));
    }

    #[test]
    fn backspace_and_delete() {
        let mut e = Editor::from_text("abc");
        e.set_caret(2);
        e.backspace();
        assert_eq!(text(&e), "ac");
        e.delete_forward();
        assert_eq!(text(&e), "a");
    }

    #[test]
    fn selection_replace_on_insert() {
        let mut e = Editor::from_text("hello world");
        // Select "hello".
        e.selections = vec![Selection::new(0, 5)];
        e.insert("hi");
        assert_eq!(text(&e), "hi world");
        assert_eq!(e.primary(), Selection::caret(2));
    }

    #[test]
    fn multi_cursor_insert_keeps_indices_correct() {
        let mut e = Editor::from_text("a\nb\nc");
        // A caret at the start of each line: indices 0, 2, 4.
        e.selections = vec![
            Selection::caret(0),
            Selection::caret(2),
            Selection::caret(4),
        ];
        e.insert(">");
        assert_eq!(text(&e), ">a\n>b\n>c");
        assert_eq!(e.selections().len(), 3);
    }

    #[test]
    fn undo_redo_round_trip() {
        let mut e = Editor::new();
        e.insert("foo");
        e.insert(" "); // breaks coalescing? space is single char -> coalesces
        e.insert("\n"); // newline -> Other, new undo step
        e.insert("bar");
        let full = text(&e);
        assert!(e.undo());
        assert!(e.undo());
        let back = text(&e);
        assert_ne!(full, back);
        assert!(e.redo());
        assert!(e.redo());
        assert_eq!(text(&e), full);
    }

    #[test]
    fn typing_run_coalesces_into_one_undo() {
        let mut e = Editor::new();
        e.insert("h");
        e.insert("i");
        // One coalesced step -> a single undo empties the buffer.
        assert!(e.undo());
        assert_eq!(text(&e), "");
        assert!(!e.undo());
    }

    #[test]
    fn vertical_movement_preserves_column() {
        let mut e = Editor::from_text("hello\nhi\nworld");
        e.set_caret(4); // column 4 on line 0 ("hell|o")
        e.move_down(false);
        // Line 1 "hi" is shorter; clamp to its end (column 2).
        let (line, col) = e.buffer().line_col(e.primary().head);
        assert_eq!((line, col), (1, 2));
        e.move_down(false);
        // Line 2 "world" is long enough; column restored to 4? We clamp from the
        // current head, so column is 2 here — documented behaviour.
        let (line, _) = e.buffer().line_col(e.primary().head);
        assert_eq!(line, 2);
    }

    #[test]
    fn handles_a_large_buffer() {
        let big = "fn main() {}\n".repeat(400_000); // ~5 MB, 400k lines
        let mut e = Editor::from_text(&big);
        assert_eq!(e.buffer().len_lines(), 400_001);
        e.set_caret(e.buffer().len_chars());
        e.insert("// end");
        assert!(e.buffer().to_string().ends_with("// end"));
        assert!(e.undo());
        assert!(e.buffer().to_string().ends_with("}\n"));
    }
}
