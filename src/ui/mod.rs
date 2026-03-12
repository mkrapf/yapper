mod help;
mod input_bar;
mod port_selector;
mod status_bar;
mod terminal_view;

use ratatui::prelude::*;

use crate::app::{App, Mode};
use crate::theme::Theme;

/// Render the entire application UI.
pub fn render(app: &App, frame: &mut Frame) {
    // Clear with background color
    let area = frame.area();

    // Main layout: status bar | terminal view | input bar | help hints
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar
            Constraint::Min(3),   // Terminal view
            Constraint::Length(1), // Input bar
            Constraint::Length(1), // Help hints
        ])
        .split(area);

    status_bar::render(app, frame, chunks[0]);
    terminal_view::render(app, frame, chunks[1]);
    input_bar::render(app, frame, chunks[2]);
    render_help_hints(app, frame, chunks[3]);

    // Overlays
    match app.mode {
        Mode::PortSelect => {
            port_selector::render(app, frame, area);
        }
        Mode::Help => {
            help::render(app, frame, area);
        }
        _ => {}
    }
}

/// Render the bottom help hints bar.
fn render_help_hints(app: &App, frame: &mut Frame, area: Rect) {
    use ratatui::text::{Line, Span};

    let hints = match app.mode {
        Mode::Normal => vec![
            Span::styled("i", Theme::help_key()),
            Span::styled(": input  ", Theme::help_bar()),
            Span::styled("j/k", Theme::help_key()),
            Span::styled(": scroll  ", Theme::help_bar()),
            Span::styled("p", Theme::help_key()),
            Span::styled(": ports  ", Theme::help_bar()),
            Span::styled("c", Theme::help_key()),
            Span::styled(": connect  ", Theme::help_bar()),
            Span::styled("t", Theme::help_key()),
            Span::styled(": timestamps  ", Theme::help_bar()),
            Span::styled("?", Theme::help_key()),
            Span::styled(": help  ", Theme::help_bar()),
            Span::styled("q", Theme::help_key()),
            Span::styled(": quit", Theme::help_bar()),
        ],
        Mode::Input => vec![
            Span::styled("Enter", Theme::help_key()),
            Span::styled(": send  ", Theme::help_bar()),
            Span::styled("Esc", Theme::help_key()),
            Span::styled(": normal mode  ", Theme::help_bar()),
            Span::styled("↑/↓", Theme::help_key()),
            Span::styled(": history", Theme::help_bar()),
        ],
        Mode::PortSelect => vec![
            Span::styled("Enter", Theme::help_key()),
            Span::styled(": connect  ", Theme::help_bar()),
            Span::styled("j/k", Theme::help_key()),
            Span::styled(": navigate  ", Theme::help_bar()),
            Span::styled("r", Theme::help_key()),
            Span::styled(": refresh  ", Theme::help_bar()),
            Span::styled("Esc", Theme::help_key()),
            Span::styled(": close", Theme::help_bar()),
        ],
        Mode::Help => vec![
            Span::styled("Esc", Theme::help_key()),
            Span::styled(": close help", Theme::help_bar()),
        ],
    };

    // Add byte counters on the left
    let rx = format_bytes(app.total_rx_bytes());
    let tx = format_bytes(app.total_tx_bytes());
    let mut line_spans = vec![
        Span::styled(format!(" RX: {}  TX: {} │ ", rx, tx), Theme::help_bar()),
    ];
    line_spans.extend(hints);

    let line = Line::from(line_spans);
    let paragraph = ratatui::widgets::Paragraph::new(line)
        .style(Theme::help_bar());
    frame.render_widget(paragraph, area);
}

/// Format byte count in a human-readable way.
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
