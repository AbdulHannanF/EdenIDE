//! Editor text rendering: cosmic-text for shaping, vello for drawing.
//!
//! Only the visible lines are ever shaped, so a 50MB file costs the same per
//! frame as a tiny one. cosmic-text lays out and shapes the glyphs (with system
//! font fallback); vello rasterises the outlines on the GPU. Eden's editor font
//! is monospace, so carets and selections are placed by `column * advance`
//! rather than by hunting individual glyph positions.

use std::collections::HashMap;

use cosmic_text::fontdb;
use cosmic_text::{Attrs, Buffer as CtBuffer, Family, FontSystem, Metrics, Shaping, Weight};
use eden_editor::Editor;
use eden_syntax::{HighlightKind, Highlights};
use eden_theme::{Palette, Rgba8, Syntax};
use vello::kurbo::{Affine, Point, Rect};
use vello::peniko::{Blob, Fill, FontData};
use vello::{Glyph, Scene};

use crate::paint::{fill_rect, fill_rrect, to_color};

// design: editor type at 14px logical, line height 1.55 (§6: "never compress").
const FONT_SIZE: f64 = 14.0;
const LINE_HEIGHT_FACTOR: f64 = 1.55;
// design: the default editor font. Bundling JetBrains Mono (§6) is pending the
// font asset; Consolas is a faithful monospace stand-in always present on
// Windows. Family resolution is the only thing that needs to change.
const EDITOR_FAMILY: Family<'static> = Family::Name("Consolas");

// design: diagnostic mark colours matching §5 Ambient Compile (rose / amber).
const MARK_ERROR: Rgba8 = Rgba8::rgb(0xE5, 0x53, 0x4B);
const MARK_WARN: Rgba8 = Rgba8::rgb(0xC7, 0x7B, 0x2C);

// ── gutter mark ───────────────────────────────────────────────────────────────

/// A diagnostic severity marker drawn in the editor gutter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GutterMark {
    /// A hard error.
    Error,
    /// A warning.
    Warning,
}

// ── view types ────────────────────────────────────────────────────────────────

/// Everything [`TextSystem::paint_editor`] needs for one frame.
pub struct EditorFrame<'a> {
    /// The absolute rect to draw into.
    pub area: Rect,
    /// The editor model to render.
    pub editor: &'a Editor,
    /// The palette (interpolated mid-crossfade).
    pub palette: &'a Palette,
    /// The syntax colours (interpolated mid-crossfade).
    pub syntax: &'a Syntax,
    /// Highlight spans for the whole document.
    pub highlights: &'a Highlights,
    /// Vertical scroll offset in physical pixels (clamped internally).
    pub scroll_px: f64,
    /// Display scale factor.
    pub scale: f64,
    /// Whether to draw carets (typically: window focused).
    pub show_caret: bool,
    /// Gutter markers, as `(zero-indexed line, mark)` pairs.
    pub gutter_marks: &'a [(u32, GutterMark)],
}

/// One row of the sidebar file tree to render.
pub struct TreeRow<'a> {
    /// File or directory name.
    pub name: &'a str,
    /// Nesting depth (root children are 0).
    pub depth: usize,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether an expanded directory.
    pub expanded: bool,
}

/// The sidebar file tree to render.
pub struct TreeView<'a> {
    /// The flattened, visible rows.
    pub rows: &'a [TreeRow<'a>],
    /// Vertical scroll offset in physical pixels (clamped internally).
    pub scroll_px: f64,
    /// The row index under the cursor, if any.
    pub hovered: Option<usize>,
}

/// The command-palette content to render.
pub struct PaletteView<'a> {
    /// The current query text.
    pub query: &'a str,
    /// The result rows (already filtered and ranked), top-first.
    pub entries: &'a [String],
    /// The index of the highlighted row.
    pub selected: usize,
}

/// A single item in the completion popup.
pub struct CompletionEntry {
    /// Display label.
    pub label: String,
    /// Optional secondary detail (type/module).
    pub detail: Option<String>,
}

/// The completion popup to render.
pub struct CompletionView<'a> {
    /// Items to show (already filtered, best-first).
    pub entries: &'a [CompletionEntry],
    /// Index of the currently highlighted item.
    pub selected: usize,
    /// Physical-pixel anchor point (top-left of the caret).
    pub anchor: Point,
}

// ── TextSystem ────────────────────────────────────────────────────────────────

