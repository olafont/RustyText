//! One open file: its text, where it lives on disk, and whether it has unsaved edits.

use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::i18n::tr;
use crate::{notes, text_utils};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub struct Document {
    /// Unique per document; keys egui's per-editor state (cursor, undo history).
    pub id: u64,
    pub path: Option<PathBuf>,
    pub text: String,
    pub dirty: bool,
    /// Current selection as char indices (egui counts chars, not bytes).
    pub selection: Range<usize>,
    /// This is your notes file (`notes::path()`), which saves itself.
    pub notes: bool,
    /// When the notes' unsaved changes are due to be saved.
    pub autosave_at: Option<Instant>,
}

impl Document {
    pub fn untitled() -> Self {
        Self::new(None, String::new())
    }

    /// A missing file opens empty and is created on first save.
    pub fn open(path: &Path) -> io::Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut doc = Self::new(Some(path.to_path_buf()), text);
        doc.notes = doc.is_notes_file();
        Ok(doc)
    }

    fn new(path: Option<PathBuf>, text: String) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            path,
            text,
            dirty: false,
            selection: 0..0,
            notes: false,
            autosave_at: None,
        }
    }

    pub fn selected_text(&self) -> &str {
        let start = text_utils::char_to_byte(&self.text, self.selection.start);
        let end = text_utils::char_to_byte(&self.text, self.selection.end);
        &self.text[start..end]
    }

    /// Whether to show the "unsaved changes" dot. Not for the notes, which save themselves.
    pub fn shows_unsaved_mark(&self) -> bool {
        self.dirty && !self.notes
    }

    pub fn title(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| tr("untitled text").to_owned(), |name| name.to_string_lossy().into_owned())
    }

    /// Writes to a temporary file first and renames it, so a failed save never
    /// leaves a half-written file behind.
    pub fn save_to(&mut self, path: PathBuf) -> io::Result<()> {
        // The folder may not exist yet (e.g. `~/.rustytext` before notes are first saved).
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        let mut tmp = path.clone().into_os_string();
        tmp.push(".rusty-text-tmp");
        fs::write(&tmp, &self.text)?;
        fs::rename(&tmp, &path)?;
        self.path = Some(path);
        self.dirty = false;
        self.autosave_at = None;
        self.notes = self.is_notes_file(); // Save As elsewhere makes it an ordinary file
        Ok(())
    }

    fn is_notes_file(&self) -> bool {
        notes::path().is_some_and(|notes| self.is_file(&notes))
    }

    pub fn is_file(&self, other: &Path) -> bool {
        let Some(path) = &self.path else { return false };
        match (fs::canonicalize(path), fs::canonicalize(other)) {
            (Ok(a), Ok(b)) => a == b,
            _ => path == other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_creates_missing_folders() {
        let folder = std::env::temp_dir().join(format!("rusty-text-test-dir-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let path = folder.join("sub").join("notes");

        let mut doc = Document::open(&path).unwrap();
        doc.text = "remember\n".to_owned();
        doc.save_to(path.clone()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "remember\n");

        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn missing_file_opens_empty_then_saves() {
        let path = std::env::temp_dir().join(format!("rusty-text-test-{}.txt", std::process::id()));
        let _ = fs::remove_file(&path);

        let mut doc = Document::open(&path).unwrap();
        assert_eq!(doc.text, "");
        assert_eq!(doc.title(), path.file_name().unwrap().to_string_lossy());

        doc.text = "hello\n".to_owned();
        doc.dirty = true;
        doc.save_to(path.clone()).unwrap();
        assert!(!doc.dirty);
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
        assert!(doc.is_file(&path));

        fs::remove_file(&path).unwrap();
    }
}
