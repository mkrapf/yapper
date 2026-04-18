use ratatui::style::{Color, Modifier, Style};

/// Theme colors and styles for the TUI.
pub struct Theme;

impl Theme {
    // ── Status bar ──────────────────────────────────────

    pub fn status_bar() -> Style {
        Style::default()
            .fg(Color::Rgb(200, 200, 220))
            .bg(Color::Rgb(40, 42, 54))
    }

    pub fn status_connected() -> Style {
        Style::default()
            .fg(Color::Rgb(80, 250, 123))
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_disconnected() -> Style {
        Style::default().fg(Color::Rgb(139, 142, 164))
    }

    pub fn status_error() -> Style {
        Style::default()
            .fg(Color::Rgb(255, 85, 85))
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_port_name() -> Style {
        Style::default()
            .fg(Color::Rgb(189, 147, 249))
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_baud() -> Style {
        Style::default().fg(Color::Rgb(255, 184, 108))
    }

    // ── Terminal output ─────────────────────────────────

    pub fn output_text() -> Style {
        Style::default().fg(Color::Rgb(248, 248, 242))
    }

    pub fn timestamp() -> Style {
        Style::default().fg(Color::Rgb(98, 114, 164))
    }

    pub fn line_ending_indicator() -> Style {
        Style::default().fg(Color::Rgb(68, 71, 90))
    }

    // ── Log level colors ────────────────────────────────

    pub fn log_error() -> Style {
        Style::default()
            .fg(Color::Rgb(255, 85, 85))
            .add_modifier(Modifier::BOLD)
    }

    pub fn log_warn() -> Style {
        Style::default().fg(Color::Rgb(255, 184, 108))
    }

    pub fn log_info() -> Style {
        Style::default().fg(Color::Rgb(80, 250, 123))
    }

    pub fn log_debug() -> Style {
        Style::default().fg(Color::Rgb(139, 142, 164))
    }

    // ── Input bar ───────────────────────────────────────

    pub fn input_bar() -> Style {
        Style::default()
            .fg(Color::Rgb(248, 248, 242))
            .bg(Color::Rgb(30, 31, 41))
    }

    pub fn input_bar_active() -> Style {
        Style::default()
            .fg(Color::Rgb(248, 248, 242))
            .bg(Color::Rgb(44, 44, 58))
    }

    pub fn input_prompt() -> Style {
        Style::default()
            .fg(Color::Rgb(80, 250, 123))
            .add_modifier(Modifier::BOLD)
    }

    pub fn input_prompt_inactive() -> Style {
        Style::default().fg(Color::Rgb(98, 114, 164))
    }

    // ── Help hints bar ──────────────────────────────────

    pub fn help_bar() -> Style {
        Style::default()
            .fg(Color::Rgb(98, 114, 164))
            .bg(Color::Rgb(30, 31, 41))
    }

    pub fn help_key() -> Style {
        Style::default()
            .fg(Color::Rgb(189, 147, 249))
            .add_modifier(Modifier::BOLD)
    }

    // ── Port selector ───────────────────────────────────

    pub fn popup_border() -> Style {
        Style::default().fg(Color::Rgb(189, 147, 249))
    }

    pub fn popup_title() -> Style {
        Style::default()
            .fg(Color::Rgb(255, 184, 108))
            .add_modifier(Modifier::BOLD)
    }

    pub fn popup_selected() -> Style {
        Style::default()
            .fg(Color::Rgb(248, 248, 242))
            .bg(Color::Rgb(68, 71, 90))
    }

    pub fn popup_item() -> Style {
        Style::default().fg(Color::Rgb(200, 200, 220))
    }

    pub fn popup_description() -> Style {
        Style::default().fg(Color::Rgb(139, 142, 164))
    }

    // ── Help overlay ────────────────────────────────────

    pub fn help_overlay_border() -> Style {
        Style::default().fg(Color::Rgb(139, 142, 164))
    }

    pub fn help_overlay_title() -> Style {
        Style::default()
            .fg(Color::Rgb(255, 184, 108))
            .add_modifier(Modifier::BOLD)
    }

    // ── Borders ─────────────────────────────────────────

    pub fn border() -> Style {
        Style::default().fg(Color::Rgb(68, 71, 90))
    }

    pub fn border_focused() -> Style {
        Style::default().fg(Color::Rgb(189, 147, 249))
    }

    // ── General ─────────────────────────────────────────

    pub fn background() -> Color {
        Color::Rgb(22, 23, 30)
    }

    pub fn title() -> Style {
        Style::default()
            .fg(Color::Rgb(189, 147, 249))
            .add_modifier(Modifier::BOLD)
    }

    /// Detect log level from line content and return appropriate style.
    pub fn style_for_line(text: &str, color_log_levels: bool) -> Style {
        if !color_log_levels {
            return Self::output_text();
        }

        let upper = text.to_uppercase();
        if upper.contains("[ERROR]") || upper.contains("ERROR:") || upper.contains("PANIC") {
            Self::log_error()
        } else if upper.contains("[WARN]") || upper.contains("WARN:") || upper.contains("WARNING") {
            Self::log_warn()
        } else if upper.contains("[INFO]") || upper.contains("INFO:") {
            Self::log_info()
        } else if upper.contains("[DEBUG]") || upper.contains("DEBUG:") || upper.contains("[TRACE]")
        {
            Self::log_debug()
        } else {
            Self::output_text()
        }
    }
}
