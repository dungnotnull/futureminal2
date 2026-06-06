//! ANSI/VT100/xterm escape sequence parser.
//!
//! Handles:
//! - CSI sequences (cursor movement, erase, SGR attributes, mode changes)
//! - OSC sequences (window title, hyperlinks, color reports, etc.)
//! - DCS sequences
//! - ESC sequences
//! - UTF-8 character decoding

use crate::grid::{CellAttributes, Color, CursorStyle, Grid, NamedColor, Row};

/// Parsed terminal actions emitted by the VT parser.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Print(char),
    Execute(u8),
    CursorForward(usize),
    CursorBackward(usize),
    CursorUp(usize),
    CursorDown(usize),
    CursorNextLine(usize),
    CursorPrevLine(usize),
    CursorPosition { row: usize, col: usize },
    CursorSave,
    CursorRestore,
    EraseLineRight,
    EraseLineLeft,
    EraseLine,
    EraseScreenBelow,
    EraseScreenAbove,
    EraseScreen,
    InsertLines(usize),
    DeleteLines(usize),
    InsertChars(usize),
    DeleteChars(usize),
    ScrollUp(usize),
    ScrollDown(usize),
    SetGraphicRendition(Vec<SgrParam>),
    SetMode { mode: Mode, enable: bool },
    SetCursorStyle { style: CursorStyle, blink: bool },
    SetTitle(String),
    SetHyperlink { params: String, uri: String },
    SetColor { kind: ColorKind, color: Color },
    Bell,
    Tab,
    Linefeed,
    CarriageReturn,
    Backspace,
    Unknown(Vec<u8>),
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Insert,
    Origin,
    LineWrap,
    BracketedPaste,
    MouseTracking,
    MouseTrackingSgr,
    ReportFocus,
    AltScreen,
    CursorKeysApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorKind {
    Foreground,
    Background,
    Cursor,
}

/// Select Graphic Rendition parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SgrParam {
    Reset,
    Bold,
    Dim,
    Italic,
    Underline,
    Blink,
    Reverse,
    Hidden,
    Strikethrough,
    Foreground(Color),
    Background(Color),
    DoubleUnderline,
    CurlyUnderline,
    Overline,
    ForegroundReset,
    BackgroundReset,
}

/// A state machine parser for terminal escape sequences.
pub struct Parser {
    state: State,
    buffer: Vec<u8>,
    params: Vec<Vec<u8>>,
    osc_params: Vec<Vec<u8>>,
    intermediate: Vec<u8>,
    utf8_parser: Utf8Parser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    OscString,
    OscEscape,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsPassthrough,
    DcsIgnore,
    SosPmApcString,
    Utf8Sequence,
}

#[derive(Debug, Default)]
struct Utf8Parser {
    bytes_needed: u8,
    bytes_seen: u8,
    buf: [u8; 4],
}

impl Utf8Parser {
    fn feed(&mut self, byte: u8) -> Option<char> {
        if self.bytes_needed == 0 {
            if byte < 0x80 {
                return Some(byte as char);
            } else if byte & 0b1110_0000 == 0b1100_0000 {
                self.bytes_needed = 1;
                self.buf[0] = byte;
                self.bytes_seen = 1;
                return None;
            } else if byte & 0b1111_0000 == 0b1110_0000 {
                self.bytes_needed = 2;
                self.buf[0] = byte;
                self.bytes_seen = 1;
                return None;
            } else if byte & 0b1111_1000 == 0b1111_0000 {
                self.bytes_needed = 3;
                self.buf[0] = byte;
                self.bytes_seen = 1;
                return None;
            } else {
                return Some('\u{FFFD}'); // Replacement character
            }
        } else {
            if byte & 0b1100_0000 != 0b1000_0000 {
                self.reset();
                return Some('\u{FFFD}');
            }
            self.buf[self.bytes_seen as usize] = byte;
            self.bytes_seen += 1;
            if self.bytes_seen > self.bytes_needed {
                let result = std::str::from_utf8(&self.buf[..self.bytes_seen as usize])
                    .ok()
                    .and_then(|s| s.chars().next());
                self.reset();
                return result;
            }
            return None;
        }
    }

