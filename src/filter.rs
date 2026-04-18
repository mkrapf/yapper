/// Regex-based line filter for show/hide of output lines.
pub struct LineFilter {
    /// Active filters.
    filters: Vec<FilterRule>,
    /// Whether filtering is enabled.
    pub is_active: bool,
}

pub struct FilterRule {
    pub pattern: String,
    pub regex: regex::Regex,
    pub mode: FilterMode,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FilterMode {
    /// Only show lines matching this pattern.
    Include,
    /// Hide lines matching this pattern.
    Exclude,
}

impl LineFilter {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            is_active: false,
        }
    }

    /// Add an include filter.
    pub fn add_include(&mut self, pattern: &str) -> Result<(), String> {
        let regex = regex::Regex::new(pattern).map_err(|e| format!("Invalid regex: {}", e))?;
        self.filters.push(FilterRule {
            pattern: pattern.to_string(),
            regex,
            mode: FilterMode::Include,
        });
        self.is_active = true;
        Ok(())
    }

    /// Add an exclude filter.
    pub fn add_exclude(&mut self, pattern: &str) -> Result<(), String> {
        let regex = regex::Regex::new(pattern).map_err(|e| format!("Invalid regex: {}", e))?;
        self.filters.push(FilterRule {
            pattern: pattern.to_string(),
            regex,
            mode: FilterMode::Exclude,
        });
        self.is_active = true;
        Ok(())
    }

    /// Check if a line should be displayed.
    pub fn should_display(&self, text: &str) -> bool {
        if !self.is_active || self.filters.is_empty() {
            return true;
        }

        let has_include_filters = self.filters.iter().any(|f| f.mode == FilterMode::Include);

        // If there are include filters, line must match at least one
        if has_include_filters {
            let matches_include = self
                .filters
                .iter()
                .filter(|f| f.mode == FilterMode::Include)
                .any(|f| f.regex.is_match(text));
            if !matches_include {
                return false;
            }
        }

        // Line must not match any exclude filter
        let matches_exclude = self
            .filters
            .iter()
            .filter(|f| f.mode == FilterMode::Exclude)
            .any(|f| f.regex.is_match(text));

        !matches_exclude
    }

    /// Clear all filters.
    pub fn clear(&mut self) {
        self.filters.clear();
        self.is_active = false;
    }

    /// Get the number of active filters.
    pub fn count(&self) -> usize {
        self.filters.len()
    }

    /// Get filter descriptions for display.
    pub fn descriptions(&self) -> Vec<String> {
        self.filters
            .iter()
            .map(|f| {
                let prefix = match f.mode {
                    FilterMode::Include => "+",
                    FilterMode::Exclude => "-",
                };
                format!("{}{}", prefix, f.pattern)
            })
            .collect()
    }

    /// Remove a filter by index.
    pub fn remove(&mut self, index: usize) {
        if index < self.filters.len() {
            self.filters.remove(index);
            self.is_active = !self.filters.is_empty();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_filters() {
        let filter = LineFilter::new();
        assert!(filter.should_display("anything"));
    }

    #[test]
    fn test_include_filter() {
        let mut filter = LineFilter::new();
        filter.add_include("ERROR|WARN").unwrap();
        assert!(filter.should_display("[ERROR] something broke"));
        assert!(filter.should_display("[WARN] low battery"));
        assert!(!filter.should_display("[INFO] all good"));
    }

    #[test]
    fn test_exclude_filter() {
        let mut filter = LineFilter::new();
        filter.add_exclude("DEBUG").unwrap();
        assert!(filter.should_display("[INFO] hello"));
        assert!(!filter.should_display("[DEBUG] verbose stuff"));
    }

    #[test]
    fn test_combined_filters() {
        let mut filter = LineFilter::new();
        filter.add_include("\\[.*\\]").unwrap(); // Must have brackets
        filter.add_exclude("DEBUG").unwrap(); // But not DEBUG
        assert!(filter.should_display("[INFO] hello"));
        assert!(!filter.should_display("[DEBUG] verbose"));
        assert!(!filter.should_display("no brackets here"));
    }

    #[test]
    fn test_clear() {
        let mut filter = LineFilter::new();
        filter.add_include("test").unwrap();
        assert_eq!(filter.count(), 1);
        filter.clear();
        assert_eq!(filter.count(), 0);
        assert!(filter.should_display("anything"));
    }

    #[test]
    fn test_invalid_regex() {
        let mut filter = LineFilter::new();
        assert!(filter.add_include("[invalid").is_err());
    }
}
