//! The Tools menu: text transformations and word count.
//!
//! Every tool is a pure `&str -> String` function, so it can be unit tested and
//! applied to either the selection or the whole document.

use std::collections::HashSet;

/// Columns per tab stop, for converting between tabs and spaces.
const TAB_WIDTH: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextTool {
    Upper,
    Lower,
    Title,
    Snake,
    Camel,
    SortLines,
    ReverseLines,
    RemoveDuplicateLines,
    RemoveBlankLines,
    TrimTrailingWhitespace,
    TabsToSpaces,
    SpacesToTabs,
    NormalizeLineEndings,
}

impl TextTool {
    pub const CASE: &[TextTool] = &[Self::Upper, Self::Lower, Self::Title, Self::Snake, Self::Camel];
    pub const LINES: &[TextTool] = &[
        Self::SortLines,
        Self::ReverseLines,
        Self::RemoveDuplicateLines,
        Self::RemoveBlankLines,
    ];
    pub const WHITESPACE: &[TextTool] = &[
        Self::TrimTrailingWhitespace,
        Self::TabsToSpaces,
        Self::SpacesToTabs,
        Self::NormalizeLineEndings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Upper => "UPPERCASE",
            Self::Lower => "lowercase",
            Self::Title => "Title Case",
            Self::Snake => "snake_case",
            Self::Camel => "camelCase",
            Self::SortLines => "Sort Lines",
            Self::ReverseLines => "Reverse Lines",
            Self::RemoveDuplicateLines => "Remove Duplicate Lines",
            Self::RemoveBlankLines => "Remove Blank Lines",
            Self::TrimTrailingWhitespace => "Trim Trailing Whitespace",
            Self::TabsToSpaces => "Tabs to Spaces",
            Self::SpacesToTabs => "Leading Spaces to Tabs",
            Self::NormalizeLineEndings => "Normalize Line Endings (LF)",
        }
    }

    pub fn apply(self, text: &str) -> String {
        match self {
            Self::Upper => text.to_uppercase(),
            Self::Lower => text.to_lowercase(),
            Self::Title => title_case(text),
            Self::Snake => map_lines(text, |line| words(line).join("_").to_lowercase()),
            Self::Camel => map_lines(text, camel_case),
            Self::SortLines => with_lines(text, |lines| {
                lines.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));
            }),
            Self::ReverseLines => with_lines(text, |lines| lines.reverse()),
            Self::RemoveDuplicateLines => with_lines(text, |lines| {
                let mut seen = HashSet::new();
                lines.retain(|line| seen.insert(line.to_owned()));
            }),
            Self::RemoveBlankLines => with_lines(text, |lines| lines.retain(|line| !line.trim().is_empty())),
            Self::TrimTrailingWhitespace => map_lines(text, |line| line.trim_end_matches([' ', '\t']).to_owned()),
            Self::TabsToSpaces => map_lines(text, expand_tabs),
            Self::SpacesToTabs => map_lines(text, leading_spaces_to_tabs),
            Self::NormalizeLineEndings => text.replace("\r\n", "\n").replace('\r', "\n"),
        }
    }
}

/// Applies `f` to each line, keeping the newlines exactly as they were.
fn map_lines(text: &str, f: impl Fn(&str) -> String) -> String {
    text.split('\n').map(f).collect::<Vec<_>>().join("\n")
}

/// Lets `f` rearrange the lines. A trailing newline stays at the end
/// instead of becoming an empty line that gets sorted or removed.
fn with_lines(text: &str, f: impl FnOnce(&mut Vec<&str>)) -> String {
    let (body, trailing_newline) = match text.strip_suffix('\n') {
        Some(body) => (body, true),
        None => (text, false),
    };
    let mut lines: Vec<&str> = body.split('\n').collect();
    f(&mut lines);
    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    out
}

/// Capitalizes the first letter of each word, lowercases the rest.
fn title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_word = false;
    for c in text.chars() {
        if in_word {
            out.extend(c.to_lowercase());
        } else {
            out.extend(c.to_uppercase());
        }
        in_word = c.is_alphanumeric() || c == '\'' || c == '’';
    }
    out
}

