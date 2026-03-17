use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode};
use crate::theme::Theme;

/// Render the input bar at the bottom of the terminal view.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let is_active = app.mode == Mode::Input;

    let bg_style = if is_active {
        Theme::input_bar_active()
    } else {
        Theme::input_bar()
    };

    let prompt = if app.hex_input_mode { " HEX❯ " } else { " ❯ " };
    let prompt_len = if app.hex_input_mode { 6u16 } else { 3u16 };

    let prompt_style = if app.hex_input_mode {
        Style::default()
            .fg(Color::Rgb(255, 184, 108))
            .add_modifier(Modifier::BOLD)
    } else if is_active {
        Theme::input_prompt()
    } else {
        Theme::input_prompt_inactive()
    };

    let mut spans = vec![
        Span::styled(prompt, prompt_style),
        Span::styled(&app.input_text, bg_style),
    ];

    // Show ghost suggestion suffix in dim text
    if is_active {
        if let Some(suggestion) = &app.ghost_suggestion {
            if suggestion.len() > app.input_text.len() {
                let suffix = &suggestion[app.input_text.len()..];
                spans.push(Span::styled(suffix, Theme::timestamp()));
            }
        }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(bg_style);
    frame.render_widget(paragraph, area);

    // Place the cursor if in input mode
    if is_active || app.hex_input_mode {
        frame.set_cursor_position((
            area.x + prompt_len + app.input_cursor as u16,
            area.y,
        ));
    }
}
