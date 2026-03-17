use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

use crate::app::App;
use crate::theme::Theme;

/// Render the help overlay popup.
pub fn render(_app: &App, frame: &mut Frame, area: Rect) {
    let popup_width = 55.min(area.width.saturating_sub(4));
    let popup_height = 30.min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(" Keybindings ", Theme::help_overlay_title()))
        .borders(Borders::ALL)
        .border_style(Theme::help_overlay_border())
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(30, 31, 41)));

    let help_lines = vec![
        section_header("Normal Mode"),
        key_line("i", "Enter input mode"),
        key_line("q", "Quit"),
        key_line("j/k  ↑/↓", "Scroll up/down"),
        key_line("G / g", "Jump to bottom / top"),
        key_line("Ctrl+D/U", "Half-page down/up"),
        key_line("/", "Search through output"),
        key_line("n / N", "Next / previous match"),
        key_line("t", "Toggle timestamps"),
        key_line("h", "Toggle hex view"),
        key_line("e", "Toggle line ending indicators"),
        key_line("l", "Toggle session logging"),
        key_line("Ctrl+L", "Clear buffer"),
        key_line("p", "Open port selector"),
        key_line("s", "Open UART settings"),
        key_line("m", "Open macro selector"),
        key_line("f", "Open filters"),
        key_line("c", "Connect/disconnect"),
        key_line("?", "Toggle this help"),
        Line::from(""),
        section_header("Input Mode"),
        key_line("Enter", "Send command"),
        key_line("↑/↓", "History previous/next"),
        key_line("Esc", "Return to normal mode"),
        key_line("Ctrl+A/E", "Home/End"),
        Line::from(""),
        section_header("Search Mode"),
        key_line("Enter", "Confirm search"),
        key_line("↑/↓", "Previous/next match"),
        key_line("Esc", "Cancel search"),
    ];

    let paragraph = Paragraph::new(help_lines).block(block);
    frame.render_widget(paragraph, popup_area);
}

fn section_header(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Theme::popup_title(),
    ))
}

fn key_line(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:14}", key), Theme::help_key()),
        Span::styled(desc.to_string(), Theme::popup_item()),
    ])
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
