# Eden

A GPU-rendered code editor built from scratch in Rust. **Phases 0–7 complete.**

Eden aims for the seam between Zed's raw performance, Linear's design discipline, Raycast's
command-driven ergonomics, and the quiet confidence of native apps.

---

<img width="1517" height="862" alt="Screenshot 2026-05-27 103432" src="https://github.com/user-attachments/assets/72139f05-f158-4443-a6a8-b179e33f2006" />


## What makes Eden different

| Feature | Description |
|---------|-------------|
| **Ambient Compile** | LSP errors cast a soft bloom glow behind the affected line — you feel the health of the file before you read the diagnostics. |
| **Focus Halo** | The sidebar and tab strip dim while you type, keeping your eyes on the text. They breathe back when you move the cursor into chrome. |
| **Whisper Palette** | `Ctrl+Shift+P` understands natural language: "show files", "undo last change", "look for text" — not just exact command labels. |
| **Time Scrubber** | `Ctrl+Shift+H` reveals a horizontal undo-history bar. Drag left to undo, right to redo. |
| **Semantic Minimap** | `Ctrl+M` shows a minimap coloured by tree-sitter syntax kinds rather than a blurry pixel downscale. |
| **Choreographed Diff** | On `F12` (go to definition) or search navigation, a ghost of the caret's previous position fades out. |

---

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+P` | Fuzzy file finder |
| `Ctrl+Shift+P` | Command palette (natural-language search) |
| `Ctrl+Shift+F` | Project search (regex / case / word) |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+T` | Cycle theme (Day → Dusk → Noir) |
| `` Ctrl+` `` | Toggle terminal |
| `Ctrl+M` | Toggle semantic minimap |
| `Ctrl+Shift+H` | Toggle time scrubber |
| `Ctrl+,` | Settings panel |
| `Ctrl+Z` / `Ctrl+Y` | Undo / Redo |
| `Ctrl+Space` | Trigger completions |
| `F12` | Go to definition |
| `Shift+scroll` | Horizontal scroll |

---

## Build

**Requirements:** Rust stable, Windows 11, DX12-capable GPU.

```powershell
cargo build                             # debug build
cargo run -p eden-app                   # open the Eden repo itself
cargo run -p eden-app -- /path/to/proj  # open a specific directory
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

---

## Architecture

```
crates/
  eden-app/        Event loop, GPU init, render glue (winit + wgpu + vello)
  eden-ui/         Chrome layout (taffy), text rendering (cosmic-text), widgets
  eden-motion/     Spring physics solver, MotionPrefs
  eden-editor/     Rope buffer (ropey), multi-cursor, undo history
  eden-syntax/     Tree-sitter Rust highlighting
  eden-search/     Nucleo fuzzy matcher + ripgrep project search
  eden-workspace/  Gitignore-aware file tree and project model
  eden-theme/      3 built-in themes + TOML serialisation
  eden-lsp/        Async LSP client (rust-analyzer over stdio)
  eden-vcs/        Git diff hunks and blame (git2)
  eden-terminal/   Embedded PTY terminal (alacritty_terminal)
assets/
  fonts/           JetBrains Mono Regular (bundled)
themes/
  eden-day.toml / eden-dusk.toml / eden-noir.toml
```

---

## Themes

- **Eden Day** — warm paper background, ink text, kingfisher-blue accent
- **Eden Dusk** — deep navy, soft gold accent
- **Eden Noir** — near-black, warm gold accent

All themes crossfade via spring animation on `Ctrl+T`.

---

## Architecture

Eden is a Cargo workspace. The crate split is the load-bearing decision that has
to hold up at 100k LOC, so it is kept clean from day one.

| Crate | Responsibility |
| --- | --- |
| `eden-app` | The binary: window, GPU device, event loop, glue |
| `eden-ui` | Widget tree, layout (taffy), render passes, theming |
| `eden-motion` | Spring-physics animations, transitions, choreography |
| `eden-editor` | Buffer (ropey), cursors, selections, undo tree, edits |
| `eden-syntax` | Tree-sitter integration, incremental highlighting, indents |
| `eden-lsp` | Language Server Protocol client pool |
| `eden-search` | Fuzzy file matching (nucleo) + content search (ripgrep) |
| `eden-vcs` | Git integration: status, blame, diff, branches |
| `eden-terminal` | Embedded terminal (alacritty_terminal) as a UI widget |
| `eden-workspace` | Project model, file tree, sessions |
| `eden-theme` | Theme schema, parser, built-in themes |
| `eden-plugin` | Plugin host (WASM via wasmtime; stubbed in v1) |

## Rendering spine

`winit` (windowing/input) → `vello` (GPU 2D vector renderer) → `wgpu` (DX12 on
Windows, Metal on macOS, Vulkan on Linux). vello renders into an intermediate
storage texture which is blitted onto the swapchain image each frame.

## Building

```sh
cargo run -p eden-app        # or: cargo run --bin eden
```

The first build compiles the full GPU stack (vello, wgpu, naga) and takes a few
minutes. Subsequent builds are incremental.
