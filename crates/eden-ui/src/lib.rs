//! `eden-ui` — widget tree, layout, render passes, and theming.

mod chrome;
mod logic_panel;
mod paint;
mod panels;
mod text;
mod widget;

pub use chrome::{Chrome, DemoScreen, Region};
pub use logic_panel::LogicPanel;
pub use paint::{fill_rect, fill_rrect, to_color, to_rgba8_alpha};
pub use panels::{
    DemoStrip, EditorArea, LeftRail, SidebarPanel, StatusBar, TabStrip, TerminalPanel, TopBar,
};
pub use text::{
    ActivityBarView, BreadcrumbView, CmdEntry, CmdPaletteView, CompletionEntry, CompletionView,
    DemoStripView, DiffMark, EditorFrame, FindBarHits, FindBarView, GutterMark, LeftRailItem,
    LeftRailView, LogicPanelView, LogicSymbol, MenuItemView, MinimapView, PaletteView,
    SearchPanelView, SearchRowView, ScrubberView, SettingsToggle, SettingsView, StatusBarView,
    TabHit, TabLabel, TerminalView, TextSystem, TopBarView, TreeRow, TreeView,
};
pub use widget::{PaintCtx, Widget};

// Re-exported so callers can build/drive an editor and highlighter without
// direct dependencies on the lower crates.
pub use eden_editor::Editor;
pub use eden_syntax::{HighlightKind, Highlighter, Highlights, Span};
