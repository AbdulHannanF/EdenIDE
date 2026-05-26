//! Low-level paint helpers over vello's [`Scene`].

use eden_theme::Rgba8;
use vello::Scene;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill};

/// Converts a theme colour into a vello brush colour.
#[must_use]
pub fn to_color(color: Rgba8) -> Color {
    Color::new(color.channels_f32())
}

/// Fills an axis-aligned rectangle with a solid colour.
pub fn fill_rect(scene: &mut Scene, rect: Rect, color: Rgba8) {
    scene.fill(Fill::NonZero, Affine::IDENTITY, to_color(color), None, &rect);
}

/// Fills a rounded rectangle with a uniform corner radius.
pub fn fill_rrect(scene: &mut Scene, rect: Rect, radius: f64, color: Rgba8) {
    let rounded = rect.to_rounded_rect(radius);
    scene.fill(Fill::NonZero, Affine::IDENTITY, to_color(color), None, &rounded);
}
