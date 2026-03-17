use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph};

use crate::app::App;
use crate::theme::Theme;

/// Render the macro selector popup overlay.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let macros = app.macros.list();

    let popup_width = 55.min(area.width.saturating_sub(4));
    let popup_height = (macros.len() as u16 + 5).min(area.height.saturating_sub(4)).max(6);
    let popup_area = centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(" Macros ", Theme::popup_title()))
        .borders(Borders::ALL)
        .border_style(Theme::popup_border())
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(Color::Rgb(30, 31, 41)));

    if macros.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "No macros configured",
                Theme::status_disconnected(),
            )),
            Line::from(Span::styled(
                "Edit ~/.config/yapper/macros.toml",
                Theme::help_bar(),
            )),
        ])
        .block(block);
        frame.render_widget(msg, popup_area);
    } else {
        let items: Vec<ListItem> = macros
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let style = if i == app.macro_select_index {
                    Theme::popup_selected()
                } else {
                    Theme::popup_item()
                };

                let prefix = if i == app.macro_select_index {
                    "▸ "
                } else {
                    "  "
                };

                let line = Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(&m.name, style.add_modifier(Modifier::BOLD)),
                    Span::styled("  ", style),
                    Span::styled(&m.description, Theme::popup_description()),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, popup_area);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
