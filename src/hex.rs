/// Format raw bytes as a hex dump view.
///
/// Output looks like:
/// ```text
/// 00000000  48 65 6c 6c 6f 20 57 6f  72 6c 64 0d 0a           |Hello World..|
/// ```
pub fn format_hex_lines(data: &[u8], offset: usize) -> Vec<HexLine> {
    let mut lines = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let chunk_end = (pos + 16).min(data.len());
        let chunk = &data[pos..chunk_end];

        let hex_left: Vec<String> = chunk
            .iter()
            .take(8)
            .map(|b| format!("{:02x}", b))
            .collect();

        let hex_right: Vec<String> = chunk
            .iter()
            .skip(8)
            .map(|b| format!("{:02x}", b))
            .collect();

        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();

        lines.push(HexLine {
            offset: offset + pos,
            hex_left: hex_left.join(" "),
            hex_right: hex_right.join(" "),
            ascii,
            byte_count: chunk.len(),
        });

        pos += 16;
    }

    lines
}

/// A single line in the hex view.
pub struct HexLine {
    pub offset: usize,
    pub hex_left: String,   // first 8 bytes
    pub hex_right: String,  // next 8 bytes
    pub ascii: String,
    pub byte_count: usize,
}

impl HexLine {
    /// Format as a complete display line.
    pub fn to_string(&self) -> String {
        // Pad hex sections to fixed width
        let left = format!("{:<23}", self.hex_left);
        let right = format!("{:<23}", self.hex_right);
        format!(
            "{:08x}  {} {} |{}|",
            self.offset,
            left,
            right,
            self.ascii
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_full_line() {
        let data = b"Hello, World!!!!";  // exactly 16 bytes
        let lines = format_hex_lines(data, 0);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.offset, 0);
        assert_eq!(line.ascii, "Hello, World!!!!");
        assert_eq!(line.byte_count, 16);
    }

    #[test]
    fn test_format_partial_line() {
        let data = b"Hi";
        let lines = format_hex_lines(data, 0);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.ascii, "Hi");
        assert_eq!(line.byte_count, 2);
    }

    #[test]
    fn test_format_multiple_lines() {
        let data = vec![0u8; 33]; // 2 full lines + 1 byte
        let lines = format_hex_lines(&data, 0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].offset, 0);
        assert_eq!(lines[1].offset, 16);
        assert_eq!(lines[2].offset, 32);
    }

    #[test]
    fn test_offset() {
        let data = b"test";
        let lines = format_hex_lines(data, 0x100);
        assert_eq!(lines[0].offset, 0x100);
    }

    #[test]
    fn test_non_printable_chars() {
        let data = &[0x00, 0x01, 0x41, 0x0a, 0x0d, 0x7f];
        let lines = format_hex_lines(data, 0);
        assert_eq!(lines[0].ascii, "..A...");
    }

    #[test]
    fn test_display_format() {
        let data = b"AB";
        let lines = format_hex_lines(data, 0);
        let display = lines[0].to_string();
        assert!(display.starts_with("00000000  41 42"));
        assert!(display.contains("|AB|"));
    }
}
