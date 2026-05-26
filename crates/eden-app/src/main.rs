//! Eden — the application binary.
//!
//! Phase 2 ("The Buffer"): the editor canvas now hosts a real, editable text
//! buffer. Typing inserts; arrows move (Shift extends); Backspace/Delete,
//! Home/End, Enter, and Tab behave; Ctrl+Z/Ctrl+Shift+Z undo/redo; Ctrl+A
//! selects all; the mouse wheel scrolls with momentum (spring-driven). Only the
//! visible lines are shaped each frame, so large files stay responsive.
//!
//! Chrome controls move under a modifier so letters type: Ctrl+B toggles the
//! sidebar, Ctrl+T crossfades the theme.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context as _, Result};
use eden_motion::{MotionPrefs, Spring, SpringConfig};
use eden_search::FuzzyMatcher;
use eden_ui::{
    Chrome, Editor, EditorFrame, Highlighter, Highlights, PaletteView, TextSystem, TreeRow,
    TreeView,
};
use eden_workspace::{FileTree, Project};
use vello::kurbo::{Point, Rect};
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const CLEAR: Color = Color::from_rgb8(0xFB, 0xF8, 0xF3);

const SAMPLE: &str = "// Eden — Phase 2: a real, editable buffer.\n\
fn main() {\n\
    let greeting = \"hello, eden\";\n\
    let mut total = 0u64;\n\
    for (i, ch) in greeting.char_indices() {\n\
        total += (i as u64) * (ch as u64);\n\
    }\n\
    println!(\"{greeting}: {total}\");\n\
}\n\
\n\
// Try it: type anywhere, select with Shift+arrows, Ctrl+Z to undo.\n\
// Ctrl+B toggles the sidebar; Ctrl+T crossfades the theme.\n";

fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("eden=info,warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let event_loop = EventLoop::new().context("create event loop")?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new();
    event_loop.run_app(&mut app).context("run event loop")?;
    app.into_result()
}

struct App {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: WindowState,
    scene: Scene,
    chrome: Option<Chrome>,
    text: Option<TextSystem>,
    editor: Editor,
    highlighter: Option<Highlighter>,
    highlights: Highlights,
    doc_dirty: bool,
    project: Project,
    files: Vec<String>,
    fuzzy: FuzzyMatcher,
    palette: Option<PaletteState>,
    tree: FileTree,
    tree_scroll: f64,
    tree_hover: Option<usize>,
    cursor: Option<Point>,
    scroll: Spring,
    prefs: MotionPrefs,
    mods: ModifiersState,
    focused: bool,
    scale: f64,
    ensure_visible: bool,
    last_frame: Instant,
    fatal: Option<anyhow::Error>,
}

enum WindowState {
    Suspended,
    Active(Box<ActiveWindow>),
}

/// State of the open Cmd-P fuzzy file finder.
struct PaletteState {
    query: String,
    /// Indices into `App::files`, ranked best-first.
    results: Vec<usize>,
    selected: usize,
}

struct ActiveWindow {
    window: Arc<Window>,
    surface: RenderSurface<'static>,
}

