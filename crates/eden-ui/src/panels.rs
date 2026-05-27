//! The editor chrome panels.
//!
//! Each panel widget is a pure background painter. Text content (tab labels,
//! status text, file tree rows, etc.) is drawn over these rects by
//! [`crate::TextSystem`] after `Chrome::paint` returns, so the text system can
//! use cosmic-text for shaping without coupling into the panel layer.

use vello::Scene;
use vello::kurbo::Rect;

use crate::paint::{fill_rect, fill_rrect};
use crate::widget::{PaintCtx, Widget};

/// The top title bar, 68px tall (activity bar 32px + breadcrumb bar 36px).
///
/// Painting breakdown:
/// - Top 32px (activity area): filled with `palette.background`.
/// - Bottom 36px (breadcrumb area): filled with `palette.surface`.
/// - A 1px horizontal hairline between the two zones.
/// - A 1px hairline at the very bottom of the 68px area.
/// - macOS-style window control circles (close/maximize/minimize) in the top-right.
///
/// The actual activity tabs and breadcrumb path text are rendered by
/// [`crate::TextSystem`] on top.
#[derive(Default)]
pub struct TitleBar;

impl Widget for TitleBar {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        let s = ctx.scale;
        let p = ctx.palette;

        // Logical heights of the two zones.
        let activity_h = 32.0 * s;

        // Activity area (top): background colour.
        fill_rect(
            scene,
            Rect::new(bounds.x0, bounds.y0, bounds.x1, bounds.y0 + activity_h),
            p.background,
        );
        // Breadcrumb area (bottom): surface colour.
        fill_rect(
            scene,
            Rect::new(bounds.x0, bounds.y0 + activity_h, bounds.x1, bounds.y1),
            p.surface,
        );

        // Hairline divider between activity and breadcrumb zones.
        let t = s.max(1.0);
        fill_rect(
            scene,
            Rect::new(bounds.x0, bounds.y0 + activity_h - t, bounds.x1, bounds.y0 + activity_h),
            p.divider,
        );

        // Window control buttons — three circles in the top-right corner of the
        // activity zone. Vertically centred in the 32px activity band.
        // design: 12×12 logical px circles, 6px corner radius (fully round).
        let cy = bounds.y0 + activity_h * 0.5;
        let r = 6.0 * s;   // half-size (radius)
        let d = 12.0 * s;  // diameter
        // Close (red accent) — rightmost.
        let close_cx = bounds.x1 - 16.0 * s;
        fill_rrect(
            scene,
            Rect::new(close_cx - r, cy - r, close_cx + r, cy + r),
            r,
            p.accent,
        );
        // Maximize (dim) — middle.
        let max_cx = bounds.x1 - 16.0 * s - d - 8.0 * s;
        fill_rrect(
            scene,
            Rect::new(max_cx - r, cy - r, max_cx + r, cy + r),
            r,
            p.fg_dim,
        );
        // Minimize (dim) — leftmost.
        let min_cx = bounds.x1 - 16.0 * s - 2.0 * (d + 8.0 * s);
        fill_rrect(
            scene,
            Rect::new(min_cx - r, cy - r, min_cx + r, cy + r),
            r,
            p.fg_dim,
        );
    }
}

/// The left sidebar background.
///
/// The interactive file tree is drawn over this rect by
/// [`crate::TextSystem::paint_file_tree`], which owns the tree model.
#[derive(Default)]
pub struct SidebarPanel;

impl Widget for SidebarPanel {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        if bounds.width() < 2.0 {
            return; // collapsed
        }
        fill_rect(scene, bounds, ctx.palette.surface);
    }
}

/// The tab strip background.
///
/// Real tab labels (filename text, accent underlines) are painted by
/// [`crate::TextSystem::paint_tab_bar`] after `Chrome::paint` returns, so the
/// text system can use cosmic-text for shaping.
#[derive(Default)]
pub struct TabStrip;

impl Widget for TabStrip {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.tab_inactive);
    }
}

/// The editor canvas background.
///
/// Real text, gutter, selections, and carets are drawn over this rect by
/// [`crate::TextSystem`], which owns the editor model and shaping.
#[derive(Default)]
pub struct EditorArea;

impl Widget for EditorArea {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.background);
    }
}

/// The terminal panel background.
///
/// Real cell content is drawn over this by
/// [`crate::TextSystem::paint_terminal`].
#[derive(Default)]
pub struct TerminalPanel;

impl Widget for TerminalPanel {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        if bounds.height() < 2.0 {
            return;
        }
        fill_rect(scene, bounds, ctx.palette.background);
    }
}

/// The bottom status bar background.
///
/// The real status content (branch, position, language) is rendered by
/// [`crate::TextSystem::paint_status_bar`].
#[derive(Default)]
pub struct StatusBar;

impl Widget for StatusBar {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.status_bar);
    }
}

