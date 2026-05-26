# Eden

A desktop code editor written in pure Rust, rendered on the GPU. Eden aims for
the seam between Zed's raw performance, Linear's design discipline, Raycast's
command-driven ergonomics, and the quiet confidence of native macOS apps.

> **Status: Phase 3 — Syntax & Files.** On top of the editable buffer (Phase 2),
> the editor now has **tree-sitter syntax highlighting** (Rust grammar; glyphs
> coloured per highlight kind, crossfading with the theme), a **gitignore-aware
> sidebar file tree** (lazy expand, virtual-scrolled, click to open), and
> **Cmd-P fuzzy file open** (Ctrl+P; nucleo-ranked, opens into the editor).
>
> Editing (Phase 2): ropey buffer, multi-cursor, selections, snapshot undo/redo
> (typing coalesces), spring momentum scroll. Type to insert; arrows move (Shift
> extends); Backspace/Delete/Home/End/Enter/Tab; Ctrl+Z / Ctrl+Shift+Z; Ctrl+A.
> Ctrl+B toggles the sidebar, Ctrl+T crossfades the theme, Ctrl+P opens files.
>
> Known gaps carried forward: incremental (InputEdit) re-highlighting, soft-wrap,
> block/column selection, a pulsing caret, click-to-place-caret in the editor,
> more grammars, and bundling JetBrains Mono (currently system Consolas).

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

## Build plan

Phase 0 Skeleton · Phase 1 The Surface · Phase 2 The Buffer · Phase 3 Syntax &
Files · Phase 4 Intelligence (LSP) · Phase 5 Surroundings · Phase 6 Signature
features · Phase 7 Polish.

## License

Licensed under either of Apache-2.0 or MIT at your option.
