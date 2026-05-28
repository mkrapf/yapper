use super::App;

impl App {
    pub fn input_char(&mut self, c: char) {
        let byte_index = crate::display::byte_index_for_char(&self.input_text, self.input_cursor);
        self.input_text.insert(byte_index, c);
        self.input_cursor += 1;
        self.update_ghost();
    }

    pub fn input_backspace(&mut self) {
        if self.input_cursor > 0 {
            let end = crate::display::byte_index_for_char(&self.input_text, self.input_cursor);
            self.input_cursor -= 1;
            let start = crate::display::byte_index_for_char(&self.input_text, self.input_cursor);
            self.input_text.replace_range(start..end, "");
            self.update_ghost();
        }
    }

    pub fn input_delete(&mut self) {
        if self.input_cursor < crate::display::char_count(&self.input_text) {
            let start = crate::display::byte_index_for_char(&self.input_text, self.input_cursor);
            let end = crate::display::byte_index_for_char(&self.input_text, self.input_cursor + 1);
            self.input_text.replace_range(start..end, "");
            self.update_ghost();
        }
    }

    pub fn input_cursor_left(&mut self) {
        self.input_cursor = self.input_cursor.saturating_sub(1);
    }

    pub fn input_cursor_right(&mut self) {
        self.input_cursor =
            (self.input_cursor + 1).min(crate::display::char_count(&self.input_text));
    }

    pub fn input_cursor_home(&mut self) {
        self.input_cursor = 0;
    }

    pub fn input_cursor_end(&mut self) {
        self.input_cursor = crate::display::char_count(&self.input_text);
    }

    pub fn update_ghost(&mut self) {
        if self.input_cursor != crate::display::char_count(&self.input_text)
            || self.input_text.is_empty()
        {
            self.ghost_suggestion = None;
            return;
        }
        self.ghost_suggestion = self
            .history
            .suggest(&self.input_text)
            .map(|s| s.to_string());
    }

    pub fn accept_suggestion(&mut self) {
        if let Some(suggestion) = self.ghost_suggestion.take() {
            self.input_text = suggestion;
            self.input_cursor = crate::display::char_count(&self.input_text);
        }
    }

    pub fn history_previous(&mut self) {
        if let Some(text) = self.history.previous(&self.input_text) {
            self.input_text = text.to_string();
            self.input_cursor = crate::display::char_count(&self.input_text);
        }
    }

    pub fn history_next(&mut self) {
        if let Some(text) = self.history.next() {
            self.input_text = text.to_string();
            self.input_cursor = crate::display::char_count(&self.input_text);
        }
    }

    pub fn input_cursor_word_left(&mut self) {
        let chars: Vec<char> = self.input_text.chars().collect();
        if self.input_cursor == 0 {
            return;
        }
        let mut pos = self.input_cursor;
        while pos > 0 && !chars[pos - 1].is_alphanumeric() {
            pos -= 1;
        }
        while pos > 0 && chars[pos - 1].is_alphanumeric() {
            pos -= 1;
        }
        self.input_cursor = pos;
    }

    pub fn input_cursor_word_right(&mut self) {
        let chars: Vec<char> = self.input_text.chars().collect();
        let len = chars.len();
        if self.input_cursor >= len {
            return;
        }
        let mut pos = self.input_cursor;
        while pos < len && chars[pos].is_alphanumeric() {
            pos += 1;
        }
        while pos < len && !chars[pos].is_alphanumeric() {
            pos += 1;
        }
        self.input_cursor = pos;
    }

    pub fn input_delete_word_back(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let old_cursor = self.input_cursor;
        self.input_cursor_word_left();
        let new_cursor = self.input_cursor;
        let chars: Vec<char> = self.input_text.chars().collect();
        self.input_text = chars[..new_cursor]
            .iter()
            .chain(chars[old_cursor..].iter())
            .collect();
        self.update_ghost();
    }

    pub fn input_kill_line(&mut self) {
        self.input_text.clear();
        self.input_cursor = 0;
        self.ghost_suggestion = None;
    }
}
