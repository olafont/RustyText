//! One open file: its text, where it lives on disk, and whether it has unsaved edits.

use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime};

use crate::i18n::tr;
use crate::{notes, text_utils};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// What a file looks like from the outside: when it was last written, and its size.
/// It changes when another program writes the file.
type DiskStamp = (SystemTime, u64);

fn disk_stamp(path: &Path) -> Option<DiskStamp> {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

/// What `Document::check_disk` found.
#[derive(Debug, PartialEq)]
pub enum DiskChange {
    /// The file is as the document last read or wrote it (or says the same as the document).
    None,
    /// Another program changed the file, and the document now shows the new text.
    Reloaded,
    /// Another program changed the file, and the document has unsaved changes of its own.
    Conflict,
}

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
    /// The file as this document last read or wrote it; `None` if it wasn't there.
    on_disk: Option<DiskStamp>,
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
        doc.on_disk = disk_stamp(path);
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
            on_disk: None,
        }
    }

    /// Looks whether another program changed the file since the document last read or
    /// wrote it. A document without unsaved changes takes the new text. One with unsaved
    /// changes is left alone and reports a conflict, once per change of the file.
    pub fn check_disk(&mut self) -> DiskChange {
        let Some(path) = &self.path else { return DiskChange::None };
        let now = disk_stamp(path);
        if now == self.on_disk {
            return DiskChange::None;
        }
        // Whatever comes next, this state of the file has been seen. A file that was
        // deleted, or can no longer be read as text, leaves the document as it is.
        self.on_disk = now;
        let Some(text) = now.and_then(|_| fs::read_to_string(path).ok()) else { return DiskChange::None };
        if text == self.text {
            self.dirty = false; // the file says what the document says
            DiskChange::None
        } else if self.dirty {
            DiskChange::Conflict
        } else {
            self.set_text_from_disk(text);
            DiskChange::Reloaded
        }
    }

    /// Replaces the text, unsaved changes included, with what the file says now.
    pub fn reload(&mut self) -> io::Result<()> {
        let Some(path) = self.path.clone() else { return Ok(()) };
        let text = fs::read_to_string(&path)?;
        self.on_disk = disk_stamp(&path);
        self.set_text_from_disk(text);
        Ok(())
    }

    fn set_text_from_disk(&mut self, text: String) {
        self.text = text;
        self.dirty = false;
        self.autosave_at = None;
        // The cursor stays where it was, or at the end of a text that got shorter.
        let length = self.text.chars().count();
        self.selection = self.selection.start.min(length)..self.selection.end.min(length);
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
        self.on_disk = disk_stamp(&path);
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
    fn a_file_changed_by_another_program_is_reloaded_or_reported() {
        let path = std::env::temp_dir().join(format!("rusty-text-test-disk-{}.txt", std::process::id()));
        fs::write(&path, "one\n").unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.selection = 4..4;
        assert_eq!(doc.check_disk(), DiskChange::None, "nothing changed");

        // No unsaved changes: the document takes the new text.
        fs::write(&path, "1\n").unwrap();
        assert_eq!(doc.check_disk(), DiskChange::Reloaded);
        assert_eq!(doc.text, "1\n");
        assert_eq!(doc.selection, 2..2, "the cursor stays inside the shorter text");
        assert!(!doc.dirty);
        assert_eq!(doc.check_disk(), DiskChange::None, "only once");

        // Its own save is not a change by another program.
        doc.text = "one and a half\n".to_owned();
        doc.save_to(path.clone()).unwrap();
        assert_eq!(doc.check_disk(), DiskChange::None);

        // Unsaved changes: the document keeps them and reports the conflict, once.
        // (Every text here differs in length from the one before it: a file rewritten
        // within the same tick of the clock is only told apart by its size.)
        doc.text = "mine\n".to_owned();
        doc.dirty = true;
        fs::write(&path, "theirs, and longer\n").unwrap();
        assert_eq!(doc.check_disk(), DiskChange::Conflict);
        assert_eq!(doc.text, "mine\n");
        assert!(doc.dirty);
        assert_eq!(doc.check_disk(), DiskChange::None, "asked once");

        // Reloading on request drops them.
        doc.reload().unwrap();
        assert_eq!(doc.text, "theirs, and longer\n");
        assert!(!doc.dirty);

        // The file now says what the unsaved document says: nothing left to save.
        doc.text = "same in both\n".to_owned();
        doc.dirty = true;
        fs::write(&path, "same in both\n").unwrap();
        assert_eq!(doc.check_disk(), DiskChange::None);
        assert!(!doc.dirty);

        // A deleted file leaves the document as it is.
        fs::remove_file(&path).unwrap();
        assert_eq!(doc.check_disk(), DiskChange::None);
        assert_eq!(doc.text, "same in both\n");
    }

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
