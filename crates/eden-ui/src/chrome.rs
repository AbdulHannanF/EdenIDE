//! The editor chrome: the empty shell of title bar, sidebar, tab strip, editor
//! canvas, and status bar — laid out with taffy, themed, and animated.

use eden_motion::{MotionPrefs, Spring, SpringConfig};
use eden_theme::{Palette, Theme};
use taffy::prelude::{auto, length, percent};
use taffy::{
    AvailableSpace, Display, FlexDirection, NodeId, Size as TaffySize, Style, TaffyTree,
};
use vello::Scene;
use vello::kurbo::{Point, Rect};

use crate::paint::fill_rect;
use crate::panels::{EditorArea, SidebarPanel, StatusBar, TabStrip, TitleBar};
use crate::widget::{PaintCtx, Widget};

// design: logical sizes (multiplied by the display scale). Heights and the
// sidebar width are on the 4px grid (§6); the open sidebar is a comfortable
// reading width for a file tree.
const TITLE_H: f64 = 38.0;
const TAB_H: f64 = 36.0;
const STATUS_H: f64 = 26.0;
const SIDEBAR_W: f64 = 248.0;

/// A logical region of the chrome. Used for hit testing and to route hover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// The top bar.
    TitleBar,
    /// The left sidebar.
    Sidebar,
    /// The tab strip above the editor.
    TabStrip,
    /// The editor canvas.
    EditorArea,
    /// The bottom status bar.
    StatusBar,
}

impl Region {
    /// Whether hovering this region produces a visible response.
    fn is_interactive(self) -> bool {
        matches!(self, Region::Sidebar | Region::TabStrip)
    }
}

/// What needs recomputing before the next paint.
///
/// vello rebuilds the whole scene each frame, so sub-rectangle damage isn't
/// actionable for *painting*; the useful granularity here is whether the taffy
/// layout must be recomputed (it must not on a hover-only change) and whether a
/// repaint is needed at all (with `ControlFlow::Wait` we only draw on demand).
#[derive(Clone, Copy, Debug, Default)]
struct Invalidation {
    layout: bool,
    paint: bool,
}

/// Node handles into the taffy tree.
struct Nodes {
    root: NodeId,
    title_bar: NodeId,
    body: NodeId,
    sidebar: NodeId,
    main_col: NodeId,
    tab_strip: NodeId,
    editor: NodeId,
    status_bar: NodeId,
}

/// The editor chrome. Owns the layout tree, the theme crossfade, and the
/// animation springs, and knows how to paint itself and hit-test a point.
pub struct Chrome {
    tree: TaffyTree,
    nodes: Nodes,
    width: f64,
    height: f64,
    scale: f64,
    prefs: MotionPrefs,

    sidebar: Spring,
    sidebar_open: bool,

    themes: Vec<Theme>,
    active_theme: usize,
    prev_palette: Palette,
    theme_mix: Spring,

    hover: Spring,
    hover_anchor: Option<Region>,

    invalid: Invalidation,

    title_bar: TitleBar,
    sidebar_panel: SidebarPanel,
    tab_strip: TabStrip,
    editor: EditorArea,
    status: StatusBar,
}

impl Chrome {
    /// Builds the chrome for a window of the given physical size and scale.
    ///
    /// # Errors
    ///
    /// Returns a [`taffy::TaffyError`] if the layout tree cannot be built (this
    /// only happens on allocation failure in practice).
    pub fn new(
        width: f64,
        height: f64,
        scale: f64,
        prefs: MotionPrefs,
    ) -> Result<Self, taffy::TaffyError> {
        let mut tree = TaffyTree::new();
        let leaf = |tree: &mut TaffyTree| tree.new_leaf(Style::default());
        let title_bar = leaf(&mut tree)?;
        let sidebar = leaf(&mut tree)?;
        let tab_strip = leaf(&mut tree)?;
        let editor = leaf(&mut tree)?;
        let status_bar = leaf(&mut tree)?;
        let main_col = tree.new_with_children(Style::default(), &[tab_strip, editor])?;
        let body = tree.new_with_children(Style::default(), &[sidebar, main_col])?;
        let root = tree.new_with_children(Style::default(), &[title_bar, body, status_bar])?;

        let themes = Theme::builtins().to_vec();
        let day = themes[0].palette;

        let mut chrome = Self {
            tree,
            nodes: Nodes {
                root,
                title_bar,
                body,
                sidebar,
                main_col,
                tab_strip,
                editor,
                status_bar,
            },
            width,
            height,
            scale,
            prefs,
            sidebar: Spring::new(SIDEBAR_W * scale),
            sidebar_open: true,
            themes,
            active_theme: 0,
            prev_palette: day,
            // At rest at 1.0 so the displayed palette is exactly the active theme.
            theme_mix: Spring::with_config(1.0, prefs.resolve(SpringConfig::UNIT)),
            hover: Spring::with_config(0.0, prefs.resolve(SpringConfig::SNAPPY)),
            hover_anchor: None,
            invalid: Invalidation { layout: true, paint: true },
            title_bar: TitleBar,
            sidebar_panel: SidebarPanel,
            tab_strip: TabStrip,
            editor: EditorArea,
            status: StatusBar,
        };
        chrome.apply_styles();
        Ok(chrome)
    }

