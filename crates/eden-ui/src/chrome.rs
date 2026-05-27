//! The editor chrome: the window shell — demo strip, top bar, left rail,
//! sidebar, editor canvas, logic panel, and status bar — laid out with taffy,
//! themed, and animated.

use eden_motion::{MotionPrefs, Spring, SpringConfig};
use eden_theme::{Palette, Syntax, Theme};
use taffy::prelude::{auto, length, percent};
use taffy::{
    AvailableSpace, Display, FlexDirection, NodeId, Size as TaffySize, Style, TaffyTree,
};
use vello::Scene;
use vello::kurbo::{Point, Rect};

use crate::logic_panel::LogicPanel;
use crate::paint::fill_rect;
use crate::panels::{DemoStrip, EditorArea, LeftRail, SidebarPanel, StatusBar, TabStrip,
                    TerminalPanel, TopBar};
use crate::widget::{PaintCtx, Widget};

// design: logical sizes on the 4px grid.
/// Demo strip height — 28px as per the claude-design prototype.
const DEMO_H: f64 = 28.0;
/// Top bar height — 36px single row (glyph + breadcrumb + chrome btns).
const TITLE_H: f64 = 36.0;
/// Left activity rail — 48px, always visible.
const RAIL_W: f64 = 48.0;
/// Tab strip height.
const TAB_H: f64 = 32.0;
/// Status bar height.
const STATUS_H: f64 = 24.0;
/// Sidebar / file-tree width.
const SIDEBAR_W: f64 = 240.0;
/// Logic / minimap panel width.
const LOGIC_W: f64 = 180.0;
/// Terminal panel height.
const TERMINAL_H: f64 = 220.0;

// ── DemoScreen ────────────────────────────────────────────────────────────────

/// Which of the nine prototype screens is currently shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DemoScreen {
    /// Welcome / landing screen (01).
    Welcome,
    /// Onboarding / setup guide (02).
    Onboarding,
    /// Main code editor — the default screen (03).
    #[default]
    Editor,
    /// Spatial canvas / multi-file layout (04).
    Spatial,
    /// Command/file palette overlay (05).
    Palette,
    /// Embedded terminal (06).
    Terminal,
    /// Debugger panel (07).
    Debugger,
    /// AI Pair programming panel (08).
    AiPair,
    /// Editor settings (09).
    Settings,
}

impl DemoScreen {
    /// Short label shown in the demo strip tab.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Welcome    => "Welcome",
            Self::Onboarding => "Onboarding",
            Self::Editor     => "Editor",
            Self::Spatial    => "Spatial",
            Self::Palette    => "Palette",
            Self::Terminal   => "Terminal",
            Self::Debugger   => "Debugger",
            Self::AiPair     => "AI Pair",
            Self::Settings   => "Settings",
        }
    }

    /// Two-digit number shown in the demo strip tab.
    #[must_use]
    pub fn number(self) -> &'static str {
        match self {
            Self::Welcome    => "01",
            Self::Onboarding => "02",
            Self::Editor     => "03",
            Self::Spatial    => "04",
            Self::Palette    => "05",
            Self::Terminal   => "06",
            Self::Debugger   => "07",
            Self::AiPair     => "08",
            Self::Settings   => "09",
        }
    }

    /// All nine screens in order.
    pub fn all() -> [Self; 9] {
        [
            Self::Welcome, Self::Onboarding, Self::Editor,
            Self::Spatial, Self::Palette,    Self::Terminal,
            Self::Debugger, Self::AiPair,    Self::Settings,
        ]
    }
}

// ── Region ───────────────────────────────────────────────────────────────────

/// A logical region of the chrome, used for hit-testing and hover routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// Demo-screen strip (28px, top).
    DemoStrip,
    /// Top bar (36px, glyph + breadcrumb + chrome btns).
    TitleBar,
    /// Left activity rail (48px).
    LeftRail,
    /// File-tree sidebar (spring width).
    Sidebar,
    /// Tab strip above the editor.
    TabStrip,
    /// Editor canvas.
    EditorArea,
    /// Embedded terminal panel.
    Terminal,
    /// Status bar.
    StatusBar,
    /// Logic / minimap panel.
    LogicPanel,
}

