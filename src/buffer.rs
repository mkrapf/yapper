use std::collections::VecDeque;

use chrono::Local;

/// Metadata attached to each line in the scrollback buffer.
#[derive(Clone, Debug)]
pub struct LineEntry {
    /// The text content of the line (without trailing newline).
    pub text: String,
    /// Timestamp when this line was received.
    pub timestamp: chrono::DateTime<Local>,
    /// The raw bytes that produced this line (for hex view).
    pub raw_bytes: Vec<u8>,
    /// Detected line ending that terminated this line.
    pub line_ending: LineEnding,
    /// Whether this line was sent by the user (vs received from serial).
    pub is_sent: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LineEnding {
    Lf,
    CrLf,
    Cr,
    /// Line is still being accumulated (no newline yet).
    None,
}

impl LineEnding {
    pub fn display(&self) -> &'static str {
        match self {
            LineEnding::Lf => "⏎",
            LineEnding::CrLf => "↵",
            LineEnding::Cr => "←",
            LineEnding::None => "",
        }
    }
}

/// Ring buffer for scrollback lines.
pub struct ScrollbackBuffer {
    lines: VecDeque<LineEntry>,
    max_lines: usize,
    /// Partial line being accumulated (not yet terminated by newline).
    partial: String,
    partial_text_bytes: Vec<u8>,
    partial_raw: Vec<u8>,
    pending_cr: bool,
}