    /// The number of built-in themes available to cycle through.
    #[must_use]
    pub fn theme_count(&self) -> usize {
        self.themes.len()
    }

    /// The name of the currently active theme.
    #[must_use]
    pub fn active_theme_name(&self) -> &str {
        &self.themes[self.active_theme].name
    }

    /// Updates the window size and scale, relaying out on the next paint.
    pub fn resize(&mut self, width: f64, height: f64, scale: f64) {
        self.width = width;
        self.height = height;
        if (scale - self.scale).abs() > f64::EPSILON {
            self.scale = scale;
            // Re-anchor the sidebar at the new scale without animating the jump.
            self.sidebar.jump_to(self.sidebar_target());
        }
        self.invalid.layout = true;
        self.invalid.paint = true;
    }

    /// Toggles the sidebar open/closed, animating its width.
    pub fn toggle_sidebar(&mut self) {
        self.sidebar_open = !self.sidebar_open;
        let target = self.sidebar_target();
        if self.prefs.reduce_motion {
            self.sidebar.jump_to(target);
        } else {
            self.sidebar.set_target(target);
        }
        self.invalid.layout = true;
        self.invalid.paint = true;
    }

    /// Advances to the next built-in theme, crossfading the palette.
    pub fn cycle_theme(&mut self) {
        self.prev_palette = self.displayed_palette();
        self.active_theme = (self.active_theme + 1) % self.themes.len();
        self.theme_mix = Spring::with_config(0.0, self.prefs.resolve(SpringConfig::UNIT));
        if self.prefs.reduce_motion {
            self.theme_mix.jump_to(1.0);
        } else {
            self.theme_mix.set_target(1.0);
        }
        self.invalid.paint = true;
    }

    /// Updates the hovered region from a cursor position (physical pixels), or
    /// clears hover when the cursor leaves the window.
    pub fn set_hover(&mut self, position: Option<Point>) {
        let region = position.and_then(|p| self.hit_test(p));
        match region.filter(|r| r.is_interactive()) {
            Some(r) => {
                if self.hover_anchor != Some(r) {
                    self.hover.jump_to(0.0);
                    self.hover_anchor = Some(r);
                }
                self.hover.set_target(1.0);
            }
            None => self.hover.set_target(0.0),
        }
        self.invalid.paint = true;
    }

    /// Steps all animations by `dt` seconds. Returns `true` while any animation
    /// is still in motion, so the caller can schedule another frame.
    pub fn step(&mut self, dt: f64) -> bool {
        let mut animating = false;
        if self.sidebar.step(dt) {
            animating = true;
            self.invalid.layout = true;
            self.invalid.paint = true;
        }
        if self.theme_mix.step(dt) {
            animating = true;
            self.invalid.paint = true;
        }
        if self.hover.step(dt) {
            animating = true;
            self.invalid.paint = true;
        }
        animating
    }

    /// Whether a repaint is pending (from input or animation).
    #[must_use]
    pub fn needs_paint(&self) -> bool {
        self.invalid.paint
    }

