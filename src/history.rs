use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Persistent command history with file-backed storage.
pub struct CommandHistory {
    entries: Vec<String>,
    max_entries: usize,
    /// Current position when navigating with ↑/↓. None = not navigating.
    position: Option<usize>,
    /// Saved input text when user starts navigating (to restore on cancel).
    saved_input: String,
    /// File path for persistent storage.
    file_path: Option<PathBuf>,
}

impl CommandHistory {
    pub fn new(max_entries: usize) -> Self {
        let file_path = dirs::data_dir().map(|d| d.join("yap").join("history"));

        let mut history = Self {
            entries: Vec::new(),
            max_entries,
            position: None,
            saved_input: String::new(),
            file_path,
        };

        history.load();
        history
    }

    /// Add a command to history. Deduplicates consecutive entries.
    pub fn push(&mut self, command: String) {
        if command.is_empty() {
            return;
        }

        // Don't add duplicate of the last entry
        if self.entries.last().map(|s| s.as_str()) == Some(&command) {
            return;
        }

        self.entries.push(command);

        // Trim to max
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }

        self.position = None;
        self.save();
    }

    /// Start navigating history. Call this before the first previous() call.
    pub fn start_navigation(&mut self, current_input: &str) {
        if self.position.is_none() {
            self.saved_input = current_input.to_string();
        }
    }

    /// Navigate to the previous (older) entry. Returns the text to display.
    pub fn previous(&mut self, current_input: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }

        self.start_navigation(current_input);

        let new_pos = match self.position {
            None => self.entries.len() - 1,
            Some(0) => return Some(&self.entries[0]),
            Some(pos) => pos - 1,
        };

        self.position = Some(new_pos);
        Some(&self.entries[new_pos])
    }

    /// Navigate to the next (newer) entry. Returns the text to display.
    pub fn next(&mut self) -> Option<&str> {
        match self.position {
            None => None,
            Some(pos) => {
                if pos + 1 >= self.entries.len() {
                    // Back to the saved input
                    self.position = None;
                    Some(&self.saved_input)
                } else {
                    let new_pos = pos + 1;
                    self.position = Some(new_pos);
                    Some(&self.entries[new_pos])
                }
            }
        }
    }

    /// Reset navigation state (e.g., after sending a command).
    pub fn reset_navigation(&mut self) {
        self.position = None;
        self.saved_input.clear();
    }

    /// Load history from file.
    fn load(&mut self) {
        let path = match &self.file_path {
            Some(p) => p,
            None => return,
        };

        if !path.exists() {
            return;
        }

        if let Ok(file) = fs::File::open(path) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if !line.is_empty() {
                        self.entries.push(line);
                    }
                }
            }

            // Trim to max
            while self.entries.len() > self.max_entries {
                self.entries.remove(0);
            }
        }
    }

    /// Save history to file.
    fn save(&self) {
        let path = match &self.file_path {
            Some(p) => p,
            None => return,
        };

        // Create parent directories
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(mut file) = fs::File::create(path) {
            for entry in &self.entries {
                let _ = writeln!(file, "{}", entry);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_history() -> CommandHistory {
        CommandHistory {
            entries: Vec::new(),
            max_entries: 10,
            position: None,
            saved_input: String::new(),
            file_path: None, // No persistence in tests
        }
    }

    #[test]
    fn test_push_and_navigate() {
        let mut h = test_history();
        h.push("cmd1".to_string());
        h.push("cmd2".to_string());
        h.push("cmd3".to_string());

        assert_eq!(h.previous(""), Some("cmd3"));
        assert_eq!(h.previous(""), Some("cmd2"));
        assert_eq!(h.previous(""), Some("cmd1"));
        // At start, stays at first
        assert_eq!(h.previous(""), Some("cmd1"));

        assert_eq!(h.next(), Some("cmd2"));
        assert_eq!(h.next(), Some("cmd3"));
        // Past end, returns saved input
        assert_eq!(h.next(), Some(""));
    }

    #[test]
    fn test_dedup_consecutive() {
        let mut h = test_history();
        h.push("cmd1".to_string());
        h.push("cmd1".to_string());
        h.push("cmd2".to_string());
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn test_empty_not_added() {
        let mut h = test_history();
        h.push("".to_string());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn test_saves_current_input() {
        let mut h = test_history();
        h.push("cmd1".to_string());
        h.push("cmd2".to_string());

        // User is typing "partial" when they press ↑
        assert_eq!(h.previous("partial"), Some("cmd2"));
        assert_eq!(h.previous("partial"), Some("cmd1"));
        // Navigate back to get the saved input
        assert_eq!(h.next(), Some("cmd2"));
        assert_eq!(h.next(), Some("partial"));
    }

    #[test]
    fn test_max_entries() {
        let mut h = test_history();
        h.max_entries = 3;
        for i in 0..5 {
            h.push(format!("cmd{}", i));
        }
        assert_eq!(h.len(), 3);
        assert_eq!(h.previous(""), Some("cmd4"));
    }
}
