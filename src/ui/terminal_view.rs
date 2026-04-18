use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode};
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

    let mut lines: Vec<Line> = Vec::with_capacity(height);

    // Extract state as primitives to avoid lifetime issues with Line<'static>
    let sel_range = if app.selection.is_selecting {
        Some(app.selection.range())
    } else {
        None
    };
    let show_ts = app.show_timestamps;
    let timestamp_format = app.timestamp_format.as_str();
    let show_le = app.show_line_endings;
    let color_log_levels = app.color_log_levels;
    let search_current = app.search.current_line();
    let search_matches: Vec<usize> = app.search.match_lines();
    let filter_active = app.filter.is_active;

    // Build visible line indices (applying filter if active)
    let mut visible_indices: Vec<usize> = Vec::new();
    for i in 0..app.buffer.len() {
        if filter_active {
            if let Some(entry) = app.buffer.get(i) {
                if !app.filter.should_display(&entry.text) {
                    continue;
                }
            }
        }
        visible_indices.push(i);
    }
    // Add partial line index if present
    let has_partial = app.buffer.partial_line().is_some();
    if has_partial {
        visible_indices.push(app.buffer.len()); // sentinel for partial line
    }

    let total_visible = visible_indices.len();

    if total_visible == 0 {
        let empty_msg = if app.is_connected() {
            if filter_active {
                "No lines match current filters"
            } else {
                "Waiting for data..."
            }
        } else if app.mode == Mode::Input {
            "Disconnected. Press Ctrl+P to choose a port, or Esc for browse mode"
        } else {
            "Disconnected. Press p to choose a port, c to connect, or ? for help"
        };
        lines.push(Line::from(Span::styled(
            format!("  {}", empty_msg),
            Theme::status_disconnected(),
        )));
    } else {
        let end = total_visible.saturating_sub(app.scroll_offset);
        let start = end.saturating_sub(height);

        for (screen_idx, vi) in (start..end).enumerate() {
            let screen_row = area.y + screen_idx as u16;
            let i = visible_indices[vi];

            let line = if i < app.buffer.len() {
                if let Some(entry) = app.buffer.get(i) {
                    let base = build_line(
                        &entry.text,
                        entry.timestamp,
                        &entry.line_ending,
                        i,
                        show_ts,
                        timestamp_format,
                        show_le,
                        color_log_levels,
                        search_current,
                        &search_matches,
                        entry.is_sent,
                    );
                    apply_selection(base, screen_row, area.x, sel_range)
                } else {
                    Line::from("")
                }
            } else {
                if let Some(partial) = app.buffer.partial_line() {
                    let mut spans = Vec::new();
                    if show_ts {
                        spans.push(Span::styled(
                            format!(" [{}] ", chrono::Local::now().format(timestamp_format)),
                            Theme::timestamp(),
                        ));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(partial.to_string(), Theme::output_text()));
                    spans.push(Span::styled("▁", Theme::status_baud()));
                    let base = Line::from(spans);
                    apply_selection(base, screen_row, area.x, sel_range)
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

    let paragraph = Paragraph::new(lines).style(Style::default().bg(Theme::background()));
    frame.render_widget(paragraph, area);
}

/// Apply text selection highlighting to a line if the selection overlaps this row.
fn apply_selection(
    line: Line<'static>,
    screen_row: u16,
    area_x: u16,
    sel_range: Option<(u16, u16, u16, u16)>,
) -> Line<'static> {
    let (sel_start_row, sel_start_col, sel_end_row, sel_end_col) = match sel_range {
        Some(r) => r,
        None => return line,
    };

    if screen_row < sel_start_row || screen_row > sel_end_row {
        return line;
    }

    let sel_style = Style::default()
        .bg(Color::Rgb(68, 71, 90))
        .fg(Color::Rgb(248, 248, 242));

    let row_sel_start = if screen_row == sel_start_row {
        sel_start_col.saturating_sub(area_x) as usize
    } else {
        0
    };
    let row_sel_end = if screen_row == sel_end_row {
        sel_end_col.saturating_sub(area_x) as usize
    } else {
        usize::MAX
    };

    let mut new_spans: Vec<Span<'static>> = Vec::new();
    let mut col: usize = 0;

    for span in line.spans {
        let span_text = span.content.to_string();
        let span_len = span_text.len();
        let span_end = col + span_len;

        if span_end <= row_sel_start || col > row_sel_end {
            new_spans.push(Span::styled(span_text, span.style));
        } else if col >= row_sel_start && span_end <= row_sel_end.saturating_add(1) {
            new_spans.push(Span::styled(span_text, sel_style));
        } else {
            let chars: Vec<char> = span_text.chars().collect();
            let mut segment = String::new();
            let mut in_sel = col >= row_sel_start && col <= row_sel_end;

            for (ci, &ch) in chars.iter().enumerate() {
                let cc = col + ci;
                let sel = cc >= row_sel_start && cc <= row_sel_end;
                if sel != in_sel && !segment.is_empty() {
                    new_spans.push(Span::styled(
                        segment.clone(),
                        if in_sel { sel_style } else { span.style },
                    ));
                    segment.clear();
                    in_sel = sel;
                }
                segment.push(ch);
            }
            if !segment.is_empty() {
                new_spans.push(Span::styled(
                    segment,
                    if in_sel { sel_style } else { span.style },
                ));
            }
        }
        col = span_end;
    }

    Line::from(new_spans)
}

fn render_hex_view(app: &App, frame: &mut Frame, area: Rect) {
    let height = area.height as usize;
    if height == 0 {
        return;
    }

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
                Span::styled(format!(" {:08x}  ", hex_line.offset), Theme::timestamp()),
                Span::styled(format!("{:<23} ", hex_line.hex_left), Theme::output_text()),
                Span::styled(format!("{:<23} ", hex_line.hex_right), Theme::output_text()),
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

    let paragraph = Paragraph::new(lines).style(Style::default().bg(Theme::background()));
    frame.render_widget(paragraph, area);
}

/// Build a single line with timestamp, search highlight, and optional line ending indicator.
/// Takes only owned/copied values to avoid lifetime conflicts.
fn build_line(
    text: &str,
    timestamp: chrono::DateTime<chrono::Local>,
    line_ending: &LineEnding,
    line_index: usize,
    show_timestamps: bool,
    timestamp_format: &str,
    show_line_endings: bool,
    color_log_levels: bool,
    search_current: Option<usize>,
    search_matches: &[usize],
    is_sent: bool,
) -> Line<'static> {
    let mut spans = Vec::new();

    if show_timestamps {
        let ts = timestamp.format(timestamp_format).to_string();
        spans.push(Span::styled(format!(" [{}] ", ts), Theme::timestamp()));
    } else {
        spans.push(Span::raw(" "));
    }

    // Sent message prefix
    if is_sent {
        spans.push(Span::styled(
            "❯ ",
            Style::default()
                .fg(Color::Rgb(139, 233, 253)) // cyan
                .add_modifier(Modifier::BOLD),
        ));
    }

    let is_current = search_current == Some(line_index);
    let is_match = search_matches.contains(&line_index);
    let owned_text = text.to_string();

    if is_current {
        spans.push(Span::styled(
            owned_text,
            Style::default()
                .fg(Color::Rgb(22, 23, 30))
                .bg(Color::Rgb(255, 184, 108))
                .add_modifier(Modifier::BOLD),
        ));
    } else if is_match {
        spans.push(Span::styled(
            owned_text,
            Style::default()
                .fg(Color::Rgb(248, 248, 242))
                .bg(Color::Rgb(60, 63, 80)),
        ));
    } else if is_sent {
        // Sent messages: use a distinct style (slightly dimmed cyan)
        spans.push(Span::styled(
            owned_text,
            Style::default().fg(Color::Rgb(139, 233, 253)),
        ));
    } else {
        // Apply syntax highlighting
        let highlights = crate::highlight::highlight_line(text);
        if highlights.is_empty() {
            let style = Theme::style_for_line(text, color_log_levels);
            spans.push(Span::styled(owned_text, style));
        } else {
            let base_style = Theme::style_for_line(text, color_log_levels);
            let mut pos = 0;
            for (range, hl_style) in &highlights {
                if range.start > pos {
                    spans.push(Span::styled(text[pos..range.start].to_string(), base_style));
                }
                spans.push(Span::styled(
                    text[range.start..range.end].to_string(),
                    *hl_style,
                ));
                pos = range.end;
            }
            if pos < text.len() {
                spans.push(Span::styled(text[pos..].to_string(), base_style));
            }
        }
    }

    if show_line_endings && *line_ending != LineEnding::None {
        spans.push(Span::styled(
            format!(" {}", line_ending.display()),
            Theme::line_ending_indicator(),
        ));
    }

    Line::from(spans)
}
