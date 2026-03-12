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

    let prompt_style = if is_active {
        Theme::input_prompt()
    } else {
        Theme::input_prompt_inactive()
    };

    let spans = vec![
        Span::styled(" ❯ ", prompt_style),
        Span::styled(&app.input_text, bg_style),
    ];

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(bg_style);
    frame.render_widget(paragraph, area);

    // Place the cursor if in input mode
    if is_active {
        frame.set_cursor_position((
            area.x + 3 + app.input_cursor as u16,
            area.y,
        ));
    }
}
