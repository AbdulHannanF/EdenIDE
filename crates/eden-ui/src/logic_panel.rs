//! Right-side logic panel: AST outline and complexity chart.
//!
//! This widget paints the background surface for the right-side logic panel.
//! Actual text content (symbol outline, complexity indicators) is drawn over
//! this rect by [`crate::TextSystem::paint_logic_panel`].

use vello::Scene;
use vello::kurbo::Rect;

use crate::paint::fill_rect;
use crate::widget::{PaintCtx, Widget};

/// The right-side logic panel showing the AST symbol outline.
///
/// Actual text content is drawn over this by
/// [`crate::TextSystem::paint_logic_panel`].
#[derive(Default)]
pub struct LogicPanel;

impl Widget for LogicPanel {
    fn paint(&self, scene: &mut Scene, bounds: Rect, ctx: &PaintCtx) {
        if bounds.width() < 2.0 {
            return; // collapsed
        }
        let s = ctx.scale;
        let p = ctx.palette;
        // Background
        fill_rect(scene, bounds, p.surface);
        // Left border divider
        fill_rect(
            scene,
            Rect::new(bounds.x0, bounds.y0, bounds.x0 + s.max(1.0), bounds.y1),
            p.divider,
        );
    }
}
