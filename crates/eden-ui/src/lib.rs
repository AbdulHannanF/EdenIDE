//! `eden-ui` — widget tree, layout, render passes, and theming.
//!
//! Phase 1 establishes the surface: a [`Widget`] trait whose implementors are
//! pure painters, a taffy-driven [`Chrome`] that lays the editor shell out and
//! animates it with springs, hit testing, and paint helpers over vello's
//! `Scene`. Text, the editor buffer, and real content arrive in later phases.

mod chrome;
mod paint;
mod panels;
mod text;
mod widget;

pub use chrome::{Chrome, Region};
pub use paint::{fill_rect, fill_rrect, to_color};
pub use panels::{EditorArea, SidebarPanel, StatusBar, TabStrip, TitleBar};
pub use text::{EditorFrame, PaletteView, TextSystem};
pub use widget::{PaintCtx, Widget};

// Re-exported so callers can build/drive an editor and highlighter without
// direct dependencies on the lower crates.
pub use eden_editor::Editor;
pub use eden_syntax::{HighlightKind, Highlighter, Highlights, Span};
