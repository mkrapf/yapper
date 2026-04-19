use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::theme::Theme;
use crate::ui::WidthMode;

/// Render the quick-send bar showing recent commands.
pub fn render(app: &App, frame: &mut Frame, area: Rect, layout_mode: WidthMode) {
    let mut spans: Vec<Span> = Vec::new();
    let max_len = match layout_mode {
        WidthMode::Full => 14,
        WidthMode::Compact => 8,
        WidthMode::Minimal => 0,
    };

    for (i, cmd) in app.quicksend.iter().enumerate().take(8) {
        if i > 0 {
            spans.push(Span::styled("  ", Theme::help_bar()));
        }
        spans.push(Span::styled(format!("F{}", i + 1), Theme::help_key()));
        spans.push(Span::styled(":", Theme::help_bar()));
        // Truncate long commands
        let display = if max_len > 0 && cmd.len() > max_len {
            format!("{}…", &cmd[..max_len.saturating_sub(1)])
        } else {
            cmd.clone()
        };
        spans.push(Span::styled(display, Theme::status_baud()));
    }

    if spans.is_empty() {
        return;
    }

    let label = match layout_mode {
        WidthMode::Full => " Quick send (F1-F8): ",
        WidthMode::Compact => " F1-F8: ",
        WidthMode::Minimal => "",
    };
    if !label.is_empty() {
        spans.insert(0, Span::styled(label, Theme::help_bar()));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Theme::help_bar());
    frame.render_widget(paragraph, area);
}
