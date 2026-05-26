//! `eden-terminal` — embedded terminal with a real PTY backend.
//!
//! [`TerminalBackend`] spawns a shell using `portable-pty` (ConPTY on Windows,
//! native PTY on Unix) and parses the output with the `vte` VT100/ANSI parser.
//! The resulting cell grid is stored in a [`parking_lot::RwLock`] that the
//! render thread reads on each frame without blocking.
//!
//! Keyboard input is sent via [`TerminalBackend::write`]. Terminal dimensions
//! are updated via [`TerminalBackend::resize`].

use std::io::{Read, Write};
use std::sync::Arc;

use anyhow::{Context as _, Result};
use eden_theme::Rgba8;
use parking_lot::RwLock;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

// ── cell types ────────────────────────────────────────────────────────────────

/// How a terminal cell colour is specified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TermColor {
    /// Use the terminal's default foreground/background.
    #[default]
    Default,
    /// One of the 256 standard ANSI palette colours.
    Index(u8),
    /// 24-bit truecolor.
    Rgb(u8, u8, u8),
}

/// A single cell in the terminal grid.
#[derive(Clone, Debug)]
pub struct TermCell {
    /// The printable character, or `' '` for an empty cell.
    pub ch: char,
    /// Foreground colour.
    pub fg: TermColor,
    /// Background colour.
    pub bg: TermColor,
    /// Bold text.
    pub bold: bool,
    /// Underlined text.
    pub underline: bool,
}

impl Default for TermCell {
    fn default() -> Self {
        Self { ch: ' ', fg: TermColor::Default, bg: TermColor::Default, bold: false, underline: false }
    }
}


/// Resolves a `TermColor` to an `Rgba8` using the 256-colour xterm palette.
#[must_use]
pub fn resolve_color(color: TermColor, is_fg: bool) -> Rgba8 {
    match color {
        TermColor::Default => {
            if is_fg {
                Rgba8::rgb(0xCC, 0xCC, 0xCC)
            } else {
                Rgba8::rgb(0x1E, 0x1E, 0x1E)
            }
        }
        TermColor::Rgb(r, g, b) => Rgba8::rgb(r, g, b),
        TermColor::Index(i) => xterm_color(i),
    }
}

fn xterm_color(i: u8) -> Rgba8 {
    // Standard 16 colours + 6×6×6 cube + greyscale ramp
    let (r, g, b) = match i {
        0 => (0x1E, 0x1E, 0x1E),
        1 => (0xCC, 0x55, 0x55),
        2 => (0x55, 0xAA, 0x55),
        3 => (0xBB, 0xAA, 0x44),
        4 => (0x55, 0x88, 0xCC),
        5 => (0xAA, 0x55, 0xAA),
        6 => (0x55, 0xAA, 0xAA),
        7 => (0xCC, 0xCC, 0xCC),
        8 => (0x55, 0x55, 0x55),
        9 => (0xFF, 0x77, 0x77),
        10 => (0x77, 0xFF, 0x77),
        11 => (0xFF, 0xFF, 0x77),
        12 => (0x77, 0xAA, 0xFF),
        13 => (0xFF, 0x77, 0xFF),
        14 => (0x77, 0xFF, 0xFF),
        15 => (0xFF, 0xFF, 0xFF),
        16..=231 => {
            let idx = i - 16;
            let b_idx = idx % 6;
            let g_idx = (idx / 6) % 6;
            let r_idx = idx / 36;
            let lvl = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (lvl(r_idx), lvl(g_idx), lvl(b_idx))
        }
        232..=255 => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    };
    Rgba8::rgb(r, g, b)
}

// ── terminal grid ─────────────────────────────────────────────────────────────

/// The terminal cell grid and VT100 parser state.
///
/// This struct also implements `vte::Perform` so it can be driven directly by
/// the VTE parser in the reader thread.
pub struct TermGrid {
    cells: Vec<TermCell>,
    /// Visible columns.
    pub cols: usize,
    /// Visible rows.
    pub rows: usize,
    /// Cursor column (0-indexed).
    pub cursor_col: usize,
    /// Cursor row (0-indexed).
    pub cursor_row: usize,
    cur_fg: TermColor,
    cur_bg: TermColor,
    cur_bold: bool,
    cur_underline: bool,
}

impl TermGrid {
    /// Creates a blank grid with `cols` columns and `rows` rows.
    #[must_use]
    pub fn new(cols: usize, rows: usize) -> Self {
        let cells = vec![TermCell::default(); cols * rows];
        Self {
            cells,
            cols,
            rows,
            cursor_col: 0,
            cursor_row: 0,
            cur_fg: TermColor::Default,
            cur_bg: TermColor::Default,
            cur_bold: false,
            cur_underline: false,
        }
    }

