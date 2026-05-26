//! The widget abstraction.

use eden_theme::Palette;
use vello::Scene;
use vello::kurbo::Rect;

/// Interaction and theming state handed to a [`Widget`] at paint time.
pub struct PaintCtx<'a> {
    /// The palette to paint with. During a theme crossfade this is the
    /// interpolated palette, not either endpoint.
    pub palette: &'a Palette,
    /// Display scale factor (physical pixels per logical pixel), so widgets can
    /// size their content crisply on HiDPI displays.
    pub scale: f64,
    /// Hover intensity for this widget in `0.0..=1.0`, driven by a spring so it
    /// eases in and out rather than toggling.
    pub hover: f64,
}

/// A paintable region of the editor chrome.
///
/// Widgets are pure painters: position and size are decided by the containing
/// [`crate::Chrome`] through taffy, and a widget is told only the absolute
/// `bounds` to draw into. Keeping layout out of the trait makes each panel
/// trivial to test and reuse, and keeps the render pass a simple walk.
pub trait Widget {
    /// Paints this widget into the given absolute bounds.
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx);
}
