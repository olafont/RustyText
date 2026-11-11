//! Your notes: `~/.rustytext/notes`, a running log opened with Tools ▸ Forever Note.
//!
//! - The first time the notes are opened on a given day, a date line is added
//!   at the end, so each day's notes start under their own heading.
//! - The cursor goes to the end, ready to type.
//! - Changes are saved automatically (see `EditorWindow::autosave_notes`).

use std::path::PathBuf;

use chrono::NaiveDate;

pub fn path() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(".rustytext").join("notes"))
}

/// The line that starts a day's notes, e.g. `--- Saturday, October 24, 2026 ---`.
pub fn date_line(date: NaiveDate) -> String {
    format!("--- {} ---", date.format("%A, %B %-d, %Y"))
}

/// Adds today's date line at the end of `text`, unless it's already there.
/// Returns whether it changed anything.
pub fn add_date_line(text: &mut String, today: NaiveDate) -> bool {
    let line = date_line(today);
    if text.lines().any(|existing| existing == line) {
        return false;
    }
    // A blank line between the previous day's notes and the new date.
    if !text.is_empty() {
        while !text.ends_with("\n\n") {
            text.push('\n');
        }
    }
    text.push_str(&line);
    text.push('\n');
    true
}

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    #[test]
    fn formats_the_date_line() {
        assert_eq!(date_line(day(24)), "--- Saturday, October 24, 2026 ---");
        assert_eq!(date_line(day(5)), "--- Monday, October 5, 2026 ---");
    }

    #[test]
    fn adds_the_date_once_a_day() {
        let mut text = String::new();
        assert!(add_date_line(&mut text, day(23)));
        assert_eq!(text, "--- Friday, October 23, 2026 ---\n");

        text.push_str("bought milk");
        assert!(!add_date_line(&mut text, day(23)), "same day: no second date line");

        assert!(add_date_line(&mut text, day(24)));
        assert_eq!(
            text,
            "--- Friday, October 23, 2026 ---\nbought milk\n\n--- Saturday, October 24, 2026 ---\n"
        );
    }
}
