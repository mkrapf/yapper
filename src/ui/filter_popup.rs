use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

use crate::app::App;
use crate::theme::Theme;

/// Render the filter manager popup overlay.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let filter_count = app.filter.count();
    let popup_width = 55.min(area.width.saturating_sub(4));
    let popup_height = (filter_count as u16 + 10)
        .min(area.height.saturating_sub(4))
        .max(10);
    let popup_area = centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(" Filters ", Theme::popup_title()))
        .borders(Borders::ALL)
        .border_style(Theme::popup_border())
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(30, 31, 41)));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let mut lines: Vec<Line> = Vec::new();

    // Active filters
    let descriptions = app.filter.descriptions();
    if descriptions.is_empty() {
        lines.push(Line::from(Span::styled(
            "No active filters",
            Theme::status_disconnected(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "Active Filters:",
            Theme::popup_title(),
        )));
        for (i, desc) in descriptions.iter().enumerate() {
            let is_selected = i == app.filter_select_index;
            let prefix = if is_selected { "▸ " } else { "  " };
            let style = if is_selected {
                Theme::popup_selected()
            } else {
                Theme::popup_item()
            };
            let (sigil, pattern) = desc.split_at(1);
            let sigil_style = if sigil == "+" {
                Style::default()
                    .fg(Color::Rgb(80, 250, 123))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Rgb(255, 85, 85))
                    .add_modifier(Modifier::BOLD)
            };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(sigil, sigil_style),
                Span::styled(pattern.to_string(), style),
            ]));
        }
    }

    lines.push(Line::from(""));

    // Mode indicator
    let mode_label = if app.filter_mode_is_exclude {
        Span::styled(
            "[Exclude -] ",
            Style::default()
                .fg(Color::Rgb(255, 85, 85))
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            "[Include +] ",
            Style::default()
                .fg(Color::Rgb(80, 250, 123))
                .add_modifier(Modifier::BOLD),
        )
    };

    // Input line
    lines.push(Line::from(vec![
        Span::styled("New: ", Theme::popup_title()),
        mode_label,
        Span::styled(&app.filter_input, Theme::output_text()),
        Span::styled("▁", Theme::status_baud()),
    ]));

    lines.push(Line::from(""));

    // Help
    lines.push(Line::from(vec![
        Span::styled("Enter", Theme::help_key()),
        Span::styled(": apply  ", Theme::popup_item()),
        Span::styled("↑/↓", Theme::help_key()),
        Span::styled(": select  ", Theme::popup_item()),
        Span::styled("Tab", Theme::help_key()),
        Span::styled(": ±mode  ", Theme::popup_item()),
        Span::styled("Del/^D", Theme::help_key()),
        Span::styled(": delete  ", Theme::popup_item()),
        Span::styled("Esc", Theme::help_key()),
        Span::styled(": close", Theme::popup_item()),
    ]));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);

    // Position cursor at the input field
    let input_row = inner.y
        + (filter_count as u16).min(inner.height.saturating_sub(4))
        + if descriptions.is_empty() { 2 } else { 3 };
    let input_col = inner.x
        + 5
        + if app.filter_mode_is_exclude { 12 } else { 12 }
        + app.filter_input.len() as u16;
    if input_row < popup_area.bottom() && input_col < popup_area.right() {
        frame.set_cursor_position((input_col, input_row));
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
