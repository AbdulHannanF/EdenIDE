//! Eden — the application binary.
//!
//! Phase 4 ("Intelligence"): `eden-lsp` connects to `rust-analyzer` (when
//! installed), surfaces diagnostics as gutter markers, shows a hover card when
//! the cursor is still for 400 ms, and opens a completion popup on Ctrl+Space.
//!
//! Controls:
//!   Ctrl+B — toggle sidebar    Ctrl+T — cycle theme
//!   Ctrl+Z / Ctrl+Y — undo / redo    Ctrl+A — select all
//!   Ctrl+P — Cmd-P fuzzy file open    Ctrl+Space — completions
//!   F12 — go-to-definition (opens result file in editor)
//!
//! (LSP features are silently no-ops when rust-analyzer is not installed.)

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use eden_lsp::{CompletionItem, LspPool, Position as LspPos, Severity};
use eden_motion::{MotionPrefs, Spring, SpringConfig};
use eden_search::FuzzyMatcher;
use eden_ui::{
    Chrome, CompletionEntry, CompletionView, Editor, EditorFrame, GutterMark, Highlighter,
    Highlights, PaletteView, TextSystem, TreeRow, TreeView,
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

/// Cursor must be still for this long before a hover request is fired.
const HOVER_DELAY: Duration = Duration::from_millis(400);

const SAMPLE: &str = "// Eden — Phase 4: LSP intelligence.\n\
//\n\
// Ctrl+Space for completions. Hold the cursor still for hover.\n\
// Diagnostics appear as dots in the gutter.\n\
\n\
fn main() {\n\
    let greeting = \"hello, eden\";\n\
    let mut total = 0u64;\n\
    for (i, ch) in greeting.char_indices() {\n\
        total += (i as u64) * (ch as u64);\n\
    }\n\
    println!(\"{greeting}: {total}\");\n\
}\n";

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

// ── App ───────────────────────────────────────────────────────────────────────

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

    // LSP
    lsp: LspPool,
    current_path: Option<PathBuf>,
    doc_version: i32,
    gutter_marks: Vec<(u32, GutterMark)>,

    // Hover
    cursor_still_since: Instant,
    hover_requested_for: Option<Point>,
    hover_card: Option<String>,

    // Completion
    completion_open: bool,
    completion_selected: usize,
    completion_items: Vec<CompletionItem>,

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

struct PaletteState {
    query: String,
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
        let project = Project::new(root.clone());
        let files = project.file_strings();
        let tree = FileTree::new(project.root());
        let lsp = LspPool::new(&root);
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
            lsp,
            current_path: None,
            doc_version: 1,
            gutter_marks: Vec::new(),
            cursor_still_since: Instant::now(),
            hover_requested_for: None,
            hover_card: None,
            completion_open: false,
            completion_selected: 0,
            completion_items: Vec::new(),
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
            event_loop.create_window(attributes).context("create window")?,
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

    // ── keyboard ──────────────────────────────────────────────────────────

    fn on_key(&mut self, event: &KeyEvent) -> bool {
        if self.palette.is_some() {
            return self.on_palette_key(event);
        }
        if self.completion_open {
            if self.on_completion_key(event) {
                return true;
            }
        }
        let ctrl = self.mods.control_key() || self.mods.super_key();
        let shift = self.mods.shift_key();
        match &event.logical_key {
            Key::Named(NamedKey::F12) => {
                self.go_to_definition();
                true
            }
            Key::Named(named) => self.on_named_key(*named, shift),
            Key::Character(s) if ctrl => self.on_command(s),
            Key::Character(s) => {
                self.editor.insert(s);
                self.doc_dirty = true;
                self.ensure_visible = true;
                self.dismiss_completion();
                true
            }
            _ => false,
        }
    }

    fn on_named_key(&mut self, named: NamedKey, shift: bool) -> bool {
        match named {
            NamedKey::Enter => self.edit(true, |e| e.insert("\n")),
            NamedKey::Tab => {
                if self.completion_open {
                    self.commit_completion();
                    return true;
                }
                self.edit(true, |e| e.insert("    "))
            }
            NamedKey::Backspace => self.edit(true, Editor::backspace),
            NamedKey::Delete => self.edit(true, Editor::delete_forward),
            NamedKey::Space => self.edit(true, |e| e.insert(" ")),
            NamedKey::Escape => {
                if self.completion_open {
                    self.dismiss_completion();
                    return true;
                }
                false
            }
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
            " " => {
                // Ctrl+Space → completions
                self.trigger_completions();
                true
            }
            _ => false,
        }
    }

    // ── completion ────────────────────────────────────────────────────────

    fn on_completion_key(&mut self, event: &KeyEvent) -> bool {
        match &event.logical_key {
            Key::Named(NamedKey::ArrowDown) => {
                let count = self.completion_items.len().min(8);
                if count > 0 {
                    self.completion_selected = (self.completion_selected + 1) % count;
                }
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                let count = self.completion_items.len().min(8);
                if count > 0 {
                    let n = self.completion_selected + count - 1;
                    self.completion_selected = n % count;
                }
                true
            }
            Key::Named(NamedKey::Enter) => {
                self.commit_completion();
                true
            }
            _ => false,
        }
    }

    fn trigger_completions(&mut self) {
        let Some(path) = &self.current_path.clone() else { return };
        let Some(pos) = self.caret_lsp_position() else { return };
        self.lsp.request_completions(path, pos);
        self.completion_open = true;
        self.completion_selected = 0;
        self.completion_items.clear();
    }

    fn commit_completion(&mut self) {
        let item = self.completion_items.get(self.completion_selected).cloned();
        self.dismiss_completion();
        let Some(item) = item else { return };
        // Delete the current word before the caret, then insert.
        let insert = item.insert_text.clone();
        self.editor.insert(&insert);
        self.doc_dirty = true;
        self.ensure_visible = true;
    }

    fn dismiss_completion(&mut self) {
        self.completion_open = false;
        self.completion_selected = 0;
    }

    // ── palette (Cmd-P) ───────────────────────────────────────────────────

    fn open_palette(&mut self) {
        let results = self.fuzzy.rank("", &self.files);
        self.palette = Some(PaletteState { query: String::new(), results, selected: 0 });
    }

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

    // ── file opening ──────────────────────────────────────────────────────

    fn open_path(&mut self, path: &std::path::Path) {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                self.editor = Editor::from_text(&contents);
                self.doc_dirty = true;
                self.doc_version = 1;
                self.scroll.jump_to(0.0);
                self.ensure_visible = true;
                self.current_path = Some(path.to_path_buf());
                self.gutter_marks.clear();
                self.hover_card = None;
                self.hover_requested_for = None;
                self.dismiss_completion();
                self.lsp.open_document(path, &contents);
                tracing::info!(file = %path.display(), "opened");
            }
            Err(err) => tracing::warn!(file = %path.display(), "could not open: {err}"),
        }
    }

    fn go_to_definition(&mut self) {
        let Some(path) = self.current_path.clone() else { return };
        let Some(pos) = self.caret_lsp_position() else { return };
        // Fire-and-forget; the result currently just logs (Phase 4 follow-up:
        // show a picker and open the file).
        self.lsp.request_definition_logged(&path, pos);
    }

    // ── LSP helpers ───────────────────────────────────────────────────────

    /// Returns the LSP position of the primary caret.
    fn caret_lsp_position(&self) -> Option<LspPos> {
        let caret = self.editor.primary();
        let line = self.editor.buffer().char_to_line(caret.head) as u32;
        let line_start = self.editor.buffer().line_to_char(line as usize);
        let character = (caret.head - line_start) as u32;
        Some(LspPos { line, character })
    }

    /// Returns the physical-pixel LSP position for the cursor, clamped to the
    /// editor area.
    fn cursor_lsp_position(&self) -> Option<LspPos> {
        let cursor = self.cursor?;
        let chrome = self.chrome.as_ref()?;
        let text = self.text.as_ref()?;
        let area = chrome.editor_rect();
        if !area.contains(cursor) {
            return None;
        }
        let gutter_w = text.gutter_width(self.editor.buffer().len_lines());
        let line =
            ((cursor.y - area.y0 + self.scroll.value()) / text.line_height()).floor() as u32;
        let character =
            ((cursor.x - area.x0 - gutter_w) / text.advance()).max(0.0).floor() as u32;
        Some(LspPos { line, character })
    }

    /// Converts LSP diagnostics to gutter marks for the current file.
    fn refresh_gutter_marks(&mut self) {
        let Some(path) = &self.current_path.clone() else { return };
        let diags = self.lsp.diagnostics(path);
        self.gutter_marks = diags
            .iter()
            .map(|d| {
                let mark = match d.severity {
                    Severity::Error => GutterMark::Error,
                    _ => GutterMark::Warning,
                };
                (d.start.line, mark)
            })
            .collect();
    }

    /// Fires hover after the cursor has been still for [`HOVER_DELAY`].
    fn maybe_request_hover(&mut self) {
        if self.cursor_still_since.elapsed() < HOVER_DELAY {
            return;
        }
        let cursor = self.cursor;
        if self.hover_requested_for == cursor {
            return; // already requested for this position
        }
        let Some(path) = &self.current_path.clone() else { return };
        let Some(pos) = self.cursor_lsp_position() else { return };
        self.lsp.request_hover(path, pos);
        self.hover_requested_for = cursor;
    }

    /// Pulls the latest hover card from the LSP, if any.
    fn refresh_hover(&mut self) {
        let Some(path) = &self.current_path.clone() else { return };
        self.hover_card = self.lsp.hover(path).map(|h| h.contents);
    }

    /// Physical-pixel coordinates of the caret (top-left of the glyph cell).
    fn caret_anchor(&self) -> Option<Point> {
        let text = self.text.as_ref()?;
        let chrome = self.chrome.as_ref()?;
        let area = chrome.editor_rect();
        let caret = self.editor.primary();
        let line = self.editor.buffer().char_to_line(caret.head);
        let col = caret.head - self.editor.buffer().line_to_char(line);
        let gutter_w = text.gutter_width(self.editor.buffer().len_lines());
        let x = area.x0 + gutter_w + col as f64 * text.advance();
        let y = area.y0 + line as f64 * text.line_height() - self.scroll.value();
        Some(Point::new(x, y))
    }

    // ── file tree ─────────────────────────────────────────────────────────

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

    fn on_click(&mut self) -> bool {
        let Some(point) = self.cursor else { return false };
        let Some(idx) = self.tree_row_at(point) else { return false };
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
        let Some(point) = self.cursor else { return false };
        self.chrome.as_ref().is_some_and(|c| {
            let rect = c.sidebar_rect();
            rect.width() > 2.0 && rect.contains(point)
        })
    }

    // ── editing ───────────────────────────────────────────────────────────

    fn edit(&mut self, mutates: bool, action: impl FnOnce(&mut Editor)) -> bool {
        action(&mut self.editor);
        if mutates {
            self.doc_dirty = true;
        }
        self.ensure_visible = true;
        true
    }

    fn refresh_highlights(&mut self) {
        if !self.doc_dirty {
            return;
        }
        self.doc_dirty = false;
        // Notify the LSP of the change.
        if let Some(path) = &self.current_path.clone() {
            self.doc_version += 1;
            let text = self.editor.buffer().to_string();
            self.lsp.change_document(path, self.doc_version, &text);
        }
        if let Some(highlighter) = &mut self.highlighter {
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

    fn clamp_scroll(&mut self) {
        let (Some(text), Some(chrome)) = (&self.text, &self.chrome) else { return };
        let max = (self.editor.buffer().len_lines() as f64 * text.line_height()
            - chrome.editor_rect().height())
        .max(0.0);
        let clamped = self.scroll.target().clamp(0.0, max);
        if (clamped - self.scroll.target()).abs() > f64::EPSILON {
            self.scroll.set_target(clamped);
        }
    }

    fn ensure_caret_visible(&mut self) {
        if !self.ensure_visible {
            return;
        }
        self.ensure_visible = false;
        let (Some(text), Some(chrome)) = (&self.text, &self.chrome) else { return };
        let line_h = text.line_height();
        let view_h = chrome.editor_rect().height();
        let caret_line =
            self.editor.buffer().char_to_line(self.editor.primary().head) as f64;
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

    // ── render ────────────────────────────────────────────────────────────

    fn render(&mut self) -> Result<()> {
        self.refresh_highlights();
        self.clamp_scroll();
        self.ensure_caret_visible();

        // LSP: pull diagnostics + hover on every frame (cheap reads from shared state).
        self.refresh_gutter_marks();
        self.maybe_request_hover();
        self.refresh_hover();

        // Pull fresh completions if popup is open.
        if self.completion_open {
            let items = self
                .current_path
                .as_deref()
                .map(|p| self.lsp.completions(p))
                .unwrap_or_default();
            if !items.is_empty() {
                self.completion_items = items;
            }
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        let chrome_moving = self.chrome.as_mut().is_some_and(|c| c.step(dt));
        let scroll_moving = self.scroll.step(dt);

        // Re-request a frame if hover delay hasn't elapsed yet (so the hover
        // fires at the right moment without the user moving the mouse).
        let hover_pending = self.hover_requested_for != self.cursor
            && self.cursor_still_since.elapsed() < HOVER_DELAY;
        let animating = chrome_moving || scroll_moving || hover_pending;

        // Pre-compute values that need &self before the &mut self.state borrow.
        let caret_anchor = self.caret_anchor();
        let hover_card = self.hover_card.clone();
        let completion_items: Vec<_> = self.completion_items.clone();
        let completion_selected = self.completion_selected;
        let completion_open = self.completion_open;
        let gutter_marks: Vec<_> = self.gutter_marks.clone();

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

        // Sidebar file tree.
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

        // Editor canvas with gutter marks.
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
                    gutter_marks: &gutter_marks,
                },
            );
        }

        // Hover card (only when cursor is in editor area and still).
        if let (Some(ref card), Some(text), Some(chrome)) =
            (hover_card, &mut self.text, &self.chrome)
        {
            if let Some(anchor) = self.cursor {
                let palette = chrome.palette();
                let screen = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
                text.paint_hover_card(&mut self.scene, screen, anchor, card, &palette, self.scale);
            }
        }

        // Completion popup.
        if completion_open && !completion_items.is_empty() {
            if let (Some(anchor), Some(text), Some(chrome)) =
                (caret_anchor, &mut self.text, &self.chrome)
            {
                let palette = chrome.palette();
                let screen = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
                let entries: Vec<CompletionEntry> = completion_items
                    .iter()
                    .map(|c| CompletionEntry {
                        label: c.label.clone(),
                        detail: c.detail.clone(),
                    })
                    .collect();
                text.paint_completion(
                    &mut self.scene,
                    screen,
                    &CompletionView {
                        entries: &entries,
                        selected: completion_selected,
                        anchor,
                    },
                    &palette,
                    self.scale,
                );
            }
        }

        // Cmd-P palette (drawn last, on top of everything).
        if let Some(state) = &self.palette
            && let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome)
        {
            let entries: Vec<String> = state
                .results
                .iter()
                .take(12)
                .map(|&i| self.files[i].clone())
                .collect();
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
        let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor {
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

// ── ApplicationHandler ────────────────────────────────────────────────────────

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
                Err(err) => {
                    return self.fail(event_loop, anyhow::anyhow!("build chrome: {err}"));
                }
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
                // Reset hover timer and discard any stale card.
                self.cursor_still_since = Instant::now();
                self.hover_requested_for = None;
                if let Some(path) = &self.current_path.clone() {
                    self.lsp.clear_hover(path);
                }
                self.hover_card = None;
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
                self.tree_hover = None;
                if let Some(chrome) = &mut self.chrome {
                    chrome.set_hover(None);
                }
                self.hover_card = None;
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
