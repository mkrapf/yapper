/// Search state for finding text in the scrollback buffer.
pub struct Search {
    /// Current search query.
    pub query: String,
    /// Whether search mode is active.
    pub is_active: bool,
    /// Indices of lines that match the query.
    pub matches: Vec<usize>,
    /// Current match index (into `matches` vec).
    pub current_match: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PatternToken {
    Literal(char),
    AnyOne,
    AnyMany,
}

impl Search {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            is_active: false,
            matches: Vec::new(),
            current_match: None,
        }
    }

    /// Activate search mode.
    pub fn activate(&mut self) {
        self.is_active = true;
        self.query.clear();
        self.matches.clear();
        self.current_match = None;
    }

    /// Deactivate search mode.
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Add a character to the search query.
    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    /// Remove the last character from the search query.
    pub fn pop_char(&mut self) {
        self.query.pop();
    }

    /// Execute the search against the buffer, updating matches.
    pub fn execute(&mut self, buffer: &crate::buffer::ScrollbackBuffer) {
        self.matches.clear();
        self.current_match = None;

        if self.query.is_empty() {
            return;
        }

        let query_lower = self.query.to_lowercase();
        let (tokens, has_wildcards) = parse_pattern(&query_lower);
        let literal_query = if has_wildcards {
            None
        } else {
            Some(
                tokens
                    .iter()
                    .filter_map(|token| match token {
                        PatternToken::Literal(ch) => Some(*ch),
                        _ => None,
                    })
                    .collect::<String>(),
            )
        };

        for i in 0..buffer.len() {
            if let Some(entry) = buffer.get(i) {
                let text = entry.text.to_lowercase();
                let is_match = match &literal_query {
                    Some(query) => text.contains(query),
                    None => wildcard_matches(&text, &tokens),
                };
                if is_match {
                    self.matches.push(i);
                }
            }
        }

        // Start at the last (most recent) match
        if !self.matches.is_empty() {
            self.current_match = Some(self.matches.len() - 1);
        }
    }

    /// Navigate to the next match (towards newer lines).
    pub fn next_match(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }

        let idx = match self.current_match {
            Some(i) => {
                if i + 1 >= self.matches.len() {
                    0 // Wrap around
                } else {
                    i + 1
                }
            }
            None => 0,
        };

        self.current_match = Some(idx);
        Some(self.matches[idx])
    }

    /// Navigate to the previous match (towards older lines).
    pub fn prev_match(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }

        let idx = match self.current_match {
            Some(0) => self.matches.len() - 1, // Wrap around
            Some(i) => i - 1,
            None => self.matches.len() - 1,
        };

        self.current_match = Some(idx);
        Some(self.matches[idx])
    }

    /// Get the current match line index.
    pub fn current_line(&self) -> Option<usize> {
        self.current_match.map(|i| self.matches[i])
    }

    /// Check if a line index is a match.
    pub fn is_match(&self, line_index: usize) -> bool {
        self.matches.contains(&line_index)
    }

    /// Get all matching line indices (cloned to avoid lifetime issues).
    pub fn match_lines(&self) -> Vec<usize> {
        self.matches.clone()
    }

    /// Get a display string for match count.
    pub fn match_status(&self) -> String {
        if self.query.is_empty() {
            String::new()
        } else if self.matches.is_empty() {
            "No matches".to_string()
        } else {
            let pos = self.current_match.map(|i| i + 1).unwrap_or(0);
            format!("{}/{}", pos, self.matches.len())
        }
    }
}

fn parse_pattern(query: &str) -> (Vec<PatternToken>, bool) {
    let mut tokens = Vec::new();
    let mut chars = query.chars().peekable();
    let mut has_wildcards = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.peek().copied() {
                Some(next @ ('*' | '?' | '\\')) => {
                    chars.next();
                    tokens.push(PatternToken::Literal(next));
                }
                Some(_) => tokens.push(PatternToken::Literal('\\')),
                None => tokens.push(PatternToken::Literal('\\')),
            },
            '*' => {
                has_wildcards = true;
                if !matches!(tokens.last(), Some(PatternToken::AnyMany)) {
                    tokens.push(PatternToken::AnyMany);
                }
            }
            '?' => {
                has_wildcards = true;
                tokens.push(PatternToken::AnyOne);
            }
            _ => tokens.push(PatternToken::Literal(ch)),
        }
    }

    (tokens, has_wildcards)
}

fn wildcard_matches(text: &str, tokens: &[PatternToken]) -> bool {
    let mut pattern = Vec::with_capacity(tokens.len() + 2);
    pattern.push(PatternToken::AnyMany);
    pattern.extend_from_slice(tokens);
    pattern.push(PatternToken::AnyMany);
    wildcard_matches_anchored(text, &pattern)
}

