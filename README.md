# Eden

A desktop code editor written in pure Rust, rendered on the GPU. Eden aims for
the seam between Zed's raw performance, Linear's design discipline, Raycast's
command-driven ergonomics, and the quiet confidence of native macOS apps.

> **Status: Phase 0 — Skeleton.** A winit window comes up, a wgpu device is
> initialised through vello, and a single rounded rectangle is rendered in the
> brand colour. The crate boundaries for the full product are in place; each is
> filled in over the phased build plan below.

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
