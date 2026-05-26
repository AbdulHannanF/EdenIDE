//! Snapshot-based undo/redo.
//!
//! Because cloning a [`Rope`] is O(1), an undo entry can be a full snapshot of
//! the document and selections rather than an inverse-edit log. This is simple
//! and impossible to get subtly wrong, and the structural sharing means a deep
//! history costs almost nothing.

use ropey::Rope;

use crate::selection::Selection;

/// The kind of the most recent edit, used to coalesce a run of single-character
/// inserts into one undo step (so typing a word is one undo, not many).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EditKind {
    Insert,
    Delete,
    Other,
}

#[derive(Clone)]
struct Snapshot {
    rope: Rope,
    selections: Vec<Selection>,
}

/// Undo and redo stacks of document snapshots.
#[derive(Default)]
pub(crate) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last: Option<EditKind>,
}

impl History {
    /// Records the pre-edit state. Consecutive single-char inserts coalesce, so
    /// only the first in a typing run pushes a snapshot.
    pub(crate) fn record(&mut self, rope: &Rope, selections: &[Selection], kind: EditKind) {
        let coalesce = kind == EditKind::Insert && self.last == Some(EditKind::Insert);
        self.redo.clear();
        if !coalesce {
            self.undo.push(Snapshot {
                rope: rope.clone(),
                selections: selections.to_vec(),
            });
        }
        self.last = Some(kind);
    }

    /// Ends the current coalescing run (e.g. after a cursor move) so the next
    /// insert starts a fresh undo step.
    pub(crate) fn break_run(&mut self) {
        self.last = None;
    }

    /// Pops an undo snapshot, pushing the current state onto the redo stack.
    pub(crate) fn undo(
        &mut self,
        rope: &Rope,
        selections: &[Selection],
    ) -> Option<(Rope, Vec<Selection>)> {
        let snap = self.undo.pop()?;
        self.redo.push(Snapshot {
            rope: rope.clone(),
            selections: selections.to_vec(),
        });
        self.last = None;
        Some((snap.rope, snap.selections))
    }

    /// Pops a redo snapshot, pushing the current state onto the undo stack.
    pub(crate) fn redo(
        &mut self,
        rope: &Rope,
        selections: &[Selection],
    ) -> Option<(Rope, Vec<Selection>)> {
        let snap = self.redo.pop()?;
        self.undo.push(Snapshot {
            rope: rope.clone(),
            selections: selections.to_vec(),
        });
        self.last = None;
        Some((snap.rope, snap.selections))
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Number of undo steps available from the current position.
    pub(crate) fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of redo steps available from the current position.
    pub(crate) fn redo_depth(&self) -> usize {
        self.redo.len()
    }
}
