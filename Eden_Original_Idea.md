# Eden — Build Prompt for Claude Code

> Paste this entire document as your first message to Claude Code at the root of a fresh directory. Then iterate.

---

## 1. Mission

Build **Eden**, a desktop code editor written in pure Rust that feels the way a Steinway feels under your fingers: responsive, weighted, alive. The product target is the seam between Zed's raw GPU performance, Linear's design discipline, Raycast's command-driven ergonomics, and the quiet confidence of native macOS apps like Things 3 and Tot. It must not look or feel like an Electron app, a VS Code fork, or a hobbyist toy. Every frame, every transition, every keystroke is part of the product.

Eden is for engineers who care about how their tools feel. The name evokes calm focus — kingfisher-blue, golden-hour light, the rare conditions under which deep work happens.

You are not building a prototype. You are bootstrapping a product. Make architectural decisions that will hold up at 100k LOC.

---

## 2. Non-Negotiable Technology Stack

Use these crates. Do not substitute without explicit justification.

**Rendering & UI**
- `wgpu` — cross-platform GPU abstraction (Metal on macOS, Vulkan on Linux, DX12 on Windows).
- `vello` — GPU-accelerated 2D vector renderer. This is what gives Eden its silk.
- `winit` — windowing and input.
- `cosmic-text` — high-quality text shaping and layout with full Unicode and bidi.
- `swash` — glyph rasterization for cosmic-text.
- `taffy` — flexbox/grid layout engine.

**Editing core**
- `ropey` — rope data structure for the buffer. Handles multi-MB files without flinching.
- `tree-sitter` (with `tree-sitter-highlight`) — incremental parsing for syntax, structure, and semantic highlighting. Bundle grammars for: Rust, TypeScript, JavaScript, TSX, Python, Go, C, C++, JSON, TOML, YAML, Markdown, HTML, CSS, SQL, Shell.
- `tower-lsp` (client side via `lsp-types` + a custom transport) — Language Server Protocol client. Spawn `rust-analyzer`, `typescript-language-server`, `pyright`, etc. as child processes.

**Async & infrastructure**
- `tokio` — async runtime, multi-thread.
- `crossbeam` — channels and concurrency primitives for the render thread ↔ logic thread boundary.
- `notify` — file-system watching.
- `ignore` — gitignore-aware directory walking.
- `git2` — Git integration (blame, diff, status, branches).
- `serde` + `toml` + `serde_json` — config and state persistence.
- `tracing` + `tracing-subscriber` — structured logging.
- `anyhow` + `thiserror` — error handling.

**Terminal**
- `alacritty_terminal` — embedded terminal emulator core. Render it through the same vello pipeline so it inherits the global motion system.

**Fuzzy matching & search**
- `nucleo` — same fuzzy matcher Helix uses; SIMD-fast, ranks like fzf.
- `grep` (the ripgrep library crates: `grep-regex`, `grep-searcher`) — project-wide search.

**Animation**
- Roll a small animation crate inside the workspace (`Eden-motion`) on top of a spring-physics model (critically-damped springs by default). Do **not** use linear easings anywhere. See §7.

**Packaging**
- `cargo-bundle` for macOS `.app`, with code signing hooks ready.

---

## 3. Workspace Layout

Cargo workspace, multi-crate. This boundary matters — keep it clean.

```
Eden/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── Eden-app/              # the binary; window, event loop, glue
│   ├── Eden-ui/               # widget tree, layout, render passes, theming
│   ├── Eden-motion/           # spring animations, transitions, choreography
│   ├── Eden-editor/           # buffer, cursors, selections, undo, edits
│   ├── Eden-syntax/           # tree-sitter integration, highlighter, indents
│   ├── Eden-lsp/              # LSP client pool, completion, diagnostics, hover
│   ├── Eden-search/           # nucleo + ripgrep wrappers, fuzzy + content search
│   ├── Eden-vcs/              # git integration, blame, diff models
│   ├── Eden-terminal/         # alacritty wrapper as a Eden-ui widget
│   ├── Eden-workspace/        # project model, file tree, sessions
│   ├── Eden-theme/            # theme schema, parser, built-in themes
│   └── Eden-plugin/           # plugin host (WASM via wasmtime, stubbed in v1)
├── themes/
├── assets/
│   ├── fonts/                    # bundled monospace + UI fonts (see §6)
│   └── icons/                    # SVG icon set
└── README.md
```

Every crate has its own `lib.rs`, its own tests, and a clear public API. No god-modules. No `mod.rs` files larger than 400 lines.

---

## 4. Core Capabilities (must all work end-to-end before v1)

