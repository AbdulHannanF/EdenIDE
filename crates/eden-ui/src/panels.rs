//! The editor chrome panels — pure background painters.
//!
//! Text content (labels, path, file tree rows, etc.) is drawn over these
//! rects by [`crate::TextSystem`] after `Chrome::paint` returns.

use vello::Scene;
use vello::kurbo::Rect;

use crate::paint::{fill_rect, fill_rrect};
use crate::widget::{PaintCtx, Widget};

// ── DemoStrip ─────────────────────────────────────────────────────────────────

/// The 28px prototype navigation strip at the very top of the window.
///
/// Background: `surface` (elevated). The screen-tab buttons are text-rendered
/// by [`crate::TextSystem::paint_demo_strip`] on top.
#[derive(Default)]
pub struct DemoStrip;

impl Widget for DemoStrip {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.surface);
    }
}

// ── TopBar (was TitleBar) ─────────────────────────────────────────────────────

/// The 36px top bar: glyph · EDEN · breadcrumb · chrome buttons.
///
/// In the prototype this is a single row (not the old 32+36 split).
/// Window control circles live in the right-hand section.
#[derive(Default)]
pub struct TopBar;

impl Widget for TopBar {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        let s = ctx.scale;
        let p = ctx.palette;

        // Single 36px surface fill.
        fill_rect(scene, bounds, p.surface);

        // Window control circles — vertically centred in the 36px band.
        let cy = bounds.y0 + bounds.height() * 0.5;
        let r  = 5.0 * s;
        let d  = 10.0 * s;

        // Close (accent) — rightmost.
        let close_cx = bounds.x1 - 14.0 * s;
        fill_rrect(scene,
            Rect::new(close_cx - r, cy - r, close_cx + r, cy + r),
            r, p.accent);
        // Maximize (dim).
        let max_cx = close_cx - d - 6.0 * s;
        fill_rrect(scene,
            Rect::new(max_cx - r, cy - r, max_cx + r, cy + r),
            r, p.fg_dim);
        // Minimize (dim).
        let min_cx = max_cx - d - 6.0 * s;
        fill_rrect(scene,
            Rect::new(min_cx - r, cy - r, min_cx + r, cy + r),
            r, p.fg_dim);
    }
}

// ── LeftRail ──────────────────────────────────────────────────────────────────

/// The 48px left activity rail.
///
/// Icon glyphs and tooltips are text-rendered by
/// [`crate::TextSystem::paint_left_rail`].
#[derive(Default)]
pub struct LeftRail;

impl Widget for LeftRail {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.background);
    }
}

// ── SidebarPanel ─────────────────────────────────────────────────────────────

/// The left sidebar / file-tree background.
#[derive(Default)]
pub struct SidebarPanel;

impl Widget for SidebarPanel {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        if bounds.width() < 2.0 {
            return;
        }
        fill_rect(scene, bounds, ctx.palette.surface);
    }
}

// ── TabStrip ──────────────────────────────────────────────────────────────────

/// The tab strip background.
#[derive(Default)]
pub struct TabStrip;

impl Widget for TabStrip {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.tab_inactive);
    }
}

// ── EditorArea ────────────────────────────────────────────────────────────────

/// The editor canvas background.
#[derive(Default)]
pub struct EditorArea;

impl Widget for EditorArea {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.background);
    }
}

// ── TerminalPanel ─────────────────────────────────────────────────────────────

/// The embedded terminal panel background.
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

// ── StatusBar ─────────────────────────────────────────────────────────────────

/// The bottom status bar background.
#[derive(Default)]
pub struct StatusBar;

impl Widget for StatusBar {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.status_bar);
    }
}
