//! Eden — the application binary.
//!
//! Phase 6 ("Signature Features"):
//!   Ambient Compile    — soft bloom behind diagnostic lines
//!   Focus Halo         — sidebar/tab dim while typing, breathe back on hover
//!   Whisper Palette    — NL intent strings for Ctrl+Shift+P commands
//!   Time Scrubber      — horizontal undo-history bar (Ctrl+Shift+H)
//!   Semantic Minimap   — syntax-coloured minimap overlay (Ctrl+M)
//!   Choreographed Diff — ghost caret on large jumps + go-to-def navigation
//!
//! All earlier controls still apply; additions:
//!   Ctrl+M             — toggle semantic minimap
//!   Ctrl+Shift+H       — toggle time scrubber
//!   F12                — go to definition (now navigates)

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use crossbeam_channel::Receiver;
use eden_lsp::{CompletionItem, LspPool, Position as LspPos, Severity};
use eden_motion::{MotionPrefs, Spring, SpringConfig};
use eden_search::{FuzzyMatcher, SearchHit, SearchQuery, search_project};
use eden_terminal::TerminalBackend;
use eden_theme::Rgba8;
use eden_ui::{
    Chrome, CmdEntry, CmdPaletteView, CompletionEntry, CompletionView, DiffMark, Editor,
    EditorFrame, GutterMark, Highlighter, Highlights, MinimapView, PaletteView, SearchPanelView,
    SearchRowView, ScrubberView, SettingsToggle, SettingsView, StatusBarView, TerminalView,
    TextSystem, TreeRow, TreeView, fill_rrect,
};
use eden_vcs::{DiffKind, GitRepo};
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

/// Full period of the caret's sine-wave brightness pulse (§7.6).
const CARET_PERIOD: f64 = 1.1;

// ── project root helpers ──────────────────────────────────────────────────────

/// Detects the workspace root in priority order:
/// 1. A directory path passed as the first CLI argument (`eden /path/to/project`).
/// 2. Walking up from the executable until `Cargo.toml` or `.git` is found —
///    prevents opening `target/debug/` when the binary is run directly from
///    inside the build output directory.
/// 3. The process's current working directory.
fn detect_project_root() -> PathBuf {
    // Priority 1: explicit path argument.
    if let Some(arg) = std::env::args().nth(1) {
        let p = PathBuf::from(&arg);
        if p.is_dir() {
            return p.canonicalize().unwrap_or(p);
        }
    }
    // Priority 2: walk up from the executable to find a workspace root.
    if let Ok(exe) = std::env::current_exe() {
        let mut candidate = exe.parent().map(|p| p.to_path_buf());
        while let Some(dir) = candidate {
            if dir.join("Cargo.toml").exists() || dir.join(".git").exists() {
                return dir;
            }
            candidate = dir.parent().map(|p| p.to_path_buf());
        }
    }
    // Priority 3: current working directory (normal `cargo run` case).
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Known text-file extensions that are safe to open in the editor.
const TEXT_EXTENSIONS: &[&str] = &[
    "rs", "toml", "md", "txt", "json", "yaml", "yml", "ts", "tsx", "js", "jsx",
    "py", "go", "c", "h", "cpp", "cc", "cxx", "css", "html", "htm", "sh",
    "lock", "env", "xml", "gitignore", "editorconfig", "cfg", "ini", "csv",
];

/// Returns `true` if the file looks like renderable text — either by having a
/// known extension or by passing a UTF-8 sniff of the first 512 bytes. Binary
/// artifacts (`.rlib`, `.d`, `.rmeta`, etc.) return `false`.
fn is_text_path(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str())
        && TEXT_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    {
        return true;
    }
    // Sniff: read the first 512 bytes and check that they are valid UTF-8.
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let mut buf = [0u8; 512];
    use std::io::Read as _;
    let n = file.read(&mut buf).unwrap_or(0);
    std::str::from_utf8(&buf[..n]).is_ok()
}

/// Returns `true` if `path` lives inside the workspace `target/` directory.
/// Used to block accidental display of build artifacts when the file tree
/// root detection falls back to a stale working directory.
fn is_in_target_dir(path: &std::path::Path, project_root: &std::path::Path) -> bool {
    path.strip_prefix(project_root)
        .map(|rel| rel.starts_with("target"))
        .unwrap_or(false)
}

/// Maps a file extension to a language identifier for syntax highlighting.
/// Returns `None` for languages not yet wired (they render as plain text).
fn language_for_path(path: &std::path::Path) -> Option<&'static str> {
    let ext = path.extension().and_then(|e| e.to_str())?.to_lowercase();
    match ext.as_str() {
        "rs" => Some("rust"),
        _ => None,
    }
}

/// The line-comment token for a file's language (Ctrl+/). Defaults to `"// "`.
fn comment_token_for_path(path: Option<&std::path::Path>) -> &'static str {
    let ext = path
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "py" | "sh" | "toml" | "yaml" | "yml" | "cfg" | "ini" | "gitignore" | "env" => "# ",
        "sql" | "lua" => "-- ",
        _ => "// ",
    }
}

// ─────────────────────────────────────────────────────────────────────────────

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

// ── command roster ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandId {
    ToggleSidebar,
    CycleTheme,
    OpenFilePalette,
    ProjectSearch,
    CommandPalette,
    ToggleTerminal,
    Undo,
    Redo,
    SelectAll,
    GoToDefinition,
    TriggerCompletions,
    ToggleMinimap,
    ToggleTimeScrubber,
}

struct Command {
    id: CommandId,
    label: &'static str,
    shortcut: Option<&'static str>,
    /// Natural-language intent phrases used by the Whisper Palette for NL
    /// matching when the label itself does not match the query.
    intent: &'static [&'static str],
}