impl Region {
    fn is_interactive(self) -> bool {
        matches!(
            self,
            Region::DemoStrip
                | Region::Sidebar
                | Region::TabStrip
                | Region::Terminal
                | Region::LogicPanel
                | Region::LeftRail
        )
    }
}

/// What needs recomputing before the next paint.
#[derive(Clone, Copy, Debug, Default)]
struct Invalidation {
    layout: bool,
    paint: bool,
}

/// Node handles into the taffy tree.
struct Nodes {
    root:       NodeId,
    demo_strip: NodeId,
    title_bar:  NodeId,
    body:       NodeId,
    left_rail:  NodeId,
    sidebar:    NodeId,
    main_col:   NodeId,
    tab_strip:  NodeId,
    editor:     NodeId,
    terminal:   NodeId,
    status_bar: NodeId,
    logic_panel: NodeId,
}

/// The editor chrome.
pub struct Chrome {
    tree:   TaffyTree,
    nodes:  Nodes,
    width:  f64,
    height: f64,
    scale:  f64,
    prefs:  MotionPrefs,

    sidebar:       Spring,
    sidebar_open:  bool,

    themes:        Vec<Theme>,
    active_theme:  usize,
    prev_palette:  Palette,
    prev_syntax:   Syntax,
    theme_mix:     Spring,

    hover:         Spring,
    hover_anchor:  Option<Region>,

    terminal_h:    Spring,
    terminal_open: bool,

    logic_panel_spring: Spring,
    logic_panel_open:   bool,

    focus_halo:    Spring,

    /// Currently active demo screen.
    demo_screen: DemoScreen,

    invalid: Invalidation,

    demo_strip:     DemoStrip,
    title_bar:      TopBar,
    left_rail_panel: LeftRail,
    sidebar_panel:  SidebarPanel,
    tab_strip:      TabStrip,
    editor:         EditorArea,
    terminal_panel: TerminalPanel,
    status:         StatusBar,
    logic_panel:    LogicPanel,
}

impl Chrome {
    /// Builds the chrome for the given physical size and scale.
    ///
    /// # Errors
    /// Returns a `taffy::TaffyError` on allocation failure.
    pub fn new(
        width: f64,
        height: f64,
        scale: f64,
        prefs: MotionPrefs,
    ) -> Result<Self, taffy::TaffyError> {
        let mut tree = TaffyTree::new();
        let leaf = |tree: &mut TaffyTree| tree.new_leaf(Style::default());

        let demo_strip = leaf(&mut tree)?;
        let title_bar  = leaf(&mut tree)?;
        let left_rail  = leaf(&mut tree)?;
        let sidebar    = leaf(&mut tree)?;
        let tab_strip  = leaf(&mut tree)?;
        let editor     = leaf(&mut tree)?;
        let terminal   = leaf(&mut tree)?;
        let status_bar = leaf(&mut tree)?;
        let logic_panel = leaf(&mut tree)?;

        let main_col = tree.new_with_children(
            Style::default(), &[tab_strip, editor, terminal])?;
        // Body row: left_rail | sidebar | main_col | logic_panel
        let body = tree.new_with_children(
            Style::default(), &[left_rail, sidebar, main_col, logic_panel])?;
        let root = tree.new_with_children(
            Style::default(), &[demo_strip, title_bar, body, status_bar])?;

        let themes = Theme::builtins().to_vec();
        let initial        = themes[0].palette;
        let initial_syntax = themes[0].syntax;

        let mut chrome = Self {
            tree,
            nodes: Nodes {
                root, demo_strip, title_bar, body,
                left_rail, sidebar, main_col,
                tab_strip, editor, terminal, status_bar, logic_panel,
            },
            width,
            height,
            scale,
            prefs,
            sidebar: Spring::new(SIDEBAR_W * scale),
            sidebar_open: true,
            themes,
            active_theme: 0,
            prev_palette: initial,
            prev_syntax: initial_syntax,
            theme_mix: Spring::with_config(1.0, prefs.resolve(SpringConfig::UNIT)),
            hover: Spring::with_config(0.0, prefs.resolve(SpringConfig::SNAPPY)),
            hover_anchor: None,
            terminal_h: Spring::with_config(0.0, prefs.resolve(SpringConfig::DEFAULT)),
            terminal_open: false,
            logic_panel_spring: Spring::new(LOGIC_W * scale),
            logic_panel_open: true,
            focus_halo: Spring::with_config(0.0, prefs.resolve(SpringConfig::DEFAULT)),
            demo_screen: DemoScreen::default(),
            invalid: Invalidation { layout: true, paint: true },
            demo_strip: DemoStrip,
            title_bar: TopBar,
            left_rail_panel: LeftRail,
            sidebar_panel: SidebarPanel,
            tab_strip: TabStrip,
            editor: EditorArea,
            terminal_panel: TerminalPanel,
            status: StatusBar,
            logic_panel: LogicPanel,
        };
        chrome.apply_styles();
        Ok(chrome)
    }

