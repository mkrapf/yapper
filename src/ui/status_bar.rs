use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, ConnectionState};
use crate::theme::Theme;
use crate::ui::WidthMode;

/// Render the status bar at the top of the screen.
pub fn render(app: &App, frame: &mut Frame, area: Rect, layout_mode: WidthMode) {
    let mut spans = vec![
        Span::styled(" yapper ", Theme::title()),
        Span::styled("── ", Theme::status_bar()),
    ];

    // Check for temporary status message first
    if let Some((msg, _)) = &app.status_message {
        spans.push(Span::styled(msg.as_str(), Theme::status_baud()));
    } else {
        match &app.connection_state {
            ConnectionState::Connected(port) => {
                let port = truncate(port, port_budget(area.width, layout_mode));
                spans.push(Span::styled(port, Theme::status_port_name()));
                match layout_mode {
                    WidthMode::Full => {
                        spans.push(Span::styled(" @ ", Theme::status_bar()));
                        spans.push(Span::styled(
                            app.serial_config.summary(),
                            Theme::status_baud(),
                        ));
                        spans.push(Span::styled("  ▸ ", Theme::status_bar()));
                        spans.push(Span::styled("Connected", Theme::status_connected()));
                        spans.push(Span::styled(" ◂", Theme::status_bar()));
                        if let Some(rt) = app.last_response_time {
                            spans.push(Span::styled(
                                format!("  ↵ {}", format_duration(rt)),
                                Theme::status_baud(),
                            ));
                        }
                    }
                    WidthMode::Compact => {
                        spans.push(Span::styled("  ", Theme::status_bar()));
                        spans.push(Span::styled(
                            app.serial_config.summary(),
                            Theme::status_baud(),
                        ));
                        spans.push(Span::styled("  ", Theme::status_bar()));
                        spans.push(Span::styled("Connected", Theme::status_connected()));
                    }
                    WidthMode::Minimal => {
                        spans.push(Span::styled("  ", Theme::status_bar()));
                        spans.push(Span::styled("Connected", Theme::status_connected()));
                    }
                }
            }
            ConnectionState::Disconnected => {
                let label = match layout_mode {
                    WidthMode::Full => "No port selected",
                    WidthMode::Compact => "No port",
                    WidthMode::Minimal => "",
                };
                if !label.is_empty() {
                    spans.push(Span::styled(label, Theme::status_disconnected()));
                    spans.push(Span::styled("  ", Theme::status_bar()));
                }
                spans.push(Span::styled("Disconnected", Theme::status_disconnected()));
            }
            ConnectionState::Reconnecting(port) => {
                let port = truncate(port, port_budget(area.width, layout_mode));
                let (attempt, remaining) = app
                    .reconnect_status(std::time::Instant::now())
                    .unwrap_or((1, std::time::Duration::ZERO));
                spans.push(Span::styled(port, Theme::status_port_name()));
                spans.push(Span::styled("  ", Theme::status_bar()));
                match layout_mode {
                    WidthMode::Full => {
                        spans.push(Span::styled(
                            format!("Reconnect #{} in {}", attempt, format_duration(remaining)),
                            Theme::status_baud(),
                        ));
                    }
                    WidthMode::Compact => {
                        spans.push(Span::styled(
                            format!("Retry #{} {}", attempt, format_duration(remaining)),
                            Theme::status_baud(),
                        ));
                    }
                    WidthMode::Minimal => {
                        spans.push(Span::styled(
                            format!("Retry {}", format_duration(remaining)),
                            Theme::status_baud(),
                        ));
                    }
                }
            }
            ConnectionState::Error(msg) => {
                spans.push(Span::styled("Error: ", Theme::status_error()));
                spans.push(Span::styled(
                    truncate(msg, area.width as usize - 20),
                    Theme::status_error(),
                ));
            }
        }
    }

    // Mode indicators on the right
    let mut indicators = Vec::new();
    if app.hex_mode {
        indicators.push("HEX");
    }
    if app.hex_input_mode {
        indicators.push("HEX▹");
    }
    if app.show_line_endings {
        indicators.push("EOL");
    }
    if !app.show_timestamps {
        indicators.push("!TS");
    }
    if app.filter.is_active {
        indicators.push("FILT");
    }
    if !indicators.is_empty() {
        spans.push(Span::styled(
            format!("  [{}]", indicators.join("|")),
            Theme::status_baud(),
        ));
    }

    // Scroll indicator
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

fn port_budget(width: u16, mode: WidthMode) -> usize {
    match mode {
        WidthMode::Full => width.saturating_sub(45) as usize,
        WidthMode::Compact => width.saturating_sub(28) as usize,
        WidthMode::Minimal => width.saturating_sub(20) as usize,
    }
    .max(8)
}

fn format_duration(duration: std::time::Duration) -> String {
    if duration.as_millis() >= 1000 {
        format!("{:.1}s", duration.as_secs_f64())
    } else {
        format!("{}ms", duration.as_millis())
    }
}