fn built_in_commands() -> Vec<Command> {
    vec![
        Command {
            id: CommandId::ToggleSidebar,
            label: "Toggle Sidebar",
            shortcut: Some("Ctrl+B"),
            intent: &["show sidebar", "hide sidebar", "file tree", "explorer", "files panel"],
        },
        Command {
            id: CommandId::CycleTheme,
            label: "Cycle Theme",
            shortcut: Some("Ctrl+T"),
            intent: &["change theme", "switch theme", "dark mode", "light mode", "color scheme"],
        },
        Command {
            id: CommandId::OpenFilePalette,
            label: "Open File…",
            shortcut: Some("Ctrl+P"),
            intent: &["open file", "find file", "go to file", "switch file", "quick open"],
        },
        Command {
            id: CommandId::ProjectSearch,
            label: "Project Search",
            shortcut: Some("Ctrl+Shift+F"),
            intent: &["search project", "find in files", "grep", "search codebase", "look for text"],
        },
        Command {
            id: CommandId::ToggleTerminal,
            label: "Toggle Terminal",
            shortcut: Some("Ctrl+`"),
            intent: &["open terminal", "show terminal", "hide terminal", "shell", "console"],
        },
        Command {
            id: CommandId::Undo,
            label: "Undo",
            shortcut: Some("Ctrl+Z"),
            intent: &["revert", "go back", "previous state", "undo last change"],
        },
        Command {
            id: CommandId::Redo,
            label: "Redo",
            shortcut: Some("Ctrl+Y"),
            intent: &["redo", "forward", "next state", "redo last change"],
        },
        Command {
            id: CommandId::SelectAll,
            label: "Select All",
            shortcut: Some("Ctrl+A"),
            intent: &["select everything", "highlight all", "mark all"],
        },
        Command {
            id: CommandId::GoToDefinition,
            label: "Go to Definition",
            shortcut: Some("F12"),
            intent: &["go to definition", "jump to definition", "find definition", "navigate to source"],
        },
        Command {
            id: CommandId::TriggerCompletions,
            label: "Trigger Completions",
            shortcut: Some("Ctrl+Space"),
            intent: &["autocomplete", "show suggestions", "intellisense", "complete code"],
        },
        Command {
            id: CommandId::CommandPalette,
            label: "Open Command Palette",
            shortcut: Some("Ctrl+Shift+P"),
            intent: &["command palette", "run command", "open palette"],
        },
        Command {
            id: CommandId::ToggleMinimap,
            label: "Toggle Minimap",
            shortcut: Some("Ctrl+M"),
            intent: &["show minimap", "hide minimap", "code overview", "semantic minimap"],
        },
        Command {
            id: CommandId::ToggleTimeScrubber,
            label: "Toggle Time Scrubber",
            shortcut: Some("Ctrl+Shift+H"),
            intent: &["history scrubber", "time travel", "undo history", "show history bar"],
        },
    ]
}

// ── modal states ──────────────────────────────────────────────────────────────

struct PaletteState {
    query: String,
    results: Vec<usize>,
    selected: usize,
}

struct SearchState {
    query: String,
    case_sensitive: bool,
    whole_word: bool,
    is_regex: bool,
    rx: Option<Receiver<SearchHit>>,
    hits: Vec<SearchHit>,
    selected: usize,
}

impl SearchState {
    fn new() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            whole_word: false,
            is_regex: false,
            rx: None,
            hits: Vec::new(),
            selected: 0,
        }
    }
}

struct CmdPaletteState {
    query: String,
    commands: Vec<Command>,
    filtered: Vec<usize>,
    selected: usize,
}

impl CmdPaletteState {
    fn new() -> Self {
        let commands = built_in_commands();
        let filtered: Vec<usize> = (0..commands.len()).collect();
        Self { query: String::new(), commands, filtered, selected: 0 }
    }

    fn refilter(&mut self) {
        let q = self.query.to_lowercase();
        if q.is_empty() {
            self.filtered = (0..self.commands.len()).collect();
        } else {
            let q_words: Vec<&str> = q.split_whitespace().collect();
            self.filtered = self
                .commands
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    // Direct label substring match.
                    if c.label.to_lowercase().contains(&q) {
                        return true;
                    }
                    // Whisper Palette: check intent phrases.
                    c.intent.iter().any(|phrase| {
                        let p = phrase.to_lowercase();
                        // All query words appear in the phrase, or the whole
                        // query is a prefix of the phrase.
                        q_words.iter().all(|w| p.contains(w)) || p.contains(&q)
                    })
                })
                .map(|(i, _)| i)
                .collect();
        }
        self.selected = 0;
    }
}

// ── settings panel ───────────────────────────────────────────────────────────

/// Live user preferences controlled by the settings panel (Ctrl+,).
///
/// Boolean feature states (minimap, scrubber, etc.) are tracked directly on
/// `App`; this struct holds the style/layout preferences that don't map to
/// existing toggles.
struct SettingsState {
    /// Font size in logical pixels (clamped 10–24).
    font_size: u32,
    /// Tab width in spaces (2, 4, or 8).
    tab_width: u32,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self { font_size: 14, tab_width: 4 }
    }
}

// ── ghost caret (choreographed diff) ─────────────────────────────────────────

/// A fading ghost of the caret's previous position, left behind after large
/// jumps (go-to-definition, search navigation, etc.).
struct GhostCaret {
    position: Point,
    fade: Spring,
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
    /// Unsaved edits since the last save/open — drives the tab `•` indicator.
    modified: bool,
    /// System clipboard handle (lazily fails closed if unavailable).
    clipboard: Option<arboard::Clipboard>,

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

    // Phase 5: project search
    search_open: bool,
    search: SearchState,

    // Phase 5: command palette
    cmd_palette: Option<CmdPaletteState>,

    // Phase 5: terminal
    terminal: Option<TerminalBackend>,

    // Phase 5: git diff
    git: Option<GitRepo>,
    diff_marks: Vec<(u32, DiffMark)>,

    // Phase 7: settings panel
    settings_open: bool,
    settings: SettingsState,

    // Phase 6: semantic minimap
    minimap_open: bool,

    // Phase 6: time scrubber
    scrubber_open: bool,
    scrubber_rect: Option<Rect>,

    // Phase 6: choreographed diff / go-to-definition navigation
    ghost_caret: Option<GhostCaret>,
    go_to_def_pending: bool,

    /// Transient status-bar message and the time it was set.
    toast: Option<(String, Instant)>,