    // ── theme ─────────────────────────────────────────────────────────────

    /// The number of built-in themes available.
    #[must_use]
    pub fn theme_count(&self) -> usize { self.themes.len() }

    /// The name of the currently active theme.
    #[must_use]
    pub fn active_theme_name(&self) -> &str { &self.themes[self.active_theme].name }

    /// The name of the theme at `index`, or `None` if out of range.
    #[must_use]
    pub fn theme_name(&self, index: usize) -> Option<&str> {
        self.themes.get(index).map(|t| t.name.as_str())
    }

    /// The index of the currently active theme.
    #[must_use]
    pub fn active_theme_index(&self) -> usize { self.active_theme }

    /// Advances to the next built-in theme, crossfading the palette.
    pub fn cycle_theme(&mut self) {
        self.prev_palette = self.displayed_palette();
        self.prev_syntax  = self.displayed_syntax();
        self.active_theme = (self.active_theme + 1) % self.themes.len();
        self.theme_mix = Spring::with_config(0.0, self.prefs.resolve(SpringConfig::UNIT));
        if self.prefs.reduce_motion {
            self.theme_mix.jump_to(1.0);
        } else {
            self.theme_mix.set_target(1.0);
        }
        self.invalid.paint = true;
    }

    // ── screen routing ────────────────────────────────────────────────────

    /// The currently active demo screen.
    #[must_use]
    pub fn demo_screen(&self) -> DemoScreen { self.demo_screen }

    /// Switches to a new demo screen, repainting.
    pub fn set_demo_screen(&mut self, screen: DemoScreen) {
        self.demo_screen = screen;
        self.invalid.paint = true;
    }

    // ── size & sidebar ────────────────────────────────────────────────────

    /// Updates the window size and scale.
    pub fn resize(&mut self, width: f64, height: f64, scale: f64) {
        self.width  = width;
        self.height = height;
        if (scale - self.scale).abs() > f64::EPSILON {
            self.scale = scale;
            self.sidebar.jump_to(self.sidebar_target());
            self.logic_panel_spring.jump_to(self.logic_panel_target());
        }
        self.invalid.layout = true;
        self.invalid.paint  = true;
    }

    /// Toggles the sidebar open/closed.
    pub fn toggle_sidebar(&mut self) {
        self.sidebar_open = !self.sidebar_open;
        let target = self.sidebar_target();
        if self.prefs.reduce_motion {
            self.sidebar.jump_to(target);
        } else {
            self.sidebar.set_target(target);
        }
        self.invalid.layout = true;
        self.invalid.paint  = true;
    }

    /// Toggles the logic panel open/closed.
    pub fn toggle_logic_panel(&mut self) {
        self.logic_panel_open = !self.logic_panel_open;
        let target = self.logic_panel_target();
        if self.prefs.reduce_motion {
            self.logic_panel_spring.jump_to(target);
        } else {
            self.logic_panel_spring.set_target(target);
        }
        self.invalid.layout = true;
        self.invalid.paint  = true;
    }

