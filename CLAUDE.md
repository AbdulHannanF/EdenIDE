# Eden — CLAUDE.md

GPU-rendered code editor in pure Rust. See `Eden_Original_Idea.md` for the full spec.

---

## Build & Run

```powershell
cargo build                     # debug (opt-level 1 for vello sanity)
cargo run -p eden-app           # launch the editor
cargo test --workspace          # all crate tests
cargo clippy --workspace -- -D warnings
```

Platform: Windows 11, DX12 backend. Shell is PowerShell; use Bash tool for POSIX scripts.

---

## Workspace Layout

```
crates/
  eden-app/        binary — winit event loop, GPU init, render glue
  eden-ui/         chrome, layout (taffy), theming, text rendering (cosmic-text)
  eden-motion/     spring solver, MotionPrefs (reduce-motion)
  eden-editor/     ropey buffer, multi-cursor selections, undo/redo
  eden-syntax/     tree-sitter highlight wrapper → Vec<Span>
  eden-search/     nucleo fuzzy matcher (Cmd-P)
  eden-workspace/  Project (file listing) + FileTree (lazy expand)
  eden-theme/      Palette, Syntax, Theme (3 built-ins + TOML round-trip)
  eden-lsp/        STUB — Phase 4
  eden-vcs/        STUB — Phase 5
  eden-terminal/   STUB — Phase 5
  eden-plugin/     STUB — Phase 7
themes/
  eden-day.toml / eden-dusk.toml / eden-noir.toml
```

---

## Phase Status

| Phase | Name | Status |
|-------|------|--------|
| 0 | Skeleton — workspace, GPU window, vello rect | **Done** (`52ee1a1`) |
| 1 | The Surface — chrome, taffy layout, spring motion, 3 themes | **Done** (`4bb18cd`) |
| 2 | The Buffer — ropey editor, cosmic-text rendering, undo, scroll | **Done** (`28cf420`) |
| 3 | Syntax & Files — tree-sitter (Rust), file tree, Cmd-P | **Done** (`750bca9`) |
| 4 | Intelligence — LSP client, completion, hover, diagnostics | **Next** |
| 5 | Surroundings — project search, command palette, terminal, git | Pending |
| 6 | Signature features (pick 6 of 9) | Pending |
| 7 | Polish — settings UI, bundle, icon, telemetry | Pending |

---

## What's Working Right Now

- **Window & GPU**: winit 0.30 + wgpu/vello, DX12, vsync, HiDPI-aware.
- **Chrome**: title bar / sidebar / tab strip / editor canvas / status bar, taffy flex layout, spring-animated sidebar toggle (`Ctrl+B`), spring theme crossfade (`Ctrl+T`), hover glow.
- **3 Themes**: Eden Day (warm paper), Eden Dusk (navy), Eden Noir (near-black gold). TOML-serialisable, hot-crossfade capable.
- **Editor buffer**: ropey rope, 50 MB capable. Typing, backspace, delete-forward, Home/End, arrows (+ Shift extend), Ctrl+A, Ctrl+Z/Ctrl+Y undo/redo with coalescing, multi-cursor `add_caret`/`select_all`.
- **Text rendering**: cosmic-text shaping + vello GPU rasterisation. Gutter line numbers, selection highlights, carets (hidden when unfocused), spring-driven scroll with caret-follow and page-scroll.
- **Syntax highlighting**: tree-sitter-rust, 15 HighlightKinds → theme Syntax colours. Full reparse on change (incremental `InputEdit` is a noted follow-up).
- **File tree**: gitignore-aware lazy expand/collapse via `eden-workspace::FileTree`. Click to open files, hover highlight, scroll.
- **Cmd-P**: nucleo fuzzy file finder over gitignore-filtered project files, keyboard navigation, Enter opens into editor.

---

## Known Gaps / Technical Debt

- **Syntax only Rust**: `Highlighter::rust()` is the only language wired. `open_path` does not detect language and does not re-init the highlighter — other files open without highlights.
- **Cursor doesn't pulse**: spec §7.6 says sine-wave pulse (~1.1 s period). Currently just a solid static caret.
- **Cmd-click multi-cursor not wired**: `Editor::add_caret` exists but `MouseButton::Left` handler in `main.rs` only routes to the file tree, not the text canvas.
- **No horizontal scroll**: text overflows the right edge silently.
- **No soft-wrap**: lines render as logical lines; very long lines run off screen.
- **OS reduce-motion not detected**: `MotionPrefs::from_env()` reads `EDEN_REDUCE_MOTION=1`; Windows API hook is a noted future item.
- **Font**: Consolas stand-in. JetBrains Mono (§6) requires bundling the font file in `assets/fonts/`.
- **No `CLAUDE.md` referenced by spec §10**: this file.
- **No `README.md` yet**.

---

## Phase 4 — Intelligence (Next)