/// Splits an identifier or phrase into words: at non-alphanumerics and at
/// lower→upper case changes ("helloWorld foo-bar" → hello, World, foo, bar).
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;
    for c in line.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower {
            words.push(std::mem::take(&mut current));
        }
        current.push(c);
        prev_lower = c.is_lowercase() || c.is_numeric();
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn camel_case(line: &str) -> String {
    words(line)
        .iter()
        .enumerate()
        .map(|(i, word)| {
            let lower = word.to_lowercase();
            if i == 0 {
                return lower;
            }
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect()
}

/// Replaces each tab with spaces up to the next tab stop.
fn expand_tabs(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut column = 0;
    for c in line.chars() {
        if c == '\t' {
            let spaces = TAB_WIDTH - column % TAB_WIDTH;
            out.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            out.push(c);
            column += 1;
        }
    }
    out
}

/// Turns each full group of `TAB_WIDTH` leading spaces into a tab.
fn leading_spaces_to_tabs(line: &str) -> String {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let tabs = indent / TAB_WIDTH;
    let spaces = indent % TAB_WIDTH;
    format!("{}{}{}", "\t".repeat(tabs), " ".repeat(spaces), &line[indent..])
}

pub struct Stats {
    pub words: usize,
    pub characters: usize,
    pub characters_no_spaces: usize,
    pub lines: usize,
    pub paragraphs: usize,
}

pub fn stats(text: &str) -> Stats {
    // A paragraph is a run of non-blank lines.
    let mut paragraphs = 0;
    let mut in_paragraph = false;
    for line in text.lines() {
        let blank = line.trim().is_empty();
        if !blank && !in_paragraph {
            paragraphs += 1;
        }
        in_paragraph = !blank;
    }
    Stats {
        words: text.split_whitespace().count(),
        characters: text.chars().count(),
        characters_no_spaces: text.chars().filter(|c| !c.is_whitespace()).count(),
        lines: crate::text_utils::line_count(text),
        paragraphs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_case() {
        assert_eq!(TextTool::Upper.apply("héllo"), "HÉLLO");
        assert_eq!(TextTool::Title.apply("the QUICK brown fox's tail"), "The Quick Brown Fox's Tail");
        assert_eq!(TextTool::Snake.apply("Hello World\nhelloWorld foo-bar"), "hello_world\nhello_world_foo_bar");
        assert_eq!(TextTool::Camel.apply("hello world\nHTTP request_id"), "helloWorld\nhttpRequestId");
    }

    #[test]
    fn line_tools_keep_trailing_newline() {
        assert_eq!(TextTool::SortLines.apply("pear\nApple\nbanana\n"), "Apple\nbanana\npear\n");
        assert_eq!(TextTool::ReverseLines.apply("1\n2\n3"), "3\n2\n1");
        assert_eq!(TextTool::RemoveDuplicateLines.apply("a\nb\na\nc\nb\n"), "a\nb\nc\n");
        assert_eq!(TextTool::RemoveBlankLines.apply("a\n\n  \nb\n"), "a\nb\n");
    }

    #[test]
    fn whitespace_tools() {
        assert_eq!(TextTool::TrimTrailingWhitespace.apply("a  \nb\t\n"), "a\nb\n");
        assert_eq!(TextTool::TabsToSpaces.apply("\tx\nab\ty"), "    x\nab  y");
        assert_eq!(TextTool::SpacesToTabs.apply("      x\n    y"), "\t  x\n\ty");
        assert_eq!(TextTool::NormalizeLineEndings.apply("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn counts_words_and_paragraphs() {
        let s = stats("Hello world.\nSecond line.\n\nNew paragraph here.\n");
        assert_eq!(s.words, 7);
        assert_eq!(s.paragraphs, 2);
        assert_eq!(s.lines, 5);
        assert_eq!(s.characters_no_spaces, 39); // 11 + 11 + 17
    }
}
