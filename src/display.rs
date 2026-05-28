use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub fn char_count(text: &str) -> usize {
    text.chars().count()
}

pub fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

pub fn byte_index_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

pub fn width_prefix_chars(text: &str, char_count: usize) -> usize {
    text.chars().take(char_count).map(char_width).sum()
}

pub fn char_index_for_width(text: &str, target_width: usize) -> usize {
    let mut width = 0;
    for (index, ch) in text.chars().enumerate() {
        let next_width = width + char_width(ch);
        if next_width > target_width {
            return index;
        }
        width = next_width;
    }
    char_count(text)
}

pub fn truncate_width(text: &str, max_width: usize) -> String {
    if width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }

    let mut truncated = String::new();
    let mut used = 0;
    let content_width = max_width.saturating_sub(1);
    for ch in text.chars() {
        let ch_width = char_width(ch);
        if used + ch_width > content_width {
            break;
        }
        truncated.push(ch);
        used += ch_width;
    }
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_indices_and_widths_handle_wide_text() {
        let text = "a界é";

        assert_eq!(char_count(text), 3);
        assert_eq!(byte_index_for_char(text, 2), "a界".len());
        assert_eq!(width_prefix_chars(text, 2), 3);
        assert_eq!(char_index_for_width(text, 2), 1);
        assert_eq!(char_index_for_width(text, 3), 2);
    }

    #[test]
    fn test_truncate_width_preserves_boundaries() {
        assert_eq!(truncate_width("abcdef", 4), "abc…");
        assert_eq!(truncate_width("a界bc", 4), "a界…");
        assert_eq!(truncate_width("界abc", 2), "…");
        assert_eq!(truncate_width("abc", 0), "");
    }
}
