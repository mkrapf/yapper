use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::buffer::LineEnding;
use crate::hex;
use crate::theme::Theme;

/// Render the scrollable terminal output view.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    if app.hex_mode {
        render_hex_view(app, frame, area);
    } else {
        render_text_view(app, frame, area);
    }
}

fn render_text_view(app: &App, frame: &mut Frame, area: Rect) {
    let height = area.height as usize;
    if height == 0 {
        return;
    }

    let total_lines = app.buffer.display_len();
    let mut lines: Vec<Line> = Vec::with_capacity(height);

    if total_lines == 0 {
        let empty_msg = if app.is_connected() {
            "Waiting for data..."
        } else {
            "Press 'p' to select a port, or 'c' to connect"
        };
        lines.push(Line::from(Span::styled(
            format!("  {}", empty_msg),
            Theme::status_disconnected(),
        )));
    } else {
        let end = total_lines.saturating_sub(app.scroll_offset);
        let start = end.saturating_sub(height);

        for i in start..end {
            let line = if i < app.buffer.len() {
                if let Some(entry) = app.buffer.get(i) {
                    render_line(app, entry, i)
                } else {
                    Line::from("")
                }
            } else {
                // Partial line
                if let Some(partial) = app.buffer.partial_line() {
                    let mut spans = Vec::new();
                    if app.show_timestamps {
                        spans.push(Span::styled(
                            format!(" [{}] ", chrono::Local::now().format("%H:%M:%S%.3f")),
                            Theme::timestamp(),
                        ));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(partial, Theme::output_text()));
                    spans.push(Span::styled("▁", Theme::status_baud()));
                    Line::from(spans)
                } else {
                    Line::from("")
                }
            };
            lines.push(line);
        }
    }

    while lines.len() < height {
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(Theme::background()));
    frame.render_widget(paragraph, area);
}

fn render_hex_view(app: &App, frame: &mut Frame, area: Rect) {
    let height = area.height as usize;
    if height == 0 {
        return;
    }

    // Collect all raw bytes from buffer
    let mut all_bytes = Vec::new();
    for i in 0..app.buffer.len() {
        if let Some(entry) = app.buffer.get(i) {
            all_bytes.extend_from_slice(&entry.raw_bytes);
        }
    }

    if all_bytes.is_empty() {
        let paragraph = Paragraph::new(Line::from(Span::styled(
            "  No data to display in hex view",
            Theme::status_disconnected(),
        )))
        .style(Style::default().bg(Theme::background()));
        frame.render_widget(paragraph, area);
        return;
    }

    let hex_lines = hex::format_hex_lines(&all_bytes, 0);
    let total = hex_lines.len();

    let end = total.saturating_sub(app.scroll_offset);
    let start = end.saturating_sub(height);

    let mut lines: Vec<Line> = Vec::with_capacity(height);

    for i in start..end {
        if let Some(hex_line) = hex_lines.get(i) {
            let line = Line::from(vec![
                Span::styled(
                    format!(" {:08x}  ", hex_line.offset),
                    Theme::timestamp(),
                ),
                Span::styled(
                    format!("{:<23} ", hex_line.hex_left),
                    Theme::output_text(),
                ),
                Span::styled(
                    format!("{:<23} ", hex_line.hex_right),
                    Theme::output_text(),
                ),
                Span::styled("|", Theme::line_ending_indicator()),
                Span::styled(&hex_line.ascii, Theme::status_baud()),
                Span::styled("|", Theme::line_ending_indicator()),
            ]);
            lines.push(line);
        }
    }

    while lines.len() < height {
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(Theme::background()));
    frame.render_widget(paragraph, area);
}

/// Render a single complete line with optional timestamp, search highlight, and log level coloring.
fn render_line(app: &App, entry: &crate::buffer::LineEntry, line_index: usize) -> Line<'static> {
    let mut spans = Vec::new();

    // Timestamp
    if app.show_timestamps {
        let ts = entry.timestamp.format("%H:%M:%S%.3f").to_string();
        spans.push(Span::styled(
            format!(" [{}] ", ts),
            Theme::timestamp(),
        ));
    } else {
        spans.push(Span::raw(" "));
    }

    // Line content with log level coloring and search highlighting
    let is_match = app.search.is_match(line_index);
    let is_current = app.search.current_line() == Some(line_index);

    if is_current {
        // Current search match: bright highlight
        spans.push(Span::styled(
            entry.text.clone(),
            Style::default()
                .fg(Color::Rgb(22, 23, 30))
                .bg(Color::Rgb(255, 184, 108))
                .add_modifier(Modifier::BOLD),
        ));
    } else if is_match {
        // Other matches: subtle highlight
        spans.push(Span::styled(
            entry.text.clone(),
            Style::default()
                .fg(Color::Rgb(248, 248, 242))
                .bg(Color::Rgb(60, 63, 80)),
        ));
    } else {
        let style = Theme::style_for_line(&entry.text);
        spans.push(Span::styled(entry.text.clone(), style));
    }

    // Line ending indicator
    if app.show_line_endings && entry.line_ending != LineEnding::None {
        spans.push(Span::styled(
            format!(" {}", entry.line_ending.display()),
            Theme::line_ending_indicator(),
        ));
    }

    Line::from(spans)
}
