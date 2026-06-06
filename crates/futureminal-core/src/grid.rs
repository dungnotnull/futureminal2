//! Terminal cell grid with scrollback, colors, and attributes.
//!
//! Models the terminal as a 2D grid of cells where each cell carries:
//! - a Unicode codepoint (or multi-cell wide characters)
//! - foreground and background colors
//! - text attributes (bold, italic, underline, strikethrough, reverse, blink)

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A single terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttributes,
    /// If > 0, this cell is a continuation of a multi-cell wide character.
    pub wide_continuation: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Named(NamedColor::Foreground),
            bg: Color::Named(NamedColor::Background),
            attrs: CellAttributes::empty(),
            wide_continuation: false,
        }
    }
}

/// A color in the terminal palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    Named(NamedColor),
    Indexed(u8),
    TrueColor { r: u8, g: u8, b: u8 },
}

/// The 16 standard ANSI colors plus foreground/background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum NamedColor {
    Black = 0,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Foreground,
    Background,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct CellAttributes: u16 {
        const BOLD = 1;
        const DIM = 2;
        const ITALIC = 4;
        const UNDERLINE = 8;
        const BLINK = 16;
        const REVERSE = 32;
        const HIDDEN = 64;
        const STRIKETHROUGH = 128;
        const DOUBLE_UNDERLINE = 256;
        const CURLY_UNDERLINE = 512;
        const DOTTED_UNDERLINE = 1024;
        const DASHED_UNDERLINE = 2048;
        const OVERLINE = 4096;
    }
}

/// A row of cells in the terminal grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub wrapped: bool,
}

impl Row {
    pub fn new(cols: usize) -> Self {
        Self {
            cells: vec![Cell::default(); cols],
            wrapped: false,
        }
    }

    pub fn resize(&mut self, new_cols: usize) {
        self.cells.resize(new_cols, Cell::default());
    }
}

/// The terminal grid with primary screen, alternate screen, and scrollback.
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub scrollback: VecDeque<Row>,
    pub scrollback_limit: usize,
    pub primary: Vec<Row>,
    pub alternate: Option<Vec<Row>>,
    pub cursor: Cursor,
    pub saved_cursor: Option<Cursor>,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttributes,
    pub auto_wrap: bool,
    pub bracketed_paste: bool,
    pub mouse_tracking: bool,
}

/// Cursor position and style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub style: CursorStyle,
    pub blink: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorStyle {
    Block,
    Underline,
    Line,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            style: CursorStyle::Block,
            blink: false,
            visible: true,
        }
    }
}

impl Grid {
    pub fn new(cols: usize, rows: usize, scrollback_limit: usize) -> Self {
        Self {
            cols,
            rows,
            scrollback: VecDeque::with_capacity(scrollback_limit.min(10000)),
            scrollback_limit,
            primary: (0..rows).map(|_| Row::new(cols)).collect(),
            alternate: None,
            cursor: Cursor::default(),
            saved_cursor: None,
            fg: Color::Named(NamedColor::Foreground),
            bg: Color::Named(NamedColor::Background),
            attrs: CellAttributes::empty(),
            auto_wrap: true,
            bracketed_paste: false,
            mouse_tracking: false,
        }
    }

    /// Write a character at the cursor position and advance.
    pub fn write_char(&mut self, ch: char) {
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as usize;
        if self.cursor.col + width > self.cols {
            if self.auto_wrap {
                self.cursor.col = 0;
                self.cursor.row += 1;
                if self.cursor.row >= self.rows {
                    self.scroll_up(1);
                    self.cursor.row = self.rows - 1;
                }
            } else {
                self.cursor.col = self.cols.saturating_sub(1);
            }
        }
        if self.cursor.row >= self.rows {
            self.scroll_up(1);
            self.cursor.row = self.rows - 1;
        }
        let cell = Cell {
            ch,
            fg: self.fg,
            bg: self.bg,
            attrs: self.attrs,
            wide_continuation: false,
        };
        let col = self.cursor.col;
        if col < self.cols {
            self.primary[self.cursor.row].cells[col] = cell;
            // Mark continuation cells for wide chars.
            for i in 1..width {
                if col + i < self.cols {
                    self.primary[self.cursor.row].cells[col + i] = Cell {
                        ch: '\0',
                        fg: self.fg,
                        bg: self.bg,
                        attrs: self.attrs,
                        wide_continuation: true,
                    };
                }
            }
        }
        self.cursor.col += width;
    }

    /// Move cursor to an absolute position (0-indexed).
    pub fn move_cursor(&mut self, row: usize, col: usize) {
        self.cursor.row = row.min(self.rows.saturating_sub(1));
        self.cursor.col = col.min(self.cols.saturating_sub(1));
    }