1. **Text editing** that handles 50MB files without lag. Multi-cursor (Cmd-click and Cmd-D add-next), block selection (Option-drag), column edits.
2. **Tree-sitter syntax highlighting** with semantic token enrichment from LSP when available. Highlights update incrementally on every keystroke without re-parsing the whole buffer.
3. **LSP** — completion, hover, signature help, go-to-definition, find-references, rename, diagnostics, code actions. Multiple servers per workspace.
4. **File tree** with virtual scrolling, gitignore filtering, drag-to-reorder, inline rename, multi-select.
5. **Project search** (Cmd-Shift-F) backed by ripgrep, with live results and regex/case/whole-word toggles.
6. **Fuzzy file open** (Cmd-P) using nucleo.
7. **Command palette** (Cmd-Shift-P) — every action in the app is a command and is reachable here.
8. **Embedded terminal** with full color, mouse support, splits.
9. **Git** — sidebar with staged/unstaged/untracked, inline diff in the gutter, blame on hover, branch switcher.
10. **Tabs and splits** — drag-tear, drag-merge, split horizontal/vertical with smooth resize.
11. **Theme system** — TOML-defined, hot-reloadable, ship at least three: `Eden Day`, `Eden Dusk`, `Eden Noir`.
12. **Settings UI** — actual native settings panel, not a JSON file (though JSON should also work for power users).

---

## 5. Signature Features — the differentiators

These are what make Eden worth installing. Build at least **six** of these to a finished state in v1. Pick the ones that compose well together; do not half-ship all of them.

- **Choreographed Diff** — when a refactor lands (LSP rename, code action, AI edit), the affected ranges don't just change; they animate. Old text dissolves, new text rises in, the cursor follows. Inspired by Keynote Magic Move.
- **Semantic Minimap** — replace the conventional minimap with a structural overview rendered from the tree-sitter AST: function shapes, comment blocks, import regions, all drawn as gentle glyphs rather than pixel-mush of source.
- **Focus Halo** — when you start typing, the surrounding UI exhales — sidebars fade 30%, tabs lose chrome, the gutter softens. Move the mouse and they breathe back in. Springy, never abrupt.
- **Constellation** — Cmd-K opens a graph view of the current file's symbols and their cross-references in the workspace, rendered as a force-directed constellation you can pan through. Click a star, jump there.
- **Time Scrubber** — every buffer maintains a rich undo history; a horizontal scrubber at the bottom edge lets you drag through your edit history with a live preview, like scrubbing video. Released in 2026 because nothing else has gotten this right.
- **Ambient Compile** — when `cargo check` or the LSP reports diagnostics, the gutter doesn't just show squiggles; the affected lines emit a faint colored bloom that decays over 1.2s. Errors glow rose, warnings amber. You feel the problem before you read it.
- **Whisper Palette** — the command palette accepts natural-language phrases ("close all but this", "split right and open lib.rs", "show git blame for this line") matched against a curated intent registry. No LLM call required for the common ones; they're fuzzy-matched against intent strings.
- **Linked Cursors** — multi-cursor across split panes. Edit a function signature in one pane, call sites update in the other if they're structurally linked via tree-sitter.
- **Quiet Mode** — a global modifier (hold Caps Lock) that mutes all animation, all chrome, all color saturation drops 40% — for when you need to think and the product needs to disappear.

Document each shipped signature feature in `README.md` with a GIF or screen recording.

---

## 6. Visual Design Language

**Typography**
- UI: **Inter** at 13px base, with optical sizes. Tabular numerals everywhere numerals appear.
- Editor: **JetBrains Mono** as default, with **Berkeley Mono** and **Geist Mono** as bundled alternatives. Ligatures on by default.
- Line height in the editor: 1.55. Letter-spacing: 0. Never compress.

