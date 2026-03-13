use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::{App, Mode};

/// Regions of the UI for click detection.
/// These are set during rendering and read during mouse handling.
pub struct LayoutRegions {
    pub status_bar: (u16, u16, u16, u16),     // x, y, w, h
    pub terminal_view: (u16, u16, u16, u16),
    pub input_bar: (u16, u16, u16, u16),
}

impl Default for LayoutRegions {
    fn default() -> Self {
        Self {
            status_bar: (0, 0, 0, 0),
            terminal_view: (0, 0, 0, 0),
            input_bar: (0, 0, 0, 0),
        }
    }
}

/// Text selection state for click-drag-copy.
pub struct TextSelection {
    /// Whether a drag selection is in progress.
    pub is_selecting: bool,
    /// Start position (column, row in terminal coords).
    pub start: (u16, u16),
    /// End position (column, row in terminal coords).
    pub end: (u16, u16),
}

impl TextSelection {
    pub fn new() -> Self {
        Self {
            is_selecting: false,
            start: (0, 0),
            end: (0, 0),
        }
    }

    pub fn clear(&mut self) {
        self.is_selecting = false;
    }

    /// Get the selection range as (start_row, start_col, end_row, end_col), normalized.
    pub fn range(&self) -> (u16, u16, u16, u16) {
        let (sr, sc, er, ec) = if self.start.1 < self.end.1
            || (self.start.1 == self.end.1 && self.start.0 <= self.end.0)
        {
            (self.start.1, self.start.0, self.end.1, self.end.0)
        } else {
            (self.end.1, self.end.0, self.start.1, self.start.0)
        };
        (sr, sc, er, ec)
    }

    /// Check if a cell (col, row) is within the selection.
    pub fn contains(&self, col: u16, row: u16) -> bool {
        if !self.is_selecting {
            return false;
        }
        let (sr, sc, er, ec) = self.range();

        if row < sr || row > er {
            return false;
        }
        if sr == er {
            // Single line selection
            col >= sc && col <= ec
        } else if row == sr {
            col >= sc
        } else if row == er {
            col <= ec
        } else {
            true // Middle lines fully selected
        }
    }
}

/// Handle a mouse event.
pub fn handle_mouse_event(app: &mut App, event: MouseEvent) {
    match event.kind {
        // ── Scroll wheel ────────────────────────────────
        MouseEventKind::ScrollUp => {
            match app.mode {
                Mode::PortSelect => {
                    if app.port_select_index > 0 {
                        app.port_select_index -= 1;
                    }
                }
                Mode::Settings => {
                    if app.settings_field > 0 {
                        app.settings_field -= 1;
                    }
                }
                _ => {
                    app.scroll_up(3);
                }
            }
        }
        MouseEventKind::ScrollDown => {
            match app.mode {
                Mode::PortSelect => {
                    if app.port_select_index + 1 < app.available_ports.len() {
                        app.port_select_index += 1;
                    }
                }
                Mode::Settings => {
                    if app.settings_field < 4 {
                        app.settings_field += 1;
                    }
                }
                _ => {
                    app.scroll_down(3);
                }
            }
        }

        // ── Click ───────────────────────────────────────
        MouseEventKind::Down(MouseButton::Left) => {
            let col = event.column;
            let row = event.row;

            match app.mode {
                Mode::Normal | Mode::Input | Mode::Search => {
                    handle_click(app, col, row);
                }
                Mode::PortSelect => {
                    handle_port_click(app, col, row);
                }
                Mode::Settings => {
                    handle_settings_click(app, row);
                }
                _ => {}
            }
        }

        // ── Drag (text selection) ───────────────────────
        MouseEventKind::Drag(MouseButton::Left) => {
            app.selection.end = (event.column, event.row);
            if !app.selection.is_selecting {
                app.selection.is_selecting = true;
                app.selection.start = (event.column, event.row);
            }
        }

        // ── Release (copy selection) ────────────────────
        MouseEventKind::Up(MouseButton::Left) => {
            if app.selection.is_selecting {
                copy_selection(app);
                // Keep selection visible briefly
            }
        }

        _ => {}
    }
}

