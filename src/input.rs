use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Mode};

/// Handle a key event and dispatch to the appropriate handler based on mode.
pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Global keybindings (work in any mode)
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        match app.mode {
            Mode::Normal => app.should_quit = true,
            _ => app.mode = Mode::Normal,
        }
        return;
    }

    match app.mode {
        Mode::Normal => handle_normal_mode(app, key),
        Mode::Input => handle_input_mode(app, key),
        Mode::Search => handle_search_mode(app, key),
        Mode::PortSelect => handle_port_select_mode(app, key),
        Mode::Help => handle_help_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        // Quit
        KeyCode::Char('q') => app.should_quit = true,

        // Enter input mode
        KeyCode::Char('i') => app.mode = Mode::Input,

        // Scrolling
        KeyCode::Char('j') | KeyCode::Down => app.scroll_down(1),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_up(1),
        KeyCode::Char('G') => app.scroll_to_bottom(),
        KeyCode::Char('g') => app.scroll_to_top(),
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_down(10);
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_up(10);
        }
        KeyCode::PageDown => app.scroll_down(20),
        KeyCode::PageUp => app.scroll_up(20),

        // Search
        KeyCode::Char('/') => app.start_search(),
        KeyCode::Char('n') => app.search_next(),
        KeyCode::Char('N') => app.search_prev(),

        // Toggles
        KeyCode::Char('t') => app.show_timestamps = !app.show_timestamps,
        KeyCode::Char('h') => app.toggle_hex_mode(),
        KeyCode::Char('e') => app.toggle_line_endings(),
        KeyCode::Char('l') => app.toggle_logging(),

        // Port selector
        KeyCode::Char('p') => app.open_port_selector(),

        // Connect/disconnect
        KeyCode::Char('c') => app.toggle_connection(),

        // Help
        KeyCode::Char('?') => app.mode = Mode::Help,

        _ => {}
    }
}

fn handle_input_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            app.send_command();
        }
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            app.input_backspace();
        }
        KeyCode::Delete => {
            app.input_delete();
        }
        KeyCode::Left => {
            app.input_cursor_left();
        }
        KeyCode::Right => {
            app.input_cursor_right();
        }
        KeyCode::Up => {
            app.history_previous();
        }
        KeyCode::Down => {
            app.history_next();
        }
        KeyCode::Home => {
            app.input_cursor_home();
        }
        KeyCode::End => {
            app.input_cursor_end();
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_cursor_home();
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_cursor_end();
        }
        KeyCode::Char(c) => {
            app.input_char(c);
        }
        _ => {}
    }
}

fn handle_search_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.end_search();
        }
        KeyCode::Enter => {
            // Confirm search and return to normal mode (matches stay highlighted)
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            app.search_backspace();
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.search_next();
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.search_prev();
        }
        KeyCode::Down => {
            app.search_next();
        }
        KeyCode::Up => {
            app.search_prev();
        }
        KeyCode::Char(c) => {
            app.search_char(c);
        }
        _ => {}
    }
}

fn handle_port_select_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            app.connect_selected_port();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.port_select_index > 0 {
                app.port_select_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.port_select_index + 1 < app.available_ports.len() {
                app.port_select_index += 1;
            }
        }
        // Refresh port list
        KeyCode::Char('r') => {
            app.available_ports = crate::serial::detector::available_ports();
            app.port_select_index = 0;
        }
        _ => {}
    }
}

fn handle_help_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
            app.mode = Mode::Normal;
        }
        _ => {}
    }
}