impl App {
    fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let project = Project::new(root);
        let files = project.file_strings();
        let tree = FileTree::new(project.root());
        Self {
            context: RenderContext::new(),
            renderers: Vec::new(),
            state: WindowState::Suspended,
            scene: Scene::new(),
            chrome: None,
            text: None,
            editor: Editor::from_text(SAMPLE),
            highlighter: Highlighter::rust().ok(),
            highlights: Highlights::default(),
            doc_dirty: true,
            project,
            files,
            fuzzy: FuzzyMatcher::new(),
            palette: None,
            tree,
            tree_scroll: 0.0,
            tree_hover: None,
            cursor: None,
            scroll: Spring::with_config(0.0, SpringConfig::DEFAULT),
            prefs: MotionPrefs::from_env(),
            mods: ModifiersState::empty(),
            focused: true,
            scale: 1.0,
            ensure_visible: false,
            last_frame: Instant::now(),
            fatal: None,
        }
    }

    fn into_result(self) -> Result<()> {
        match self.fatal {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn activate(&mut self, event_loop: &ActiveEventLoop) -> Result<ActiveWindow> {
        let attributes = WindowAttributes::default()
            .with_title("Eden")
            .with_inner_size(LogicalSize::new(1100.0, 720.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("create window")?,
        );

        let size = window.inner_size();
        let surface = pollster::block_on(self.context.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))
        .context("create surface")?;

        self.ensure_renderer(surface.dev_id)?;
        Ok(ActiveWindow { window, surface })
    }

    fn ensure_renderer(&mut self, dev_id: usize) -> Result<()> {
        if self.renderers.len() <= dev_id {
            self.renderers.resize_with(dev_id + 1, || None);
        }
        if self.renderers[dev_id].is_none() {
            let device = &self.context.devices[dev_id].device;
            let renderer = Renderer::new(
                device,
                RendererOptions {
                    use_cpu: false,
                    antialiasing_support: AaSupport::area_only(),
                    num_init_threads: std::num::NonZeroUsize::new(1),
                    pipeline_cache: None,
                },
            )
            .map_err(|e| anyhow::anyhow!("initialise vello renderer: {e:?}"))?;
            self.renderers[dev_id] = Some(renderer);
        }
        Ok(())
    }

    fn request_redraw(&self) {
        if let WindowState::Active(active) = &self.state {
            active.window.request_redraw();
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let WindowState::Active(active) = &mut self.state else {
            return;
        };
        self.context.resize_surface(&mut active.surface, width, height);
        self.scale = active.window.scale_factor();
        if let Some(chrome) = &mut self.chrome {
            chrome.resize(f64::from(width), f64::from(height), self.scale);
        }
        active.window.request_redraw();
    }

    /// Handles a key press. Returns `true` if a repaint is needed.
    fn on_key(&mut self, event: &KeyEvent) -> bool {
        if self.palette.is_some() {
            return self.on_palette_key(event);
        }
        let ctrl = self.mods.control_key() || self.mods.super_key();
        let shift = self.mods.shift_key();
        match &event.logical_key {
            Key::Named(named) => self.on_named_key(*named, shift),
            Key::Character(s) if ctrl => self.on_command(s),
            Key::Character(s) => {
                self.editor.insert(s);
                self.doc_dirty = true;
                self.ensure_visible = true;
                true
            }
            _ => false,
        }
    }

    fn on_named_key(&mut self, named: NamedKey, shift: bool) -> bool {
        match named {
            NamedKey::Enter => self.edit(true, |e| e.insert("\n")),
            NamedKey::Tab => self.edit(true, |e| e.insert("    ")),
            NamedKey::Backspace => self.edit(true, Editor::backspace),
            NamedKey::Delete => self.edit(true, Editor::delete_forward),
            NamedKey::Space => self.edit(true, |e| e.insert(" ")),
            NamedKey::ArrowLeft => self.edit(false, |e| e.move_left(shift)),
            NamedKey::ArrowRight => self.edit(false, |e| e.move_right(shift)),
            NamedKey::ArrowUp => self.edit(false, |e| e.move_up(shift)),
            NamedKey::ArrowDown => self.edit(false, |e| e.move_down(shift)),
            NamedKey::Home => self.edit(false, |e| e.move_line_start(shift)),
            NamedKey::End => self.edit(false, |e| e.move_line_end(shift)),
            NamedKey::PageUp => {
                self.scroll_by_page(-1.0);
                true
            }
            NamedKey::PageDown => {
                self.scroll_by_page(1.0);
                true
            }
            _ => false,
        }
    }

    fn on_command(&mut self, key: &str) -> bool {
        match key {
            "b" | "B" => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.toggle_sidebar();
                }
                true
            }
            "t" | "T" => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.cycle_theme();
                }
                true
            }
            "z" | "Z" => self.edit(true, |e| {
                e.undo();
            }),
            "y" | "Y" => self.edit(true, |e| {
                e.redo();
            }),
            "a" | "A" => self.edit(false, Editor::select_all),
            "p" | "P" => {
                self.open_palette();
                true
            }
            _ => false,
        }
    }

    fn open_palette(&mut self) {
        let results = self.fuzzy.rank("", &self.files);
        self.palette = Some(PaletteState {
            query: String::new(),
            results,
            selected: 0,
        });
    }

    /// Handles a key press while the Cmd-P palette is open.
    fn on_palette_key(&mut self, event: &KeyEvent) -> bool {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.palette = None;
                true
            }
            Key::Named(NamedKey::Enter) => {
                self.open_selected_file();
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                if let Some(p) = &mut self.palette {
                    let count = p.results.len();
                    if count > 0 {
                        p.selected = (p.selected + 1) % count;
                    }
                }
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                if let Some(p) = &mut self.palette {
                    let count = p.results.len();
                    if count > 0 {
                        p.selected = (p.selected + count - 1) % count;
                    }
                }
                true
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(p) = &mut self.palette {
                    p.query.pop();
                }
                self.refilter_palette();
                true
            }
            Key::Named(NamedKey::Space) => {
                if let Some(p) = &mut self.palette {
                    p.query.push(' ');
                }
                self.refilter_palette();
                true
            }
            Key::Character(s) => {
                if let Some(p) = &mut self.palette {
                    p.query.push_str(s);
                }
                self.refilter_palette();
                true
            }
            _ => false,
        }
    }

    fn refilter_palette(&mut self) {
        if let Some(p) = &mut self.palette {
            p.results = self.fuzzy.rank(&p.query, &self.files);
            p.selected = 0;
        }
    }

    /// Opens the file currently selected in the palette into the editor.
    fn open_selected_file(&mut self) {
        let Some(p) = &self.palette else { return };
        let Some(&file_idx) = p.results.get(p.selected) else {
            self.palette = None;
            return;
        };
        let path = self.project.root().join(&self.files[file_idx]);
        self.open_path(&path);
        self.palette = None;
    }

    /// Reads `path` into the editor, resetting highlights and scroll.
    fn open_path(&mut self, path: &std::path::Path) {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                self.editor = Editor::from_text(&contents);
                self.doc_dirty = true;
                self.scroll.jump_to(0.0);
                self.ensure_visible = true;
                tracing::info!(file = %path.display(), "opened");
            }
            Err(err) => tracing::warn!(file = %path.display(), "could not open: {err}"),
        }
    }

    /// The file-tree row at a physical-pixel point, if the cursor is over the
    /// (open) sidebar.
    fn tree_row_at(&self, point: Point) -> Option<usize> {
        let text = self.text.as_ref()?;
        let chrome = self.chrome.as_ref()?;
        let rect = chrome.sidebar_rect();
        if rect.width() < 2.0 || !rect.contains(point) {
            return None;
        }
        let row_h = text.line_height();
        let idx = ((point.y - rect.y0 + self.tree_scroll) / row_h).floor();
        if idx < 0.0 {
            return None;
        }
        let idx = idx as usize;
        (idx < self.tree.entries().len()).then_some(idx)
    }

    /// Handles a left click: toggles a tree directory or opens a tree file.
    fn on_click(&mut self) -> bool {
        let Some(point) = self.cursor else {
            return false;
        };
        let Some(idx) = self.tree_row_at(point) else {
            return false;
        };
        let entry = &self.tree.entries()[idx];
        if entry.is_dir {
            self.tree.toggle(idx);
        } else {
            let path = entry.path.clone();
            self.open_path(&path);
        }
        true
    }

    fn over_sidebar(&self) -> bool {
        let Some(point) = self.cursor else {
            return false;
        };
        self.chrome.as_ref().is_some_and(|c| {
            let rect = c.sidebar_rect();
            rect.width() > 2.0 && rect.contains(point)
        })
    }

    /// Runs an editor action and schedules a scroll-to-caret. `mutates` marks
    /// the document dirty so highlights are recomputed before the next paint.
    fn edit(&mut self, mutates: bool, action: impl FnOnce(&mut Editor)) -> bool {
        action(&mut self.editor);
        if mutates {
            self.doc_dirty = true;
        }
        self.ensure_visible = true;
        true
    }

    /// Recomputes syntax highlights if the document changed since last paint.
    fn refresh_highlights(&mut self) {
        if !self.doc_dirty {
            return;
        }
        self.doc_dirty = false;
        if let Some(highlighter) = &mut self.highlighter {
            // Full reparse on change. tree-sitter is fast for typical files;
            // incremental edits (InputEdit) are a documented follow-up.
            let source = self.editor.buffer().to_string();
            self.highlights = Highlights::new(highlighter.highlight(&source));
        }
    }

    fn scroll_by_page(&mut self, pages: f64) {
        let view_h = self
            .chrome
            .as_ref()
            .map_or(400.0, |c| c.editor_rect().height());
        self.scroll.set_target(self.scroll.target() + pages * view_h * 0.9);
    }

    /// Clamps the scroll target to the valid range for the current content.
    fn clamp_scroll(&mut self) {
        let (Some(text), Some(chrome)) = (&self.text, &self.chrome) else {
            return;
        };
        let max = (self.editor.buffer().len_lines() as f64 * text.line_height()
            - chrome.editor_rect().height())
        .max(0.0);
        let clamped = self.scroll.target().clamp(0.0, max);
        if (clamped - self.scroll.target()).abs() > f64::EPSILON {
            self.scroll.set_target(clamped);
        }
    }

    /// Scrolls just enough to bring the primary caret into view.
    fn ensure_caret_visible(&mut self) {
        if !self.ensure_visible {
            return;
        }
        self.ensure_visible = false;
        let (Some(text), Some(chrome)) = (&self.text, &self.chrome) else {
            return;
        };
        let line_h = text.line_height();
        let view_h = chrome.editor_rect().height();
        let caret_line = self.editor.buffer().char_to_line(self.editor.primary().head) as f64;
        let caret_top = caret_line * line_h;
        let mut target = self.scroll.target();
        if caret_top < target {
            target = caret_top;
        } else if caret_top + line_h > target + view_h {
            target = caret_top + line_h - view_h;
        }
        let max = (self.editor.buffer().len_lines() as f64 * line_h - view_h).max(0.0);
        self.scroll.set_target(target.clamp(0.0, max));
    }

    fn render(&mut self) -> Result<()> {
        self.refresh_highlights();
        self.clamp_scroll();
        self.ensure_caret_visible();

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        let chrome_moving = self.chrome.as_mut().is_some_and(|c| c.step(dt));
        let scroll_moving = self.scroll.step(dt);
        let animating = chrome_moving || scroll_moving;

        let WindowState::Active(active) = &mut self.state else {
            return Ok(());
        };
        let width = active.surface.config.width;
        let height = active.surface.config.height;
        let dev_id = active.surface.dev_id;

        use wgpu::CurrentSurfaceTexture as Acquired;
        let surface_texture = match active.surface.surface.get_current_texture() {
            Acquired::Success(texture) | Acquired::Suboptimal(texture) => texture,
            Acquired::Timeout | Acquired::Occluded => return Ok(()),
            Acquired::Outdated | Acquired::Lost => {
                self.context.configure_surface(&active.surface);
                active.window.request_redraw();
                return Ok(());
            }
            Acquired::Validation => anyhow::bail!("surface texture validation error"),
        };

        self.scene.reset();
        if let Some(chrome) = &mut self.chrome {
            chrome.paint(&mut self.scene);
        }
        if let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome) {
            let rect = chrome.sidebar_rect();
            if rect.width() > 2.0 {
                let palette = chrome.palette();
                let rows: Vec<TreeRow<'_>> = self
                    .tree
                    .entries()
                    .iter()
                    .map(|e| TreeRow {
                        name: &e.name,
                        depth: e.depth,
                        is_dir: e.is_dir,
                        expanded: e.expanded,
                    })
                    .collect();
                self.tree_scroll = text.paint_file_tree(
                    &mut self.scene,
                    rect,
                    &TreeView {
                        rows: &rows,
                        scroll_px: self.tree_scroll,
                        hovered: self.tree_hover,
                    },
                    &palette,
                    self.scale,
                );
            }
        }
        if let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome) {
            let palette = chrome.palette();
            let syntax = chrome.syntax();
            text.paint_editor(
                &mut self.scene,
                &EditorFrame {
                    area: chrome.editor_rect(),
                    editor: &self.editor,
                    palette: &palette,
                    syntax: &syntax,
                    highlights: &self.highlights,
                    scroll_px: self.scroll.value(),
                    scale: self.scale,
                    show_caret: self.focused,
                },
            );
        }
        if let Some(state) = &self.palette
            && let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome)
        {
            let entries: Vec<String> =
                state.results.iter().take(12).map(|&i| self.files[i].clone()).collect();
            let screen = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
            text.paint_palette(
                &mut self.scene,
                screen,
                &PaletteView {
                    query: &state.query,
                    entries: &entries,
                    selected: state.selected,
                },
                &chrome.palette(),
                self.scale,
            );
        }

        let device_handle = &self.context.devices[dev_id];
        let renderer = self.renderers[dev_id]
            .as_mut()
            .context("no renderer for surface device")?;

        renderer
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &self.scene,
                &active.surface.target_view,
                &RenderParams {
                    base_color: CLEAR,
                    width,
                    height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| anyhow::anyhow!("vello render: {e:?}"))?;

        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("eden.surface_blit"),
                });
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: Some(active.surface.format),
                ..Default::default()
            });
        active.surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &active.surface.target_view,
            &surface_view,
        );
        device_handle.queue.submit([encoder.finish()]);
        active.window.pre_present_notify();
        surface_texture.present();

        if animating {
            active.window.request_redraw();
        }
        Ok(())
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, err: anyhow::Error) {
        tracing::error!("{err:#}");
        if self.fatal.is_none() {
            self.fatal = Some(err);
        }
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, WindowState::Active(_)) {
            return;
        }
        let active = match self.activate(event_loop) {
            Ok(active) => active,
            Err(err) => return self.fail(event_loop, err),
        };

        self.scale = active.window.scale_factor();
        if self.chrome.is_none() {
            let size = active.window.inner_size();
            match Chrome::new(
                f64::from(size.width.max(1)),
                f64::from(size.height.max(1)),
                self.scale,
                self.prefs,
            ) {
                Ok(chrome) => self.chrome = Some(chrome),
                Err(err) => return self.fail(event_loop, anyhow::anyhow!("build chrome: {err}")),
            }
        }
        if self.text.is_none() {
            self.text = Some(TextSystem::new(self.scale));
        }

        self.last_frame = Instant::now();
        active.window.request_redraw();
        self.state = WindowState::Active(Box::new(active));
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.state = WindowState::Suspended;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.mods = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let point = Point::new(position.x, position.y);
                self.cursor = Some(point);
                if let Some(chrome) = &mut self.chrome {
                    chrome.set_hover(Some(point));
                }
                self.tree_hover = self.tree_row_at(point);
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
                self.tree_hover = None;
                if let Some(chrome) = &mut self.chrome {
                    chrome.set_hover(None);
                }
                self.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let changed = self.on_click();
                if changed {
                    self.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let line_h = self.text.as_ref().map_or(20.0, TextSystem::line_height);
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y) * line_h * 3.0,
                    MouseScrollDelta::PixelDelta(p) => p.y,
                };
                if self.over_sidebar() {
                    self.tree_scroll = (self.tree_scroll - dy).max(0.0);
                } else {
                    self.scroll.set_target(self.scroll.target() - dy);
                }
                self.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    key_event @ KeyEvent {
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                let changed = self.on_key(&key_event);
                if changed {
                    self.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render() {
                    self.fail(event_loop, err);
                }
            }
            _ => {}
        }
    }
}
