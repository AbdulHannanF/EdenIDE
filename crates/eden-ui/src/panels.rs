//! The editor chrome panels.
//!
//! These are deliberately content-light: Eden has no text renderer yet (that
//! arrives in Phase 2), so each panel paints abstract placeholder shapes — bars
//! and chips, never fake letterforms — to convey structure and let the theming
//! and motion systems be seen working.

use eden_theme::Rgba8;
use vello::Scene;
use vello::kurbo::Rect;

use crate::paint::{fill_rect, fill_rrect};
use crate::widget::{PaintCtx, Widget};

/// Returns `color` with its alpha replaced — for drawing faint placeholders.
fn with_alpha(color: Rgba8, alpha: u8) -> Rgba8 {
    Rgba8::rgba(color.r, color.g, color.b, alpha)
}

/// Draws a small rounded bar at an absolute position.
fn bar(scene: &mut Scene, x: f64, y: f64, w: f64, h: f64, color: Rgba8) {
    fill_rrect(scene, Rect::new(x, y, x + w, y + h), h.min(w) * 0.5, color);
}

/// The top bar: a brand mark on the left and the active-file chip centered.
#[derive(Default)]
pub struct TitleBar;

impl Widget for TitleBar {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        let s = ctx.scale;
        let p = ctx.palette;
        fill_rect(scene, bounds, p.surface_raised);

        let cy = bounds.center().y;
        // Brand dot.
        let dot = 9.0 * s;
        bar(scene, bounds.x0 + 16.0 * s, cy - dot / 2.0, dot, dot, p.accent);

        // Centered file chip.
        let chip_w = (220.0 * s).min(bounds.width() * 0.5);
        let chip_h = 20.0 * s;
        let chip_x = bounds.center().x - chip_w / 2.0;
        let chip_y = cy - chip_h / 2.0;
        fill_rrect(
            scene,
            Rect::new(chip_x, chip_y, chip_x + chip_w, chip_y + chip_h),
            8.0 * s,
            p.surface,
        );
        // Filename placeholder inside the chip.
        bar(
            scene,
            chip_x + 14.0 * s,
            cy - 3.0 * s,
            chip_w - 28.0 * s,
            6.0 * s,
            with_alpha(p.text_muted, 0x9C),
        );
    }
}

/// The left sidebar: an explorer header and placeholder file rows.
#[derive(Default)]
pub struct SidebarPanel;

impl Widget for SidebarPanel {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        if bounds.width() < 2.0 {
            return; // collapsed
        }
        let s = ctx.scale;
        let p = ctx.palette;
        fill_rect(scene, bounds, p.surface);

        let pad = 16.0 * s;
        // Section header.
        bar(scene, bounds.x0 + pad, bounds.y0 + 16.0 * s, 96.0 * s, 6.0 * s, with_alpha(p.text_muted, 0xC0));

        // File rows. Indentation cycles to suggest a tree.
        let row_h = 26.0 * s;
        let mut y = bounds.y0 + 44.0 * s;
        let indents = [0.0, 0.0, 1.0, 1.0, 2.0, 1.0, 0.0, 0.0, 1.0];
        let widths = [120.0, 96.0, 80.0, 104.0, 72.0, 88.0, 132.0, 100.0, 76.0];
        for (indent, width) in indents.iter().zip(widths.iter()) {
            if y + row_h > bounds.y1 {
                break;
            }
            let x = bounds.x0 + pad + indent * 14.0 * s;
            let icon = 9.0 * s;
            let row_cy = y + row_h / 2.0;
            // Icon glyph placeholder.
            fill_rrect(
                scene,
                Rect::new(x, row_cy - icon / 2.0, x + icon, row_cy + icon / 2.0),
                2.0 * s,
                with_alpha(p.text_muted, 0x80),
            );
            // Filename bar.
            bar(scene, x + icon + 8.0 * s, row_cy - 3.0 * s, width * s, 6.0 * s, with_alpha(p.text_muted, 0x9A));
            y += row_h;
        }

        // Hover wash.
        if ctx.hover > 0.001 {
            let alpha = (ctx.hover * f64::from(0x16)).round() as u8;
            fill_rect(scene, bounds, with_alpha(p.accent, alpha));
        }
    }
}

/// The tab strip: a few tab placeholders, the first one active.
#[derive(Default)]
pub struct TabStrip;

impl Widget for TabStrip {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        let s = ctx.scale;
        let p = ctx.palette;
        fill_rect(scene, bounds, p.surface_raised);

        let tab_w = (160.0 * s).min(bounds.width() / 3.0);
        let labels = [124.0_f64, 96.0, 110.0];
        for (i, label_w) in labels.iter().enumerate() {
            let x0 = bounds.x0 + tab_w * i as f64;
            let tab = Rect::new(x0, bounds.y0, x0 + tab_w, bounds.y1);
            let active = i == 0;
            if active {
                fill_rect(scene, tab, p.tab_active);
                // Accent underline.
                let uh = 2.0 * s;
                fill_rect(scene, Rect::new(tab.x0, tab.y1 - uh, tab.x1, tab.y1), p.accent);
            } else if ctx.hover > 0.001 {
                let alpha = (ctx.hover * f64::from(0x12)).round() as u8;
                fill_rect(scene, tab, with_alpha(p.text, alpha));
            }
            let label_color = if active {
                with_alpha(p.text, 0xE0)
            } else {
                with_alpha(p.text_muted, 0xC0)
            };
            let cy = bounds.center().y;
            bar(scene, x0 + 16.0 * s, cy - 3.0 * s, label_w.min(tab_w - 40.0) * s, 6.0 * s, label_color);
        }
    }
}

/// The editor canvas. Just the background — real text, gutter, selections, and
/// carets are drawn over this rect by [`crate::TextSystem`], which owns the
/// editor model and shaping.
#[derive(Default)]
pub struct EditorArea;

impl Widget for EditorArea {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        fill_rect(scene, bounds, ctx.palette.background);
    }
}

/// The bottom status bar: a branch chip on the left, position/lang on the right.
#[derive(Default)]
pub struct StatusBar;

impl Widget for StatusBar {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        let s = ctx.scale;
        let p = ctx.palette;
        fill_rect(scene, bounds, p.status_bar);

        let cy = bounds.center().y;
        // Left: brand dot + branch placeholder.
        let dot = 7.0 * s;
        bar(scene, bounds.x0 + 12.0 * s, cy - dot / 2.0, dot, dot, p.accent);
        bar(scene, bounds.x0 + 12.0 * s + dot + 8.0 * s, cy - 2.5 * s, 72.0 * s, 5.0 * s, with_alpha(p.text_muted, 0xB0));

        // Right: position + language placeholders.
        let right = bounds.x1 - 12.0 * s;
        bar(scene, right - 56.0 * s, cy - 2.5 * s, 56.0 * s, 5.0 * s, with_alpha(p.text_muted, 0xB0));
        bar(scene, right - 56.0 * s - 12.0 * s - 40.0 * s, cy - 2.5 * s, 40.0 * s, 5.0 * s, with_alpha(p.text_muted, 0x90));
    }
}