    fn reset(&mut self) {
        self.bytes_needed = 0;
        self.bytes_seen = 0;
        self.buf = [0; 4];
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            buffer: Vec::new(),
            params: Vec::new(),
            osc_params: Vec::new(),
            intermediate: Vec::new(),
            utf8_parser: Utf8Parser::default(),
        }
    }

    /// Feed bytes into the parser and return a vector of actions.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Action> {
        let mut actions = Vec::new();
        for &byte in data {
            self.advance(byte, &mut actions);
        }
        actions
    }

    fn advance(&mut self, byte: u8, actions: &mut Vec<Action>) {
        match self.state {
            State::Ground => {
                if byte >= 0x20 && byte <= 0x7F {
                    if let Some(ch) = self.utf8_parser.feed(byte) {
                        actions.push(Action::Print(ch));
                    }
                } else if byte >= 0x80 {
                    if let Some(ch) = self.utf8_parser.feed(byte) {
                        actions.push(Action::Print(ch));
                    }
                } else {
                    self.handle_control(byte, actions);
                }
            }
            State::Escape => {
                self.handle_escape(byte, actions);
            }
            State::EscapeIntermediate => {
                if byte.is_ascii_alphabetic() {
                    actions.push(Action::Unknown(vec![0x1b, byte]));
                    self.reset();
                } else if byte >= 0x20 && byte <= 0x2F {
                    self.intermediate.push(byte);
                } else {
                    self.state = State::Ground;
                }
            }
            State::CsiEntry => {
                if byte >= 0x30 && byte <= 0x3F {
                    self.state = State::CsiParam;
                    self.params.push(vec![byte]);
                } else if byte >= 0x20 && byte <= 0x2F {
                    self.state = State::CsiIntermediate;
                    self.intermediate.push(byte);
                } else if byte >= 0x40 && byte <= 0x7E {
                    self.dispatch_csi(byte, actions);
                    self.reset();
                } else {
                    self.state = State::CsiIgnore;
                }
            }
            State::CsiParam => {
                if byte == 0x3B {
                    self.params.push(Vec::new());
                } else if byte >= 0x30 && byte <= 0x3F {
                    if let Some(last) = self.params.last_mut() {
                        last.push(byte);
                    }
                } else if byte >= 0x20 && byte <= 0x2F {
                    self.state = State::CsiIntermediate;
                    self.intermediate.push(byte);
                } else if byte >= 0x40 && byte <= 0x7E {
                    self.dispatch_csi(byte, actions);
                    self.reset();
                } else {
                    self.state = State::CsiIgnore;
                }
            }
            State::CsiIntermediate => {
                if byte >= 0x20 && byte <= 0x2F {
                    self.intermediate.push(byte);
                } else if byte >= 0x40 && byte <= 0x7E {
                    self.dispatch_csi(byte, actions);
                    self.reset();
                } else {
                    self.state = State::CsiIgnore;
                }
            }
            State::CsiIgnore => {
                if byte >= 0x40 && byte <= 0x7E {
                    self.reset();
                }
            }
            State::OscString => {
                if byte == 0x07 {
                    self.dispatch_osc(actions);
                    self.reset();
                } else if byte == 0x1B {
                    self.state = State::OscEscape;
                } else {
                    if let Some(last) = self.osc_params.last_mut() {
                        last.push(byte);
                    } else {
                        self.osc_params.push(vec![byte]);
                    }
                }
            }
            State::OscEscape => {
                if byte == 0x5C {
                    self.dispatch_osc(actions);
                    self.reset();
                } else {
                    if let Some(last) = self.osc_params.last_mut() {
                        last.push(0x1b);
                        last.push(byte);
                    }
                    self.state = State::OscString;
                }
            }
            _ => {
                // Other states (DCS, SOS/PM/APC) — not handled in standard terminal emulation.
                if byte >= 0x40 && byte <= 0x7E {
                    self.reset();
                }
            }
        }
    }

    fn handle_control(&mut self, byte: u8, actions: &mut Vec<Action>) {
        match byte {
            0x00 => {}
            0x07 => actions.push(Action::Bell),
            0x08 => actions.push(Action::Backspace),
            0x09 => actions.push(Action::Tab),
            0x0A | 0x0B | 0x0C => actions.push(Action::Linefeed),
            0x0D => actions.push(Action::CarriageReturn),
            0x0E | 0x0F => {}
            0x1B => self.state = State::Escape,
            0x18 | 0x1A => self.reset(),
            0x84..=0x89 | 0x8A | 0x8C | 0x8D | 0x8E | 0x8F | 0x90 | 0x9C | 0x9D | 0x9E | 0x9F => {}
            _ => {}
        }
    }

    fn handle_escape(&mut self, byte: u8, actions: &mut Vec<Action>) {
        match byte {
            0x5B => {
                self.state = State::CsiEntry;
                self.params.clear();
                self.intermediate.clear();
            }
            0x5D => {
                self.state = State::OscString;
                self.osc_params.clear();
                self.osc_params.push(Vec::new());
            }
            0x50 => {
                self.state = State::DcsPassthrough;
            }
            0x58 | 0x5E | 0x5F => {
                self.state = State::SosPmApcString;
            }
            0x7 => {
                actions.push(Action::Bell);
                self.reset();
            }
            0x44 => {
                actions.push(Action::Linefeed);
                self.reset();
            }
            0x45 => {
                actions.push(Action::CarriageReturn);
                actions.push(Action::Linefeed);
                self.reset();
            }
            0x4D => {
                actions.push(Action::Reverse);
                self.reset();
            }
            0x5B..=0x5F | 0x60..=0x7E => {
                actions.push(Action::Unknown(vec![0x1b, byte]));
                self.reset();
            }
            0x20..=0x2F => {
                self.intermediate.push(byte);
                self.state = State::EscapeIntermediate;
            }
            _ => self.reset(),
        }
    }

    fn dispatch_csi(&mut self, byte: u8, actions: &mut Vec<Action>) {
        let params: Vec<usize> = self
            .params
            .iter()
            .map(|p| {
                let s = String::from_utf8_lossy(p);
                s.parse().unwrap_or(0)
            })
            .collect();

        match byte {
            b'@' => actions.push(Action::InsertChars(params.get(0).copied().unwrap_or(1))),
            b'A' => actions.push(Action::CursorUp(params.get(0).copied().unwrap_or(1))),
            b'B' => actions.push(Action::CursorDown(params.get(0).copied().unwrap_or(1))),
            b'C' => actions.push(Action::CursorForward(params.get(0).copied().unwrap_or(1))),
            b'D' => actions.push(Action::CursorBackward(params.get(0).copied().unwrap_or(1))),
            b'E' => actions.push(Action::CursorNextLine(params.get(0).copied().unwrap_or(1))),
            b'F' => actions.push(Action::CursorPrevLine(params.get(0).copied().unwrap_or(1))),
            b'G' => actions.push(Action::CursorPosition {
                row: 0,
                col: params.get(0).copied().unwrap_or(1).saturating_sub(1),
            }),
            b'H' | b'f' => {
                let row = params.get(0).copied().unwrap_or(1).saturating_sub(1);
                let col = params.get(1).copied().unwrap_or(1).saturating_sub(1);
                actions.push(Action::CursorPosition { row, col });
            }
            b'J' => {
                let mode = params.get(0).copied().unwrap_or(0);
                match mode {
                    0 => actions.push(Action::EraseScreenBelow),
                    1 => actions.push(Action::EraseScreenAbove),
                    2 => actions.push(Action::EraseScreen),
                    3 => {
                        actions.push(Action::EraseScreen);
                    }
                    _ => {}
                }
            }
            b'K' => {
                let mode = params.get(0).copied().unwrap_or(0);
                match mode {
                    0 => actions.push(Action::EraseLineRight),
                    1 => actions.push(Action::EraseLineLeft),
                    2 => actions.push(Action::EraseLine),
                    _ => {}
                }
            }
            b'L' => actions.push(Action::InsertLines(params.get(0).copied().unwrap_or(1))),
            b'M' => actions.push(Action::DeleteLines(params.get(0).copied().unwrap_or(1))),
            b'P' => actions.push(Action::DeleteChars(params.get(0).copied().unwrap_or(1))),
            b'S' => actions.push(Action::ScrollUp(params.get(0).copied().unwrap_or(1))),
            b'T' => actions.push(Action::ScrollDown(params.get(0).copied().unwrap_or(1))),
            b'X' => actions.push(Action::DeleteChars(params.get(0).copied().unwrap_or(1))),
            b'd' => {
                let row = params.get(0).copied().unwrap_or(1).saturating_sub(1);
                actions.push(Action::CursorPosition { row, col: 0 });
            }
            b'h' => {
                if let Some(&param) = params.first() {
                    actions.push(Action::SetMode { mode: parse_mode(param), enable: true });
                }
            }
            b'l' => {
                if let Some(&param) = params.first() {
                    actions.push(Action::SetMode { mode: parse_mode(param), enable: false });
                }
            }
            b'm' => {
                let sgr = parse_sgr(&params);
                actions.push(Action::SetGraphicRendition(sgr));
            }
            b'n' => {}
            b'r' => {}
            b's' => actions.push(Action::CursorSave),
            b'u' => actions.push(Action::CursorRestore),
            _ => {}
        }
    }

    fn dispatch_osc(&mut self, actions: &mut Vec<Action>) {
        if self.osc_params.is_empty() {
            return;
        }
        let kind = String::from_utf8_lossy(&self.osc_params[0]);
        let kind: u8 = kind.parse().unwrap_or(0);
        match kind {
            0 | 2 => {
                if self.osc_params.len() > 1 {
                    let title = String::from_utf8_lossy(&self.osc_params[1]).to_string();
                    actions.push(Action::SetTitle(title));
                }
            }
            8 => {
                if self.osc_params.len() >= 3 {
                    let params = String::from_utf8_lossy(&self.osc_params[1]).to_string();
                    let uri = String::from_utf8_lossy(&self.osc_params[2]).to_string();
                    actions.push(Action::SetHyperlink { params, uri });
                }
            }
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.state = State::Ground;
        self.buffer.clear();
        self.params.clear();
        self.osc_params.clear();
        self.intermediate.clear();
        self.utf8_parser.reset();
    }
}

