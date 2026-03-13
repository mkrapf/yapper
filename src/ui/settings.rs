use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

use crate::app::App;
use crate::theme::Theme;

/// Common baud rates for quick selection.
pub const BAUD_RATES: &[u32] = &[
    300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
];

/// Render the UART settings popup.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 16.min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(" Serial Settings ", Theme::popup_title()))
        .borders(Borders::ALL)
        .border_style(Theme::help_overlay_border())
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(30, 31, 41)));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let config = &app.serial_config;
    let selected = app.settings_field;

    let fields = vec![
        ("Baud Rate", format!("◂ {} ▸", config.baud_rate)),
        ("Data Bits", format!("◂ {} ▸", data_bits_str(config.data_bits))),
        ("Parity", format!("◂ {} ▸", parity_str(config.parity))),
        ("Stop Bits", format!("◂ {} ▸", stop_bits_str(config.stop_bits))),
        ("Flow Ctrl", format!("◂ {} ▸", flow_control_str(config.flow_control))),
    ];

    let mut lines = Vec::new();

    for (i, (label, value)) in fields.iter().enumerate() {
        let is_selected = i == selected;

        let label_style = if is_selected {
            Style::default()
                .fg(Color::Rgb(139, 233, 253))
                .add_modifier(Modifier::BOLD)
        } else {
            Theme::popup_item()
        };

        let value_style = if is_selected {
            Style::default()
                .fg(Color::Rgb(80, 250, 123))
                .add_modifier(Modifier::BOLD)
        } else {
            Theme::status_baud()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:12}", label), label_style),
            Span::styled(value.clone(), value_style),
        ]));
        lines.push(Line::from("")); // spacing
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ↑/↓", Theme::help_key()),
        Span::styled(" select  ", Theme::popup_item()),
        Span::styled("←/→", Theme::help_key()),
        Span::styled(" change  ", Theme::popup_item()),
        Span::styled("Enter", Theme::help_key()),
        Span::styled(" apply  ", Theme::popup_item()),
        Span::styled("Esc", Theme::help_key()),
        Span::styled(" cancel", Theme::popup_item()),
    ]));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn data_bits_str(db: serialport::DataBits) -> &'static str {
    match db {
        serialport::DataBits::Five => "5",
        serialport::DataBits::Six => "6",
        serialport::DataBits::Seven => "7",
        serialport::DataBits::Eight => "8",
    }
}

fn parity_str(p: serialport::Parity) -> &'static str {
    match p {
        serialport::Parity::None => "None",
        serialport::Parity::Odd => "Odd",
        serialport::Parity::Even => "Even",
    }
}

fn stop_bits_str(sb: serialport::StopBits) -> &'static str {
    match sb {
        serialport::StopBits::One => "1",
        serialport::StopBits::Two => "2",
    }
}

fn flow_control_str(fc: serialport::FlowControl) -> &'static str {
    match fc {
        serialport::FlowControl::None => "None",
        serialport::FlowControl::Software => "Software",
        serialport::FlowControl::Hardware => "Hardware",
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
