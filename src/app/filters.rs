use super::{App, Mode};

impl App {
    pub fn add_filter_include(&mut self, pattern: &str) {
        match self.filter.add_include(pattern) {
            Ok(_) => self.set_status(format!("Filter +{}", pattern)),
            Err(e) => self.set_status(e),
        }
    }

    pub fn add_filter_exclude(&mut self, pattern: &str) {
        match self.filter.add_exclude(pattern) {
            Ok(_) => self.set_status(format!("Filter -{}", pattern)),
            Err(e) => self.set_status(e),
        }
    }

    pub fn clear_filters(&mut self) {
        self.filter.clear();
        self.set_status("Filters cleared".to_string());
    }

    pub fn open_filter_popup(&mut self) {
        self.filter_input.clear();
        self.filter_select_index = 0;
        self.open_overlay(Mode::Filter);
    }

    pub fn submit_filter(&mut self) {
        if !self.filter_input.is_empty() {
            let pattern = self.filter_input.clone();
            if self.filter_mode_is_exclude {
                self.add_filter_exclude(&pattern);
            } else {
                self.add_filter_include(&pattern);
            }
            self.filter_input.clear();
        }
        self.restore_mode();
    }

    pub fn remove_filter(&mut self, index: usize) {
        self.filter.remove(index);
        if self.filter.count() == 0 {
            self.set_status("All filters removed".to_string());
        }
        if self.filter_select_index >= self.filter.count() && self.filter_select_index > 0 {
            self.filter_select_index -= 1;
        }
    }
}
