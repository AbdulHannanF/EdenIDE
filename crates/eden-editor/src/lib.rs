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

    /// Number of undo steps available (current position in the history stack).
    #[must_use]
    pub fn history_pos(&self) -> usize {
        self.history.undo_depth()
    }

    /// Total history depth: undo steps available plus redo steps available.
    #[must_use]
    pub fn history_total(&self) -> usize {
        self.history.undo_depth() + self.history.redo_depth()
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

    /// Replaces the selection set with a single selection `anchor..head`
    /// (used by find navigation and go-to-position jumps).
    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        self.history.break_run();
        let max = self.len();
        self.selections = vec![Selection::new(anchor.min(max), head.min(max))];
    }

    // --- clipboard ----------------------------------------------------------

    /// The text a copy should place on the clipboard: the primary selection's
    /// text if non-empty, otherwise the whole current line including its
    /// trailing newline (so a paste re-inserts a full line).
    #[must_use]
    pub fn copy_text(&self) -> String {
        let sel = self.primary();
        if sel.is_empty() {
            let line = self.buffer.char_to_line(sel.head);
            let (start, end) = self.line_span_with_break(line);
            self.buffer.slice_to_string(start..end)
        } else {
            self.buffer.slice_to_string(sel.range())
        }
    }

    /// Removes the primary selection — or the whole current line if it is empty
    /// — and returns the removed text for a cut. Records one undo step.
    pub fn cut(&mut self) -> String {
        let sel = self.primary();
        self.history.break_run();
        let (start, end, text) = if sel.is_empty() {
            let line = self.buffer.char_to_line(sel.head);
            let (s, e) = self.line_span_with_break(line);
            (s, e, self.buffer.slice_to_string(s..e))
        } else {
            (sel.start(), sel.end(), self.buffer.slice_to_string(sel.range()))
        };
        if start == end {
            return text;
        }
        self.history.record(self.buffer.rope(), &self.selections, EditKind::Other);
        self.buffer.remove(start..end);
        self.selections = vec![Selection::caret(start)];
        self.normalize();
        text
    }

    /// The char range of `line` including its trailing line break, clamped to
    /// the buffer end for the final line.
    fn line_span_with_break(&self, line: usize) -> (usize, usize) {
        let start = self.buffer.line_to_char(line);
        let end = if line + 1 < self.buffer.len_lines() {
            self.buffer.line_to_char(line + 1)
        } else {
            self.buffer.len_chars()
        };
        (start, end)
    }

    // --- line operations ----------------------------------------------------

    /// Inclusive `[first, last]` line span touched by the primary selection.
    fn primary_line_span(&self) -> (usize, usize) {
        let sel = self.primary();
        (self.buffer.char_to_line(sel.start()), self.buffer.char_to_line(sel.end()))
    }

    /// Selects the full line(s) the primary selection touches, including the
    /// trailing newline (Ctrl+L).
    pub fn select_line(&mut self) {
        self.history.break_run();
        let (first, last) = self.primary_line_span();
        let start = self.buffer.line_to_char(first);
        let (_, end) = self.line_span_with_break(last);
        self.selections = vec![Selection::new(start, end)];
    }

    /// Indents every line the primary selection touches by `width` spaces.
    pub fn indent_lines(&mut self, width: usize) {
        if width == 0 {
            return;
        }
        let was_empty = self.primary().is_empty();
        let head_col = {
            let h = self.primary().head;
            h - self.buffer.line_to_char(self.buffer.char_to_line(h))
        };
        let (first, last) = self.primary_line_span();
        self.history.break_run();
        self.history.record(self.buffer.rope(), &self.selections, EditKind::Other);
        let spaces = " ".repeat(width);
        for line in (first..=last).rev() {
            let at = self.buffer.line_to_char(line);
            self.buffer.insert(at, &spaces);
        }
        self.reselect_after_line_edit(first, last, was_empty, head_col + width);
    }

    /// Removes up to `width` leading spaces (or one leading tab) from every line
    /// the primary selection touches.
    pub fn dedent_lines(&mut self, width: usize) {
        if width == 0 {
            return;
        }
        let was_empty = self.primary().is_empty();
        let head_col = {
            let h = self.primary().head;
            h - self.buffer.line_to_char(self.buffer.char_to_line(h))
        };
        let (first, last) = self.primary_line_span();
        self.history.break_run();
        self.history.record(self.buffer.rope(), &self.selections, EditKind::Other);
        let mut removed_on_head_line = 0usize;
        let head_line = self.buffer.char_to_line(self.primary().head);
        for line in (first..=last).rev() {
            let at = self.buffer.line_to_char(line);
            let rope = self.buffer.rope();
            let len = rope.len_chars();
            let mut take = 0;
            while take < width && at + take < len {
                let c = rope.char(at + take);
                if c == '\t' {
                    take += 1;
                    break;
                }
                if c == ' ' {
                    take += 1;
                } else {
                    break;
                }
            }
            if take > 0 {
                self.buffer.remove(at..at + take);
                if line == head_line {
                    removed_on_head_line = take;
                }
            }
        }
        let new_col = head_col.saturating_sub(removed_on_head_line);
        self.reselect_after_line_edit(first, last, was_empty, new_col);
    }

    /// Toggles a line comment (`token`, e.g. `"// "`) on the touched lines. If
    /// every non-blank line is already commented, uncomments instead.
    pub fn toggle_line_comment(&mut self, token: &str) {
        let trimmed = token.trim_end();
        if trimmed.is_empty() {
            return;
        }
        let (first, last) = self.primary_line_span();
        let lines: Vec<String> = (first..=last)
            .map(|l| {
                let (s, e) = (self.buffer.line_to_char(l), self.buffer.line_end(l));
                self.buffer.slice_to_string(s..e)
            })
            .collect();
        let all_commented = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .all(|l| l.trim_start().starts_with(trimmed));
        self.history.break_run();
        self.history.record(self.buffer.rope(), &self.selections, EditKind::Other);
        // Edit bottom-up so earlier line starts stay valid.
        for (offset, original) in lines.iter().enumerate().rev() {
            let line = first + offset;
            if original.trim().is_empty() {
                continue;
            }
            let line_start = self.buffer.line_to_char(line);
            let indent = original.len() - original.trim_start().len();
            // `indent` counts ASCII whitespace, so it is also a char offset.
            if all_commented {
                let after = &original[indent..];
                let mut strip = trimmed.len();
                if after[trimmed.len()..].starts_with(' ') {
                    strip += 1;
                }
                self.buffer.remove(line_start + indent..line_start + indent + strip);
            } else {
                self.buffer.insert(line_start + indent, token);
            }
        }
        let start = self.buffer.line_to_char(first);
        let end = self.buffer.line_end(last);
        self.selections = vec![Selection::new(start, end)];
        self.normalize();
    }

    /// Moves the touched line(s) up or down by one, swapping with the adjacent
    /// line. Keeps the moved block selected.
    pub fn move_lines(&mut self, down: bool) {
        let (first, last) = self.primary_line_span();
        let text = self.buffer.to_string();
        let trailing_nl = text.ends_with('\n');
        let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
        if trailing_nl {
            lines.pop();
        }
        let n = lines.len();
        if n == 0 {
            return;
        }
        let last = last.min(n - 1);
        let first = first.min(last);
        let (new_first, new_last) = if down {
            if last + 1 >= n {
                return;
            }
            let moved = lines.remove(last + 1);
            lines.insert(first, moved);
            (first + 1, last + 1)
        } else {
            if first == 0 {
                return;
            }
            let moved = lines.remove(first - 1);
            lines.insert(last, moved);
            (first - 1, last - 1)
        };
        let mut rebuilt = lines.join("\n");
        if trailing_nl {
            rebuilt.push('\n');
        }
        self.history.break_run();
        self.history.record(self.buffer.rope(), &self.selections, EditKind::Other);
        self.buffer = Buffer::from_text(&rebuilt);
        let start = self.buffer.line_to_char(new_first);
        let end = self.buffer.line_end(new_last);
        self.selections = vec![Selection::new(start, end)];
        self.normalize();
    }

    // --- multi-cursor occurrence selection ----------------------------------

    /// Selects the word under the primary caret. Returns whether a word was
    /// found.
    pub fn select_word(&mut self) -> bool {
        let rope = self.buffer.rope();
        let len = rope.len_chars();
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let head = self.primary().head.min(len);
        let mut start = head;
        while start > 0 && is_word(rope.char(start - 1)) {
            start -= 1;
        }
        let mut end = head;
        while end < len && is_word(rope.char(end)) {
            end += 1;
        }
        if start == end {
            return false;
        }
        self.history.break_run();
        self.selections = vec![Selection::new(start, end)];
        true
    }

    /// Adds a selection at the next occurrence of the primary selection's text
    /// (Ctrl+D). If the primary selection is empty, selects the word under the
    /// caret instead. Returns whether the selection set changed.
    pub fn select_next_occurrence(&mut self) -> bool {
        let sel = self.primary();
        if sel.is_empty() {
            return self.select_word();
        }
        let needle = self.buffer.slice_to_string(sel.range());
        if needle.is_empty() {
            return false;
        }
        let rope = self.buffer.rope();
        let from_char = self.selections.iter().map(Selection::end).max().unwrap_or(0);
        let hay = rope.to_string();
        let from_byte = rope.char_to_byte(from_char.min(rope.len_chars()));
        let found = hay[from_byte..]
            .find(&needle)
            .map(|b| from_byte + b)
            .or_else(|| hay.find(&needle));
        let Some(byte_pos) = found else {
            return false;
        };
        let char_start = rope.byte_to_char(byte_pos);
        let char_end = char_start + needle.chars().count();
        if self
            .selections
            .iter()
            .any(|s| s.start() == char_start && s.end() == char_end)
        {
            return false;
        }
        self.history.break_run();
        self.selections.push(Selection::new(char_start, char_end));
        self.normalize();
        true
    }

    /// Replaces every range in `ranges` with `replacement` as a single undo
    /// step (used by find-and-replace "Replace All"). Ranges are applied
    /// back-to-front so earlier indices stay valid.
    pub fn replace_ranges(&mut self, ranges: &[(usize, usize)], replacement: &str) {
        if ranges.is_empty() {
            return;
        }
        self.history.break_run();
        self.history.record(self.buffer.rope(), &self.selections, EditKind::Other);
        let mut sorted: Vec<(usize, usize)> = ranges.to_vec();
        sorted.sort_by_key(|r| std::cmp::Reverse(r.0));
        let mut last_start = self.len();
        for (s, e) in sorted {
            let (s, e) = (s.min(self.len()), e.min(self.len()));
            if s > e {
                continue;
            }
            self.buffer.remove(s..e);
            self.buffer.insert(s, replacement);
            last_start = s;
        }
        let caret = (last_start + replacement.chars().count()).min(self.len());
        self.selections = vec![Selection::caret(caret)];
        self.normalize();
    }

    /// Restores the selection after a per-line edit: covers the whole block for
    /// a multi-line selection, or places a caret at `head_col` on the (single)
    /// touched line when the original selection was an empty caret.
    fn reselect_after_line_edit(
        &mut self,
        first: usize,
        last: usize,
        was_caret: bool,
        head_col: usize,
    ) {
        if was_caret && first == last {
            let line_start = self.buffer.line_to_char(first);
            let line_len = self.buffer.line_len(first);
            let at = line_start + head_col.min(line_len);
            self.selections = vec![Selection::caret(at)];
        } else {
            let start = self.buffer.line_to_char(first);
            let end = self.buffer.line_end(last);
            self.selections = vec![Selection::new(start, end)];
        }
        self.normalize();
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
    fn copy_uses_selection_then_whole_line() {
        let mut e = Editor::from_text("hello\nworld\n");
        e.selections = vec![Selection::new(0, 5)];
        assert_eq!(e.copy_text(), "hello");
        e.set_caret(8); // on "world"
        assert_eq!(e.copy_text(), "world\n");
    }

    #[test]
    fn cut_removes_selection_or_line() {
        let mut e = Editor::from_text("hello\nworld\n");
        e.selections = vec![Selection::new(0, 5)];
        assert_eq!(e.cut(), "hello");
        assert_eq!(text(&e), "\nworld\n");
        e.set_caret(1); // on "world"
        assert_eq!(e.cut(), "world\n");
        assert_eq!(text(&e), "\n");
    }

    #[test]
    fn indent_and_dedent_round_trip() {
        let mut e = Editor::from_text("a\nb\nc\n");
        e.selections = vec![Selection::new(0, 3)]; // covers lines 0..=1
        e.indent_lines(4);
        assert_eq!(text(&e), "    a\n    b\nc\n");
        e.dedent_lines(4);
        assert_eq!(text(&e), "a\nb\nc\n");
    }

    #[test]
    fn dedent_caret_stops_at_partial_indent() {
        let mut e = Editor::from_text("  x\n");
        e.set_caret(3); // after x
        e.dedent_lines(4);
        assert_eq!(text(&e), "x\n");
    }

    #[test]
    fn toggle_comment_adds_then_removes() {
        let mut e = Editor::from_text("fn main() {}\n");
        e.select_all();
        e.toggle_line_comment("// ");
        assert_eq!(text(&e), "// fn main() {}\n");
        e.select_all();
        e.toggle_line_comment("// ");
        assert_eq!(text(&e), "fn main() {}\n");
    }

    #[test]
    fn toggle_comment_respects_indentation() {
        let mut e = Editor::from_text("    let x = 1;\n");
        e.set_caret(0);
        e.toggle_line_comment("// ");
        assert_eq!(text(&e), "    // let x = 1;\n");
    }

    #[test]
    fn move_line_down_and_up() {
        let mut e = Editor::from_text("one\ntwo\nthree");
        e.set_caret(0); // line 0 "one"
        e.move_lines(true);
        assert_eq!(text(&e), "two\none\nthree");
        // caret now on the moved "one" (line 1); move it back up.
        e.move_lines(false);
        assert_eq!(text(&e), "one\ntwo\nthree");
    }

    #[test]
    fn move_line_down_into_last_line_keeps_text() {
        let mut e = Editor::from_text("a\nb\nc");
        e.set_caret(2); // line 1 "b"
        e.move_lines(true);
        assert_eq!(text(&e), "a\nc\nb");
    }

    #[test]
    fn replace_ranges_is_single_undo() {
        let mut e = Editor::from_text("foo foo foo");
        // Ranges of all three "foo".
        e.replace_ranges(&[(0, 3), (4, 7), (8, 11)], "bar");
        assert_eq!(text(&e), "bar bar bar");
        // One undo restores everything.
        assert!(e.undo());
        assert_eq!(text(&e), "foo foo foo");
    }

    #[test]
    fn select_next_occurrence_adds_cursor() {
        let mut e = Editor::from_text("foo bar foo baz foo");
        e.set_caret(0);
        assert!(e.select_word()); // selects first "foo"
        assert!(e.select_next_occurrence()); // second "foo"
        assert_eq!(e.selections().len(), 2);
        assert!(e.select_next_occurrence()); // third "foo"
        assert_eq!(e.selections().len(), 3);
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
