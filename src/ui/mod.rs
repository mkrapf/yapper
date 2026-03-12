mod help;
mod input_bar;
mod port_selector;
mod status_bar;
mod terminal_view;

use ratatui::prelude::*;
use ratatui::text::{Line, Span};

use crate::app::{App, Mode};
use crate::theme::Theme;

/// Render the entire application UI.
pub fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Layout depends on whether search bar is visible
    let has_search = app.mode == Mode::Search || !app.search.query.is_empty();

    let constraints = if has_search {
        vec![
            Constraint::Length(1), // Status bar
            Constraint::Min(3),   // Terminal view
            Constraint::Length(1), // Search bar
            Constraint::Length(1), // Input bar
            Constraint::Length(1), // Help hints
        ]
    } else {
        vec![
            Constraint::Length(1), // Status bar
            Constraint::Min(3),   // Terminal view
            Constraint::Length(1), // Input bar
            Constraint::Length(1), // Help hints
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    if has_search {
        status_bar::render(app, frame, chunks[0]);
        terminal_view::render(app, frame, chunks[1]);
        render_search_bar(app, frame, chunks[2]);
        input_bar::render(app, frame, chunks[3]);
        render_help_hints(app, frame, chunks[4]);
    } else {
        status_bar::render(app, frame, chunks[0]);
        terminal_view::render(app, frame, chunks[1]);
        input_bar::render(app, frame, chunks[2]);
        render_help_hints(app, frame, chunks[3]);
    }

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

/// Render the search bar.
fn render_search_bar(app: &App, frame: &mut Frame, area: Rect) {
    let is_active = app.mode == Mode::Search;

    let style = if is_active {
        Theme::input_bar_active()
    } else {
        Theme::input_bar()
    };

    let status = app.search.match_status();

    let spans = vec![
        Span::styled(" /", Theme::help_key()),
        Span::styled(&app.search.query, style),
        Span::styled("  ", style),
        Span::styled(status, Theme::timestamp()),
    ];

    let line = Line::from(spans);
    let paragraph = ratatui::widgets::Paragraph::new(line).style(style);
    frame.render_widget(paragraph, area);

    if is_active {
        frame.set_cursor_position((
            area.x + 2 + app.search.query.len() as u16,
            area.y,
        ));
    }
}

/// Render the bottom help hints bar.
fn render_help_hints(app: &App, frame: &mut Frame, area: Rect) {
    let hints = match app.mode {
        Mode::Normal => vec![
            Span::styled("i", Theme::help_key()),
            Span::styled(": input  ", Theme::help_bar()),
            Span::styled("j/k", Theme::help_key()),
            Span::styled(": scroll  ", Theme::help_bar()),
            Span::styled("/", Theme::help_key()),
            Span::styled(": search  ", Theme::help_bar()),
            Span::styled("h", Theme::help_key()),
            Span::styled(": hex  ", Theme::help_bar()),
            Span::styled("p", Theme::help_key()),
            Span::styled(": ports  ", Theme::help_bar()),
            Span::styled("l", Theme::help_key()),
            Span::styled(": log  ", Theme::help_bar()),
            Span::styled("?", Theme::help_key()),
            Span::styled(": help", Theme::help_bar()),
        ],
        Mode::Input => vec![
            Span::styled("Enter", Theme::help_key()),
            Span::styled(": send  ", Theme::help_bar()),
            Span::styled("↑/↓", Theme::help_key()),
            Span::styled(": history  ", Theme::help_bar()),
            Span::styled("Esc", Theme::help_key()),
            Span::styled(": normal", Theme::help_bar()),
        ],
        Mode::Search => vec![
            Span::styled("Enter", Theme::help_key()),
            Span::styled(": confirm  ", Theme::help_bar()),
            Span::styled("↑/↓", Theme::help_key()),
            Span::styled(": prev/next  ", Theme::help_bar()),
            Span::styled("Esc", Theme::help_key()),
            Span::styled(": cancel", Theme::help_bar()),
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

    // Byte counters + logging indicator
    let rx = format_bytes(app.total_rx_bytes());
    let tx = format_bytes(app.total_tx_bytes());
    let mut prefix = format!(" RX: {}  TX: {}", rx, tx);
    if app.logger.is_active {
        prefix.push_str("  ●REC");
    }
    prefix.push_str(" │ ");

    let mut line_spans = vec![
        Span::styled(prefix, Theme::help_bar()),
    ];
    line_spans.extend(hints);

    let line = Line::from(line_spans);
    let paragraph = ratatui::widgets::Paragraph::new(line)
        .style(Theme::help_bar());
    frame.render_widget(paragraph, area);
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