    /// Hit-tests a physical-pixel point to the region containing it.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<Region> {
        self.regions()
            .into_iter()
            .find(|(_, rect)| rect.contains(point))
            .map(|(region, _)| region)
    }

    /// Paints the chrome into `scene`, recomputing layout first if needed.
    pub fn paint(&mut self, scene: &mut Scene) {
        if self.invalid.layout {
            self.apply_styles();
            self.invalid.layout = false;
        }
        let palette = self.displayed_palette();
        let regions = self.regions();

        // Base canvas.
        fill_rect(scene, Rect::new(0.0, 0.0, self.width, self.height), palette.background);

        // Panels.
        for (region, rect) in regions {
            let ctx = PaintCtx {
                palette: &palette,
                scale: self.scale,
                hover: self.hover_for(region),
            };
            self.widget(region).paint(scene, rect, &ctx);
        }

        self.paint_dividers(scene, &palette);
        self.invalid.paint = false;
    }

    // --- internals -------------------------------------------------------

    fn sidebar_target(&self) -> f64 {
        if self.sidebar_open {
            SIDEBAR_W * self.scale
        } else {
            0.0
        }
    }

    fn displayed_palette(&self) -> Palette {
        self.prev_palette
            .lerp(self.themes[self.active_theme].palette, self.theme_mix.value())
    }

    fn hover_for(&self, region: Region) -> f64 {
        if self.hover_anchor == Some(region) {
            self.hover.value()
        } else {
            0.0
        }
    }

    fn widget(&self, region: Region) -> &dyn Widget {
        match region {
            Region::TitleBar => &self.title_bar,
            Region::Sidebar => &self.sidebar_panel,
            Region::TabStrip => &self.tab_strip,
            Region::EditorArea => &self.editor,
            Region::StatusBar => &self.status,
        }
    }

    /// Pushes current styles (driven by size, scale, and the sidebar spring)
    /// into the taffy tree and recomputes the layout.
    fn apply_styles(&mut self) {
        let s = self.scale;
        let row = |height: f64| Style {
            size: TaffySize {
                width: percent(1.0_f32),
                height: length((height * s) as f32),
            },
            flex_shrink: 0.0,
            ..Style::default()
        };

        let root_style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            size: TaffySize {
                width: length(self.width as f32),
                height: length(self.height as f32),
            },
            ..Style::default()
        };
        let body_style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_grow: 1.0,
            size: TaffySize {
                width: percent(1.0_f32),
                height: auto(),
            },
            ..Style::default()
        };
        let sidebar_style = Style {
            size: TaffySize {
                width: length(self.sidebar.value().max(0.0) as f32),
                height: percent(1.0_f32),
            },
            flex_shrink: 0.0,
            ..Style::default()
        };
        let main_style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            size: TaffySize {
                width: auto(),
                height: percent(1.0_f32),
            },
            ..Style::default()
        };
        let editor_style = Style {
            flex_grow: 1.0,
            size: TaffySize {
                width: percent(1.0_f32),
                height: auto(),
            },
            ..Style::default()
        };

        // Errors here only occur for invalid node ids, which cannot happen.
        let _ = self.tree.set_style(self.nodes.root, root_style);
        let _ = self.tree.set_style(self.nodes.title_bar, row(TITLE_H));
        let _ = self.tree.set_style(self.nodes.body, body_style);
        let _ = self.tree.set_style(self.nodes.sidebar, sidebar_style);
        let _ = self.tree.set_style(self.nodes.main_col, main_style);
        let _ = self.tree.set_style(self.nodes.tab_strip, row(TAB_H));
        let _ = self.tree.set_style(self.nodes.editor, editor_style);
        let _ = self.tree.set_style(self.nodes.status_bar, row(STATUS_H));

        let _ = self.tree.compute_layout(
            self.nodes.root,
            TaffySize {
                width: AvailableSpace::Definite(self.width as f32),
                height: AvailableSpace::Definite(self.height as f32),
            },
        );
    }

    /// Computes absolute rects for each painted region from the taffy layout.
    fn regions(&self) -> [(Region, Rect); 5] {
        let loc = |node: NodeId| {
            self.tree
                .layout(node)
                .map(|l| (f64::from(l.location.x), f64::from(l.location.y), f64::from(l.size.width), f64::from(l.size.height)))
                .unwrap_or((0.0, 0.0, 0.0, 0.0))
        };
        let rect = |ox: f64, oy: f64, t: (f64, f64, f64, f64)| {
            Rect::new(ox + t.0, oy + t.1, ox + t.0 + t.2, oy + t.1 + t.3)
        };

        let title = loc(self.nodes.title_bar);
        let body = loc(self.nodes.body);
        let status = loc(self.nodes.status_bar);
        let sidebar = loc(self.nodes.sidebar);
        let main = loc(self.nodes.main_col);
        let tab = loc(self.nodes.tab_strip);
        let editor = loc(self.nodes.editor);

        let body_ox = body.0;
        let body_oy = body.1;
        let main_ox = body_ox + main.0;
        let main_oy = body_oy + main.1;

        [
            (Region::TitleBar, rect(0.0, 0.0, title)),
            (Region::Sidebar, rect(body_ox, body_oy, sidebar)),
            (Region::TabStrip, rect(main_ox, main_oy, tab)),
            (Region::EditorArea, rect(main_ox, main_oy, editor)),
            (Region::StatusBar, rect(0.0, 0.0, status)),
        ]
    }

    fn paint_dividers(&self, scene: &mut Scene, palette: &Palette) {
        let t = self.scale.max(1.0); // ~1 logical px
        let regions = self.regions();
        let find = |want: Region| regions.iter().find(|(r, _)| *r == want).map(|(_, rc)| *rc);
        let (Some(title), Some(sidebar), Some(tab), Some(status)) = (
            find(Region::TitleBar),
            find(Region::Sidebar),
            find(Region::TabStrip),
            find(Region::StatusBar),
        ) else {
            return;
        };

        // Under the title bar.
        fill_rect(scene, Rect::new(0.0, title.y1 - t, self.width, title.y1), palette.divider);
        // Above the status bar.
        fill_rect(scene, Rect::new(0.0, status.y0, self.width, status.y0 + t), palette.divider);
        // Under the tab strip.
        fill_rect(scene, Rect::new(tab.x0, tab.y1 - t, self.width, tab.y1), palette.divider);
        // Between sidebar and main column (only when the sidebar is showing).
        if sidebar.width() > 2.0 {
            fill_rect(scene, Rect::new(sidebar.x1 - t, title.y1, sidebar.x1, status.y0), palette.divider);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome() -> Chrome {
        Chrome::new(1200.0, 800.0, 1.0, MotionPrefs::default()).expect("build chrome")
    }

    #[test]
    fn regions_tile_the_window() {
        let c = chrome();
        let regions = c.regions();
        // Title spans full width at the top.
        let title = regions.iter().find(|(r, _)| *r == Region::TitleBar).unwrap().1;
        assert_eq!(title.x0, 0.0);
        assert!((title.width() - 1200.0).abs() < 0.5);
        // Status sits at the bottom.
        let status = regions.iter().find(|(r, _)| *r == Region::StatusBar).unwrap().1;
        assert!((status.y1 - 800.0).abs() < 0.5);
        // Editor is to the right of the sidebar.
        let sidebar = regions.iter().find(|(r, _)| *r == Region::Sidebar).unwrap().1;
        let editor = regions.iter().find(|(r, _)| *r == Region::EditorArea).unwrap().1;
        assert!(editor.x0 >= sidebar.x1 - 0.5);
        assert!((sidebar.width() - SIDEBAR_W).abs() < 0.5);
    }

    #[test]
    fn hit_test_resolves_regions() {
        let c = chrome();
        assert_eq!(c.hit_test(Point::new(600.0, 5.0)), Some(Region::TitleBar));
        assert_eq!(c.hit_test(Point::new(10.0, 400.0)), Some(Region::Sidebar));
        assert_eq!(c.hit_test(Point::new(600.0, 790.0)), Some(Region::StatusBar));
        assert_eq!(c.hit_test(Point::new(600.0, 400.0)), Some(Region::EditorArea));
    }

    #[test]
    fn collapsing_sidebar_animates_then_frees_space() {
        let mut c = chrome();
        c.toggle_sidebar();
        // Drive the animation to completion.
        let mut frames = 0;
        while c.step(1.0 / 60.0) {
            frames += 1;
            assert!(frames < 600);
        }
        c.apply_styles();
        let sidebar = c.regions().iter().find(|(r, _)| *r == Region::Sidebar).unwrap().1;
        assert!(sidebar.width() < 0.5, "sidebar did not collapse: {}", sidebar.width());
    }

    #[test]
    fn theme_cycling_wraps() {
        let mut c = chrome();
        let count = c.theme_count();
        let first = c.active_theme_name().to_owned();
        for _ in 0..count {
            c.cycle_theme();
        }
        assert_eq!(c.active_theme_name(), first);
    }
}