    cursor: Option<Point>,
    scroll: Spring,
    // design: horizontal scroll offset in physical pixels. No spring — pixel
    // exact so long lines stay readable during horizontal arrow/wheel movement.
    h_scroll: f64,
    // design: caret pulse — phase advances by dt/CARET_PERIOD each frame and
    // resets on keystrokes so a bright spike greets each new character (§7.6).
    caret_phase: f64,
    // Timestamp of the last horizontal scroll event; drives the fade-out timer.
    h_scroll_last: Instant,
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

struct ActiveWindow {
    window: Arc<Window>,
    surface: RenderSurface<'static>,
}

impl App {
    fn new() -> Self {
        let root = detect_project_root();
        let project = Project::new(root.clone());
        let files = project.file_strings();
        let tree = FileTree::new(project.root());
        let lsp = LspPool::new(&root);
        let git = GitRepo::discover(&root).ok();
        Self {
            context: RenderContext::new(),
            renderers: Vec::new(),
            state: WindowState::Suspended,
            scene: Scene::new(),
            chrome: None,
            text: None,
            editor: Editor::from_text("// Welcome to Eden\n"),
            highlighter: None,
            highlights: Highlights::default(),
            doc_dirty: true,
            modified: false,
            clipboard: arboard::Clipboard::new().ok(),
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
            search_open: false,
            search: SearchState::new(),
            cmd_palette: None,
            terminal: None,
            git,
            diff_marks: Vec::new(),
            settings_open: false,
            settings: SettingsState::default(),
            minimap_open: false,
            scrubber_open: false,
            scrubber_rect: None,
            ghost_caret: None,
            go_to_def_pending: false,
            toast: None,
            cursor: None,
            scroll: Spring::with_config(0.0, SpringConfig::DEFAULT),
            h_scroll: 0.0,
            caret_phase: 0.0,
            h_scroll_last: Instant::now(),
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
        if let (Some(term), Some(chrome), Some(text)) =
            (&mut self.terminal, &self.chrome, &self.text)
        {
            let rect = chrome.terminal_rect();
            let cols = (rect.width() / text.advance()).floor() as usize;
            let rows = (rect.height() / text.line_height()).floor() as usize;
            if cols > 0 && rows > 0 {
                term.resize(cols, rows);
            }
        }
        active.window.request_redraw();
    }

    // ── keyboard ──────────────────────────────────────────────────────────

    fn on_key(&mut self, event: &KeyEvent) -> bool {
        if self.terminal.is_some()
            && self.chrome.as_ref().is_some_and(|c| c.terminal_open())
            && self.on_terminal_key(event)
        {
            return true;
        }

        if let Some(ref mut _cp) = self.cmd_palette {
            return self.on_cmd_palette_key(event);
        }
        if self.palette.is_some() {
            return self.on_palette_key(event);
        }
        if self.search_open && self.on_search_key(event) {
            return true;
        }
        if self.completion_open && self.on_completion_key(event) {
            return true;
        }
        let ctrl = self.mods.control_key() || self.mods.super_key();
        let shift = self.mods.shift_key();
        let alt = self.mods.alt_key();
        match &event.logical_key {
            Key::Named(NamedKey::F12) => {
                self.go_to_definition();
                true
            }
            Key::Named(NamedKey::F2) => {
                self.toast("Rename Symbol — not yet implemented");
                true
            }
            Key::Named(named) => self.on_named_key(*named, ctrl, shift, alt),
            Key::Character(s) if ctrl => self.on_command(s, shift),
            Key::Character(s) => {
                self.editor.insert(s);
                self.doc_dirty = true;
                self.modified = true;
                self.ensure_visible = true;
                self.dismiss_completion();
                // Bright spike on each keystroke (§7.6).
                self.caret_phase = 0.0;
                // Focus Halo: dim chrome when typing in the editor.
                if let Some(chrome) = &mut self.chrome {
                    chrome.enter_typing();
                }
                true
            }
            _ => false,
        }
    }

    fn on_named_key(&mut self, named: NamedKey, ctrl: bool, shift: bool, alt: bool) -> bool {
        match named {
            NamedKey::Enter => self.edit(true, |e| e.insert("\n")),
            NamedKey::Tab => {
                if self.completion_open {
                    self.commit_completion();
                    return true;
                }
                if shift {
                    let w = self.settings.tab_width as usize;
                    return self.edit(true, |e| e.dedent_lines(w));
                }
                // Indent the block when there's a selection, else insert spaces.
                let w = self.settings.tab_width as usize;
                if self.editor.primary().is_empty() {
                    self.edit(true, move |e| e.insert(&" ".repeat(w)))
                } else {
                    self.edit(true, move |e| e.indent_lines(w))
                }
            }
            NamedKey::Backspace => self.edit(true, Editor::backspace),
            NamedKey::Delete => self.edit(true, Editor::delete_forward),
            NamedKey::Space => self.edit(true, |e| e.insert(" ")),
            NamedKey::Escape => {
                if self.close_top_overlay() {
                    return true;
                }
                false
            }
            NamedKey::ArrowLeft => self.edit(false, |e| e.move_left(shift)),
            NamedKey::ArrowRight => self.edit(false, |e| e.move_right(shift)),
            NamedKey::ArrowUp => {
                if self.search_open {
                    let count = self.search.hits.len();
                    if count > 0 {
                        self.search.selected = (self.search.selected + count - 1) % count;
                    }
                    return true;
                }
                if alt {
                    return self.edit(true, |e| e.move_lines(false));
                }
                if ctrl {
                    self.scroll.set_target(self.scroll.target() - self.line_h() * 3.0);
                    return true;
                }
                self.edit(false, |e| e.move_up(shift))
            }
            NamedKey::ArrowDown => {
                if self.search_open {
                    let count = self.search.hits.len();
                    if count > 0 {
                        self.search.selected = (self.search.selected + 1) % count;
                    }
                    return true;
                }
                if alt {
                    return self.edit(true, |e| e.move_lines(true));
                }
                if ctrl {
                    self.scroll.set_target(self.scroll.target() + self.line_h() * 3.0);
                    return true;
                }
                self.edit(false, |e| e.move_down(shift))
            }
            NamedKey::Home => {
                if ctrl {
                    self.edit(false, |e| e.set_selection(if shift { e.primary().anchor } else { 0 }, 0))
                } else {
                    self.edit(false, |e| e.move_line_start(shift))
                }
            }
            NamedKey::End => {
                if ctrl {
                    let end = self.editor.buffer().len_chars();
                    self.edit(false, move |e| {
                        let anchor = if shift { e.primary().anchor } else { end };
                        e.set_selection(anchor, end);
                    })
                } else {
                    self.edit(false, |e| e.move_line_end(shift))
                }
            }
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

    /// The editor line height in physical pixels, or a sane default.
    fn line_h(&self) -> f64 {
        self.text.as_ref().map_or(20.0, TextSystem::line_height)
    }

    /// A transient status-bar message (auto-clears after a few seconds).
    fn toast(&mut self, msg: &str) {
        self.toast = Some((msg.to_owned(), Instant::now()));
    }

    /// Closes the topmost open overlay (palette, search, completion, etc).
    /// Returns whether anything was closed.
    fn close_top_overlay(&mut self) -> bool {
        if self.completion_open {
            self.dismiss_completion();
            return true;
        }
        if self.cmd_palette.is_some() {
            self.cmd_palette = None;
            return true;
        }
        if self.palette.is_some() {
            self.palette = None;
            return true;
        }
        if self.settings_open {
            self.settings_open = false;
            return true;
        }
        if self.search_open {
            self.search_open = false;
            return true;
        }
        false
    }

    fn on_command(&mut self, key: &str, shift: bool) -> bool {
        match (key, shift) {
            ("b" | "B", false) => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.toggle_sidebar();
                }
                true
            }
            ("t" | "T", false) => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.cycle_theme();
                }
                true
            }
            ("z" | "Z", false) => self.edit(true, |e| {
                e.undo();
            }),
            // Redo: both Ctrl+Y and Ctrl+Shift+Z.
            ("y" | "Y", false) | ("z" | "Z", true) => self.edit(true, |e| {
                e.redo();
            }),
            ("a" | "A", false) => self.edit(false, Editor::select_all),
            ("c" | "C", false) => {
                self.clipboard_copy();
                true
            }
            ("x" | "X", false) => {
                self.clipboard_cut();
                true
            }
            ("v" | "V", false) => {
                self.clipboard_paste();
                true
            }
            ("d" | "D", false) => self.edit(false, |e| {
                e.select_next_occurrence();
            }),
            ("l" | "L", false) => self.edit(false, Editor::select_line),
            ("/", false) => {
                let token = comment_token_for_path(self.current_path.as_deref());
                self.edit(true, |e| e.toggle_line_comment(token))
            }
            ("s" | "S", false) => {
                self.save_current();
                true
            }
            ("s" | "S", true) => {
                self.save_current_as();
                true
            }
            ("n" | "N", false) => {
                self.new_file();
                true
            }
            ("o" | "O", false) => {
                self.open_file_dialog();
                true
            }
            // Font zoom.
            ("=" | "+", false) => {
                self.adjust_font_size(1.0);
                true
            }
            ("-" | "_", false) => {
                self.adjust_font_size(-1.0);
                true
            }
            ("0", false) => {
                if let Some(text) = &mut self.text {
                    text.set_font_size(14.0);
                }
                self.settings.font_size = 14;
                true
            }
            ("p" | "P", false) => {
                self.open_palette();
                true
            }
            ("p" | "P", true) => {
                self.open_cmd_palette();
                true
            }
            ("f" | "F", true) => {
                self.search_open = !self.search_open;
                true
            }
            (" ", false) => {
                self.trigger_completions();
                true
            }
            ("`", false) => {
                self.toggle_terminal();
                true
            }
            ("m" | "M", false) => {
                // Phase 6: toggle semantic minimap.
                self.minimap_open = !self.minimap_open;
                true
            }
            ("h" | "H", true) => {
                // Phase 6: toggle time scrubber (Ctrl+Shift+H).
                self.scrubber_open = !self.scrubber_open;
                true
            }
            (",", false) => {
                // Phase 7: open settings panel (Ctrl+,).
                self.settings_open = !self.settings_open;
                true
            }
            _ => false,
        }
    }

