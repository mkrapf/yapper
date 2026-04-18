use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::{App, Mode};

/// Regions of the UI for click detection.
/// These are set during rendering and read during mouse handling.
pub struct LayoutRegions {
    pub status_bar: (u16, u16, u16, u16), // x, y, w, h
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
        MouseEventKind::ScrollUp => match app.mode {
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
        },
        MouseEventKind::ScrollDown => match app.mode {
            Mode::PortSelect => {
                if app.port_select_index + 1 < app.available_ports.len() {
                    app.port_select_index += 1;
                }
            }
            Mode::Settings => {
                if app.settings_field < 5 {
                    app.settings_field += 1;
                }
            }
            _ => {
                app.scroll_down(3);
            }
        },

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
    // Settings popup has 6 fields with spacing. Fields are at rows 3, 5, 7, 9, 11, 13
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
            if field_index < 6 {
                app.settings_field = field_index;
            }
        }
    }
}

/// Copy the selected text to clipboard, formatted to match the rendered view.
fn copy_selection(app: &mut App) {
    if !app.selection.is_selecting {
        return;
    }

    let (start_row, _start_col, end_row, _end_col) = app.selection.range();
    let regions = &app.layout;
    let (_, ty, _, th) = regions.terminal_view;

    if app.hex_mode {
        copy_hex_selection(app, start_row, end_row, ty, th);
        return;
    }

    let mut selected_text = String::new();

    // Build the same filtered visible indices as the renderer
    let filter_active = app.filter.is_active;
    let mut visible_indices: Vec<usize> = Vec::new();
    for i in 0..app.buffer.len() {
        if filter_active {
            if let Some(entry) = app.buffer.get(i) {
                if !app.filter.should_display(&entry.text) {
                    continue;
                }
            }
        }
        visible_indices.push(i);
    }
    if app.buffer.partial_line().is_some() {
        visible_indices.push(app.buffer.len()); // sentinel for partial line
    }

    let total_visible = visible_indices.len();
    let end = total_visible.saturating_sub(app.scroll_offset);
    let start = end.saturating_sub(th as usize);

    for screen_row in start_row..=end_row {
        if screen_row < ty || screen_row >= ty + th {
            continue;
        }
        let line_offset = (screen_row - ty) as usize;
        let vi = start + line_offset;

        if vi >= end {
            continue;
        }

        let i = visible_indices[vi];

        let formatted = if i < app.buffer.len() {
            if let Some(entry) = app.buffer.get(i) {
                format_entry_for_copy(
                    &entry.text,
                    entry.timestamp,
                    &entry.line_ending,
                    entry.is_sent,
                    app.show_timestamps,
                    &app.timestamp_format,
                    app.show_line_endings,
                )
            } else {
                continue;
            }
        } else {
            // Partial line
            if let Some(partial) = app.buffer.partial_line() {
                let mut line = String::new();
                if app.show_timestamps {
                    line.push_str(&format!(
                        "[{}] ",
                        chrono::Local::now().format(&app.timestamp_format)
                    ));
                }
                line.push_str(partial);
                line
            } else {
                continue;
            }
        };

        if !selected_text.is_empty() {
            selected_text.push('\n');
        }
        selected_text.push_str(&formatted);
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

/// Format a single buffer entry for clipboard copy, matching the rendered view.
fn format_entry_for_copy(
    text: &str,
    timestamp: chrono::DateTime<chrono::Local>,
    line_ending: &crate::buffer::LineEnding,
    is_sent: bool,
    show_timestamps: bool,
    timestamp_format: &str,
    show_line_endings: bool,
) -> String {
    let mut line = String::new();

    if show_timestamps {
        line.push_str(&format!("[{}] ", timestamp.format(timestamp_format)));
    }

    if is_sent {
        line.push_str("❯ ");
    }

    line.push_str(text);

    if show_line_endings && *line_ending != crate::buffer::LineEnding::None {
        line.push(' ');
        line.push_str(line_ending.display());
    }

    line
}

/// Copy hex view selection to clipboard.
fn copy_hex_selection(app: &mut App, start_row: u16, end_row: u16, ty: u16, th: u16) {
    let mut all_bytes = Vec::new();
    for i in 0..app.buffer.len() {
        if let Some(entry) = app.buffer.get(i) {
            all_bytes.extend_from_slice(&entry.raw_bytes);
        }
    }

    if all_bytes.is_empty() {
        return;
    }

    let hex_lines = crate::hex::format_hex_lines(&all_bytes, 0);
    let total = hex_lines.len();
    let end = total.saturating_sub(app.scroll_offset);
    let start = end.saturating_sub(th as usize);

    let mut selected_text = String::new();

    for screen_row in start_row..=end_row {
        if screen_row < ty || screen_row >= ty + th {
            continue;
        }
        let line_offset = (screen_row - ty) as usize;
        let hex_idx = start + line_offset;

        if hex_idx >= end {
            continue;
        }

        if let Some(hex_line) = hex_lines.get(hex_idx) {
            if !selected_text.is_empty() {
                selected_text.push('\n');
            }
            selected_text.push_str(&format!(
                "{:08x}  {:<23} {:<23} |{}|",
                hex_line.offset, hex_line.hex_left, hex_line.hex_right, hex_line.ascii
            ));
        }
    }

    if !selected_text.is_empty() {
        match cli_clipboard::set_contents(selected_text) {
            Ok(_) => {
                let lines = end_row - start_row + 1;
                app.set_status_pub(format!("Copied {} hex line(s)", lines));
            }
            Err(_) => {
                app.set_status_pub("Clipboard unavailable".to_string());
            }
        }
    }
}