Goal: bring `eden-lsp` to life. Spec §4 items 2–3, §8 Phase 4.

### Tasks

1. **Add LSP deps to `eden-lsp/Cargo.toml`**: `lsp-types`, `tower-lsp` (client side), `tokio` (multi-thread), `serde_json`.
2. **`LspClient`**: async task that spawns a language server child process (`rust-analyzer` first), speaks JSON-RPC over stdio, and translates LSP messages to/from Eden domain types.
3. **Server pool** (`LspPool`): keyed by language id, one client per server binary.
4. **`textDocument/didOpen` / `didChange`**: call on every `doc_dirty` flip; feed the full buffer text (incremental sync in follow-up).
5. **Diagnostics**: collect `textDocument/publishDiagnostics` and expose a `Vec<Diagnostic>` with line/col. Paint squiggles + gutter dot in `eden-ui/text.rs`.
6. **Hover**: on `CursorMoved` + delay, fire `textDocument/hover`, render a floating card above/below the hovered token.
7. **Completion**: on typing trigger chars or `Ctrl+Space`, fire `textDocument/completion`, show a popup list ranked by LSP score. `Tab`/`Enter` inserts.
8. **Go-to-definition** (`F12`): fire `textDocument/definition`, open the result file, spring-scroll to the line.
9. **Wire into `eden-app`**: `App` holds an `Arc<LspPool>`, feeds `tokio::spawn`ed background tasks, pulls results via `crossbeam` channel on each frame.

### Design constraints

- LSP client lives on a tokio thread; results arrive via a channel; the render thread never blocks.
- `eden-lsp`'s public API is synchronous from the caller's perspective: `LspPool::diagnostics(path) -> Vec<Diagnostic>`, `LspPool::hover(path, pos) -> Option<HoverCard>`, etc. Staleness is fine; the render thread just draws whatever is current.
- Use `anyhow` for all error paths; no `unwrap` outside tests.

---

## Phase 5 — Surroundings (After Phase 4)

1. **Project search** (`Ctrl+Shift+F`): `grep-regex` + `grep-searcher` (ripgrep library crates); results panel with live streaming; regex/case/whole-word toggles.
2. **Command palette** (`Ctrl+Shift+P`): list of `Command { id, label, intent_strings, action }` structs; nucleo-ranked; natural-language intent matching for the common dozen actions (no LLM).
3. **Terminal** (`eden-terminal`): `alacritty_terminal` crate as the PTY backend; render through the same vello pipeline as the editor.
4. **Git sidebar** (`eden-vcs`): `git2` for staged/unstaged/untracked status; gutter diff markers (added/changed/deleted lines); blame-on-hover card.

---

## Phase 6 — Signature Features (Pick 6)

From spec §5. Recommended first six for composability:

| Feature | Why pick it |
|---------|-------------|
| **Ambient Compile** | Requires only diagnostics (Phase 4 output) + a glyph bloom shader |
| **Focus Halo** | Pure UI — sidebar/tab fade on typing start, breathe back on mouse |
| **Whisper Palette** | Extend the Phase 5 command palette with intent strings + NL matching |
| **Time Scrubber** | Phase 2 undo history already exists; add a scrubber widget + preview |
| **Semantic Minimap** | tree-sitter AST already parsed; render structural glyphs instead of pixel-mush |
| **Choreographed Diff** | LSP rename/code-action result → dissolve old, rise new, cursor traces ghost |

---

## Phase 7 — Polish

- Settings UI panel (native, TOML also accepted).
- Bundle JetBrains Mono + Inter into `assets/fonts/`.
- App icon + Windows `.exe` manifest.
- `crash-reporter` crate (breadcrumbs to local log, telemetry opt-in off by default).
- Final `README.md` with GIF/recording per shipped signature feature.
- `CHANGELOG.md`.

---

## Quality Bar (non-negotiable, §9)

- `cargo clippy --workspace -- -D warnings` must be clean before every commit.
- Zero `unwrap()` outside tests and `main()`.
- Every public function has a doc comment.
- Unit tests in every crate; editor integration tests (insert/delete/undo/multi-cursor/large-file); LSP integration tests (mock server, hover, completion).
- Cold start ≤ 500 ms (Windows mid-range).
- Keystroke-to-glyph ≤ 16 ms at 60 Hz.

---

## Conventions

- **Commits**: Conventional Commits — `feat(phase-N): description`.
- **Spring constants**: `stiffness=170, damping=26` default (`SpringConfig::DEFAULT`). Tune per-interaction; leave a `// design:` comment with the reason.
- **Spacing**: 4 px grid. Corner radii from `{4, 8, 12, 16}` only.
- **Deps**: add only what the current phase needs. Justify any crate not in the §2 stack with a comment.
- **No `mod.rs` over 400 lines**.
- **No linear easings** — springs everywhere.
- **Commit after each phase** (or logical sub-milestone within a phase). Don't batch.
