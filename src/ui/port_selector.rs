use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph};

use crate::app::App;
use crate::theme::Theme;

/// Render the port selector popup overlay.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    // Center the popup
    let popup_width = 60.min(area.width.saturating_sub(4));
    let popup_height = (app.available_ports.len() as u16 + 6)
        .min(area.height.saturating_sub(4))
        .max(8);
    let popup_area = centered_rect(popup_width, popup_height, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(" Select Serial Port ", Theme::popup_title()))
        .borders(Borders::ALL)
        .border_style(Theme::popup_border())
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(Color::Rgb(30, 31, 41)));

    if app.available_ports.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "No serial ports found",
                Theme::status_disconnected(),
            )),
            Line::from(Span::styled(
                "Press r to refresh or Esc to return",
                Theme::help_bar(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Linux: check USB cable, power, and dialout/uucp permissions",
                Theme::popup_description(),
            )),
        ])
        .block(block);
        frame.render_widget(msg, popup_area);
    } else {
        let items: Vec<ListItem> = app
            .available_ports
            .iter()
            .enumerate()
            .map(|(i, port)| {
                let style = if i == app.port_select_index {
                    Theme::popup_selected()
                } else {
                    Theme::popup_item()
                };

                let prefix = if i == app.port_select_index {
                    "▸ "
                } else {
                    "  "
                };

                let line = Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(&port.name, style.add_modifier(Modifier::BOLD)),
                    Span::styled("  ", style),
                    Span::styled(&port.description, Theme::popup_description()),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, popup_area);
    }
}

/// Helper function to create a centered Rect.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
