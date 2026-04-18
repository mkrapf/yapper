use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::theme::Theme;

/// Render the quick-send bar showing frequently used commands.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();

    for (i, cmd) in app.quicksend.iter().enumerate().take(8) {
        if i > 0 {
            spans.push(Span::styled("  ", Theme::help_bar()));
        }
        spans.push(Span::styled(format!("F{}", i + 1), Theme::help_key()));
        spans.push(Span::styled(":", Theme::help_bar()));
        // Truncate long commands
        let display = if cmd.len() > 12 {
            format!("{}…", &cmd[..11])
        } else {
            cmd.clone()
        };
        spans.push(Span::styled(display, Theme::status_baud()));
    }

    if spans.is_empty() {
        return;
    }

    spans.insert(0, Span::styled(" Quick send (F1-F8): ", Theme::help_bar()));

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Theme::help_bar());
    frame.render_widget(paragraph, area);
}