/// Owns the font system and shaping buffers, and draws editor content.
pub struct TextSystem {
    font_system: FontSystem,
    text_buf: CtBuffer,
    aux_buf: CtBuffer,
    fonts: HashMap<fontdb::ID, FontData>,
    scale: f64,
    font_size_px: f64,
    line_h: f64,
    advance: f64,
}

impl TextSystem {
    /// Builds the text system for the given display scale.
    #[must_use]
    pub fn new(scale: f64) -> Self {
        let mut font_system = FontSystem::new();
        let (font_size_px, line_h) = Self::metrics_for(scale);
        let metrics = Metrics::new(font_size_px as f32, line_h as f32);
        let text_buf = CtBuffer::new(&mut font_system, metrics);
        let aux_buf = CtBuffer::new(&mut font_system, metrics);
        let mut system = Self {
            font_system,
            text_buf,
            aux_buf,
            fonts: HashMap::new(),
            scale,
            font_size_px,
            line_h,
            advance: font_size_px * 0.6,
        };
        system.measure_advance();
        system
    }

    /// The line height in physical pixels.
    #[must_use]
    pub fn line_height(&self) -> f64 {
        self.line_h
    }

    /// The glyph advance (character width) in physical pixels.
    #[must_use]
    pub fn advance(&self) -> f64 {
        self.advance
    }

    /// The gutter width for a buffer with `total_lines` lines, in physical px.
    #[must_use]
    pub fn gutter_width(&self, total_lines: usize) -> f64 {
        let digits = total_lines.max(1).to_string().len();
        self.advance * digits as f64 + 28.0 * self.scale
    }

    fn metrics_for(scale: f64) -> (f64, f64) {
        let font_size_px = FONT_SIZE * scale;
        (font_size_px, font_size_px * LINE_HEIGHT_FACTOR)
    }

    fn ensure_metrics(&mut self, scale: f64) {
        if (scale - self.scale).abs() < f64::EPSILON {
            return;
        }
        self.scale = scale;
        let (font_size_px, line_h) = Self::metrics_for(scale);
        self.font_size_px = font_size_px;
        self.line_h = line_h;
        let metrics = Metrics::new(font_size_px as f32, line_h as f32);
        self.text_buf.set_metrics(metrics);
        self.aux_buf.set_metrics(metrics);
        self.measure_advance();
    }

    fn measure_advance(&mut self) {
        shape_buffer(
            &mut self.aux_buf,
            &mut self.font_system,
            "0000000000",
            None,
            self.line_h as f32,
        );
        let width = self
            .aux_buf
            .layout_runs()
            .next()
            .map_or(0.0, |run| f64::from(run.line_w));
        if width > 0.0 {
            self.advance = width / 10.0;
        }
    }

    /// Builds (and caches) a vello font for a cosmic-text font id.
    fn font_for(&mut self, id: fontdb::ID) -> Option<FontData> {
        if let Some(font) = self.fonts.get(&id) {
            return Some(font.clone());
        }
        let index = self.font_system.db().face(id).map_or(0, |face| face.index);
        let font = self.font_system.get_font(id, Weight::NORMAL)?;
        let blob = Blob::new(std::sync::Arc::new(font.data().to_vec()));
        let data = FontData::new(blob, index);
        self.fonts.insert(id, data.clone());
        Some(data)
    }

    // ── editor ────────────────────────────────────────────────────────────

    /// Paints the editor's gutter, text, selections, and carets into `area`.
    ///
    /// `scroll_px` is the vertical scroll offset in physical pixels; it is
    /// clamped to the content height here and the clamped value returned, so
    /// the caller can keep its scroll spring in range.
    pub fn paint_editor(&mut self, scene: &mut Scene, frame: &EditorFrame<'_>) -> f64 {
        let EditorFrame {
            area,
            editor,
            palette,
            syntax,
            highlights,
            scroll_px,
            scale,
            show_caret,
            gutter_marks,
        } = *frame;
        self.ensure_metrics(scale);
        let buffer = editor.buffer();
        let total_lines = buffer.len_lines();
        let line_h = self.line_h;
        let advance = self.advance;

        let digits = total_lines.max(1).to_string().len();
        let gutter_w = advance * digits as f64 + 28.0 * scale;
        let text_x = area.x0 + gutter_w;

        let max_scroll = (total_lines as f64 * line_h - area.height()).max(0.0);
        let scroll = scroll_px.clamp(0.0, max_scroll);
        let first_line = (scroll / line_h).floor() as usize;
        let frac = scroll - first_line as f64 * line_h;
        let rows = ((area.height() + frac) / line_h).ceil() as usize + 1;
        let last_line = (first_line + rows).min(total_lines);

        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &area);