    /// Resizes the grid, preserving as much content as possible.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let mut new_cells = vec![TermCell::default(); cols * rows];
        for r in 0..rows.min(self.rows) {
            for c in 0..cols.min(self.cols) {
                new_cells[r * cols + c] = self.cells[r * self.cols + c].clone();
            }
        }
        self.cells = new_cells;
        self.cols = cols;
        self.rows = rows;
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
        self.cursor_row = self.cursor_row.min(rows.saturating_sub(1));
    }

    /// Returns a slice of cells for `row` (row-major, 0-indexed).
    ///
    /// Returns an empty slice if `row >= self.rows` so callers never panic on a
    /// stale row index.
    #[must_use]
    pub fn row(&self, row: usize) -> &[TermCell] {
        if row >= self.rows || self.cols == 0 {
            return &[];
        }
        let start = row * self.cols;
        &self.cells[start..start + self.cols]
    }

    fn cell_mut(&mut self, row: usize, col: usize) -> Option<&mut TermCell> {
        if row < self.rows && col < self.cols {
            Some(&mut self.cells[row * self.cols + col])
        } else {
            None
        }
    }

    fn put_char(&mut self, c: char) {
        let (fg, bg, bold, underline) = (self.cur_fg, self.cur_bg, self.cur_bold, self.cur_underline);
        if let Some(cell) = self.cell_mut(self.cursor_row, self.cursor_col) {
            cell.ch = c;
            cell.fg = fg;
            cell.bg = bg;
            cell.bold = bold;
            cell.underline = underline;
        }
        self.cursor_col += 1;
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.advance_row();
        }
    }

    fn advance_row(&mut self) {
        if self.rows == 0 {
            return;
        }
        self.cursor_row += 1;
        if self.cursor_row >= self.rows {
            self.scroll_up(1);
            self.cursor_row = self.rows - 1;
        }
    }

    fn scroll_up(&mut self, n: usize) {
        let n = n.min(self.rows);
        self.cells.drain(0..n * self.cols);
        self.cells.extend(std::iter::repeat_with(TermCell::default).take(n * self.cols));
    }

    fn erase_display(&mut self, mode: u16) {
        match mode {
            0 => {
                // from cursor to end
                let start = self.cursor_row * self.cols + self.cursor_col;
                for c in &mut self.cells[start..] {
                    *c = TermCell::default();
                }
            }
            1 => {
                // from start to cursor
                let end = self.cursor_row * self.cols + self.cursor_col + 1;
                for c in &mut self.cells[..end] {
                    *c = TermCell::default();
                }
            }
            2 | 3 => {
                // entire display
                for c in &mut self.cells {
                    *c = TermCell::default();
                }
                self.cursor_row = 0;
                self.cursor_col = 0;
            }
            _ => {}
        }
    }

    fn erase_line(&mut self, mode: u16) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        match mode {
            0 => {
                // from cursor to end of line
                for c in col..self.cols {
                    if let Some(cell) = self.cell_mut(row, c) {
                        *cell = TermCell::default();
                    }
                }
            }
            1 => {
                // from start to cursor
                for c in 0..=col {
                    if let Some(cell) = self.cell_mut(row, c) {
                        *cell = TermCell::default();
                    }
                }
            }
            2 => {
                // entire line
                for c in 0..self.cols {
                    if let Some(cell) = self.cell_mut(row, c) {
                        *cell = TermCell::default();
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_sgr(&mut self, params: &vte::Params) {
        let mut iter = params.iter();
        while let Some(ps) = iter.next() {
            let p = *ps.first().unwrap_or(&0);
            match p {
                0 => {
                    self.cur_fg = TermColor::Default;
                    self.cur_bg = TermColor::Default;
                    self.cur_bold = false;
                    self.cur_underline = false;
                }
                1 => self.cur_bold = true,
                4 => self.cur_underline = true,
                22 => self.cur_bold = false,
                24 => self.cur_underline = false,
                30..=37 => self.cur_fg = TermColor::Index(p as u8 - 30),
                38 => {
                    if ps.len() >= 5 && ps[1] == 2 {
                        self.cur_fg = TermColor::Rgb(ps[2] as u8, ps[3] as u8, ps[4] as u8);
                    } else if ps.len() >= 3 && ps[1] == 5 {
                        self.cur_fg = TermColor::Index(ps[2] as u8);
                    } else {
                        match iter.next().map(|ps| *ps.first().unwrap_or(&0)) {
                            Some(2) => {
                                let r = iter.next().and_then(|ps| ps.first().copied()).unwrap_or(0) as u8;
                                let g = iter.next().and_then(|ps| ps.first().copied()).unwrap_or(0) as u8;
                                let b = iter.next().and_then(|ps| ps.first().copied()).unwrap_or(0) as u8;
                                self.cur_fg = TermColor::Rgb(r, g, b);
                            }
                            Some(5) => {
                                let n = iter.next().and_then(|ps| ps.first().copied()).unwrap_or(0) as u8;
                                self.cur_fg = TermColor::Index(n);
                            }
                            _ => {}
                        }
                    }
                }
                39 => self.cur_fg = TermColor::Default,
                40..=47 => self.cur_bg = TermColor::Index(p as u8 - 40),
                48 => {
                    if ps.len() >= 5 && ps[1] == 2 {
                        self.cur_bg = TermColor::Rgb(ps[2] as u8, ps[3] as u8, ps[4] as u8);
                    } else if ps.len() >= 3 && ps[1] == 5 {
                        self.cur_bg = TermColor::Index(ps[2] as u8);
                    } else {
                        match iter.next().map(|ps| *ps.first().unwrap_or(&0)) {
                            Some(2) => {
                                let r = iter.next().and_then(|ps| ps.first().copied()).unwrap_or(0) as u8;
                                let g = iter.next().and_then(|ps| ps.first().copied()).unwrap_or(0) as u8;
                                let b = iter.next().and_then(|ps| ps.first().copied()).unwrap_or(0) as u8;
                                self.cur_bg = TermColor::Rgb(r, g, b);
                            }
                            Some(5) => {
                                let n = iter.next().and_then(|ps| ps.first().copied()).unwrap_or(0) as u8;
                                self.cur_bg = TermColor::Index(n);
                            }
                            _ => {}
                        }
                    }
                }
                49 => self.cur_bg = TermColor::Default,
                90..=97 => self.cur_fg = TermColor::Index(p as u8 - 90 + 8),
                100..=107 => self.cur_bg = TermColor::Index(p as u8 - 100 + 8),
                _ => {}
            }
        }
    }
}

impl vte::Perform for TermGrid {
    fn print(&mut self, c: char) {
        self.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0B | 0x0C => self.advance_row(),
            b'\r' => self.cursor_col = 0,
            0x08 if self.cursor_col > 0 => {
                self.cursor_col -= 1;
            }
            b'\t' => {
                self.cursor_col = ((self.cursor_col / 8) + 1) * 8;
                if self.cursor_col >= self.cols {
                    self.cursor_col = self.cols - 1;
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let first = |params: &vte::Params| {
            params.iter().next().and_then(|ps| ps.first().copied()).unwrap_or(0)
        };
        let second = |params: &vte::Params| {
            params.iter().nth(1).and_then(|ps| ps.first().copied()).unwrap_or(0)
        };
        match action {
            'H' | 'f' => {
                let row = (first(params) as usize).saturating_sub(1);
                let col = (second(params) as usize).saturating_sub(1);
                self.cursor_row = row.min(self.rows.saturating_sub(1));
                self.cursor_col = col.min(self.cols.saturating_sub(1));
            }
            'A' => {
                let n = first(params).max(1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            'B' | 'e' => {
                let n = first(params).max(1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1));
            }
            'C' | 'a' => {
                let n = first(params).max(1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1));
            }
            'D' => {
                let n = first(params).max(1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            'G' | '`' => {
                let col = (first(params) as usize).saturating_sub(1);
                self.cursor_col = col.min(self.cols.saturating_sub(1));
            }
            'd' => {
                let row = (first(params) as usize).saturating_sub(1);
                self.cursor_row = row.min(self.rows.saturating_sub(1));
            }
            'J' => self.erase_display(first(params)),
            'K' => self.erase_line(first(params)),
            'm' => self.apply_sgr(params),
            'S' => {
                let n = first(params).max(1) as usize;
                self.scroll_up(n);
            }
            'T' => {
                // scroll down: shift rows down, blank at top
                let n = first(params).max(1) as usize;
                let n = n.min(self.rows);
                let old_cells = self.cells.clone();
                for c in &mut self.cells {
                    *c = TermCell::default();
                }
                let dest_start = n * self.cols;
                let copy_rows = self.rows.saturating_sub(n);
                for r in 0..copy_rows {
                    for c in 0..self.cols {
                        self.cells[(dest_start + r * self.cols) + c] =
                            old_cells[r * self.cols + c].clone();
                    }
                }
            }
            'r' => {
                // set scrolling region — ignore for now
            }
            'P' => {
                // delete characters
                let n = first(params).max(1) as usize;
                let row = self.cursor_row;
                let col = self.cursor_col;
                let row_start = row * self.cols;
                let end = self.cols;
                for c in col..end.saturating_sub(n) {
                    let src = row_start + c + n;
                    if src < row_start + end {
                        self.cells[row_start + c] = self.cells[src].clone();
                    }
                }
                for c in end.saturating_sub(n)..end {
                    self.cells[row_start + c] = TermCell::default();
                }
            }
            '@' => {
                // insert characters
                let n = first(params).max(1) as usize;
                let row = self.cursor_row;
                let col = self.cursor_col;
                let row_start = row * self.cols;
                let end = self.cols;
                for c in (col..end.saturating_sub(n)).rev() {
                    let dst = row_start + c + n;
                    if dst < row_start + end {
                        self.cells[dst] = self.cells[row_start + c].clone();
                    }
                }
                for c in col..(col + n).min(end) {
                    self.cells[row_start + c] = TermCell::default();
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        if byte == b'M' {
            // reverse index: scroll down when at top
            if self.cursor_row == 0 {
                let old = self.cells.clone();
                for c in &mut self.cells {
                    *c = TermCell::default();
                }
                let copy_rows = self.rows.saturating_sub(1);
                for r in 0..copy_rows {
                    for c in 0..self.cols {
                        self.cells[(r + 1) * self.cols + c] = old[r * self.cols + c].clone();
                    }
                }
            } else {
                self.cursor_row -= 1;
            }
        }
    }
}

// ── TerminalBackend ───────────────────────────────────────────────────────────

/// Owns the PTY process and cell grid. Safe to clone (grid is Arc-shared).
pub struct TerminalBackend {
    grid: Arc<RwLock<TermGrid>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
}

impl TerminalBackend {
    /// Spawns the system shell in a new PTY with `cols × rows` dimensions.
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY cannot be created or the shell cannot be
    /// spawned.
    pub fn spawn(cols: usize, rows: usize) -> Result<Self> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows as u16,
                cols: cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("open PTY")?;

        let shell = if cfg!(windows) { "powershell.exe" } else { "bash" };
        let mut cmd = CommandBuilder::new(shell);
        cmd.env("TERM", "xterm-256color");
        if cfg!(windows) {
            cmd.env("PSModulePath", "");
        }
        let _child = pair.slave.spawn_command(cmd).context("spawn shell")?;
        // Close slave end in this process so EOF is delivered when the child exits.
        drop(pair.slave);

        let writer = pair.master.take_writer().context("PTY writer")?;
        let mut reader = pair.master.try_clone_reader().context("PTY reader")?;

        let grid = Arc::new(RwLock::new(TermGrid::new(cols, rows)));
        let grid_reader = grid.clone();

        std::thread::Builder::new()
            .name("eden-pty-reader".into())
            .spawn(move || {
                let mut parser = vte::Parser::new();
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let mut g = grid_reader.write();
                            for &byte in &buf[..n] {
                                parser.advance(&mut *g, byte);
                            }
                        }
                    }
                }
            })
            .context("spawn PTY reader thread")?;

        Ok(Self { grid, writer, master: pair.master })
    }

    /// Sends bytes to the PTY (keyboard input, pastes, etc.).
    pub fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Sends a single key string (e.g. `"\r"` for Enter).
    pub fn write_str(&mut self, s: &str) {
        self.write(s.as_bytes());
    }

    /// Resizes the PTY and cell grid.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let _ = self.master.resize(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.grid.write().resize(cols, rows);
    }

    /// Returns a read-lock on the current cell grid.
    ///
    /// Hold this only long enough to copy data for rendering — do not hold
    /// across frames, as the reader thread needs to write to it.
    pub fn grid(&self) -> parking_lot::RwLockReadGuard<'_, TermGrid> {
        self.grid.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn term_grid_basic_print() {
        let mut g = TermGrid::new(10, 5);
        let mut p = vte::Parser::new();
        p.advance(&mut g, b'A');
        assert_eq!(g.row(0)[0].ch, 'A');
        assert_eq!(g.cursor_col, 1);
    }

    #[test]
    fn term_grid_newline() {
        let mut g = TermGrid::new(10, 5);
        let mut p = vte::Parser::new();
        p.advance(&mut g, b'A');
        p.advance(&mut g, b'\n');
        assert_eq!(g.cursor_row, 1);
    }

    #[test]
    fn term_grid_erase_display() {
        let mut g = TermGrid::new(5, 3);
        let mut p = vte::Parser::new();
        for b in b"AB\x1b[2J" {
            p.advance(&mut g, *b);
        }
        assert_eq!(g.row(0)[0].ch, ' ');
    }

    #[test]
    fn xterm_color_index_round_trips() {
        let c = xterm_color(0);
        assert_eq!(c.r, 0x1E);
        let c16 = xterm_color(16); // first cube colour
        assert_eq!((c16.r, c16.g, c16.b), (0, 0, 0));
        let c232 = xterm_color(232); // first greyscale
        assert_eq!(c232.r, c232.g);
    }
}