    /// Toggles the terminal panel open/closed.
    pub fn toggle_terminal(&mut self) {
        self.terminal_open = !self.terminal_open;
        let target = self.terminal_target();
        if self.prefs.reduce_motion {
            self.terminal_h.jump_to(target);
        } else {
            self.terminal_h.set_target(target);
        }
        self.invalid.layout = true;
        self.invalid.paint  = true;
    }

    /// Whether the terminal panel is currently open.
    #[must_use]
    pub fn terminal_open(&self) -> bool { self.terminal_open }

    // ── hover / typing ────────────────────────────────────────────────────

    /// Updates the hovered region from a cursor position.
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
        if matches!(
            region,
            Some(
                Region::Sidebar
                    | Region::TabStrip
                    | Region::TitleBar
                    | Region::StatusBar
                    | Region::LogicPanel
                    | Region::DemoStrip
                    | Region::LeftRail
            )
        ) {
            self.focus_halo.set_target(0.0);
        }
        self.invalid.paint = true;
    }

    /// Called when the user types. Activates the focus halo.
    pub fn enter_typing(&mut self) {
        self.focus_halo.set_target(1.0);
        self.invalid.paint = true;
    }

    /// The current focus-halo intensity (0 = chrome visible, 1 = dimmed).
    #[must_use]
    pub fn focus_halo_value(&self) -> f64 { self.focus_halo.value() }

    // ── animation ─────────────────────────────────────────────────────────

    /// Steps all animations by `dt` seconds. Returns `true` while animating.
    pub fn step(&mut self, dt: f64) -> bool {
        let mut animating = false;
        if self.sidebar.step(dt) {
            animating = true;
            self.invalid.layout = true;
            self.invalid.paint  = true;
        }
        if self.theme_mix.step(dt) {
            animating = true;
            self.invalid.paint = true;
        }
        if self.hover.step(dt) {
            animating = true;
            self.invalid.paint = true;
        }
        if self.terminal_h.step(dt) {
            animating = true;
            self.invalid.layout = true;
            self.invalid.paint  = true;
        }
        if self.logic_panel_spring.step(dt) {
            animating = true;
            self.invalid.layout = true;
            self.invalid.paint  = true;
        }
        if self.focus_halo.step(dt) {
            animating = true;
            self.invalid.paint = true;
        }
        animating
    }

    /// Whether a repaint is pending.
    #[must_use]
    pub fn needs_paint(&self) -> bool { self.invalid.paint }

    /// The palette currently being displayed (interpolated mid-crossfade).
    #[must_use]
    pub fn palette(&self) -> Palette { self.displayed_palette() }

    /// The syntax colours currently being displayed.
    #[must_use]
    pub fn syntax(&self) -> Syntax { self.displayed_syntax() }

    // ── rect accessors ────────────────────────────────────────────────────

    /// The absolute rect of the top bar (glyph + breadcrumb row).
    #[must_use]
    pub fn title_bar_rect(&self) -> Rect { self.region_rect(Region::TitleBar) }

    /// The absolute rect of the editor canvas.
    #[must_use]
    pub fn editor_rect(&self) -> Rect { self.region_rect(Region::EditorArea) }

    /// The absolute rect of the sidebar.
    #[must_use]
    pub fn sidebar_rect(&self) -> Rect { self.region_rect(Region::Sidebar) }

    /// The absolute rect of the tab strip.
    #[must_use]
    pub fn tab_strip_rect(&self) -> Rect { self.region_rect(Region::TabStrip) }

    /// The absolute rect of the terminal panel.
    #[must_use]
    pub fn terminal_rect(&self) -> Rect { self.region_rect(Region::Terminal) }

    /// The absolute rect of the status bar.
    #[must_use]
    pub fn status_bar_rect(&self) -> Rect { self.region_rect(Region::StatusBar) }

    /// The absolute rect of the logic panel.
    #[must_use]
    pub fn logic_panel_rect(&self) -> Rect { self.region_rect(Region::LogicPanel) }

    /// The absolute rect of the demo strip.
    #[must_use]
    pub fn demo_strip_rect(&self) -> Rect { self.region_rect(Region::DemoStrip) }

    /// The absolute rect of the left rail.
    #[must_use]
    pub fn left_rail_rect(&self) -> Rect { self.region_rect(Region::LeftRail) }

    fn region_rect(&self, want: Region) -> Rect {
        self.regions()
            .into_iter()
            .find(|(region, _)| *region == want)
            .map_or(Rect::ZERO, |(_, rect)| rect)
    }

    // ── hit testing ───────────────────────────────────────────────────────

    /// Hit-tests a physical-pixel point to the region containing it.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<Region> {
        self.regions()
            .into_iter()
            .find(|(_, rect)| rect.contains(point))
            .map(|(region, _)| region)
    }

    // ── paint ─────────────────────────────────────────────────────────────

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

        // Focus halo: dim sidebar + tab strip + logic panel when typing.
        let halo = self.focus_halo.value();
        if halo > 0.01 {
            let alpha = (halo * 96.0) as u8;
            let dim = crate::paint::to_rgba8_alpha(palette.background, alpha);
            let sidebar_r  = self.region_rect(Region::Sidebar);
            let tab_r      = self.region_rect(Region::TabStrip);
            let logic_r    = self.region_rect(Region::LogicPanel);
            let rail_r     = self.region_rect(Region::LeftRail);
            for rect in [sidebar_r, tab_r, logic_r, rail_r] {
                if rect.width() > 1.0 && rect.height() > 1.0 {
                    fill_rect(scene, rect, dim);
                }
            }
        }

        self.invalid.paint = false;
    }

    // ── internals ─────────────────────────────────────────────────────────

    fn sidebar_target(&self) -> f64 {
        if self.sidebar_open { SIDEBAR_W * self.scale } else { 0.0 }
    }

    fn terminal_target(&self) -> f64 {
        if self.terminal_open { TERMINAL_H * self.scale } else { 0.0 }
    }

    fn logic_panel_target(&self) -> f64 {
        if self.logic_panel_open { LOGIC_W * self.scale } else { 0.0 }
    }

    fn displayed_palette(&self) -> Palette {
        self.prev_palette.lerp(self.themes[self.active_theme].palette, self.theme_mix.value())
    }

    fn displayed_syntax(&self) -> Syntax {
        self.prev_syntax.lerp(self.themes[self.active_theme].syntax, self.theme_mix.value())
    }

    fn hover_for(&self, region: Region) -> f64 {
        if self.hover_anchor == Some(region) { self.hover.value() } else { 0.0 }
    }

    fn widget(&self, region: Region) -> &dyn Widget {
        match region {
            Region::DemoStrip  => &self.demo_strip,
            Region::TitleBar   => &self.title_bar,
            Region::LeftRail   => &self.left_rail_panel,
            Region::Sidebar    => &self.sidebar_panel,
            Region::TabStrip   => &self.tab_strip,
            Region::EditorArea => &self.editor,
            Region::Terminal   => &self.terminal_panel,
            Region::StatusBar  => &self.status,
            Region::LogicPanel => &self.logic_panel,
        }
    }

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
            size: TaffySize { width: percent(1.0_f32), height: auto() },
            ..Style::default()
        };
        let left_rail_style = Style {
            size: TaffySize {
                width: length((RAIL_W * s) as f32),
                height: percent(1.0_f32),
            },
            flex_shrink: 0.0,
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
            size: TaffySize { width: auto(), height: percent(1.0_f32) },
            ..Style::default()
        };
        let editor_style = Style {
            flex_grow: 1.0,
            size: TaffySize { width: percent(1.0_f32), height: auto() },
            ..Style::default()
        };
        let terminal_h_px = self.terminal_h.value().max(0.0) as f32;
        let terminal_style = Style {
            size: TaffySize {
                width: percent(1.0_f32),
                height: length(terminal_h_px),
            },
            flex_shrink: 0.0,
            ..Style::default()
        };
        let logic_panel_style = Style {
            size: TaffySize {
                width: length(self.logic_panel_spring.value().max(0.0) as f32),
                height: percent(1.0_f32),
            },
            flex_shrink: 0.0,
            ..Style::default()
        };

        let _ = self.tree.set_style(self.nodes.root,        root_style);
        let _ = self.tree.set_style(self.nodes.demo_strip,  row(DEMO_H));
        let _ = self.tree.set_style(self.nodes.title_bar,   row(TITLE_H));
        let _ = self.tree.set_style(self.nodes.body,        body_style);
        let _ = self.tree.set_style(self.nodes.left_rail,   left_rail_style);
        let _ = self.tree.set_style(self.nodes.sidebar,     sidebar_style);
        let _ = self.tree.set_style(self.nodes.main_col,    main_style);
        let _ = self.tree.set_style(self.nodes.tab_strip,   row(TAB_H));
        let _ = self.tree.set_style(self.nodes.editor,      editor_style);
        let _ = self.tree.set_style(self.nodes.terminal,    terminal_style);
        let _ = self.tree.set_style(self.nodes.status_bar,  row(STATUS_H));
        let _ = self.tree.set_style(self.nodes.logic_panel, logic_panel_style);

        let _ = self.tree.compute_layout(
            self.nodes.root,
            TaffySize {
                width:  AvailableSpace::Definite(self.width as f32),
                height: AvailableSpace::Definite(self.height as f32),
            },
        );
    }

    /// Computes absolute rects for each painted region.
    fn regions(&self) -> [(Region, Rect); 9] {
        let loc = |node: NodeId| {
            self.tree.layout(node)
                .map(|l| (f64::from(l.location.x), f64::from(l.location.y),
                          f64::from(l.size.width),  f64::from(l.size.height)))
                .unwrap_or((0.0, 0.0, 0.0, 0.0))
        };
        let rect = |ox: f64, oy: f64, t: (f64, f64, f64, f64)| {
            Rect::new(ox + t.0, oy + t.1, ox + t.0 + t.2, oy + t.1 + t.3)
        };

        let demo  = loc(self.nodes.demo_strip);
        let title = loc(self.nodes.title_bar);
        let body  = loc(self.nodes.body);
        let status = loc(self.nodes.status_bar);

        let rail    = loc(self.nodes.left_rail);
        let sidebar = loc(self.nodes.sidebar);
        let main    = loc(self.nodes.main_col);
        let tab     = loc(self.nodes.tab_strip);
        let editor  = loc(self.nodes.editor);
        let terminal = loc(self.nodes.terminal);
        let logic   = loc(self.nodes.logic_panel);

        let body_ox = body.0;
        let body_oy = body.1;
        let main_ox = body_ox + main.0;
        let main_oy = body_oy + main.1;

        [
            (Region::DemoStrip,  rect(0.0, 0.0, demo)),
            (Region::TitleBar,   rect(0.0, 0.0, title)),
            (Region::LeftRail,   rect(body_ox, body_oy, rail)),
            (Region::Sidebar,    rect(body_ox, body_oy, sidebar)),
            (Region::TabStrip,   rect(main_ox, main_oy, tab)),
            (Region::EditorArea, rect(main_ox, main_oy, editor)),
            (Region::Terminal,   rect(main_ox, main_oy, terminal)),
            (Region::StatusBar,  rect(0.0, 0.0, status)),
            (Region::LogicPanel, rect(body_ox, body_oy, logic)),
        ]
    }

    fn paint_dividers(&self, scene: &mut Scene, palette: &Palette) {
        let t = self.scale.max(1.0);
        let regions = self.regions();
        let find = |want: Region| {
            regions.iter().find(|(r, _)| *r == want).map(|(_, rc)| *rc)
        };
        let (
            Some(demo), Some(title), Some(sidebar), Some(tab),
            Some(terminal), Some(status), Some(logic), Some(rail),
        ) = (
            find(Region::DemoStrip), find(Region::TitleBar),
            find(Region::Sidebar),   find(Region::TabStrip),
            find(Region::Terminal),  find(Region::StatusBar),
            find(Region::LogicPanel), find(Region::LeftRail),
        ) else {
            return;
        };

        // Under the demo strip.
        fill_rect(scene,
            Rect::new(0.0, demo.y1 - t, self.width, demo.y1),
            palette.divider);
        // Under the top bar.
        fill_rect(scene,
            Rect::new(0.0, title.y1 - t, self.width, title.y1),
            palette.divider);
        // Above the status bar.
        fill_rect(scene,
            Rect::new(0.0, status.y0, self.width, status.y0 + t),
            palette.divider);
        // Under the tab strip.
        fill_rect(scene,
            Rect::new(tab.x0, tab.y1 - t, tab.x1 + logic.width(), tab.y1),
            palette.divider);
        // Right border of the left rail.
        fill_rect(scene,
            Rect::new(rail.x1 - t, title.y1, rail.x1, status.y0),
            palette.divider);
        // Right border of the sidebar (when visible).
        if sidebar.width() > 2.0 {
            fill_rect(scene,
                Rect::new(sidebar.x1 - t, title.y1, sidebar.x1, status.y0),
                palette.divider);
        }
        // Above the terminal panel.
        if terminal.height() > 2.0 {
            fill_rect(scene,
                Rect::new(terminal.x0, terminal.y0, terminal.x1, terminal.y0 + t),
                palette.divider);
        }
        // Left border of the logic panel.
        if logic.width() > 2.0 {
            fill_rect(scene,
                Rect::new(logic.x0, title.y1, logic.x0 + t, status.y0),
                palette.divider);
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
        assert_eq!(regions.len(), 9);

        let demo = regions.iter().find(|(r, _)| *r == Region::DemoStrip).unwrap().1;
        assert_eq!(demo.x0, 0.0);
        assert!((demo.width() - 1200.0).abs() < 0.5);
        assert!((demo.height() - DEMO_H).abs() < 0.5);

        let title = regions.iter().find(|(r, _)| *r == Region::TitleBar).unwrap().1;
        assert_eq!(title.x0, 0.0);
        assert!((title.width() - 1200.0).abs() < 0.5);
        assert!((title.height() - TITLE_H).abs() < 0.5);

        let status = regions.iter().find(|(r, _)| *r == Region::StatusBar).unwrap().1;
        assert!((status.y1 - 800.0).abs() < 0.5);
        assert!((status.height() - STATUS_H).abs() < 0.5);

        let rail = regions.iter().find(|(r, _)| *r == Region::LeftRail).unwrap().1;
        assert!((rail.width() - RAIL_W).abs() < 0.5);

        let sidebar = regions.iter().find(|(r, _)| *r == Region::Sidebar).unwrap().1;
        let editor  = regions.iter().find(|(r, _)| *r == Region::EditorArea).unwrap().1;
        assert!(editor.x0 >= sidebar.x1 - 0.5);
        assert!((sidebar.width() - SIDEBAR_W).abs() < 0.5);

        let terminal = regions.iter().find(|(r, _)| *r == Region::Terminal).unwrap().1;
        assert!(terminal.height() < 0.5);

        let logic = regions.iter().find(|(r, _)| *r == Region::LogicPanel).unwrap().1;
        assert!((logic.width() - LOGIC_W).abs() < 0.5);
        assert!(logic.x0 >= editor.x1 - 0.5);

        let tab = regions.iter().find(|(r, _)| *r == Region::TabStrip).unwrap().1;
        assert!((tab.height() - TAB_H).abs() < 0.5);
    }

    #[test]
    fn demo_screen_round_trips() {
        let mut c = chrome();
        assert_eq!(c.demo_screen(), DemoScreen::Editor);
        c.set_demo_screen(DemoScreen::Welcome);
        assert_eq!(c.demo_screen(), DemoScreen::Welcome);
        c.set_demo_screen(DemoScreen::Settings);
        assert_eq!(c.demo_screen(), DemoScreen::Settings);
    }
}