        // Canvas + gutter band.
        fill_rect(scene, area, palette.background);
        fill_rect(scene, Rect::new(area.x0, area.y0, text_x, area.y1), palette.surface);

        let row_top = |line: usize| area.y0 - frac + (line - first_line) as f64 * line_h;

        // Gutter marks (diagnostic dots at far-left of gutter).
        let mark_sz = 5.0 * scale;
        let mark_x = area.x0 + 4.0 * scale;
        for &(mark_line, mark) in gutter_marks.iter() {
            let ml = mark_line as usize;
            if ml < first_line || ml >= last_line {
                continue;
            }
            let y = row_top(ml);
            let my = y + (line_h - mark_sz) / 2.0;
            let color = match mark {
                GutterMark::Error => MARK_ERROR,
                GutterMark::Warning => MARK_WARN,
            };
            fill_rrect(
                scene,
                Rect::new(mark_x, my, mark_x + mark_sz, my + mark_sz),
                mark_sz / 2.0,
                color,
            );
        }

        // Selection highlights, behind the text.
        for sel in editor.selections() {
            if sel.is_empty() {
                continue;
            }
            let (start, end) = (sel.start(), sel.end());
            let first = buffer.char_to_line(start).max(first_line);
            let last = buffer.char_to_line(end).min(last_line.saturating_sub(1));
            for line in first..=last {
                let line_start = buffer.line_to_char(line);
                let line_len = buffer.line_len(line);
                let col0 = start.saturating_sub(line_start).min(line_len);
                let col1 = if buffer.char_to_line(end) == line {
                    end - line_start
                } else {
                    line_len + 1
                };
                let y = row_top(line);
                let x0 = text_x + col0 as f64 * advance;
                let x1 = text_x + col1 as f64 * advance;
                fill_rect(
                    scene,
                    Rect::new(x0, y, x1.max(x0 + 2.0), y + line_h),
                    palette.selection,
                );
            }
        }

        // Text glyphs for the visible lines.
        let char_start = buffer.line_to_char(first_line);
        let char_end = buffer.line_to_char(last_line);
        let visible = buffer.slice_to_string(char_start..char_end);
        shape_buffer(
            &mut self.text_buf,
            &mut self.font_system,
            &visible,
            None,
            (rows as f64 * line_h) as f32,
        );
        let rope = buffer.rope();
        let total_chars = buffer.len_chars();
        let runs: Vec<RunData> = self
            .text_buf
            .layout_runs()
            .enumerate()
            .map(|(i, run)| {
                let line_start = buffer.line_to_char(first_line + i);
                let glyphs = run
                    .glyphs
                    .iter()
                    .enumerate()
                    .map(|(col, g)| {
                        let char_idx = line_start + col;
                        let kind = if char_idx < total_chars {
                            highlights.kind_at(rope.char_to_byte(char_idx))
                        } else {
                            HighlightKind::Default
                        };
                        GlyphData::new(g, syntax_color(syntax, kind, palette.text))
                    })
                    .collect();
                RunData { line_y: run.line_y, glyphs }
            })
            .collect();
        let origin_y = area.y0 - frac;
        for run in &runs {
            self.draw_glyphs(scene, &run.glyphs, text_x, origin_y + f64::from(run.line_y));
        }

        // Line numbers, right-aligned in the gutter.
        let mut numbers = String::new();
        for line in first_line..last_line {
            use std::fmt::Write as _;
            let _ = writeln!(numbers, "{:>width$}", line + 1, width = digits);
        }
        shape_buffer(
            &mut self.aux_buf,
            &mut self.font_system,
            numbers.trim_end_matches('\n'),
            None,
            (rows as f64 * line_h) as f32,
        );
        let number_color = with_alpha(palette.text_muted, 0xB0);
        let number_runs: Vec<RunData> = self
            .aux_buf
            .layout_runs()
            .map(|run| RunData {
                line_y: run.line_y,
                glyphs: run.glyphs.iter().map(|g| GlyphData::new(g, number_color)).collect(),
            })
            .collect();
        let number_x = area.x0 + 12.0 * scale;
        for run in &number_runs {
            self.draw_glyphs(scene, &run.glyphs, number_x, origin_y + f64::from(run.line_y));
        }

