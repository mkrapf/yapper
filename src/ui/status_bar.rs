use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, ConnectionState};
use crate::theme::Theme;

/// Render the status bar at the top of the screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let mut spans = vec![
        Span::styled(" yap ", Theme::title()),
        Span::styled("── ", Theme::status_bar()),
    ];

    match &app.connection_state {
        ConnectionState::Connected(port) => {
            spans.push(Span::styled(port.as_str(), Theme::status_port_name()));
            spans.push(Span::styled(" @ ", Theme::status_bar()));
            spans.push(Span::styled(
                app.serial_config.summary(),
                Theme::status_baud(),
            ));
            spans.push(Span::styled("  ▸ ", Theme::status_bar()));
            spans.push(Span::styled("Connected", Theme::status_connected()));
            spans.push(Span::styled(" ◂", Theme::status_bar()));
        }
        ConnectionState::Disconnected => {
            spans.push(Span::styled(
                "No port selected",
                Theme::status_disconnected(),
            ));
            spans.push(Span::styled("  ▸ ", Theme::status_bar()));
            spans.push(Span::styled("Disconnected", Theme::status_disconnected()));
            spans.push(Span::styled(" ◂", Theme::status_bar()));
        }
        ConnectionState::Error(msg) => {
            spans.push(Span::styled("Error: ", Theme::status_error()));
            spans.push(Span::styled(
                truncate(msg, area.width as usize - 20),
                Theme::status_error(),
            ));
        }
    }

    // Scroll indicator on the right side
    if app.scroll_offset > 0 {
        let indicator = format!("  ↑{}", app.scroll_offset);
        spans.push(Span::styled(indicator, Theme::status_baud()));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Theme::status_bar());
    frame.render_widget(paragraph, area);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}