    /// Clear from cursor to end of line.
    pub fn clear_line_right(&mut self) {
        if self.cursor.row < self.rows {
            for c in self.cursor.col..self.cols {
                self.primary[self.cursor.row].cells[c] = Cell::default();
            }
        }
    }

    /// Clear from start of line to cursor.
    pub fn clear_line_left(&mut self) {
        if self.cursor.row < self.rows {
            for c in 0..=self.cursor.col.min(self.cols - 1) {
                self.primary[self.cursor.row].cells[c] = Cell::default();
            }
        }
    }

    /// Clear entire line.
    pub fn clear_line(&mut self) {
        if self.cursor.row < self.rows {
            self.primary[self.cursor.row] = Row::new(self.cols);
        }
    }

    /// Clear from cursor to end of screen.
    pub fn clear_screen_below(&mut self) {
        self.clear_line_right();
        for r in (self.cursor.row + 1)..self.rows {
            self.primary[r] = Row::new(self.cols);
        }
    }

    /// Clear from start of screen to cursor.
    pub fn clear_screen_above(&mut self) {
        self.clear_line_left();
        for r in 0..self.cursor.row {
            self.primary[r] = Row::new(self.cols);
        }
    }

    /// Clear entire screen.
    pub fn clear_screen(&mut self) {
        for r in 0..self.rows {
            self.primary[r] = Row::new(self.cols);
        }
    }

    /// Insert `n` blank lines at cursor row, scrolling down.
    pub fn insert_lines(&mut self, n: usize) {
        let n = n.min(self.rows - self.cursor.row);
        let removed: Vec<Row> = self.primary.drain(self.cursor.row..self.cursor.row + n).collect();
        for row in removed {
            self.push_scrollback(row);
        }
        for _ in 0..n {
            self.primary.insert(self.cursor.row, Row::new(self.cols));
        }
        self.primary.truncate(self.rows);
    }

    /// Delete `n` lines at cursor row, scrolling up.
    pub fn delete_lines(&mut self, n: usize) {
        let n = n.min(self.rows - self.cursor.row);
        let removed: Vec<Row> = self.primary.drain(self.cursor.row..self.cursor.row + n).collect();
        for row in removed {
            self.push_scrollback(row);
        }
        for _ in 0..n {
            self.primary.push(Row::new(self.cols));
        }
        self.primary.truncate(self.rows);
    }

    /// Scroll the entire grid up by `n` lines.
    pub fn scroll_up(&mut self, n: usize) {
        let n = n.min(self.rows);
        for _ in 0..n {
            if let Some(row) = self.primary.first().cloned() {
                self.push_scrollback(row);
            }
            self.primary.remove(0);
            self.primary.push(Row::new(self.cols));
        }
    }

    /// Scroll the entire grid down by `n` lines.
    pub fn scroll_down(&mut self, n: usize) {
        let n = n.min(self.rows);
        for _ in 0..n {
            self.primary.pop();
            self.primary.insert(0, Row::new(self.cols));
        }
    }

    /// Resize the grid to new dimensions.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        for row in &mut self.primary {
            row.resize(cols);
        }
        if self.primary.len() < rows {
            self.primary.resize(rows, Row::new(cols));
        } else if self.primary.len() > rows {
            let excess: Vec<Row> = self.primary.drain(rows..).collect();
            for row in excess {
                self.push_scrollback(row);
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.cursor.row = self.cursor.row.min(rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(cols.saturating_sub(1));
    }

    fn push_scrollback(&mut self, row: Row) {
        if self.scrollback_limit > 0 {
            if self.scrollback.len() >= self.scrollback_limit {
                self.scrollback.pop_front();
            }
            self.scrollback.push_back(row);
        }
    }

    /// Get a row from the visible screen (0-indexed from top).
    pub fn visible_row(&self, row: usize) -> Option<&Row> {
        self.primary.get(row)
    }

    /// Total visible + scrollback lines.
    pub fn total_lines(&self) -> usize {
        self.scrollback.len() + self.primary.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_write_and_scroll() {
        let mut grid = Grid::new(80, 24, 1000);
        grid.write_char('A');
        assert_eq!(grid.primary[0].cells[0].ch, 'A');
        grid.scroll_up(1);
        assert_eq!(grid.scrollback.len(), 1);
    }

    #[test]
    fn test_grid_resize() {
        let mut grid = Grid::new(80, 24, 100);
        grid.write_char('X');
        grid.resize(40, 12);
        assert_eq!(grid.cols, 40);
        assert_eq!(grid.rows, 12);
        assert_eq!(grid.primary[0].cells[0].ch, 'X');
    }

    #[test]
    fn test_wide_char() {
        let mut grid = Grid::new(80, 24, 100);
        grid.write_char('\u{4e2d}'); // CJK, width 2
        assert_eq!(grid.primary[0].cells[0].ch, '\u{4e2d}');
        assert!(grid.primary[0].cells[1].wide_continuation);
    }
}