        // Carets on top.
        if show_caret {
            for sel in editor.selections() {
                let line = buffer.char_to_line(sel.head);
                if line < first_line || line >= last_line {
                    continue;
                }
                let col = sel.head - buffer.line_to_char(line);
                let x = text_x + col as f64 * advance;
                let y = row_top(line);
                let caret_w = (2.0 * scale).max(1.5);
                fill_rect(
                    scene,
                    Rect::new(x, y + 2.0 * scale, x + caret_w, y + line_h - 2.0 * scale),
                    palette.accent,
                );
            }
        }

        scene.pop_layer();
        scroll
    }

    // ── sidebar file tree ─────────────────────────────────────────────────

    /// Paints the sidebar file tree into `area` with virtual scrolling.
    /// Returns the clamped scroll offset.
    pub fn paint_file_tree(
        &mut self,
        scene: &mut Scene,
        area: Rect,
        view: &TreeView<'_>,
        palette: &Palette,
        scale: f64,
    ) -> f64 {
        let rows = view.rows;
        let scroll_px = view.scroll_px;
        let hovered = view.hovered;
        self.ensure_metrics(scale);
        let row_h = self.line_h;

        let max_scroll = (rows.len() as f64 * row_h - area.height()).max(0.0);
        let scroll = scroll_px.clamp(0.0, max_scroll);
        let first = (scroll / row_h).floor() as usize;
        let frac = scroll - first as f64 * row_h;
        let count = ((area.height() + frac) / row_h).ceil() as usize + 1;
        let last = (first + count).min(rows.len());

        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &area);
        fill_rect(scene, area, palette.surface);

        for (i, row) in rows.iter().enumerate().take(last).skip(first) {
            let top = area.y0 - frac + (i - first) as f64 * row_h;
            if hovered == Some(i) {
                fill_rect(
                    scene,
                    Rect::new(area.x0, top, area.x1, top + row_h),
                    with_alpha(palette.accent, 0x16),
                );
            }
            let indent = area.x0 + 10.0 * scale + row.depth as f64 * 14.0 * scale;
            let marker = 7.0 * scale;
            let my = top + row_h * 0.5 - marker / 2.0;
            if row.is_dir {
                let color = if row.expanded {
                    palette.accent
                } else {
                    with_alpha(palette.text_muted, 0xC0)
                };
                fill_rrect(
                    scene,
                    Rect::new(indent, my, indent + marker, my + marker),
                    2.0 * scale,
                    color,
                );
            } else {
                let dot = marker * 0.55;
                let dy = top + row_h * 0.5 - dot / 2.0;
                fill_rrect(
                    scene,
                    Rect::new(
                        indent + (marker - dot) / 2.0,
                        dy,
                        indent + (marker + dot) / 2.0,
                        dy + dot,
                    ),
                    dot / 2.0,
                    with_alpha(palette.text_muted, 0x80),
                );
            }
            let name_x = indent + marker + 8.0 * scale;
            let baseline = top + row_h * 0.7;
            let color = if row.is_dir {
                palette.text
            } else {
                with_alpha(palette.text, 0xDD)
            };
            self.draw_text(scene, row.name, name_x, baseline, color);
        }

        scene.pop_layer();
        scroll
    }

    // ── command palette ───────────────────────────────────────────────────

    /// Paints the Cmd-P style command palette as a floating overlay over
    /// `screen`. Returns the number of result rows actually drawn.
    pub fn paint_palette(
        &mut self,
        scene: &mut Scene,
        screen: Rect,
        view: &PaletteView<'_>,
        palette: &Palette,
        scale: f64,
    ) -> usize {
        self.ensure_metrics(scale);
        fill_rect(scene, screen, Rgba8::rgba(0, 0, 0, 0x4D));

        let row_h = 30.0 * scale;
        let query_h = 44.0 * scale;
        let max_rows = 12usize;
        let shown = view.entries.len().min(max_rows);

        let width = (screen.width() * 0.6).clamp(360.0 * scale, 760.0 * scale);
        let height = query_h + shown as f64 * row_h + 12.0 * scale;
        let x0 = screen.x0 + (screen.width() - width) / 2.0;
        let y0 = screen.y0 + 76.0 * scale;
        let panel = Rect::new(x0, y0, x0 + width, y0 + height);

        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &panel);
        fill_rrect(scene, panel, 12.0 * scale, palette.surface_raised);

        let pad = 18.0 * scale;
        let query_baseline = y0 + query_h * 0.64;
        let prompt = if view.query.is_empty() {
            "Open file…".to_string()
        } else {
            view.query.to_string()
        };
        let prompt_color = if view.query.is_empty() {
            with_alpha(palette.text_muted, 0xC0)
        } else {
            palette.text
        };
        self.draw_text(scene, &prompt, x0 + pad, query_baseline, prompt_color);
        let div = scale.max(1.0);
        fill_rect(
            scene,
            Rect::new(x0, y0 + query_h, x0 + width, y0 + query_h + div),
            palette.divider,
        );

        for (i, entry) in view.entries.iter().take(shown).enumerate() {
            let row_top = y0 + query_h + i as f64 * row_h;
            if i == view.selected {
                fill_rect(
                    scene,
                    Rect::new(x0, row_top, x0 + width, row_top + row_h),
                    palette.selection,
                );
            }
            let color = if i == view.selected {
                palette.text
            } else {
                with_alpha(palette.text, 0xCC)
            };
            self.draw_text(scene, entry, x0 + pad, row_top + row_h * 0.66, color);
        }

        scene.pop_layer();
        shown
    }

    // ── hover card ────────────────────────────────────────────────────────

    /// Paints a floating hover tooltip anchored to `anchor` (physical-pixel
    /// top of the hovered line).
    pub fn paint_hover_card(
        &mut self,
        scene: &mut Scene,
        screen: Rect,
        anchor: Point,
        content: &str,
        palette: &Palette,
        scale: f64,
    ) {
        self.ensure_metrics(scale);
        // Keep to first 6 non-empty lines; strip markdown fence delimiters.
        let lines: Vec<&str> = content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("```"))
            .take(6)
            .collect();
        if lines.is_empty() {
            return;
        }

        let pad = 10.0 * scale;
        let row_h = self.line_h;
        let width = (screen.width() * 0.5).clamp(220.0 * scale, 560.0 * scale);
        let height = lines.len() as f64 * row_h + pad * 2.0;

        // Position above the anchor if space allows, otherwise below.
        let y_above = anchor.y - height - 4.0 * scale;
        let y_below = anchor.y + self.line_h + 4.0 * scale;
        let y0 = if y_above >= screen.y0 { y_above } else { y_below };
        let x0 = anchor.x.clamp(screen.x0 + 8.0 * scale, screen.x1 - width - 8.0 * scale);
        let card = Rect::new(x0, y0, x0 + width, y0 + height);

        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &card);
        fill_rrect(scene, card, 8.0 * scale, palette.surface_raised);

        for (i, line) in lines.iter().enumerate() {
            // Clip very long lines visually.
            let display = if line.len() > 100 { &line[..100] } else { line };
            let baseline = y0 + pad + (i as f64 + 0.72) * row_h;
            self.draw_text(scene, display, x0 + pad, baseline, palette.text);
        }

        scene.pop_layer();
    }

    // ── completion popup ──────────────────────────────────────────────────

    /// Paints the completion popup below (or above) the caret.
    pub fn paint_completion(
        &mut self,
        scene: &mut Scene,
        screen: Rect,
        view: &CompletionView<'_>,
        palette: &Palette,
        scale: f64,
    ) {
        if view.entries.is_empty() {
            return;
        }
        self.ensure_metrics(scale);

        let shown = view.entries.len().min(8);
        let row_h = 26.0 * scale;
        let pad_x = 12.0 * scale;
        let width = (screen.width() * 0.35).clamp(200.0 * scale, 440.0 * scale);
        let height = shown as f64 * row_h + 6.0 * scale;

        // Prefer below the caret; flip up if near the bottom.
        let y_below = view.anchor.y + self.line_h + 2.0 * scale;
        let y_above = view.anchor.y - height - 2.0 * scale;
        let y0 = if y_below + height <= screen.y1 { y_below } else { y_above };
        let x0 = view.anchor.x.clamp(screen.x0, screen.x1 - width);
        let panel = Rect::new(x0, y0, x0 + width, y0 + height);

        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &panel);
        fill_rrect(scene, panel, 8.0 * scale, palette.surface_raised);

        for (i, entry) in view.entries.iter().take(shown).enumerate() {
            let row_top = y0 + 3.0 * scale + i as f64 * row_h;
            if i == view.selected {
                fill_rect(
                    scene,
                    Rect::new(x0, row_top, x0 + width, row_top + row_h),
                    palette.selection,
                );
            }
            let label_color = if i == view.selected {
                palette.text
            } else {
                with_alpha(palette.text, 0xCC)
            };
            self.draw_text(scene, &entry.label, x0 + pad_x, row_top + row_h * 0.7, label_color);
            if let Some(ref detail) = entry.detail {
                let detail_color = with_alpha(palette.text_muted, 0xA0);
                // Right-align detail inside the panel.
                let detail_x = x0 + width - pad_x - detail.len() as f64 * self.advance * 0.6;
                if detail_x > x0 + width * 0.5 {
                    self.draw_text(scene, detail, detail_x, row_top + row_h * 0.7, detail_color);
                }
            }
        }

        scene.pop_layer();
    }

    // ── shared drawing helpers ────────────────────────────────────────────

    /// Draws a single line of UI text with its baseline at `baseline`.
    pub fn draw_text(&mut self, scene: &mut Scene, text: &str, x: f64, baseline: f64, color: Rgba8) {
        shape_buffer(&mut self.aux_buf, &mut self.font_system, text, None, self.line_h as f32);
        let runs: Vec<RunData> = self
            .aux_buf
            .layout_runs()
            .map(|run| RunData {
                line_y: run.line_y,
                glyphs: run.glyphs.iter().map(|g| GlyphData::new(g, color)).collect(),
            })
            .collect();
        for run in &runs {
            self.draw_glyphs(scene, &run.glyphs, x, baseline);
        }
    }

    fn draw_glyphs(
        &mut self,
        scene: &mut Scene,
        glyphs: &[GlyphData],
        origin_x: f64,
        baseline: f64,
    ) {
        let mut i = 0;
        while i < glyphs.len() {
            let font_id = glyphs[i].font_id;
            let color = glyphs[i].color;
            let start = i;
            while i < glyphs.len() && glyphs[i].font_id == font_id && glyphs[i].color == color {
                i += 1;
            }
            let Some(font) = self.font_for(font_id) else {
                continue;
            };
            let slice = &glyphs[start..i];
            scene
                .draw_glyphs(&font)
                .font_size(self.font_size_px as f32)
                .brush(to_color(color))
                .transform(Affine::translate((origin_x, baseline)))
                .draw(
                    Fill::NonZero,
                    slice.iter().map(|g| Glyph {
                        id: u32::from(g.glyph_id),
                        x: g.x,
                        y: g.y,
                    }),
                );
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn with_alpha(color: Rgba8, alpha: u8) -> Rgba8 {
    Rgba8::rgba(color.r, color.g, color.b, alpha)
}

fn shape_buffer(
    buf: &mut CtBuffer,
    fs: &mut FontSystem,
    text: &str,
    width: Option<f32>,
    height: f32,
) {
    let attrs = Attrs::new().family(EDITOR_FAMILY);
    buf.set_text(text, &attrs, Shaping::Advanced, None);
    buf.set_size(width, Some(height));
    buf.shape_until_scroll(fs, false);
}

/// Maps a tree-sitter highlight kind to a theme syntax colour.
fn syntax_color(syntax: &Syntax, kind: HighlightKind, default: Rgba8) -> Rgba8 {
    match kind {
        HighlightKind::Keyword | HighlightKind::Label => syntax.keyword,
        HighlightKind::Function => syntax.function,
        HighlightKind::Type | HighlightKind::Constructor => syntax.type_,
        HighlightKind::Property | HighlightKind::Variable => syntax.variable,
        HighlightKind::Constant => syntax.constant,
        HighlightKind::String | HighlightKind::Escape => syntax.string,
        HighlightKind::Comment => syntax.comment,
        HighlightKind::Operator => syntax.operator,
        HighlightKind::Punctuation => syntax.punctuation,
        HighlightKind::Attribute => syntax.attribute,
        HighlightKind::Default => default,
    }
}

// ── per-glyph data ────────────────────────────────────────────────────────────

/// Per-glyph data decoupled from cosmic-text's borrow so we can shape into
/// `text_buf` and then draw without holding an immutable borrow across the
/// mutable `font_for` calls.
struct GlyphData {
    font_id: fontdb::ID,
    glyph_id: u16,
    x: f32,
    y: f32,
    color: Rgba8,
}

impl GlyphData {
    fn new(glyph: &cosmic_text::LayoutGlyph, color: Rgba8) -> Self {
        Self {
            font_id: glyph.font_id,
            glyph_id: glyph.glyph_id,
            x: glyph.x,
            y: glyph.y,
            color,
        }
    }
}

struct RunData {
    line_y: f32,
    glyphs: Vec<GlyphData>,
}
