# Changelog

All notable changes to Eden are documented here.

## Phase 7 — Polish (current)

- **JetBrains Mono** bundled in `assets/fonts/`; loaded at startup with Consolas fallback.
- **Pulsing caret** — sine-wave brightness cycle at 1.1 s period; bright spike on each keystroke.
- **Click-to-place caret** — left-click in the editor canvas moves the caret to the clicked position.
- **Horizontal scrollbar** — 4 px thumb, fades 1.5 s after last scroll event; Shift+scroll or natural horizontal scroll to pan.
- **Settings panel** — `Ctrl+,` opens a floating panel showing font size, tab width, active theme, and feature toggle states.

## Phase 6 — Signature Features

- **Ambient Compile** — multi-pass bloom glow behind LSP error/warning lines.
- **Focus Halo** — sidebar and tab strip dim while typing; breathe back on cursor hover.
- **Whisper Palette** — natural-language intent phrases in `Ctrl+Shift+P` command matching.
- **Time Scrubber** — `Ctrl+Shift+H` reveals a horizontal undo-history bar; drag to travel.
- **Semantic Minimap** — `Ctrl+M` shows a syntax-coloured minimap on the editor right edge.
- **Choreographed Diff** — ghost caret fades from the old position on `F12` / search jumps.

## Phase 5 — Surroundings

- **Project search** — `Ctrl+Shift+F` opens a streaming ripgrep-backed search panel with regex/case/whole-word toggles.
- **Command palette** — `Ctrl+Shift+P` with Whisper intent matching across 13 built-in commands.
- **Terminal** — `Ctrl+\`` toggles an embedded PTY terminal (alacritty_terminal backend).
- **Git sidebar** — diff hunk markers (added/modified/deleted) in the editor gutter via `git2`.

## Phase 4 — Intelligence

- **LSP client** — async rust-analyzer integration via JSON-RPC over stdio.
- **Diagnostics** — squiggle-free gutter dots (rose = error, amber = warning) from `publishDiagnostics`.
- **Hover** — floating card appears after 400 ms cursor idle.
- **Completions** — `Ctrl+Space` popup, Tab/Enter to commit.
- **Go to definition** — `F12` navigates to the definition with a ghost caret left behind.

## Phase 3 — Syntax & Files

- **Tree-sitter Rust** highlighting — 15 syntax kinds mapped to theme colours.
- **File tree** — gitignore-aware lazy expand/collapse sidebar with hover states.
- **Cmd-P** — nucleo fuzzy file finder over gitignore-filtered project files.

## Phase 2 — The Buffer

- **Rope buffer** — ropey-backed, 50 MB capable, with multi-cursor selections.
- **Cosmic-text rendering** — GPU glyph rasterisation via vello.
- **Undo/redo** — coalesced keystroke runs, `Ctrl+Z` / `Ctrl+Y`.
- **Scroll** — spring-animated vertical scroll with caret follow.

## Phase 1 — The Surface

- **Chrome layout** — title bar, sidebar, tab strip, editor canvas, status bar via taffy flex.
- **Spring motion** — sidebar toggle, theme crossfade, hover glow (stiffness=170, damping=26).
- **3 themes** — Eden Day (warm paper), Eden Dusk (navy), Eden Noir (near-black gold).

## Phase 0 — Skeleton

- winit 0.30 event loop, wgpu/vello DX12 renderer, HiDPI-aware window.
