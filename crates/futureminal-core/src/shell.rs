//! Shell integration and OSC sequence parsing.
//!
//! Shell integration scripts emit OSC (Operating System Command) sequences
//! that the terminal can intercept to track:
//! - Current working directory (OSC 7)
//! - Command boundaries (OSC 133)
//! - Exit codes (OSC 133;D)
//! - Environment variable hints (OSC 777)
//!
//! This module parses those sequences from PTY output bytes.

use std::path::PathBuf;
use tracing::{debug, trace};

/// Parsed OSC sequence extracted from terminal output.
#[derive(Debug, Clone, PartialEq)]
pub enum OscSequence {
    /// OSC 7 — Current working directory (e.g., from `vte` or `iterm2` shell integration).
    CurrentDirectory(PathBuf),
    /// OSC 133 — Shell command boundaries (A=start, B=end, C=output, D=done with exit code).
    CommandBoundary { kind: CommandKind, exit_code: Option<i32> },
    /// OSC 777 — Environment variable notification.
    EnvironmentVar { name: String, value: String },
    /// OSC 8 — Hyperlink.
    Hyperlink { params: String, uri: String },
    /// OSC 52 — Clipboard data.
    Clipboard { target: String, data: String },
    /// Unknown OSC sequence (preserved for debugging).
    Unknown { code: u16, payload: String },
}

/// Sub-kinds of OSC 133 command sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// `A` — Prompt start.
    PromptStart,
    /// `B` — Command line start.
    CommandLineStart,
    /// `C` — Command line output.
    CommandLineOutput,
    /// `D` — Command done.
    CommandDone,
}

/// Parsed state of the current shell line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShellState {
    pub cwd: Option<PathBuf>,
    pub last_command: Option<String>,
    pub last_exit_code: Option<i32>,
    pub env_vars: std::collections::HashMap<String, String>,
}

/// An incremental parser for OSC sequences embedded in terminal output.
///
/// This is a simple state-machine parser that scans for `ESC ]` (`\x1b]`)
/// followed by an OSC sequence terminated by `BEL` (`\x07`) or `ESC \\`.
#[derive(Debug, Default)]
pub struct OscParser {
    state: ShellState,
    pending_sequences: Vec<OscSequence>,
}

impl OscParser {
    /// Create a new OSC parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw PTY bytes into the parser. Returns any newly completed OSC sequences.
    pub fn feed(&mut self, data: &[u8],
    ) -> Vec<OscSequence> {
        let mut sequences = Vec::new();
        let mut i = 0;

        while i < data.len() {
            // Look for ESC ] (0x1b 0x5d) which starts an OSC sequence.
            if i + 1 < data.len() && data[i] == 0x1b && data[i + 1] == 0x5d {
                if let Some((seq, consumed)) = Self::parse_osc(&data[i..]) {
                    self.apply_sequence(&seq);
                    sequences.push(seq);
                    i += consumed;
                    continue;
                }
            }
            i += 1;
        }

        sequences
    }

    /// Parse a single OSC sequence starting at `data[0]` == ESC ].
    /// Returns the sequence and the number of bytes consumed.
    fn parse_osc(data: &[u8]) -> Option<(OscSequence, usize)> {
        if data.len() < 3 || data[0] != 0x1b || data[1] != 0x5d {
            return None;
        }

        // Find terminator: BEL (0x07) or ESC \ (0x1b 0x5c)
        let mut payload_end = None;
        for j in 2..data.len() {
            if data[j] == 0x07 {
                payload_end = Some(j);
                break;
            }
            if j + 1 < data.len() && data[j] == 0x1b && data[j + 1] == 0x5c {
                payload_end = Some(j);
                break;
            }
        }

        let end = payload_end?;
        let payload = String::from_utf8_lossy(&data[2..end]);
        let consumed = if data[end] == 0x07 { end + 1 } else { end + 2 };

        let seq = Self::decode_osc_payload(&payload)?;
        Some((seq, consumed))
    }

