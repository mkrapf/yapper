use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Mode};

/// Handle a key event and dispatch to the appropriate handler based on mode.
pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Global keybindings (work in any mode)
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        // Always return to Input mode (never quit — users press Ctrl+C after selecting text)
        app.mode = Mode::Input;
        return;
    }

    // Clear buffer (works in Normal and Input modes)
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
        if matches!(app.mode, Mode::Normal | Mode::Input) {
            app.clear_buffer();
            return;
        }
    }

    // F-key quick-send (works in Normal and Input modes)
    if matches!(app.mode, Mode::Normal | Mode::Input) {
        match key.code {
            KeyCode::F(n) if n >= 1 && n <= 8 => {
                app.send_quicksend((n - 1) as usize);
                return;
            }
            _ => {}
        }
    }

    match app.mode {
        Mode::Normal => handle_normal_mode(app, key),
        Mode::Input => handle_input_mode(app, key),
        Mode::Search => handle_search_mode(app, key),
        Mode::PortSelect => handle_port_select_mode(app, key),
        Mode::Settings => handle_settings_mode(app, key),
        Mode::Help => handle_help_mode(app, key),
        Mode::MacroSelect => handle_macro_select_mode(app, key),
        Mode::Filter => handle_filter_mode(app, key),
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
        KeyCode::Char('x') => app.show_sent = !app.show_sent,
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

        // UART settings
        KeyCode::Char('s') => app.open_settings(),

        // Macro selector
        KeyCode::Char('m') => app.open_macro_selector(),

        // Filter popup
        KeyCode::Char('f') => app.open_filter_popup(),

        // Connect/disconnect
        KeyCode::Char('c') => app.toggle_connection(),

        // Help
        KeyCode::Char('?') => app.open_help(),

        // Any other printable char: auto-enter input mode and insert
        KeyCode::Char(c) => {
            app.mode = Mode::Input;
            app.input_char(c);
        }
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
        // Word-level cursor movement (must come before unguarded Left/Right)
        KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_cursor_word_left();
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_cursor_word_right();
        }
        KeyCode::Left => {
            app.input_cursor_left();
        }
        KeyCode::Right => {
            // If cursor is at end and ghost suggestion exists, accept it
            if app.input_cursor == app.input_text.len() && app.ghost_suggestion.is_some() {
                app.accept_suggestion();
            } else {
                app.input_cursor_right();
            }
        }
        KeyCode::Tab => {
            // Accept ghost suggestion
            if app.ghost_suggestion.is_some() {
                app.accept_suggestion();
            }
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
        // Scroll without leaving input mode
        KeyCode::PageUp => app.scroll_up(20),
        KeyCode::PageDown => app.scroll_down(20),
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_delete_word_back();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_kill_line();
        }
        KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_hex_input();
        }
        // Quick access keybinds (stay in input mode after closing popup)
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_port_selector();
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_settings();
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
            // Confirm search and restore the previous mode (matches stay highlighted)
            app.restore_mode();
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
            app.restore_mode();
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
        // Auto-detect baud rate
        KeyCode::Char('a') => {
            if let Some(port) = app.available_ports.get(app.port_select_index) {
                let port_name = port.name.clone();
                app.auto_detect_baud(&port_name);
            }
        }
        _ => {}
    }
}

fn handle_help_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
            app.restore_mode();
        }
        _ => {}
    }
}

fn handle_settings_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.cancel_settings();
        }
        KeyCode::Enter => {
            app.apply_settings();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.settings_field > 0 {
                app.settings_field -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.settings_field < 5 {
                app.settings_field += 1;
            }
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            app.settings_next_value();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.settings_prev_value();
        }
        _ => {}
    }
}

fn handle_macro_select_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.restore_mode();
        }
        KeyCode::Enter => {
            app.execute_selected_macro();
            app.restore_mode();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.macro_select_index > 0 {
                app.macro_select_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let count = app.macros.list().len();
            if app.macro_select_index + 1 < count {
                app.macro_select_index += 1;
            }
        }
        _ => {}
    }
}

fn handle_filter_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.restore_mode();
        }
        KeyCode::Enter => {
            app.submit_filter();
        }
        KeyCode::Tab => {
            app.filter_mode_is_exclude = !app.filter_mode_is_exclude;
        }
        KeyCode::Backspace => {
            app.filter_input.pop();
        }
        KeyCode::Delete => {
            let count = app.filter.count();
            if count > 0 && app.filter_select_index < count {
                app.remove_filter(app.filter_select_index);
            }
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let count = app.filter.count();
            if count > 0 && app.filter_select_index < count {
                app.remove_filter(app.filter_select_index);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.filter_select_index > 0 {
                app.filter_select_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let count = app.filter.count();
            if app.filter_select_index + 1 < count {
                app.filter_select_index += 1;
            }
        }
        KeyCode::Char(c) => {
            app.filter_input.push(c);
        }
        _ => {}
    }
}