fn parse_mode(param: usize) -> Mode {
    match param {
        4 => Mode::Insert,
        6 => Mode::Origin,
        7 => Mode::LineWrap,
        47 | 1047 | 1049 => Mode::AltScreen,
        1000 => Mode::MouseTracking,
        1006 => Mode::MouseTrackingSgr,
        1004 => Mode::ReportFocus,
        2004 => Mode::BracketedPaste,
        1 => Mode::CursorKeysApp,
        _ => Mode::LineWrap,
    }
}

fn parse_sgr(params: &[usize]) -> Vec<SgrParam> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => result.push(SgrParam::Reset),
            1 => result.push(SgrParam::Bold),
            2 => result.push(SgrParam::Dim),
            3 => result.push(SgrParam::Italic),
            4 => result.push(SgrParam::Underline),
            5 | 6 => result.push(SgrParam::Blink),
            7 => result.push(SgrParam::Reverse),
            8 => result.push(SgrParam::Hidden),
            9 => result.push(SgrParam::Strikethrough),
            21 => result.push(SgrParam::DoubleUnderline),
            22 => result.push(SgrParam::Reset), // Bold reset
            23 => result.push(SgrParam::Reset), // Italic reset
            24 => result.push(SgrParam::Reset), // Underline reset
            25 => result.push(SgrParam::Reset), // Blink reset
            27 => result.push(SgrParam::Reset), // Reverse reset
            28 => result.push(SgrParam::Reset), // Hidden reset
            29 => result.push(SgrParam::Reset), // Strikethrough reset
            30..=37 => {
                let color = ansi_color_to_named(params[i] - 30);
                result.push(SgrParam::Foreground(color));
            }
            38 => {
                if i + 1 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            result.push(SgrParam::Foreground(Color::Indexed(params[i + 2] as u8)));
                            i += 2;
                        }
                        2 if i + 4 < params.len() => {
                            result.push(SgrParam::Foreground(Color::TrueColor {
                                r: params[i + 2] as u8,
                                g: params[i + 3] as u8,
                                b: params[i + 4] as u8,
                            }));
                            i += 4;
                        }
                        _ => {}
                    }
                }
            }
            39 => result.push(SgrParam::ForegroundReset),
            40..=47 => {
                let color = ansi_color_to_named(params[i] - 40);
                result.push(SgrParam::Background(color));
            }
            48 => {
                if i + 1 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            result.push(SgrParam::Background(Color::Indexed(params[i + 2] as u8)));
                            i += 2;
                        }
                        2 if i + 4 < params.len() => {
                            result.push(SgrParam::Background(Color::TrueColor {
                                r: params[i + 2] as u8,
                                g: params[i + 3] as u8,
                                b: params[i + 4] as u8,
                            }));
                            i += 4;
                        }
                        _ => {}
                    }
                }
            }
            49 => result.push(SgrParam::BackgroundReset),
            90..=97 => {
                let color = ansi_bright_color_to_named(params[i] - 90);
                result.push(SgrParam::Foreground(color));
            }
            100..=107 => {
                let color = ansi_bright_color_to_named(params[i] - 100);
                result.push(SgrParam::Background(color));
            }
            _ => {}
        }
        i += 1;
    }
    if result.is_empty() {
        result.push(SgrParam::Reset);
    }
    result
}