fn wildcard_matches_anchored(text: &str, pattern: &[PatternToken]) -> bool {
    let text_chars: Vec<char> = text.chars().collect();
    let mut previous = vec![false; pattern.len() + 1];
    previous[0] = true;

    for j in 1..=pattern.len() {
        if pattern[j - 1] == PatternToken::AnyMany {
            previous[j] = previous[j - 1];
        }
    }

    for ch in text_chars {
        let mut current = vec![false; pattern.len() + 1];
        for j in 1..=pattern.len() {
            current[j] = match pattern[j - 1] {
                PatternToken::Literal(expected) => previous[j - 1] && expected == ch,
                PatternToken::AnyOne => previous[j - 1],
                PatternToken::AnyMany => current[j - 1] || previous[j],
            };
        }
        previous = current;
    }

    previous[pattern.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::ScrollbackBuffer;

    fn make_buffer(lines: &[&str]) -> ScrollbackBuffer {
        let mut buf = ScrollbackBuffer::new(100);
        for line in lines {
            buf.push_bytes(format!("{}\n", line).as_bytes());
        }
        buf
    }

    #[test]
    fn test_search_basic() {
        let buf = make_buffer(&["hello world", "foo bar", "hello again"]);
        let mut search = Search::new();
        search.query = "hello".to_string();
        search.execute(&buf);

        assert_eq!(search.matches.len(), 2);
        assert_eq!(search.matches[0], 0);
        assert_eq!(search.matches[1], 2);
    }

    #[test]
    fn test_search_case_insensitive() {
        let buf = make_buffer(&["Hello World", "HELLO", "hello"]);
        let mut search = Search::new();
        search.query = "hello".to_string();
        search.execute(&buf);

        assert_eq!(search.matches.len(), 3);
    }

    #[test]
    fn test_search_navigation() {
        let buf = make_buffer(&["match1", "no", "match2", "no", "match3"]);
        let mut search = Search::new();
        search.query = "match".to_string();
        search.execute(&buf);

        assert_eq!(search.matches.len(), 3);
        // Starts at last match
        assert_eq!(search.current_line(), Some(4));

        // Next wraps to first
        assert_eq!(search.next_match(), Some(0));
        assert_eq!(search.next_match(), Some(2));
        assert_eq!(search.next_match(), Some(4));
        assert_eq!(search.next_match(), Some(0)); // wrap

        // Prev
        assert_eq!(search.prev_match(), Some(4)); // wrap back
        assert_eq!(search.prev_match(), Some(2));
    }

    #[test]
    fn test_search_no_matches() {
        let buf = make_buffer(&["hello", "world"]);
        let mut search = Search::new();
        search.query = "xyz".to_string();
        search.execute(&buf);

        assert!(search.matches.is_empty());
        assert_eq!(search.match_status(), "No matches");
    }

    #[test]
    fn test_match_status() {
        let buf = make_buffer(&["a", "b", "a"]);
        let mut search = Search::new();
        search.query = "a".to_string();
        search.execute(&buf);

        assert_eq!(search.match_status(), "2/2");
        search.prev_match();
        assert_eq!(search.match_status(), "1/2");
    }

    #[test]
    fn test_search_wildcards() {
        let buf = make_buffer(&["err abc 42", "warn", "error 42"]);
        let mut search = Search::new();
        search.query = "err*42".to_string();
        search.execute(&buf);

        assert_eq!(search.matches, vec![0, 2]);
    }

    #[test]
    fn test_search_question_mark_wildcard() {
        let buf = make_buffer(&["AT+CWJAP", "AT+CW1AP", "AT+CW12AP"]);
        let mut search = Search::new();
        search.query = "AT+CW?AP".to_string();
        search.execute(&buf);

        assert_eq!(search.matches, vec![0, 1]);
    }

    #[test]
    fn test_search_escaped_wildcards_and_trailing_backslash() {
        let buf = make_buffer(&["literal *", "path\\", "plain"]);
        let mut search = Search::new();
        search.query = "\\*".to_string();
        search.execute(&buf);
        assert_eq!(search.matches, vec![0]);

        search.query = "\\".to_string();
        search.execute(&buf);
        assert_eq!(search.matches, vec![1]);
    }

    #[test]
    fn test_search_is_unanchored_for_wildcards() {
        let buf = make_buffer(&["prefix err xyz 42 suffix", "no match"]);
        let mut search = Search::new();
        search.query = "err*42".to_string();
        search.execute(&buf);

        assert_eq!(search.matches, vec![0]);
    }
}