    // ── clipboard ─────────────────────────────────────────────────────────

    fn clipboard_copy(&mut self) {
        let text = self.editor.copy_text();
        if let Some(cb) = &mut self.clipboard
            && let Err(err) = cb.set_text(text)
        {
            tracing::warn!("clipboard copy failed: {err}");
        }
    }

    fn clipboard_cut(&mut self) {
        let text = self.editor.cut();
        self.doc_dirty = true;
        self.modified = true;
        self.ensure_visible = true;
        if let Some(cb) = &mut self.clipboard
            && let Err(err) = cb.set_text(text)
        {
            tracing::warn!("clipboard cut failed: {err}");
        }
    }

    fn clipboard_paste(&mut self) {
        let Some(cb) = &mut self.clipboard else { return };
        match cb.get_text() {
            Ok(text) if !text.is_empty() => {
                // Normalise CRLF so pasted Windows text doesn't leave stray \r.
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                self.editor.insert(&text);
                self.doc_dirty = true;
                self.modified = true;
                self.ensure_visible = true;
            }
            Ok(_) => {}
            Err(err) => tracing::warn!("clipboard paste failed: {err}"),
        }
    }

    // ── font size ─────────────────────────────────────────────────────────

    fn adjust_font_size(&mut self, delta: f64) {
        if let Some(text) = &mut self.text {
            let next = (text.font_size_logical() + delta).clamp(9.0, 28.0);
            text.set_font_size(next);
            self.settings.font_size = next.round() as u32;
            self.ensure_visible = true;
        }
    }

    // ── save / open ─────────────────────────────────────────────────────────

    fn save_current(&mut self) {
        let Some(path) = self.current_path.clone() else {
            self.save_current_as();
            return;
        };
        self.write_to(&path);
    }

    fn save_current_as(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        if let Some(dir) = self.current_path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.save_file() {
            self.write_to(&path);
            self.current_path = Some(path);
        }
    }

    fn write_to(&mut self, path: &std::path::Path) {
        let contents = self.editor.buffer().to_string();
        match std::fs::write(path, &contents) {
            Ok(()) => {
                self.modified = false;
                self.refresh_diff_marks();
                self.toast(&format!("Saved {}", path.display()));
                tracing::info!(file = %path.display(), "saved");
            }
            Err(err) => {
                self.toast(&format!("Save failed: {err}"));
                tracing::warn!(file = %path.display(), "save failed: {err}");
            }
        }
    }

    fn new_file(&mut self) {
        self.editor = Editor::from_text("");
        self.highlighter = None;
        self.highlights = Highlights::default();
        self.current_path = None;
        self.modified = false;
        self.doc_dirty = false;
        self.doc_version = 1;
        self.scroll.jump_to(0.0);
        self.h_scroll = 0.0;
        self.gutter_marks.clear();
        self.diff_marks.clear();
        self.ensure_visible = true;
    }

