use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::buffer::LineEnding;
use crate::theme::Theme;

/// Render the scrollable terminal output view.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let height = area.height as usize;
    if height == 0 {
        return;
    }

    let total_lines = app.buffer.display_len();
    let mut lines: Vec<Line> = Vec::with_capacity(height);

    if total_lines == 0 {
        // Empty state
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
        // Calculate visible range (scroll_offset = 0 means bottom)
        let end = total_lines.saturating_sub(app.scroll_offset);
        let start = end.saturating_sub(height);

        for i in start..end {
            let line = if i < app.buffer.len() {
                // Complete line
                if let Some(entry) = app.buffer.get(i) {
                    render_line(app, entry, false)
                } else {
                    Line::from("")
                }
            } else {
                // Partial line (the incomplete one at the end)
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
                    // Show a blinking cursor indicator for partial line
                    spans.push(Span::styled("▁", Theme::status_baud()));

                    Line::from(spans)
                } else {
                    Line::from("")
                }
            };
            lines.push(line);
        }
    }

    // Pad with empty lines if needed
    while lines.len() < height {
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(Theme::background()));
    frame.render_widget(paragraph, area);
}

/// Render a single complete line with optional timestamp and log level coloring.
fn render_line(app: &App, entry: &crate::buffer::LineEntry, _is_partial: bool) -> Line<'static> {
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

    // Line content with log level coloring
    let style = Theme::style_for_line(&entry.text);
    spans.push(Span::styled(entry.text.clone(), style));

    // Line ending indicator
    if entry.line_ending != LineEnding::None {
        spans.push(Span::styled(
            format!(" {}", entry.line_ending.display()),
            Theme::line_ending_indicator(),
        ));
    }

    Line::from(spans)
}
