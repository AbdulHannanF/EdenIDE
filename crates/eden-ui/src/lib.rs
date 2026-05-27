//! `eden-ui` — widget tree, layout, render passes, and theming.
//!
//! Phase 1 establishes the surface: a [`Widget`] trait whose implementors are
//! pure painters, a taffy-driven [`Chrome`] that lays the editor shell out and
//! animates it with springs, hit testing, and paint helpers over vello's
//! `Scene`. Text, the editor buffer, and real content arrive in later phases.

mod chrome;
mod logic_panel;
mod paint;
mod panels;
mod text;
mod widget;

pub use chrome::{Chrome, Region};
pub use logic_panel::LogicPanel;
pub use paint::{fill_rect, fill_rrect, to_color};
pub use panels::{EditorArea, SidebarPanel, StatusBar, TabStrip, TerminalPanel, TitleBar};
pub use text::{
    CmdEntry, CmdPaletteView, CompletionEntry, CompletionView, DiffMark, EditorFrame, FindBarHits,
    FindBarView, GutterMark, MenuItemView, MinimapView, PaletteView, SearchPanelView,
    SearchRowView, ScrubberView, SettingsToggle, SettingsView, StatusBarView, TabHit, TabLabel,
    TerminalView, TextSystem, TreeRow, TreeView,
};
pub use paint::to_rgba8_alpha;
pub use widget::{PaintCtx, Widget};

// Re-exported so callers can build/drive an editor and highlighter without
// direct dependencies on the lower crates.
pub use eden_editor::Editor;
pub use eden_syntax::{HighlightKind, Highlighter, Highlights, Span};
