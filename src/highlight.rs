use ratatui::style::{Color, Modifier, Style};
use regex::Regex;
use std::ops::Range;
use std::sync::LazyLock;

/// A highlight rule: regex pattern mapped to a style.
struct HighlightRule {
    regex: Regex,
    style: Style,
}

static RULES: LazyLock<Vec<HighlightRule>> = LazyLock::new(|| {
    vec![
        // AT commands (AT+CMD...)
        HighlightRule {
            regex: Regex::new(r"(?i)\bAT\+?\w*").unwrap(),
            style: Style::default().fg(Color::Rgb(189, 147, 249)), // purple
        },
        // OK / READY responses
        HighlightRule {
            regex: Regex::new(r"\b(OK|READY|SUCCESS|DONE|PASS(ED)?)\b").unwrap(),
            style: Style::default()
                .fg(Color::Rgb(80, 250, 123))
                .add_modifier(Modifier::BOLD), // green
        },
        // ERROR / FAIL responses
        HighlightRule {
            regex: Regex::new(r"(?i)\b(ERROR|FAIL(ED|URE)?|FAULT|PANIC|ABORT)\b").unwrap(),
            style: Style::default()
                .fg(Color::Rgb(255, 85, 85))
                .add_modifier(Modifier::BOLD), // red
        },
        // Hex values (0xFF)
        HighlightRule {
            regex: Regex::new(r"\b0x[0-9A-Fa-f]+\b").unwrap(),
            style: Style::default().fg(Color::Rgb(255, 184, 108)), // orange
        },
        // IP addresses
        HighlightRule {
            regex: Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
            style: Style::default().fg(Color::Rgb(139, 233, 253)), // cyan
        },
        // Quoted strings
        HighlightRule {
            regex: Regex::new(r#""[^"]*""#).unwrap(),
            style: Style::default().fg(Color::Rgb(241, 250, 140)), // yellow
        },
        // Numbers (standalone)
        HighlightRule {
            regex: Regex::new(r"\b\d+(\.\d+)?\b").unwrap(),
            style: Style::default().fg(Color::Rgb(80, 250, 123)), // green
        },
    ]
});

/// Compute highlighted ranges for a line of text.
/// Returns (byte_range, style) pairs, sorted by start position.
/// Non-overlapping: first match wins for any position.
pub fn highlight_line(text: &str) -> Vec<(Range<usize>, Style)> {
    let mut ranges: Vec<(Range<usize>, Style)> = Vec::new();

    for rule in RULES.iter() {
        for m in rule.regex.find_iter(text) {
            let range = m.start()..m.end();
            // Check for overlap with existing ranges
            let overlaps = ranges
                .iter()
                .any(|(r, _)| r.start < range.end && range.start < r.end);
            if !overlaps {
                ranges.push((range, rule.style));
            }
        }
    }

    ranges.sort_by_key(|(r, _)| r.start);
    ranges
}
