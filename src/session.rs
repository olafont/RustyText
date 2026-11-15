//! The files that were open, so that the app can open them again the next time it starts.
//!
//! They live in `~/.rustytext/session`, a small text file: one file per line, a `*` in front
//! of the one whose tab was showing, and an empty line between two windows.
//!
//! ```text
//! */Users/you/notes.md
//!  /Users/you/todo.txt
//!
//! */Users/you/src/main.rs
//! ```

use std::fs;
use std::path::{Path, PathBuf};

/// The files of one window, in tab order.
#[derive(Debug, Default, PartialEq)]
pub struct Window {
    pub files: Vec<PathBuf>,
    /// Which of `files` was showing.
    pub active: usize,
}

pub fn path() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(".rustytext").join("session"))
}

/// What `file` remembers; nothing if it isn't there or can't be read.
pub fn load(file: &Path) -> Vec<Window> {
    fs::read_to_string(file).map(|text| parse(&text)).unwrap_or_default()
}

pub fn save(file: &Path, text: &str) -> std::io::Result<()> {
    if let Some(folder) = file.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(file, text)
}

pub fn format(windows: &[Window]) -> String {
    let mut text = String::new();
    for window in windows.iter().filter(|window| !window.files.is_empty()) {
        if !text.is_empty() {
            text.push('\n');
        }
        for (index, file) in window.files.iter().enumerate() {
            text.push(if index == window.active { '*' } else { ' ' });
            text.push_str(&file.to_string_lossy());
            text.push('\n');
        }
    }
    text
}

fn parse(text: &str) -> Vec<Window> {
    let mut windows = vec![Window::default()];
    for line in text.lines() {
        let window = windows.last_mut().expect("starts with one");
        if line.is_empty() {
            if !window.files.is_empty() {
                windows.push(Window::default());
            }
            continue;
        }
        let (active, file) = match line.strip_prefix('*') {
            Some(file) => (true, file),
            None => (false, line.strip_prefix(' ').unwrap_or(line)),
        };
        if active {
            window.active = window.files.len();
        }
        window.files.push(PathBuf::from(file));
    }
    windows.retain(|window| !window.files.is_empty());
    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_saved_is_what_is_loaded() {
        let windows = vec![
            Window { files: vec!["/a/one.md".into(), "/a/two words.txt".into(), "/a/*star".into()], active: 1 },
            Window { files: vec!["/b/main.rs".into()], active: 0 },
        ];
        let text = format(&windows);
        assert_eq!(text, " /a/one.md\n*/a/two words.txt\n /a/*star\n\n*/b/main.rs\n");
        assert_eq!(parse(&text), windows);
    }

    #[test]
    fn windows_without_files_are_left_out() {
        let windows = vec![Window::default(), Window { files: vec!["/b/main.rs".into()], active: 0 }];
        assert_eq!(format(&windows), "*/b/main.rs\n");
        assert_eq!(format(&[]), "");
        assert!(parse("").is_empty());
        assert!(parse("\n\n").is_empty());
    }

    #[test]
    fn a_missing_session_file_remembers_nothing() {
        assert!(load(Path::new("/nonexistent/rusty-text/session")).is_empty());
    }
}