    /// Decode an OSC payload string into a typed `OscSequence`.
    fn decode_osc_payload(payload: &str) -> Option<OscSequence> {
        // OSC format: <code>;<payload>
        let mut parts = payload.splitn(2, ';');
        let code_str = parts.next()?;
        let rest = parts.next().unwrap_or("");

        let code: u16 = code_str.parse().ok()?;

        match code {
            7 => {
                // OSC 7: file://hostname/path
                let path = rest.strip_prefix("file://").unwrap_or(rest);
                // Drop hostname if present.
                let path = if let Some(idx) = path.find('/') {
                    &path[idx..]
                } else {
                    path
                };
                Some(OscSequence::CurrentDirectory(PathBuf::from(path)))
            }
            8 => {
                let mut sub = rest.splitn(2, ';');
                let params = sub.next()?.to_string();
                let uri = sub.next()?.to_string();
                Some(OscSequence::Hyperlink { params, uri })
            }
            52 => {
                let mut sub = rest.splitn(2, ';');
                let target = sub.next()?.to_string();
                let data = sub.next()?.to_string();
                Some(OscSequence::Clipboard { target, data })
            }
            133 => {
                let mut sub = rest.splitn(2, ';');
                let kind_char = sub.next()?;
                let params = sub.next();

                let kind = match kind_char {
                    "A" => CommandKind::PromptStart,
                    "B" => CommandKind::CommandLineStart,
                    "C" => CommandKind::CommandLineOutput,
                    "D" => CommandKind::CommandDone,
                    _ => return Some(OscSequence::Unknown { code, payload: rest.to_string() }),
                };

                let exit_code = if kind == CommandKind::CommandDone {
                    params.and_then(|p: &str| p.parse::<i32>().ok())
                } else {
                    None
                };

                Some(OscSequence::CommandBoundary { kind, exit_code })
            }
            777 => {
                let mut sub = rest.splitn(3, ';');
                let _notify = sub.next()?;
                let name = sub.next()?.to_string();
                let value = sub.next()?.to_string();
                Some(OscSequence::EnvironmentVar { name, value })
            }
            _ => Some(OscSequence::Unknown {
                code,
                payload: rest.to_string(),
            }),
        }
    }

    /// Update internal shell state based on a parsed sequence.
    fn apply_sequence(&mut self, seq: &OscSequence) {
        match seq {
            OscSequence::CurrentDirectory(path) => {
                self.state.cwd = Some(path.clone());
                trace!("Shell CWD updated to {:?}", path);
            }
            OscSequence::CommandBoundary { kind, exit_code } => {
                if *kind == CommandKind::CommandDone {
                    self.state.last_exit_code = *exit_code;
                    debug!("Command completed with exit code {:?}", exit_code);
                }
            }
            OscSequence::EnvironmentVar { name, value } => {
                self.state.env_vars.insert(name.clone(), value.clone());
                trace!("Shell env var {} updated", name);
            }
            _ => {}
        }
    }

    /// Returns a clone of the current tracked shell state.
    pub fn state(&self) -> ShellState {
        self.state.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_osc7_cwd() {
        let mut parser = OscParser::new();
        let data = b"\x1b]7;file://host/home/user\x07";
        let seqs = parser.feed(data);
        assert_eq!(seqs.len(), 1);
        assert_eq!(
            seqs[0],
            OscSequence::CurrentDirectory(PathBuf::from("/home/user"))
        );
    }

    #[test]
    fn test_parse_osc133_command_done() {
        let mut parser = OscParser::new();
        let data = b"\x1b]133;D;0\x07";
        let seqs = parser.feed(data);
        assert_eq!(seqs.len(), 1);
        assert_eq!(
            seqs[0],
            OscSequence::CommandBoundary {
                kind: CommandKind::CommandDone,
                exit_code: Some(0),
            }
        );
        assert_eq!(parser.state().last_exit_code, Some(0));
    }

    #[test]
    fn test_parse_osc777_env() {
        let mut parser = OscParser::new();
        let data = b"\x1b]777;notify;FOO;bar\x07";
        let seqs = parser.feed(data);
        assert_eq!(seqs.len(), 1);
        assert_eq!(
            seqs[0],
            OscSequence::EnvironmentVar {
                name: "FOO".into(),
                value: "bar".into(),
            }
        );
    }

    #[test]
    fn test_ignores_plain_text() {
        let mut parser = OscParser::new();
        let data = b"hello world, no sequences here!";
        let seqs = parser.feed(data);
        assert!(seqs.is_empty());
    }
}