impl ScrollbackBuffer {
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(max_lines.min(1024)),
            max_lines,
            partial: String::new(),
            partial_text_bytes: Vec::new(),
            partial_raw: Vec::new(),
            pending_cr: false,
        }
    }

    /// Push raw bytes into the buffer. Splits on newlines and accumulates
    /// partial lines until a newline is received.
    pub fn push_bytes(&mut self, data: &[u8]) {
        for &byte in data {
            if self.pending_cr && byte != b'\n' {
                self.commit_line(LineEnding::Cr);
            }

            self.partial_raw.push(byte);

            match byte {
                b'\n' => {
                    let line_ending = if self.pending_cr {
                        self.pending_cr = false;
                        LineEnding::CrLf
                    } else {
                        LineEnding::Lf
                    };
                    self.commit_line(line_ending);
                }
                b'\r' => {
                    self.pending_cr = true;
                }
                byte => {
                    self.partial_text_bytes.push(byte);
                    self.partial = String::from_utf8_lossy(&self.partial_text_bytes).into_owned();
                }
            }
        }
    }

    fn commit_line(&mut self, line_ending: LineEnding) {
        let text = std::mem::take(&mut self.partial);
        self.partial_text_bytes.clear();
        let raw = std::mem::take(&mut self.partial_raw);
        self.pending_cr = false;
        self.push_line(LineEntry {
            text,
            timestamp: Local::now(),
            raw_bytes: raw,
            line_ending,
            is_sent: false,
        });
    }

    fn push_line(&mut self, entry: LineEntry) {
        if self.lines.len() >= self.max_lines {
            self.lines.pop_front();
        }
        self.lines.push_back(entry);
    }

    /// Push a sent command as a line entry (not from serial data).
    pub fn push_sent_line(&mut self, text: String) {
        self.push_line(LineEntry {
            text,
            timestamp: Local::now(),
            raw_bytes: Vec::new(),
            line_ending: LineEnding::None,
            is_sent: true,
        });
    }

    /// Get the total number of complete lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.partial.is_empty() && !self.pending_cr
    }

    /// Get a specific line by index.
    pub fn get(&self, index: usize) -> Option<&LineEntry> {
        self.lines.get(index)
    }

    /// Get the current partial (unterminated) line, if any.
    pub fn partial_line(&self) -> Option<&str> {
        if self.partial.is_empty() && !self.pending_cr {
            None
        } else {
            Some(&self.partial)
        }
    }

    /// Total number of displayable lines (complete + partial).
    pub fn display_len(&self) -> usize {
        self.lines.len()
            + if self.partial.is_empty() && !self.pending_cr {
                0
            } else {
                1
            }
    }

    pub fn max_lines(&self) -> usize {
        self.max_lines
    }

    /// Iterate over all complete lines.
    pub fn iter(&self) -> impl Iterator<Item = &LineEntry> {
        self.lines.iter()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.partial.clear();
        self.partial_text_bytes.clear();
        self.partial_raw.clear();
        self.pending_cr = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_simple_lines() {
        let mut buf = ScrollbackBuffer::new(100);
        buf.push_bytes(b"hello\nworld\n");
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.get(0).unwrap().text, "hello");
        assert_eq!(buf.get(0).unwrap().line_ending, LineEnding::Lf);
        assert_eq!(buf.get(1).unwrap().text, "world");
    }

    #[test]
    fn test_push_crlf() {
        let mut buf = ScrollbackBuffer::new(100);
        buf.push_bytes(b"hello\r\nworld\r\n");
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.get(0).unwrap().text, "hello");
        assert_eq!(buf.get(0).unwrap().line_ending, LineEnding::CrLf);
        assert_eq!(buf.get(1).unwrap().text, "world");
        assert_eq!(buf.get(1).unwrap().line_ending, LineEnding::CrLf);
    }

    #[test]
    fn test_partial_line() {
        let mut buf = ScrollbackBuffer::new(100);
        buf.push_bytes(b"hello");
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.partial_line(), Some("hello"));

        buf.push_bytes(b" world\n");
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.get(0).unwrap().text, "hello world");
        assert_eq!(buf.partial_line(), None);
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut buf = ScrollbackBuffer::new(3);
        buf.push_bytes(b"a\nb\nc\nd\ne\n");
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.get(0).unwrap().text, "c");
        assert_eq!(buf.get(1).unwrap().text, "d");
        assert_eq!(buf.get(2).unwrap().text, "e");
    }

    #[test]
    fn test_cr_only_line_ending() {
        let mut buf = ScrollbackBuffer::new(100);
        buf.push_bytes(b"hello\rworld");
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.get(0).unwrap().text, "hello");
        assert_eq!(buf.get(0).unwrap().line_ending, LineEnding::Cr);
        assert_eq!(buf.partial_line(), Some("world"));
    }

    #[test]
    fn test_incremental_bytes() {
        let mut buf = ScrollbackBuffer::new(100);
        buf.push_bytes(b"hel");
        buf.push_bytes(b"lo\r");
        buf.push_bytes(b"\nworld\n");
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.get(0).unwrap().text, "hello");
        assert_eq!(buf.get(0).unwrap().line_ending, LineEnding::CrLf);
        assert_eq!(buf.get(1).unwrap().text, "world");
    }

    #[test]
    fn test_utf8_split_across_chunks() {
        let mut buf = ScrollbackBuffer::new(100);
        buf.push_bytes(&[0xc3]);
        buf.push_bytes(&[0xa9, b'\n']);

        assert_eq!(buf.get(0).unwrap().text, "é");
        assert_eq!(buf.get(0).unwrap().raw_bytes, vec![0xc3, 0xa9, b'\n']);
    }

    #[test]
    fn test_invalid_utf8_uses_replacement_character() {
        let mut buf = ScrollbackBuffer::new(100);
        buf.push_bytes(b"ok \xff\n");

        assert_eq!(buf.get(0).unwrap().text, "ok �");
        assert_eq!(buf.get(0).unwrap().raw_bytes, b"ok \xff\n");
    }

    #[test]
    fn test_display_len() {
        let mut buf = ScrollbackBuffer::new(100);
        assert_eq!(buf.display_len(), 0);

        buf.push_bytes(b"hello");
        assert_eq!(buf.display_len(), 1); // partial line

        buf.push_bytes(b"\n");
        assert_eq!(buf.display_len(), 1); // complete line, no partial

        buf.push_bytes(b"world");
        assert_eq!(buf.display_len(), 2); // complete + partial
    }

    #[test]
    fn test_clear() {
        let mut buf = ScrollbackBuffer::new(100);
        buf.push_bytes(b"hello\nworld");
        buf.clear();
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
        assert_eq!(buf.partial_line(), None);
    }
}