    fn open_file_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        if let Some(dir) = self.current_path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        } else {
            dialog = dialog.set_directory(self.project.root());
        }
        if let Some(path) = dialog.pick_file() {
            self.open_path(&path);
        }
    }

    // ── terminal ──────────────────────────────────────────────────────────

    fn toggle_terminal(&mut self) {
        if let Some(chrome) = &mut self.chrome {
            chrome.toggle_terminal();
            if chrome.terminal_open() && self.terminal.is_none() {
                let (cols, rows) = self.terminal_dimensions();
                match TerminalBackend::spawn(cols.max(20), rows.max(4)) {
                    Ok(t) => self.terminal = Some(t),
                    Err(err) => tracing::warn!("terminal spawn failed: {err:#}"),
                }
            }
        }
    }

    fn terminal_dimensions(&self) -> (usize, usize) {
        let Some(chrome) = &self.chrome else { return (80, 24) };
        let Some(text) = &self.text else { return (80, 24) };
        let rect = chrome.terminal_rect();
        let cols = (rect.width() / text.advance()).floor() as usize;
        let rows = (rect.height() / text.line_height()).floor() as usize;
        (cols.max(1), rows.max(1))
    }

    fn on_terminal_key(&mut self, event: &KeyEvent) -> bool {
        let ctrl = self.mods.control_key();
        if ctrl && let Key::Character(s) = &event.logical_key {
            if s == "`" {
                return false;
            }
            if let Some(c) = s.chars().next() {
                let code = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                if let Some(term) = &mut self.terminal {
                    term.write(&[code]);
                }
                return true;
            }
        }
        let bytes: Option<&[u8]> = match &event.logical_key {
            Key::Named(NamedKey::Enter) => Some(b"\r"),
            Key::Named(NamedKey::Backspace) => Some(b"\x7f"),
            Key::Named(NamedKey::Delete) => Some(b"\x1b[3~"),
            Key::Named(NamedKey::Escape) => Some(b"\x1b"),
            Key::Named(NamedKey::Tab) => Some(b"\t"),
            Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A"),
            Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B"),
            Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C"),
            Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D"),
            Key::Named(NamedKey::Home) => Some(b"\x1b[H"),
            Key::Named(NamedKey::End) => Some(b"\x1b[F"),
            Key::Character(s) if !ctrl => {
                if let Some(term) = &mut self.terminal {
                    term.write_str(s);
                }
                return true;
            }
            _ => None,
        };
        if let (Some(bytes), Some(term)) = (bytes, &mut self.terminal) {
            term.write(bytes);
            return true;
        }
        false
    }

    // ── project search ────────────────────────────────────────────────────

    fn on_search_key(&mut self, event: &KeyEvent) -> bool {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.search_open = false;
                true
            }
            Key::Named(NamedKey::Enter) => {
                self.open_search_result();
                true
            }
            Key::Named(NamedKey::Backspace) => {
                self.search.query.pop();
                self.start_search();
                true
            }
            Key::Character(s) => {
                let ctrl = self.mods.control_key();
                let shift = self.mods.shift_key();
                if ctrl {
                    match (s.as_str(), shift) {
                        ("i" | "I", false) => {
                            self.search.case_sensitive = !self.search.case_sensitive;
                            self.start_search();
                        }
                        ("r" | "R", false) => {
                            self.search.is_regex = !self.search.is_regex;
                            self.start_search();
                        }
                        ("w" | "W", false) => {
                            self.search.whole_word = !self.search.whole_word;
                            self.start_search();
                        }
                        _ => return false,
                    }
                } else {
                    self.search.query.push_str(s);
                    self.start_search();
                }
                true
            }
            _ => false,
        }
    }

    fn start_search(&mut self) {
        self.search.hits.clear();
        self.search.selected = 0;
        self.search.rx = None;
        if self.search.query.is_empty() {
            return;
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        let query = SearchQuery {
            text: self.search.query.clone(),
            case_sensitive: self.search.case_sensitive,
            whole_word: self.search.whole_word,
            is_regex: self.search.is_regex,
        };
        search_project(self.project.root(), query, tx);
        self.search.rx = Some(rx);
    }

    fn open_search_result(&mut self) {
        let hit = self.search.hits.get(self.search.selected).cloned();
        let Some(hit) = hit else { return };
        self.record_ghost_caret();
        self.open_path(&hit.path);
        let line = (hit.line_no as usize).saturating_sub(1);
        let char_idx = self.editor.buffer().line_to_char(line);
        self.editor.set_caret(char_idx);
        self.ensure_visible = true;
    }

    // ── command palette ───────────────────────────────────────────────────

    fn open_cmd_palette(&mut self) {
        self.cmd_palette = Some(CmdPaletteState::new());
    }

    fn on_cmd_palette_key(&mut self, event: &KeyEvent) -> bool {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.cmd_palette = None;
                true
            }
            Key::Named(NamedKey::Enter) => {
                self.execute_selected_command();
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                if let Some(cp) = &mut self.cmd_palette {
                    let count = cp.filtered.len();
                    if count > 0 {
                        cp.selected = (cp.selected + 1) % count;
                    }
                }
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                if let Some(cp) = &mut self.cmd_palette {
                    let count = cp.filtered.len();
                    if count > 0 {
                        cp.selected = (cp.selected + count - 1) % count;
                    }
                }
                true
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(cp) = &mut self.cmd_palette {
                    cp.query.pop();
                    cp.refilter();
                }
                true
            }
            Key::Character(s) => {
                if let Some(cp) = &mut self.cmd_palette {
                    cp.query.push_str(s);
                    cp.refilter();
                }
                true
            }
            _ => false,
        }
    }

    fn execute_selected_command(&mut self) {
        let Some(cp) = &self.cmd_palette else { return };
        let cmd_id = cp
            .filtered
            .get(cp.selected)
            .and_then(|&i| cp.commands.get(i))
            .map(|c| c.id);
        self.cmd_palette = None;
        let Some(id) = cmd_id else { return };
        match id {
            CommandId::ToggleSidebar => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.toggle_sidebar();
                }
            }
            CommandId::CycleTheme => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.cycle_theme();
                }
            }
            CommandId::OpenFilePalette => self.open_palette(),
            CommandId::ProjectSearch => {
                self.search_open = true;
            }
            CommandId::CommandPalette => self.open_cmd_palette(),
            CommandId::ToggleTerminal => self.toggle_terminal(),
            CommandId::Undo => {
                self.editor.undo();
                self.doc_dirty = true;
            }
            CommandId::Redo => {
                self.editor.redo();
                self.doc_dirty = true;
            }
            CommandId::SelectAll => {
                self.editor.select_all();
            }
            CommandId::GoToDefinition => self.go_to_definition(),
            CommandId::TriggerCompletions => self.trigger_completions(),
            CommandId::ToggleMinimap => {
                self.minimap_open = !self.minimap_open;
            }
            CommandId::ToggleTimeScrubber => {
                self.scrubber_open = !self.scrubber_open;
            }
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
        // Guard: never open build artifacts from target/.
        if is_in_target_dir(path, self.project.root()) {
            tracing::debug!(file = %path.display(), "skipped target/ artifact");
            return;
        }

        // Guard: binary files show a placeholder instead of raw bytes.
        if !is_text_path(path) {
            tracing::debug!(file = %path.display(), "binary file, showing placeholder");
            self.editor = Editor::from_text("// Binary file — cannot display.\n// Open a source file from the sidebar.\n");
            self.highlighter = None;
            self.highlights = Highlights::default();
            self.doc_dirty = false;
            self.modified = false;
            self.doc_version = 1;
            self.scroll.jump_to(0.0);
            self.h_scroll = 0.0;
            self.ensure_visible = true;
            self.current_path = Some(path.to_path_buf());
            self.gutter_marks.clear();
            self.diff_marks.clear();
            self.hover_card = None;
            self.hover_requested_for = None;
            self.dismiss_completion();
            return;
        }

        match std::fs::read_to_string(path) {
            Ok(contents) => {
                self.editor = Editor::from_text(&contents);
                // Re-init syntax highlighter for the opened language.
                let lang = language_for_path(path);
                self.highlighter = match lang {
                    Some("rust") => {
                        match Highlighter::rust() {
                            Ok(h) => Some(h),
                            Err(e) => {
                                tracing::warn!("rust highlighter: {e}");
                                None
                            }
                        }
                    }
                    _ => None,
                };
                self.highlights = Highlights::default();
                self.doc_dirty = true;
                self.modified = false;
                self.doc_version = 1;
                self.scroll.jump_to(0.0);
                self.h_scroll = 0.0;
                self.ensure_visible = true;
                self.current_path = Some(path.to_path_buf());
                self.gutter_marks.clear();
                self.diff_marks.clear();
                self.hover_card = None;
                self.hover_requested_for = None;
                self.dismiss_completion();
                self.lsp.open_document(path, &contents);
                self.refresh_diff_marks();
                tracing::info!(
                    file = %path.display(),
                    lang = lang.unwrap_or("plaintext"),
                    "opened"
                );
            }
            Err(err) => tracing::warn!(file = %path.display(), "could not open: {err}"),
        }
    }

    fn go_to_definition(&mut self) {
        let Some(path) = self.current_path.clone() else { return };
        let Some(pos) = self.caret_lsp_position() else { return };
        self.lsp.request_definition(&path, pos);
        self.go_to_def_pending = true;
    }

    /// Navigates to a definition URI+position returned by the LSP.
    fn navigate_to_definition(&mut self, uri: &str, pos: LspPos) {
        let Some(dest) = uri_to_path(uri) else {
            tracing::warn!("could not parse definition URI: {uri}");
            return;
        };
        self.record_ghost_caret();
        if self.current_path.as_deref() != Some(&dest) {
            self.open_path(&dest);
        }
        let char_idx =
            self.editor.buffer().line_to_char(pos.line as usize) + pos.character as usize;
        self.editor.set_caret(char_idx.min(self.editor.buffer().len_chars()));
        self.ensure_visible = true;
    }

    // ── choreographed diff helpers ────────────────────────────────────────

    /// Records the current caret's physical position as a ghost that fades out.
    fn record_ghost_caret(&mut self) {
        if let Some(pt) = self.caret_anchor() {
            // design: ghost fades with a slow spring so it's readable during the jump.
            let mut fade = Spring::with_config(1.0, self.prefs.resolve(SpringConfig::UNIT));
            fade.set_target(0.0);
            self.ghost_caret = Some(GhostCaret { position: pt, fade });
        }
    }

    // ── git diff marks ────────────────────────────────────────────────────

    fn refresh_diff_marks(&mut self) {
        let Some(path) = &self.current_path.clone() else { return };
        let Some(git) = &self.git else { return };
        match git.diff_hunks(path) {
            Ok(hunks) => {
                self.diff_marks = hunks
                    .iter()
                    .flat_map(|h| {
                        let kind = match h.kind {
                            DiffKind::Added => DiffMark::Added,
                            DiffKind::Modified => DiffMark::Modified,
                            DiffKind::Deleted => DiffMark::Deleted,
                        };
                        (h.start_line..=h.end_line).map(move |line| (line, kind))
                    })
                    .collect();
            }
            Err(err) => tracing::debug!("diff_hunks: {err:#}"),
        }
    }

    // ── LSP helpers ───────────────────────────────────────────────────────

    fn caret_lsp_position(&self) -> Option<LspPos> {
        let caret = self.editor.primary();
        let line = self.editor.buffer().char_to_line(caret.head) as u32;
        let line_start = self.editor.buffer().line_to_char(line as usize);
        let character = (caret.head - line_start) as u32;
        Some(LspPos { line, character })
    }

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

    fn maybe_request_hover(&mut self) {
        if self.cursor_still_since.elapsed() < HOVER_DELAY {
            return;
        }
        let cursor = self.cursor;
        if self.hover_requested_for == cursor {
            return;
        }
        let Some(path) = &self.current_path.clone() else { return };
        let Some(pos) = self.cursor_lsp_position() else { return };
        self.lsp.request_hover(path, pos);
        self.hover_requested_for = cursor;
    }

    fn refresh_hover(&mut self) {
        let Some(path) = &self.current_path.clone() else { return };
        self.hover_card = self.lsp.hover(path).map(|h| h.contents);
    }

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
        let row_h = text.sidebar_row_height();
        let idx = ((point.y - rect.y0 + self.tree_scroll) / row_h).floor();
        if idx < 0.0 {
            return None;
        }
        let idx = idx as usize;
        (idx < self.tree.entries().len()).then_some(idx)
    }

    fn on_click(&mut self) -> bool {
        let Some(point) = self.cursor else { return false };

        // Phase 6: Time scrubber click — jump to that undo position.
        if let Some(rect) = self.scrubber_rect
            && rect.contains(point)
        {
            let total = self.editor.history_total();
            if total > 0 {
                let t = ((point.x - rect.x0) / rect.width()).clamp(0.0, 1.0);
                let target = (t * total as f64).round() as usize;
                let current = self.editor.history_pos();
                if target < current {
                    for _ in 0..(current - target) {
                        self.editor.undo();
                    }
                } else {
                    for _ in 0..(target - current) {
                        self.editor.redo();
                    }
                }
                self.doc_dirty = true;
            }
            return true;
        }

        // Click in the editor canvas → place caret at the clicked line/column.
        if let (Some(chrome), Some(text)) = (&self.chrome, &self.text) {
            let area = chrome.editor_rect();
            if area.contains(point) {
                let gutter_w = text.gutter_width(self.editor.buffer().len_lines());
                let line = ((point.y - area.y0 + self.scroll.value()) / text.line_height())
                    .floor()
                    .max(0.0) as usize;
                let line = line.min(self.editor.buffer().len_lines().saturating_sub(1));
                let col = ((point.x - area.x0 - gutter_w) / text.advance())
                    .floor()
                    .max(0.0) as usize;
                let line_len = self.editor.buffer().line_len(line);
                let col = col.min(line_len);
                let char_idx = self.editor.buffer().line_to_char(line) + col;
                self.editor.set_caret(char_idx);
                self.caret_phase = 0.0;
                return true;
            }
        }

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
            self.modified = true;
            // Focus Halo: dim chrome on every mutating key.
            if let Some(chrome) = &mut self.chrome {
                chrome.enter_typing();
            }
        }
        self.ensure_visible = true;
        true
    }

    fn refresh_highlights(&mut self) {
        if !self.doc_dirty {
            return;
        }
        self.doc_dirty = false;
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

        self.refresh_gutter_marks();
        self.maybe_request_hover();
        self.refresh_hover();

        // Choreographed Diff: poll the LSP definition result and navigate.
        if self.go_to_def_pending
            && let Some(path) = &self.current_path.clone()
            && let Some(result) = self.lsp.definition_result(path)
        {
            let uri = result.uri.clone();
            let pos = result.position;
            self.lsp.clear_definition(path);
            self.go_to_def_pending = false;
            self.navigate_to_definition(&uri, pos);
        }

        // Drain search results from the background thread.
        if let Some(rx) = &self.search.rx {
            while let Ok(hit) = rx.try_recv() {
                self.search.hits.push(hit);
            }
        }

        // Pull fresh completions.
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
        // Advance the caret pulse phase and keep animating while the window is focused.
        if self.focused {
            self.caret_phase = (self.caret_phase + dt / CARET_PERIOD) % 1.0;
        }
        let chrome_moving = self.chrome.as_mut().is_some_and(|c| c.step(dt));
        let scroll_moving = self.scroll.step(dt);

        // Step ghost caret fade spring.
        let ghost_moving = if let Some(ghost) = &mut self.ghost_caret {
            let still_moving = ghost.fade.step(dt);
            if ghost.fade.value() < 0.005 {
                self.ghost_caret = None;
                false
            } else {
                still_moving
            }
        } else {
            false
        };

        let hover_pending = self.hover_requested_for != self.cursor
            && self.cursor_still_since.elapsed() < HOVER_DELAY;
        let search_streaming = self.search.rx.is_some() && self.search_open;
        let def_pending = self.go_to_def_pending;
        let animating = chrome_moving
            || scroll_moving
            || ghost_moving
            || hover_pending
            || search_streaming
            || def_pending
            || self.focused; // caret pulse always drives frames while focused

        let caret_anchor = self.caret_anchor();
        let hover_card = self.hover_card.clone();
        let completion_items: Vec<_> = self.completion_items.clone();
        let completion_selected = self.completion_selected;
        let completion_open = self.completion_open;
        let gutter_marks: Vec<_> = self.gutter_marks.clone();
        let diff_marks: Vec<_> = self.diff_marks.clone();

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

        // Tab bar: overlay the real filename over the tab strip background that
        // Chrome just filled. Only shown when a file is open (current_path set).
        let modified = self.modified;
        if let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome) {
            let label = self.current_path.as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| if modified { format!("\u{2022} {s}") } else { s.to_owned() });
            let tab_rect = chrome.tab_strip_rect();
            let palette = chrome.palette();
            text.paint_tab_bar(&mut self.scene, tab_rect, label.as_deref(), &palette, self.scale);
        }

        // Status bar: real branch, language, and cursor position text.
        let toast_msg = self
            .toast
            .as_ref()
            .filter(|(_, t)| t.elapsed() < Duration::from_secs(4))
            .map(|(m, _)| m.clone());
        let diag_counts = self.gutter_marks.iter().fold((0usize, 0usize), |(e, w), (_, m)| {
            match m {
                GutterMark::Error => (e + 1, w),
                GutterMark::Warning => (e, w + 1),
            }
        });
        if let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome) {
            let status_rect = chrome.status_bar_rect();
            let palette = chrome.palette();
            let branch = self.git.as_ref().and_then(|g| g.branch_name());
            let lang = self.current_path.as_ref()
                .and_then(|p| language_for_path(p))
                .map(str::to_uppercase);
            let caret = self.editor.primary();
            let line = self.editor.buffer().char_to_line(caret.head) + 1;
            let line_start = self.editor.buffer().line_to_char(line.saturating_sub(1));
            let col = caret.head.saturating_sub(line_start) + 1;
            text.paint_status_bar(
                &mut self.scene,
                status_rect,
                &StatusBarView {
                    branch: branch.as_deref(),
                    language: lang.as_deref(),
                    line,
                    col,
                    diagnostics: diag_counts,
                    message: toast_msg.as_deref(),
                },
                &palette,
                self.scale,
            );
        }

        // Sidebar: either file tree or search panel.
        if let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome) {
            let rect = chrome.sidebar_rect();
            if rect.width() > 2.0 {
                let palette = chrome.palette();
                if self.search_open {
                    let hit_rows: Vec<SearchRowView> = self
                        .search
                        .hits
                        .iter()
                        .take(200)
                        .map(|h| {
                            let rel = h
                                .path
                                .strip_prefix(self.project.root())
                                .unwrap_or(&h.path)
                                .to_string_lossy()
                                .replace('\\', "/");
                            SearchRowView {
                                path: rel,
                                line_no: h.line_no,
                                line: h.line.clone(),
                                match_start: h.match_start,
                                match_end: h.match_end,
                            }
                        })
                        .collect();
                    text.paint_search_panel(
                        &mut self.scene,
                        rect,
                        &SearchPanelView {
                            query: &self.search.query,
                            is_regex: self.search.is_regex,
                            case_sensitive: self.search.case_sensitive,
                            whole_word: self.search.whole_word,
                            rows: &hit_rows,
                            selected: self.search.selected,
                        },
                        &palette,
                        self.scale,
                    );
                } else {
                    let selected_row = self.current_path.as_ref().and_then(|p| {
                        self.tree.entries().iter().position(|e| e.path == *p)
                    });
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
                            selected: selected_row,
                        },
                        &palette,
                        self.scale,
                    );
                }
            }
        }

        // Editor canvas.
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
                    caret_phase: self.caret_phase,
                    h_scroll_px: self.h_scroll,
                    h_scroll_fade: {
                        let idle = self.h_scroll_last.elapsed().as_secs_f64();
                        // Fade out linearly from 1.0 → 0.0 over 0.5 s after 1.5 s idle.
                        (1.0 - (idle - 1.5).clamp(0.0, 0.5) / 0.5).max(0.0)
                    },
                    gutter_marks: &gutter_marks,
                    diff_marks: &diff_marks,
                },
            );
        }

        // Phase 6 — Semantic Minimap (painted over the editor right edge).
        if self.minimap_open
            && let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome)
        {
            let palette = chrome.palette();
            let syntax = chrome.syntax();
            text.paint_minimap(
                &mut self.scene,
                chrome.editor_rect(),
                &MinimapView {
                    editor: &self.editor,
                    highlights: &self.highlights,
                    syntax: &syntax,
                    scroll_px: self.scroll.value(),
                },
                &palette,
                self.scale,
            );
        }

        // Phase 6 — Time Scrubber.
        if self.scrubber_open {
            if let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome) {
                let palette = chrome.palette();
                self.scrubber_rect = text.paint_time_scrubber(
                    &mut self.scene,
                    chrome.editor_rect(),
                    &ScrubberView {
                        undo_pos: self.editor.history_pos(),
                        total: self.editor.history_total(),
                    },
                    &palette,
                    self.scale,
                );
            }
        } else {
            self.scrubber_rect = None;
        }

        // Phase 6 — Ghost caret (Choreographed Diff).
        if let (Some(ghost), Some(chrome), Some(text)) =
            (&self.ghost_caret, &self.chrome, &self.text)
        {
            let palette = chrome.palette();
            let alpha = (ghost.fade.value() * 200.0) as u8;
            let ghost_color = Rgba8::rgba(palette.accent.r, palette.accent.g, palette.accent.b, alpha);
            let caret_w = (2.0 * self.scale).max(1.5);
            let line_h = text.line_height();
            fill_rrect(
                &mut self.scene,
                Rect::new(
                    ghost.position.x,
                    ghost.position.y + 2.0 * self.scale,
                    ghost.position.x + caret_w * 3.0,
                    ghost.position.y + line_h - 2.0 * self.scale,
                ),
                caret_w,
                ghost_color,
            );
        }

        // Terminal panel.
        if let (Some(term), Some(text), Some(chrome)) =
            (&self.terminal, &mut self.text, &self.chrome)
        {
            let rect = chrome.terminal_rect();
            if rect.height() > 2.0 {
                let grid = term.grid();
                let row_slices: Vec<&[eden_terminal::TermCell]> =
                    (0..grid.rows).map(|r| grid.row(r)).collect();
                text.paint_terminal(
                    &mut self.scene,
                    rect,
                    &TerminalView {
                        rows: &row_slices,
                        cols: grid.cols,
                        cursor_row: grid.cursor_row,
                        cursor_col: grid.cursor_col,
                        focused: self.focused,
                    },
                    self.scale,
                );
            }
        }

        // Hover card.
        if let (Some(ref card), Some(text), Some(chrome), Some(anchor)) =
            (hover_card, &mut self.text, &self.chrome, self.cursor)
        {
            let palette = chrome.palette();
            let screen = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
            text.paint_hover_card(&mut self.scene, screen, anchor, card, &palette, self.scale);
        }

        // Completion popup.
        if completion_open
            && !completion_items.is_empty()
            && let (Some(anchor), Some(text), Some(chrome)) =
                (caret_anchor, &mut self.text, &self.chrome)
        {
            let palette = chrome.palette();
            let screen = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
            let entries: Vec<CompletionEntry> = completion_items
                .iter()
                .map(|c| CompletionEntry { label: c.label.clone(), detail: c.detail.clone() })
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

        // Command palette.
        if let (Some(cp), Some(text), Some(chrome)) =
            (&self.cmd_palette, &mut self.text, &self.chrome)
        {
            let entries: Vec<CmdEntry> = cp
                .filtered
                .iter()
                .take(10)
                .filter_map(|&i| cp.commands.get(i))
                .map(|c| CmdEntry {
                    label: c.label.to_owned(),
                    shortcut: c.shortcut.map(str::to_owned),
                })
                .collect();
            let screen = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
            text.paint_cmd_palette(
                &mut self.scene,
                screen,
                &CmdPaletteView {
                    query: &cp.query,
                    entries: &entries,
                    selected: cp.selected,
                },
                &chrome.palette(),
                self.scale,
            );
        }

        // Settings panel (Ctrl+,) — painted above everything else.
        if self.settings_open
            && let (Some(text), Some(chrome)) = (&mut self.text, &self.chrome)
        {
            let screen = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
            let theme_names: Vec<&str> = (0..chrome.theme_count())
                .map(|i| chrome.theme_name(i).unwrap_or("—"))
                .collect();
            let toggles = [
                SettingsToggle { label: "Minimap", enabled: self.minimap_open },
                SettingsToggle { label: "Time Scrubber", enabled: self.scrubber_open },
                SettingsToggle { label: "Focus Halo", enabled: true },
            ];
            text.paint_settings(
                &mut self.scene,
                screen,
                &SettingsView {
                    font_size: self.settings.font_size,
                    tab_width: self.settings.tab_width,
                    themes: &theme_names,
                    active_theme: chrome.active_theme_index(),
                    toggles: &toggles,
                },
                &chrome.palette(),
                self.scale,
            );
        }

        // Cmd-P file palette (topmost).
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

// ── URI helpers ───────────────────────────────────────────────────────────────

/// Converts a `file://` URI from the LSP to a local [`PathBuf`].
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let without_scheme = uri.strip_prefix("file://")?;
    // On Windows: file:///C:/path → /C:/path → strip leading slash.
    let path_str = if cfg!(windows) && without_scheme.starts_with('/') {
        without_scheme.trim_start_matches('/')
    } else {
        without_scheme
    };
    let decoded = path_str
        .replace("%20", " ")
        .replace("%2F", "/")
        .replace("%5C", "\\")
        .replace("%25", "%");
    Some(PathBuf::from(decoded))
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
        // Open startup file on first launch — prefer crates/eden-app/src/main.rs
        // relative to the workspace root; fall back to empty welcome buffer.
        if self.current_path.is_none() {
            let startup = self.project.root()
                .join("crates")
                .join("eden-app")
                .join("src")
                .join("main.rs");
            if startup.exists() {
                self.open_path(&startup);
            }
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
                let advance = self.text.as_ref().map_or(8.0, TextSystem::advance);
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (f64::from(x) * advance * 3.0, f64::from(y) * line_h * 3.0)
                    }
                    MouseScrollDelta::PixelDelta(p) => (p.x, p.y),
                };
                // Shift+scroll or natural horizontal scroll → horizontal offset.
                if self.mods.shift_key() || dx.abs() > dy.abs() {
                    let amount = if dx.abs() > dy.abs() { -dx } else { -dy };
                    self.h_scroll = (self.h_scroll + amount).max(0.0);
                    self.h_scroll_last = Instant::now();
                } else if self.over_sidebar() {
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
