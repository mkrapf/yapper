use super::App;

impl App {
    pub fn scroll_up(&mut self, lines: usize) {
        let view_height = self.layout.terminal_view.3 as usize;
        let max_scroll = self.buffer.display_len().saturating_sub(view_height);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
        if max_scroll > 0 {
            self.follow_output = false;
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        if self.scroll_offset == 0 {
            self.follow_output = true;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.follow_output = true;
    }

    pub fn scroll_to_top(&mut self) {
        let max_scroll = self.buffer.display_len().saturating_sub(1);
        self.scroll_offset = max_scroll;
        self.follow_output = false;
    }

    pub fn scroll_to_line(&mut self, line_index: usize) {
        let total = self.buffer.display_len();
        if total == 0 {
            return;
        }
        self.scroll_offset = total.saturating_sub(line_index + 1);
        self.follow_output = false;
    }

    pub fn start_search(&mut self) {
        self.search.activate();
        self.open_overlay(super::Mode::Search);
    }

    pub fn search_char(&mut self, c: char) {
        self.search.push_char(c);
        self.search.execute(&self.buffer);
        if let Some(line) = self.search.current_line() {
            self.scroll_to_line(line);
        }
    }

    pub fn search_backspace(&mut self) {
        self.search.pop_char();
        self.search.execute(&self.buffer);
    }

    pub fn search_next(&mut self) {
        if let Some(line) = self.search.next_match() {
            self.scroll_to_line(line);
        }
    }

    pub fn search_prev(&mut self) {
        if let Some(line) = self.search.prev_match() {
            self.scroll_to_line(line);
        }
    }

    pub fn end_search(&mut self) {
        self.search.deactivate();
        self.restore_mode();
    }

    pub fn scroll_help_up(&mut self, lines: u16) {
        self.help_scroll = self.help_scroll.saturating_sub(lines);
    }

    pub fn scroll_help_down(&mut self, lines: u16) {
        self.help_scroll = self
            .help_scroll
            .saturating_add(lines)
            .min(self.help_scroll_max);
    }

    pub fn set_help_scroll_max(&mut self, max_scroll: u16) {
        self.help_scroll_max = max_scroll;
        self.help_scroll = self.help_scroll.min(self.help_scroll_max);
    }
}
