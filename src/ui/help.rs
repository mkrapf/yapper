use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

use crate::app::App;
use crate::theme::Theme;

/// Render the help overlay popup.
pub fn render(_app: &App, frame: &mut Frame, area: Rect) {
    let popup_width = 78.min(area.width.saturating_sub(4));
    let popup_height = 38.min(area.height.saturating_sub(4));
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
        key_line("x", "Toggle sent-message echo"),
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
        key_line("Tab or →", "Accept ghost suggestion"),
        key_line("Esc", "Return to normal mode"),
        key_line("Ctrl+A/E", "Home/End"),
        key_line("Ctrl+W/U", "Delete word / clear line"),
        key_line("Ctrl+H", "Toggle hex input"),
        key_line("Ctrl+P/S", "Ports / settings"),
        key_line("F1-F8", "Quick send frequent commands"),
        Line::from(""),
        section_header("Search Mode"),
        key_line("Enter", "Confirm search"),
        key_line("↑/↓", "Previous/next match"),
        key_line("Ctrl+N/P", "Next/previous match"),
        key_line("Esc", "Cancel search"),
        Line::from(""),
        section_header("Port Selector"),
        key_line("Enter", "Connect to selected port"),
        key_line("j/k  ↑/↓", "Move selection"),
        key_line("a", "Auto-detect baud on selected port"),
        key_line("r", "Refresh detected ports"),
        key_line("Esc or q", "Close selector"),
        Line::from(""),
        section_header("Settings"),
        key_line("↑/↓", "Select field"),
        key_line("←/→ or h/l", "Change selected value"),
        key_line("Tab", "Next value"),
        key_line("Enter", "Apply settings"),
        key_line("Esc or q", "Cancel and restore previous values"),
        Line::from(""),
        section_header("Macros and Filters"),
        key_line("m", "Open macro selector"),
        key_line("Macro Enter", "Run selected macro"),
        key_line("f", "Open regex filters"),
        key_line("Filter Tab", "Toggle include/exclude mode"),
        key_line("Filter Del or Ctrl+D", "Delete selected filter"),
        key_line("Filter Enter", "Apply filter and return"),
        Line::from(""),
        section_header("Mouse"),
        key_line("Wheel", "Scroll output or navigate popups"),
        key_line("Click input bar", "Focus input"),
        key_line("Click status bar", "Connect on left, settings on right"),
        key_line("Click-drag", "Select text and copy to clipboard"),
        Line::from(""),
        section_header("Status Legend"),
        key_line("HEX / HEX▹", "Hex output mode / hex input mode"),
        key_line(
            "EOL / !TS / FILT",
            "Line endings / timestamps off / filters active",
        ),
        key_line("↑N", "Scrolled N lines away from the bottom"),
        key_line("↵ 42ms", "Last command response time"),
    ];

    let paragraph = Paragraph::new(help_lines).block(block);
    frame.render_widget(paragraph, popup_area);
}

fn section_header(title: &str) -> Line<'static> {
    Line::from(Span::styled(title.to_string(), Theme::popup_title()))
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
