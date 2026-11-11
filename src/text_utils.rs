//! Pure text helpers: line/column math, searching and replacing.
//!
//! egui addresses text by *character* index, while Rust's `str` methods use *byte*
//! offsets (a char can be 1–4 bytes in UTF-8). The public functions here take and
//! return char indices and convert internally.

use std::ops::Range;

/// Byte offset of the `char_index`-th character (or the end of the string).
pub fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(byte, _)| byte)
}

/// Number of characters before `byte_index`.
pub fn byte_to_char(text: &str, byte_index: usize) -> usize {
    text[..byte_index].chars().count()
}

/// Number of lines; an empty text, or a trailing newline, still counts as a line.
pub fn line_count(text: &str) -> usize {
    text.bytes().filter(|&b| b == b'\n').count() + 1
}

/// 1-based (line, column) of the character at `char_index`.
pub fn line_col(text: &str, char_index: usize) -> (usize, usize) {
    let before = &text[..char_to_byte(text, char_index)];
    let line = line_count(before);
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let column = before[line_start..].chars().count() + 1;
    (line, column)
}

/// Char range of 1-based `line`, including its newline. Clamps to the last line.
pub fn line_range(text: &str, line: usize) -> Range<usize> {
    let mut start = 0;
    for _ in 1..line.max(1) {
        match text[start..].find('\n') {
            Some(i) if start + i + 1 < text.len() => start += i + 1,
            _ => break,
        }
    }
    let end = text[start..].find('\n').map_or(text.len(), |i| start + i + 1);
    byte_to_char(text, start)..byte_to_char(text, end)
}

/// Finds `query` starting at char index `from`, wrapping around the end (or start,
/// when searching backwards). Returns the match as a char range.
///
/// Case-insensitive matching only folds ASCII letters, which keeps byte offsets
/// identical between the original and the folded text.
pub fn find(text: &str, query: &str, from: usize, forward: bool, match_case: bool) -> Option<Range<usize>> {
    if query.is_empty() {
        return None;
    }
    let (hay, needle) = if match_case {
        (text.to_owned(), query.to_owned())
    } else {
        (text.to_ascii_lowercase(), query.to_ascii_lowercase())
    };
    let from = char_to_byte(text, from);

    let found = if forward {
        hay[from..].find(&needle).map(|i| from + i).or_else(|| hay.find(&needle))
    } else {
        hay[..from].rfind(&needle).or_else(|| hay.rfind(&needle))
    };
    found.map(|start| byte_to_char(text, start)..byte_to_char(text, start + needle.len()))
}

/// Does the text in char range `range` equal `query`?
pub fn matches_at(text: &str, range: &Range<usize>, query: &str, match_case: bool) -> bool {
    let selected = &text[char_to_byte(text, range.start)..char_to_byte(text, range.end)];
    if match_case { selected == query } else { selected.eq_ignore_ascii_case(query) }
}

/// Replaces every occurrence of `query`; returns the new text and how many were replaced.
pub fn replace_all(text: &str, query: &str, replacement: &str, match_case: bool) -> (String, usize) {
    if query.is_empty() {
        return (text.to_owned(), 0);
    }
    if match_case {
        let count = text.matches(query).count();
        return (text.replace(query, replacement), count);
    }
    let hay = text.to_ascii_lowercase();
    let needle = query.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut count = 0;
    for (start, _) in hay.match_indices(&needle) {
        out.push_str(&text[last..start]);
        out.push_str(replacement);
        last = start + needle.len();
        count += 1;
    }
    out.push_str(&text[last..]);
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_lines() {
        assert_eq!(line_count(""), 1);
        assert_eq!(line_count("a"), 1);
        assert_eq!(line_count("a\n"), 2);
        assert_eq!(line_count("a\nb\nc"), 3);
    }

    #[test]
    fn line_and_column_handle_multibyte_chars() {
        let text = "héllo\nwörld";
        assert_eq!(line_col(text, 0), (1, 1));
        assert_eq!(line_col(text, 2), (1, 3));
        assert_eq!(line_col(text, 6), (2, 1));
        assert_eq!(line_col(text, 11), (2, 6));
    }

    #[test]
    fn line_ranges() {
        let text = "one\ntwo\nthree";
        assert_eq!(line_range(text, 1), 0..4);
        assert_eq!(line_range(text, 2), 4..8);
        assert_eq!(line_range(text, 3), 8..13);
        assert_eq!(line_range(text, 99), 8..13);
    }

    #[test]
    fn find_wraps_around() {
        let text = "cat dog cat";
        assert_eq!(find(text, "cat", 1, true, true), Some(8..11));
        assert_eq!(find(text, "cat", 9, true, true), Some(0..3));
        assert_eq!(find(text, "cat", 8, false, true), Some(0..3));
        assert_eq!(find(text, "CAT", 0, true, true), None);
        assert_eq!(find(text, "CAT", 0, true, false), Some(0..3));
    }

    #[test]
    fn replaces_all_ignoring_case() {
        assert_eq!(replace_all("Cat cat CAT", "cat", "dog", false), ("dog dog dog".to_owned(), 3));
        assert_eq!(replace_all("Cat cat CAT", "cat", "dog", true), ("Cat dog CAT".to_owned(), 1));
    }
}