fn ansi_color_to_named(n: usize) -> Color {
    match n {
        0 => Color::Named(NamedColor::Black),
        1 => Color::Named(NamedColor::Red),
        2 => Color::Named(NamedColor::Green),
        3 => Color::Named(NamedColor::Yellow),
        4 => Color::Named(NamedColor::Blue),
        5 => Color::Named(NamedColor::Magenta),
        6 => Color::Named(NamedColor::Cyan),
        7 => Color::Named(NamedColor::White),
        _ => Color::Named(NamedColor::Foreground),
    }
}

fn ansi_bright_color_to_named(n: usize) -> Color {
    match n {
        0 => Color::Named(NamedColor::BrightBlack),
        1 => Color::Named(NamedColor::BrightRed),
        2 => Color::Named(NamedColor::BrightGreen),
        3 => Color::Named(NamedColor::BrightYellow),
        4 => Color::Named(NamedColor::BrightBlue),
        5 => Color::Named(NamedColor::BrightMagenta),
        6 => Color::Named(NamedColor::BrightCyan),
        7 => Color::Named(NamedColor::BrightWhite),
        _ => Color::Named(NamedColor::Foreground),
    }
}

/// Apply a parsed action to a grid.
pub fn apply_action(grid: &mut Grid, action: &Action) {
    match action {
        Action::Print(ch) => grid.write_char(*ch),
        Action::Backspace => {
            if grid.cursor.col > 0 {
                grid.cursor.col -= 1;
            }
        }
        Action::Tab => {
            let next_tab = ((grid.cursor.col / 8) + 1) * 8;
            grid.cursor.col = next_tab.min(grid.cols - 1);
        }
        Action::Linefeed => {
            grid.cursor.row += 1;
            if grid.cursor.row >= grid.rows {
                grid.scroll_up(1);
                grid.cursor.row = grid.rows - 1;
            }
        }
        Action::CarriageReturn => grid.cursor.col = 0,
        Action::CursorForward(n) => grid.cursor.col = (grid.cursor.col + n).min(grid.cols - 1),
        Action::CursorBackward(n) => grid.cursor.col = grid.cursor.col.saturating_sub(*n),
        Action::CursorUp(n) => grid.cursor.row = grid.cursor.row.saturating_sub(*n),
        Action::CursorDown(n) => grid.cursor.row = (grid.cursor.row + n).min(grid.rows - 1),
        Action::CursorPosition { row, col } => grid.move_cursor(*row, *col),
        Action::CursorSave => grid.saved_cursor = Some(grid.cursor),
        Action::CursorRestore => {
            if let Some(saved) = grid.saved_cursor {
                grid.cursor = saved;
            }
        }
        Action::EraseLineRight => grid.clear_line_right(),
        Action::EraseLineLeft => grid.clear_line_left(),
        Action::EraseLine => grid.clear_line(),
        Action::EraseScreenBelow => grid.clear_screen_below(),
        Action::EraseScreenAbove => grid.clear_screen_above(),
        Action::EraseScreen => grid.clear_screen(),
        Action::InsertLines(n) => grid.insert_lines(*n),
        Action::DeleteLines(n) => grid.delete_lines(*n),
        Action::ScrollUp(n) => grid.scroll_up(*n),
        Action::ScrollDown(n) => grid.scroll_down(*n),
        Action::SetGraphicRendition(params) => {
            for param in params {
                match param {
                    SgrParam::Reset => {
                        grid.fg = Color::Named(NamedColor::Foreground);
                        grid.bg = Color::Named(NamedColor::Background);
                        grid.attrs = CellAttributes::empty();
                    }
                    SgrParam::Bold => grid.attrs |= CellAttributes::BOLD,
                    SgrParam::Dim => grid.attrs |= CellAttributes::DIM,
                    SgrParam::Italic => grid.attrs |= CellAttributes::ITALIC,
                    SgrParam::Underline => grid.attrs |= CellAttributes::UNDERLINE,
                    SgrParam::Blink => grid.attrs |= CellAttributes::BLINK,
                    SgrParam::Reverse => grid.attrs |= CellAttributes::REVERSE,
                    SgrParam::Hidden => grid.attrs |= CellAttributes::HIDDEN,
                    SgrParam::Strikethrough => grid.attrs |= CellAttributes::STRIKETHROUGH,
                    SgrParam::DoubleUnderline => grid.attrs |= CellAttributes::DOUBLE_UNDERLINE,
                    SgrParam::CurlyUnderline => grid.attrs |= CellAttributes::CURLY_UNDERLINE,
                    SgrParam::Overline => grid.attrs |= CellAttributes::OVERLINE,
                    SgrParam::Foreground(c) => grid.fg = *c,
                    SgrParam::Background(c) => grid.bg = *c,
                    SgrParam::ForegroundReset => grid.fg = Color::Named(NamedColor::Foreground),
                    SgrParam::BackgroundReset => grid.bg = Color::Named(NamedColor::Background),
                }
            }
        }
        Action::SetMode { mode, enable } => {
            match mode {
                Mode::LineWrap => grid.auto_wrap = *enable,
                Mode::BracketedPaste => grid.bracketed_paste = *enable,
                Mode::MouseTracking => grid.mouse_tracking = *enable,
                Mode::AltScreen => {
                    if *enable {
                        if grid.alternate.is_none() {
                            grid.alternate = Some(vec![Row::new(grid.cols); grid.rows]);
                        }
                        std::mem::swap(&mut grid.primary, grid.alternate.as_mut().unwrap());
                    } else if let Some(ref mut alt) = grid.alternate {
                        std::mem::swap(&mut grid.primary, alt);
                    }
                }
                _ => {}
            }
        }
        Action::SetTitle(title) => {
            tracing::debug!("Window title: {}", title);
        }
        Action::Bell => {}
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cursor_movement() {
        let mut parser = Parser::new();
        let actions = parser.feed(b"\x1b[10;20H");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], Action::CursorPosition { row: 9, col: 19 });
    }

    #[test]
    fn test_parse_sgr_colors() {
        let mut parser = Parser::new();
        let actions = parser.feed(b"\x1b[31;42m");
        assert_eq!(actions.len(), 1);
        if let Action::SetGraphicRendition(params) = &actions[0] {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], SgrParam::Foreground(Color::Named(NamedColor::Red)));
            assert_eq!(params[1], SgrParam::Background(Color::Named(NamedColor::Green)));
        } else {
            panic!("expected SGR");
        }
    }

    #[test]
    fn test_parse_truecolor() {
        let mut parser = Parser::new();
        let actions = parser.feed(b"\x1b[38;2;255;128;0m");
        assert_eq!(actions.len(), 1);
        if let Action::SetGraphicRendition(params) = &actions[0] {
            assert_eq!(params[0], SgrParam::Foreground(Color::TrueColor { r: 255, g: 128, b: 0 }));
        } else {
            panic!("expected SGR");
        }
    }

    #[test]
    fn test_apply_to_grid() {
        let mut grid = Grid::new(80, 24, 100);
        let mut parser = Parser::new();
        let actions = parser.feed(b"Hello\x1b[31mWorld\x1b[0m!");
        for action in &actions {
            apply_action(&mut grid, action);
        }
        assert_eq!(grid.primary[0].cells[0].ch, 'H');
        assert_eq!(grid.primary[0].cells[5].ch, 'W');
        assert_eq!(grid.primary[0].cells[5].fg, Color::Named(NamedColor::Red));
    }
}