fn handle_click(app: &mut App, col: u16, row: u16) {
    let regions = &app.layout;

    // Clear any existing selection
    app.selection.clear();

    // Click on status bar
    let (sx, sy, sw, _sh) = regions.status_bar;
    if row == sy && col >= sx && col < sx + sw {
        // Left half: toggle connection, right half: open settings
        if col < sx + sw / 2 {
            app.toggle_connection();
        } else {
            app.open_settings();
        }
        return;
    }

    // Click on input bar
    let (ix, iy, _iw, _ih) = regions.input_bar;
    if row == iy && col >= ix {
        if app.mode != Mode::Input {
            app.mode = Mode::Input;
        }
        // Position cursor roughly
        let prompt_len = 4; // "> > " prefix
        let click_pos = (col as usize).saturating_sub(ix as usize + prompt_len);
        app.input_cursor = click_pos.min(app.input_text.len());
        return;
    }

    // Click on terminal view — start selection or just switch to normal mode
    let (_tx, ty, _tw, th) = regions.terminal_view;
    if row >= ty && row < ty + th {
        if app.mode == Mode::Input {
            app.mode = Mode::Normal;
        }
        // Set selection start for potential drag
        app.selection.start = (col, row);
        app.selection.end = (col, row);
    }
}

fn handle_port_click(app: &mut App, _col: u16, row: u16) {
    // The port selector is a centered popup. We need to figure out which
    // port was clicked based on the row. The popup has a 1-row title + 1-row padding,
    // so items start at roughly row offset 3 from the popup top.
    // For simplicity, we'll calculate based on terminal height.
    let total_height = app.layout.terminal_view.3 + 4; // rough terminal height
    let popup_height = (app.available_ports.len() as u16 + 6).min(total_height - 4);
    let popup_y = (total_height.saturating_sub(popup_height)) / 2;
    let item_start = popup_y + 3; // title + border + padding

    if row >= item_start {
        let clicked_index = (row - item_start) as usize;
        if clicked_index < app.available_ports.len() {
            app.port_select_index = clicked_index;
        }
    }
}

fn handle_settings_click(app: &mut App, row: u16) {
    // Settings popup has 5 fields with spacing. Fields are at rows 3, 5, 7, 9, 11
    // relative to the popup top.
    let total_height = app.layout.terminal_view.3 + 4;
    let popup_height = 16.min(total_height - 4);
    let popup_y = (total_height.saturating_sub(popup_height)) / 2;
    let field_start = popup_y + 2; // border + padding

    if row >= field_start {
        let relative = (row - field_start) as usize;
        // Fields are at relative positions 0, 2, 4, 6, 8 (with blank lines between)
        if relative % 2 == 0 {
            let field_index = relative / 2;
            if field_index < 5 {
                app.settings_field = field_index;
            }
        }
    }
}

/// Copy the selected text to clipboard.
fn copy_selection(app: &mut App) {
    if !app.selection.is_selecting {
        return;
    }

    let (start_row, _start_col, end_row, _end_col) = app.selection.range();
    let regions = &app.layout;
    let (_, ty, _, th) = regions.terminal_view;

    let mut selected_text = String::new();

    // Map screen rows to buffer lines
    let total_lines = app.buffer.display_len();
    let view_end = total_lines.saturating_sub(app.scroll_offset);
    let view_start = view_end.saturating_sub(th as usize);

    for screen_row in start_row..=end_row {
        if screen_row < ty || screen_row >= ty + th {
            continue;
        }
        let line_offset = (screen_row - ty) as usize;
        let line_index = view_start + line_offset;

        if line_index < app.buffer.len() {
            if let Some(entry) = app.buffer.get(line_index) {
                if !selected_text.is_empty() {
                    selected_text.push('\n');
                }
                selected_text.push_str(&entry.text);
            }
        }
    }

    if !selected_text.is_empty() {
        match cli_clipboard::set_contents(selected_text) {
            Ok(_) => {
                let lines = end_row - start_row + 1;
                app.set_status_pub(format!("Copied {} line(s)", lines));
            }
            Err(_) => {
                app.set_status_pub("Clipboard unavailable".to_string());
            }
        }
    }
}
