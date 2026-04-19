mod filter_popup;
mod help;
mod input_bar;
mod macro_selector;
mod port_selector;
mod quicksend_bar;
pub mod settings;
mod status_bar;
mod terminal_view;

use ratatui::prelude::*;
use ratatui::text::{Line, Span};

use crate::app::{App, Mode};
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WidthMode {
    Full,
    Compact,
    Minimal,
}

pub(crate) fn width_mode(width: u16) -> WidthMode {
    match width {
        0..=79 => WidthMode::Minimal,
        80..=99 => WidthMode::Compact,
        _ => WidthMode::Full,
    }
}

/// Render the entire application UI.
pub fn render(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let layout_mode = width_mode(area.width);

    // Layout depends on whether search bar and quicksend bar are visible
    let has_search = app.mode == Mode::Search || !app.search.query.is_empty();
    let has_quicksend = !app.quicksend.is_empty() && layout_mode != WidthMode::Minimal;

    let mut constraints = vec![
        Constraint::Length(1), // Status bar
    ];
    constraints.push(Constraint::Min(3)); // Terminal view
    if has_quicksend {
        constraints.push(Constraint::Length(1)); // Quick-send bar
    }
    if has_search {
        constraints.push(Constraint::Length(1)); // Search bar
    }
    constraints.push(Constraint::Length(1)); // Input bar
    constraints.push(Constraint::Length(1)); // Help hints

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0;

    // Status bar
    let r = chunks[idx];
    app.layout.status_bar = (r.x, r.y, r.width, r.height);
    status_bar::render(app, frame, chunks[idx], layout_mode);
    idx += 1;

    // Terminal view
    let r = chunks[idx];
    app.layout.terminal_view = (r.x, r.y, r.width, r.height);
    terminal_view::render(app, frame, chunks[idx]);
    idx += 1;

    // Quick-send bar
    if has_quicksend {
        quicksend_bar::render(app, frame, chunks[idx], layout_mode);
        idx += 1;
    }

    // Search bar
    if has_search {
        render_search_bar(app, frame, chunks[idx]);
        idx += 1;
    }

    // Input bar
    let r = chunks[idx];
    app.layout.input_bar = (r.x, r.y, r.width, r.height);
    input_bar::render(app, frame, chunks[idx]);
    idx += 1;

    // Help hints
    render_help_hints(app, frame, chunks[idx], layout_mode);

    // Overlays
    match app.mode {
        Mode::PortSelect => {
            port_selector::render(app, frame, area);
        }
        Mode::Settings => {
            settings::render(app, frame, area);
        }
        Mode::Help => {
            help::render(app, frame, area);
        }
        Mode::MacroSelect => {
            macro_selector::render(app, frame, area);
        }
        Mode::Filter => {
            filter_popup::render(app, frame, area);
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
        frame.set_cursor_position((area.x + 2 + app.search.query.len() as u16, area.y));
    }
}

/// Render the bottom help hints bar.
fn render_help_hints(app: &App, frame: &mut Frame, area: Rect, layout_mode: WidthMode) {
    let hints: Vec<(&str, &str)> = match (app.mode, layout_mode) {
        (Mode::Normal, WidthMode::Full) => vec![
            ("i", "input"),
            ("j/k", "scroll"),
            ("/", "search"),
            ("p", "ports"),
            ("s", "settings"),
            ("m", "macros"),
            ("M", "rerun"),
            ("f", "filters"),
            ("?", "help"),
        ],
        (Mode::Normal, WidthMode::Compact) => vec![
            ("i", "input"),
            ("/", "search"),
            ("p", "ports"),
            ("m", "macro"),
            ("M", "rerun"),
            ("?", "help"),
        ],
        (Mode::Normal, WidthMode::Minimal) => {
            vec![("i", "input"), ("/", "find"), ("p", "ports"), ("?", "help")]
        }
        (Mode::Input, WidthMode::Full) => vec![
            ("Enter", "send"),
            ("↑/↓", "history"),
            ("Tab", "accept"),
            ("F1-F8", "quick"),
            ("^P", "ports"),
            ("^S", "settings"),
            ("^H", "hex"),
            ("M", "rerun"),
            ("Esc", "browse"),
        ],
        (Mode::Input, WidthMode::Compact) => vec![
            ("Enter", "send"),
            ("Tab", "accept"),
            ("F1-F8", "quick"),
            ("M", "rerun"),
            ("Esc", "browse"),
        ],
        (Mode::Input, WidthMode::Minimal) => {
            vec![("Enter", "send"), ("Tab", "accept"), ("Esc", "back")]
        }
        (Mode::Search, WidthMode::Full | WidthMode::Compact) => vec![
            ("Enter", "confirm"),
            ("↑/↓", "prev/next"),
            ("*", "wildcard"),
            ("Esc", "close"),
        ],
        (Mode::Search, WidthMode::Minimal) => {
            vec![("Enter", "ok"), ("↑/↓", "move"), ("Esc", "close")]
        }
        (Mode::PortSelect, WidthMode::Full) => vec![
            ("Enter", "connect"),
            ("j/k", "move"),
            ("a", "auto-baud"),
            ("r", "refresh"),
            ("Esc", "close"),
        ],
        (Mode::PortSelect, WidthMode::Compact | WidthMode::Minimal) => vec![
            ("Enter", "connect"),
            ("a", "baud"),
            ("r", "refresh"),
            ("Esc", "close"),
        ],
        (Mode::Settings, WidthMode::Full | WidthMode::Compact) => vec![
            ("↑/↓", "field"),
            ("←/→", "change"),
            ("Enter", "apply"),
            ("Esc", "cancel"),
        ],
        (Mode::Settings, WidthMode::Minimal) => {
            vec![("←/→", "change"), ("Enter", "apply"), ("Esc", "cancel")]
        }
        (Mode::Help, WidthMode::Full | WidthMode::Compact) => {
            vec![("j/k", "scroll"), ("PgUp/Dn", "page"), ("Esc", "close")]
        }
        (Mode::Help, WidthMode::Minimal) => vec![("↑/↓", "scroll"), ("Esc", "close")],
        (Mode::MacroSelect, WidthMode::Full | WidthMode::Compact) => vec![
            ("Enter", "run"),
            ("r", "reload"),
            ("j/k", "move"),
            ("Esc", "close"),
        ],
        (Mode::MacroSelect, WidthMode::Minimal) => {
            vec![("Enter", "run"), ("r", "reload"), ("Esc", "close")]
        }
        (Mode::Filter, WidthMode::Full | WidthMode::Compact) => vec![
            ("Enter", "apply"),
            ("↑/↓", "select"),
            ("Tab", "+/-"),
            ("Del/^D", "delete"),
            ("Esc", "close"),
        ],
        (Mode::Filter, WidthMode::Minimal) => {
            vec![("Enter", "apply"), ("Del", "drop"), ("Esc", "close")]
        }
    };

    // Byte counters + logging indicator
    let rx = format_bytes(app.total_rx_bytes());
    let tx = format_bytes(app.total_tx_bytes());
    let mut prefix = match layout_mode {
        WidthMode::Full => format!(" RX: {}  TX: {}", rx, tx),
        WidthMode::Compact => format!(" RX {}  TX {}", rx, tx),
        WidthMode::Minimal => String::new(),
    };
    if app.logger.is_active {
        if !prefix.is_empty() {
            prefix.push_str("  ");
        }
        prefix.push_str("●REC");
    }
    if !prefix.is_empty() {
        prefix.push_str(" │ ");
    }

    let mut line_spans = vec![Span::styled(prefix, Theme::help_bar())];
    for (index, (key, label)) in hints.iter().enumerate() {
        if index > 0 {
            line_spans.push(Span::styled("  ", Theme::help_bar()));
        }
        line_spans.push(Span::styled(*key, Theme::help_key()));
        line_spans.push(Span::styled(": ", Theme::help_bar()));
        line_spans.push(Span::styled(*label, Theme::help_bar()));
    }

    let line = Line::from(line_spans);
    let paragraph = ratatui::widgets::Paragraph::new(line).style(Theme::help_bar());
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

#[cfg(test)]
mod tests {
    use super::{width_mode, WidthMode};

    #[test]
    fn test_width_mode_thresholds() {
        assert_eq!(width_mode(120), WidthMode::Full);
        assert_eq!(width_mode(100), WidthMode::Full);
        assert_eq!(width_mode(99), WidthMode::Compact);
        assert_eq!(width_mode(80), WidthMode::Compact);
        assert_eq!(width_mode(79), WidthMode::Minimal);
        assert_eq!(width_mode(60), WidthMode::Minimal);
    }
}