**Color**
- Three first-party themes, all hand-tuned, all WCAG AA on body text:
  - `Eden Day` — warm paper white background (#FBF8F3), ink (#1B1B1F), accents kingfisher-blue (#2A6BC8) and amber (#C77B2C).
  - `Eden Dusk` — desaturated navy (#1A1F2E) base, off-white text (#E6E4DC), accents in muted teal and rose.
  - `Eden Noir` — near-black (#0E0E10), high-contrast, single accent in molten gold.
- Syntax palettes are tuned per-theme by a human, not autogenerated. No neon. No saturation above 75% anywhere in the app.

**Surfaces**
- Single elevation level in light mode (no shadows). In dark modes, one soft shadow layer for floating panels (command palette, hover cards): `0 10px 40px rgba(0,0,0,0.35)`.
- Corner radius scale: 4 / 8 / 12 / 16. Never odd numbers, never 6, never 10. Pick from the scale.
- Dividers are 1px, color `text * 0.08`, never pure black.

**Spacing**
- 4px base grid. Layout in multiples of 4. The grid is sacred.

**Icons**
- Custom 1.5px-stroke SVG set, 16px and 20px sizes. Inspired by Phosphor's regular weight and SF Symbols' geometric clarity. No filled icons except for states (active, selected).

**References to study and internalize before writing UI code**
- Linear (web app) — surface restraint, motion choreography
- Raycast — command surfaces, density
- Arc browser — sidebar behavior, transitions between modes
- Things 3 — typography on macOS, restraint
- Tot — extreme reduction
- Zed — what's achievable in Rust on the GPU
- Nothing OS — the dot-matrix discipline (use sparingly, as Easter eggs)

---

## 7. Animation Principles

This is where most editors fail. Eden will not.

1. **Spring physics, not easings.** Every transition that moves, scales, or fades uses a critically-damped spring with `stiffness=170, damping=26` as the default. Tune per-interaction; document the spring in the call site.
2. **Choreography.** When more than one element animates simultaneously, stagger them by 20–40ms. Things that move together feel cheap; things that move in sequence feel directed.
3. **The 60ms rule.** No animation under 60ms (the eye reads it as a jump). No state-change animation over 350ms (the user is waiting on you).
4. **Reduced motion.** Honor the OS-level "reduce motion" preference. Reduce, don't eliminate — keep ≤80ms cross-fades.
5. **Motion is information.** Every animation answers a question: where did this come from, where did it go, what changed. Decorative animation is forbidden. If you can't articulate the question an animation answers, delete it.
6. **The cursor is alive.** The text cursor doesn't blink on a fixed interval — it pulses on a slow sine wave (~1.1s period), and on keystroke it momentarily brightens and snaps to the new position with a 90ms spring.
7. **Frame budget.** 120Hz target on capable displays, 60Hz floor everywhere. Profile with `puffin` or `tracy`. No frame may exceed 8ms of CPU work in the hot path.

---

## 8. Build Plan — phased

**Phase 0 — Skeleton (Day 1)**
Workspace, crates, window opens via winit, wgpu device initialized, vello renders a single rounded rectangle in the brand color. Verify it runs on macOS, Linux, and Windows. Commit.

**Phase 1 — The Surface (Days 2–3)**
- `Eden-ui`: widget trait, layout via taffy, dirty-region invalidation, hit testing.
- `Eden-motion`: spring solver, animation driver tied to the frame loop.
- Theme loading. Render the empty editor chrome: title bar, sidebar shell, tab strip, status bar — all themed, all spring-resizable.

**Phase 2 — The Buffer (Days 4–6)**
- `Eden-editor`: ropey-backed buffer, cursors, selections, undo tree, edit operations.
- Text rendering through cosmic-text + vello. Caret. Selection highlighting. Soft wrap. Smooth scroll (spring-driven, momentum-based, not linear).

**Phase 3 — Syntax & Files (Days 7–9)**
- `Eden-syntax`: tree-sitter wiring, incremental highlighting.
- `Eden-workspace`: project model, file tree widget, gitignore-filtered walk.
- Cmd-P fuzzy open via nucleo.

**Phase 4 — Intelligence (Days 10–13)**
- `Eden-lsp`: client, server pool, completion popup, hover card, diagnostics in gutter and inline.
- Code actions, rename, go-to-definition with animated jump (the editor scrolls to destination with a spring; cursor traces a brief ghost trail).

**Phase 5 — Surroundings (Days 14–16)**
- Project search (Cmd-Shift-F).
- Command palette with the natural-language intent registry.
- Embedded terminal via alacritty_terminal.
- Git sidebar, gutter diff markers, blame on hover.

**Phase 6 — Signature features (Days 17–22)**
Pick six from §5. Ship them to a polished state, each with its own demo recording in `README.md`.

**Phase 7 — Polish (Days 23–25)**
- Settings UI.
- Three themes finalized.
- App icon, `.app` bundle, signing-ready.
- Crash reporter, telemetry opt-in (off by default).
- A `CHANGELOG.md` and a serious `README.md`.

---

## 9. Quality Bar — non-negotiable

- **Cold start to first paint:** ≤ 250ms on M-series Mac, ≤ 500ms on mid-range Linux laptop.
- **Keystroke-to-glyph latency:** ≤ 16ms at 60Hz, ≤ 8ms at 120Hz, measured end-to-end.
- **Memory:** ≤ 250MB resident with a 10kLOC workspace open.
- **Binary size:** ≤ 60MB stripped (excluding bundled fonts).
- **Zero `unwrap()`** outside of tests and `main()`. Every fallible path returns `Result`.
- **Every public function in every crate has a doc comment.** No exceptions.
- **`cargo clippy -- -D warnings` is clean.**
- **Tests:** unit tests in every crate, integration tests for the editor (insert, delete, undo, multi-cursor, large file) and the LSP client (mock server, hover, completion).

---

## 10. How to work

- Commit after every phase. Conventional Commits format.
- Before adding any dependency not listed in §2, write one paragraph in the PR description justifying it.
- When you make a design choice (spring constants, color, spacing), leave a `// design:` comment with the reasoning. Future-you will thank present-you.
- If you find yourself reaching for an Electron-y pattern (HTML inside the app, JS plugin running, web view), stop. There is a better Rust answer; find it.
- When in doubt about a visual choice, study the references in §6 and choose restraint.
- Read every SKILL.md you have access to before generating files.

---

## 11. What "done" looks like

A `Eden` binary that opens in under a quarter-second, lets a serious engineer edit a real Rust project with full LSP, syntax, git, search, terminal, and three signature features they've never seen before — and that, when they switch back to their old editor, makes that old editor feel a little embarrassed.

Make it beautiful. Make it fast. Make it last.

Begin.
