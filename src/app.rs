//! The editor windows: the title bar with its menus, tabs, the text area with its line-number gutter,
//! the find bar, the status bar and the dialogs.
//!
//! `EditorApp` owns a list of `EditorWindow`s. The first one is eframe's main
//! ("root") window; the others are extra native windows ("viewports" in egui).
//!
//! egui is an *immediate mode* GUI: `ui()` runs every frame and redraws the whole
//! window from the app's state. There are no widget objects to keep around; a
//! button "exists" because this frame's code drew it, and `.clicked()` reports
//! whether the user clicked it.

use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;

use crate::document::{DiskChange, Document};
use crate::tools::{self, TextTool};
use crate::terminal::Terminal;
use crate::i18n::{tr, tr_fill};
use crate::settings::{self, Language, Theme};
use crate::{glow, look, mcp, notes, session, text_utils, title_bar};

mod agent_tools;
mod chrome;

#[cfg(test)]
pub use chrome::texts as ribbon_texts;

/// Everything the user can ask for, from a menu or a keyboard shortcut.
/// Both paths produce an `Action`, and `perform()` is the one place that handles them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    New,
    Open,
    /// Handled by `EditorApp`, like `Quit`, since it affects more than one window.
    NewWindow,
    Save,
    SaveAs,
    CloseTab,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    FindReplace,
    FindNext,
    FindPrevious,
    UseSelectionForFind,
    GoToLine,
    ToggleSoftWrap,
    /// Insert text at the cursor, replacing the selection (used by the Emoji menu).
    Insert(&'static str),
    /// Transform the selection, or the whole document if nothing is selected.
    Tool(TextTool),
    WordCount,
    /// Show or hide the Glow-style Markdown preview.
    ToggleGlow,
    /// Show or hide the terminal pane at the bottom of the window.
    ToggleTerminal,
    /// Reopen your notes (after their tab was closed) as the leftmost tab.
    OpenNotes,
    Preferences,
    /// Put the keyboard in the ribbon's command search.
    CommandSearch,
    /// Open the user manual.
    Help,
}

/// How long after the notes change they are saved.
/// Behind every occurrence of what the find bar searches for.
const MATCH_HIGHLIGHT: egui::Color32 = egui::Color32::from_rgba_premultiplied(120, 95, 0, 110);
/// Behind the occurrence that is selected: the one Find Next stopped at.
const CURRENT_MATCH_HIGHLIGHT: egui::Color32 = egui::Color32::from_rgba_premultiplied(225, 120, 0, 200);
const AUTOSAVE_DELAY: Duration = Duration::from_secs(1);
/// How often the open files are checked for changes made by other programs.
const DISK_CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// How many lines tall the terminal pane is when it first opens.
const TERMINAL_START_ROWS: u16 = 5;

const fn cmd(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}

const fn cmd_shift(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers { shift: true, ..Modifiers::COMMAND }, key)
}

const fn cmd_alt(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers { alt: true, ..Modifiers::COMMAND }, key)
}

/// Most specific first: egui lets extra Shift/Alt match a shortcut that doesn't mention them.
const SHORTCUTS: &[(KeyboardShortcut, Action)] = &[
    (cmd_shift(Key::N), Action::NewWindow),
    (cmd_shift(Key::S), Action::SaveAs),
    (cmd_shift(Key::G), Action::FindPrevious),
    (cmd_shift(Key::P), Action::CommandSearch),
    (cmd_alt(Key::F), Action::FindReplace),
    (cmd(Key::N), Action::New),
    (cmd(Key::O), Action::Open),
    (cmd(Key::S), Action::Save),
    (cmd(Key::W), Action::CloseTab),
    (cmd(Key::Q), Action::Quit),
    (cmd(Key::F), Action::Find),
    (cmd(Key::G), Action::FindNext),
    (cmd(Key::E), Action::UseSelectionForFind),
    (cmd(Key::J), Action::GoToLine),
    (cmd(Key::Comma), Action::Preferences),
    (KeyboardShortcut::new(Modifiers::CTRL, Key::Backtick), Action::ToggleTerminal),
];

/// Keys egui's text editor already handles itself. Listed only to label the Edit menu;
/// consuming them in `SHORTCUTS` would take them away from the editor.
const TEXT_EDIT_SHORTCUTS: &[(KeyboardShortcut, Action)] = &[
    (cmd(Key::Z), Action::Undo),
    (cmd_shift(Key::Z), Action::Redo),
    (cmd(Key::X), Action::Cut),
    (cmd(Key::C), Action::Copy),
    (cmd(Key::V), Action::Paste),
    (cmd(Key::A), Action::SelectAll),
];

fn shortcut_for(action: Action) -> Option<KeyboardShortcut> {
    SHORTCUTS
        .iter()
        .chain(TEXT_EDIT_SHORTCUTS)
        .find(|(_, a)| *a == action)
        .map(|(shortcut, _)| *shortcut)
}

enum Dialog {
    GoToLine { input: String },
    ConfirmClose { tab: usize },
    /// Another program changed the file of a document that has unsaved changes.
    ChangedOnDisk { doc: u64, title: String },
    Error(String),
    WordCount { of_selection: bool, stats: tools::Stats },
    /// Theme, language and default folder. Changes apply as they are made.
    Preferences,
}

/// What the user picked in a dialog, applied after the dialog is drawn.
enum Choice {
    GoTo(usize),
    Save(usize),
    Discard(usize),
    /// Take the file's new text, by document id.
    Reload(u64),
    Cancel,
}

#[derive(Default)]
struct FindBar {
    visible: bool,
    show_replace: bool,
    query: String,
    replacement: String,
    match_case: bool,
    focus_query: bool,
}

/// Everything one window shows: its tabs, find bar, dialogs and view options.
struct EditorWindow {
    /// Unique per window; keys its native window and its panels' egui state.
    id: u64,
    docs: Vec<Document>,
    active: usize,
    find: FindBar,
    dialog: Option<Dialog>,
    soft_wrap: bool,
    /// The user turned on Glow; it only shows while a Markdown tab is active.
    glow: bool,
    /// The shell keeps running while the pane is hidden, until the window closes.
    terminal: Option<Terminal>,
    terminal_visible: bool,
    /// Closing tabs one by one (asking about unsaved ones); once none are left,
    /// `EditorApp` removes the window (or exits, for the main window).
    closing: bool,
    /// Scroll the editor to the selection on the next frame (after find / go to line).
    scroll_to_selection: bool,
    /// Select this in the active tab on the next frame (selecting needs the egui context,
    /// which a window that is still being created doesn't have yet).
    pending_select: Option<Range<usize>>,
    /// Your notes are open in some window (set by `EditorApp` every frame);
    /// View ▸ Forever Note can only be used when they aren't.
    notes_open: bool,
    /// Why agents can't connect although Preferences allow it (set by `EditorApp` every frame).
    agent_server_error: Option<String>,
    window_title: String,
    /// When to look next for files changed by other programs.
    next_disk_check: Instant,
    /// The ribbon's tab and command search (see `chrome.rs`).
    ribbon: chrome::Ribbon,
    /// The app's icon, drawn in the window bar.
    logo: Option<egui::TextureHandle>,
    /// The tab the window bar last scrolled to, to follow the active one when it changes.
    bar_tab_shown: usize,
}

impl EditorWindow {
    fn new(id: u64, paths: Vec<PathBuf>) -> Self {
        let mut window = Self {
            id,
            docs: Vec::new(),
            active: 0,
            find: FindBar::default(),
            dialog: None,
            soft_wrap: false,
            glow: false,
            terminal: None,
            terminal_visible: false,
            closing: false,
            scroll_to_selection: false,
            pending_select: None,
            notes_open: false,
            agent_server_error: None,
            window_title: String::new(),
            next_disk_check: Instant::now(),
            ribbon: chrome::Ribbon::default(),
            logo: None,
            bar_tab_shown: 0,
        };
        for path in paths {
            window.open_path(path);
        }
        if window.docs.is_empty() {
            window.docs.push(Document::untitled());
            window.pending_select = Some(0..0);
        }
        window
    }

    /// Opens the files this window showed the last time, those that are still there.
    fn restore(&mut self, remembered: session::Window) {
        let active = remembered.files.get(remembered.active).cloned();
        for file in remembered.files.iter().filter(|file| file.is_file()) {
            self.open_path(file.clone());
        }
        if let Some(active) = active.filter(|file| file.is_file()) {
            self.open_path(active); // already open: shows its tab
        }
        self.dialog = None; // a file that can't be read is passed over too, without a word
    }

    /// The files to open again the next time: the documents that have one.
    fn session(&self) -> session::Window {
        let has_file = |doc: &&Document| doc.path.is_some();
        let files: Vec<PathBuf> = self
            .docs
            .iter()
            .filter_map(|doc| doc.path.as_deref())
            .map(|file| std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf()))
            .collect();
        let active = match self.docs.get(self.active) {
            Some(doc) if doc.path.is_some() => self.docs[..self.active].iter().filter(has_file).count(),
            _ => files.len(), // an untitled document is showing: no file is marked
        };
        session::Window { files, active }
    }

    /// An egui id that is unique to this window (panels, dialogs).
    fn ui_id(&self, name: &str) -> egui::Id {
        egui::Id::unique((name, self.id))
    }

    fn doc(&mut self) -> &mut Document {
        &mut self.docs[self.active]
    }

    fn is_markdown(&self) -> bool {
        self.docs.get(self.active).is_some_and(|doc| glow::is_markdown(doc.path.as_deref()))
    }

    fn glow_visible(&self) -> bool {
        self.glow && self.is_markdown()
    }

    // MARK: - Actions

    fn perform(&mut self, action: Action, ctx: &egui::Context) {
        if self.docs.is_empty() {
            return; // closing
        }
        match action {
            Action::NewWindow | Action::Quit => {} // handled by EditorApp
            Action::New => {
                self.docs.push(Document::untitled());
                self.active = self.docs.len() - 1;
                self.pending_select = Some(0..0); // ready to type
            }
            Action::Open => {
                for path in file_dialog().pick_files().unwrap_or_default() {
                    self.open_path(path);
                }
            }
            Action::Save => {
                self.save(self.active, false);
            }
            Action::SaveAs => {
                self.save(self.active, true);
            }
            Action::CloseTab => self.request_close(self.active),
            Action::Undo => self.undo_redo(ctx, false),
            Action::Redo => self.undo_redo(ctx, true),
            Action::Cut => {
                let selected = self.doc().selected_text().to_owned();
                if !selected.is_empty() {
                    ctx.copy_text(selected);
                    self.insert_text(ctx, "");
                }
            }
            Action::Copy => {
                let selected = self.doc().selected_text().to_owned();
                if !selected.is_empty() {
                    ctx.copy_text(selected);
                }
            }
            Action::Paste => {
                // egui only receives pasted text on ⌘V, so read the clipboard directly.
                match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                    Ok(text) => self.insert_text(ctx, &text),
                    Err(e) => self.dialog = Some(Dialog::Error(tr_fill("Couldn’t read the clipboard: {}", &[&e.to_string()]))),
                }
            }
            Action::SelectAll => {
                let len = self.doc().text.chars().count();
                self.select(ctx, 0..len);
            }
            Action::Find | Action::FindReplace => {
                self.find.visible = true;
                self.find.show_replace = action == Action::FindReplace;
                self.find.focus_query = true;
            }
            Action::FindNext => self.find_next(ctx, true),
            Action::FindPrevious => self.find_next(ctx, false),
            Action::UseSelectionForFind => self.find.query = self.docs[self.active].selected_text().to_owned(),
            Action::GoToLine => self.dialog = Some(Dialog::GoToLine { input: String::new() }),
            Action::ToggleSoftWrap => self.soft_wrap = !self.soft_wrap,
            Action::Insert(text) => self.insert_text(ctx, text),
            Action::Tool(tool) => self.apply_tool(ctx, tool),
            Action::ToggleGlow => self.glow = !self.glow,
            Action::ToggleTerminal => self.toggle_terminal(ctx),
            Action::OpenNotes => self.open_notes(),
            Action::Preferences => self.dialog = Some(Dialog::Preferences),
            Action::CommandSearch => self.ribbon.focus_search = true,
            Action::Help => ctx.open_url(egui::OpenUrl::new_tab(chrome::MANUAL_URL)),
            Action::WordCount => {
                let doc = &self.docs[self.active];
                let of_selection = !doc.selection.is_empty();
                let text = if of_selection { doc.selected_text() } else { &doc.text };
                self.dialog = Some(Dialog::WordCount { of_selection, stats: tools::stats(text) });
            }
        }
    }

    /// Shows the terminal pane (starting a shell the first time, in the current
    /// file's folder) or hides it.
    fn toggle_terminal(&mut self, ctx: &egui::Context) {
        if self.terminal_visible {
            self.terminal_visible = false;
            return;
        }
        if self.terminal.is_none() {
            let cwd = self.docs[self.active]
                .path
                .as_deref()
                .and_then(std::path::Path::parent)
                .map(std::path::Path::to_path_buf)
                .or_else(settings::default_folder)
                .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
                .unwrap_or_else(|| PathBuf::from("/"));
            match Terminal::spawn(&cwd, ctx.clone()) {
                Ok(terminal) => self.terminal = Some(terminal),
                Err(e) => {
                    self.dialog = Some(Dialog::Error(e));
                    return;
                }
            }
        }
        self.terminal_visible = true;
        ctx.memory_mut(|memory| memory.request_focus(self.ui_id("terminal_view")));
    }

    /// Runs `tool` on the selection (and reselects the result), or on the whole
    /// document when nothing is selected (keeping the cursor where it was).
    fn apply_tool(&mut self, ctx: &egui::Context, tool: TextTool) {
        let doc = self.doc();
        let had_selection = !doc.selection.is_empty();
        let range = if had_selection { doc.selection.clone() } else { 0..doc.text.chars().count() };
        let start = text_utils::char_to_byte(&doc.text, range.start);
        let end = text_utils::char_to_byte(&doc.text, range.end);

        let result = tool.apply(&doc.text[start..end]);
        if result == doc.text[start..end] {
            return;
        }
        let result_len = result.chars().count();
        doc.text.replace_range(start..end, &result);
        doc.dirty = true;

        let new_selection = if had_selection {
            range.start..range.start + result_len
        } else {
            let cursor = doc.selection.start.min(result_len);
            cursor..cursor
        };
        self.select(ctx, new_selection);
    }

    /// Steps through the editor's own undo history (the same one ⌘Z uses).
    fn undo_redo(&mut self, ctx: &egui::Context, redo: bool) {
        let doc = self.doc();
        let id = editor_id(doc);
        let Some(mut state) = TextEditState::load(ctx, id) else { return };
        let mut undoer = state.undoer();
        let cursor = state.cursor.char_range().unwrap_or_else(|| {
            CCursorRange::two(CCursor::new(doc.selection.start), CCursor::new(doc.selection.end))
        });
        let current = (cursor, doc.text.clone());
        let restored = if redo { undoer.redo(&current) } else { undoer.undo(&current) }.cloned();
        let Some((cursor, text)) = restored else { return };

        doc.text = text;
        doc.dirty = true;
        let range = cursor.as_sorted_char_range();
        doc.selection = range.start.0..range.end.0;
        state.cursor.set_char_range(Some(cursor));
        state.set_undoer(undoer);
        state.store(ctx, id);
        ctx.memory_mut(|memory| memory.request_focus(id));
    }

    fn insert_text(&mut self, ctx: &egui::Context, text: &str) {
        let doc = self.doc();
        let start = text_utils::char_to_byte(&doc.text, doc.selection.start);
        let end = text_utils::char_to_byte(&doc.text, doc.selection.end);
        doc.text.replace_range(start..end, text);
        doc.dirty = true;
        let cursor = doc.selection.start + text.chars().count();
        self.select(ctx, cursor..cursor);
    }

    fn open_path(&mut self, path: PathBuf) {
        if let Some(index) = self.docs.iter().position(|doc| doc.is_file(&path)) {
            self.active = index;
        } else {
            match Document::open(&path) {
                Ok(doc) => {
                    // Replace a lone, untouched untitled tab instead of piling up empty ones.
                    if self.docs.len() == 1 && self.docs[0].path.is_none() && !self.docs[0].dirty && self.docs[0].text.is_empty() {
                        self.docs.clear();
                    }
                    self.docs.push(doc);
                    self.active = self.docs.len() - 1;
                    // A Markdown file opens with its rendered pane beside it.
                    if self.is_markdown() {
                        self.glow = true;
                    }
                }
                Err(e) => {
                    self.dialog = Some(Dialog::Error(tr_fill("Couldn’t open {}: {}", &[&path.display().to_string(), &e.to_string()])));
                    return;
                }
            }
        }
        if self.docs[self.active].notes {
            self.start_notes_entry();
        } else {
            // Put the cursor in the opened document, ready to type.
            self.pending_select = Some(self.docs[self.active].selection.clone());
        }
    }

    /// Opens your notes as the first tab.
    fn open_notes(&mut self) {
        let Some(path) = notes::path() else { return };
        if self.docs.iter().any(|doc| doc.notes) {
            self.open_path(path); // already here: just switch to it
            return;
        }
        match Document::open(&path) {
            Ok(doc) => {
                // As in `open_path`: take the place of a lone, untouched untitled tab.
                if self.docs.len() == 1 && self.docs[0].path.is_none() && !self.docs[0].dirty && self.docs[0].text.is_empty() {
                    self.docs.clear();
                }
                self.docs.insert(0, doc);
                self.active = 0;
                self.start_notes_entry();
            }
            Err(e) => self.dialog = Some(Dialog::Error(tr_fill("Couldn’t open your notes: {}", &[&e.to_string()]))),
        }
    }

    /// Opening the notes: add today's date if this is the first time today,
    /// and put the cursor at the end, ready to write.
    fn start_notes_entry(&mut self) {
        let doc = self.doc();
        if notes::add_date_line(&mut doc.text, notes::today()) {
            doc.dirty = true; // autosave writes the new date line
        }
        let end = doc.text.chars().count();
        self.pending_select = Some(end..end);
    }

    /// Saves the notes shortly after they change, so they never need ⌘S.
    fn autosave_notes(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        for index in 0..self.docs.len() {
            let doc = &mut self.docs[index];
            if !doc.notes || !doc.dirty {
                doc.autosave_at = None;
                continue;
            }
            let due = *doc.autosave_at.get_or_insert(now + AUTOSAVE_DELAY);
            if now < due {
                ctx.request_repaint_after(due - now); // wake up to save even if idle
            } else if !self.save(index, false) {
                // Don't retry every frame; the next edit tries again.
                self.docs[index].autosave_at = Some(Instant::now() + Duration::from_secs(3600));
            }
        }
    }

    /// Shows what other programs wrote to the open files: looks at them once a second.
    fn follow_files_on_disk(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if now < self.next_disk_check {
            ctx.request_repaint_after(self.next_disk_check - now);
            return;
        }
        self.next_disk_check = now + DISK_CHECK_INTERVAL;
        ctx.request_repaint_after(DISK_CHECK_INTERVAL); // wake up to look even if idle
        self.check_files_on_disk(ctx);
    }

    fn check_files_on_disk(&mut self, ctx: &egui::Context) {
        for index in 0..self.docs.len() {
            // One question at a time; the files not looked at now are looked at once it is answered.
            if self.dialog.is_some() || self.closing {
                break;
            }
            let doc = &mut self.docs[index];
            match doc.check_disk() {
                DiskChange::None => {}
                DiskChange::Reloaded => agent_tools::store_cursor(ctx, doc),
                DiskChange::Conflict => {
                    self.dialog = Some(Dialog::ChangedOnDisk { doc: doc.id, title: doc.title() });
                    self.active = index;
                }
            }
        }
    }

    /// Returns false if the user cancelled the save panel or the write failed.
    fn save(&mut self, index: usize, force_panel: bool) -> bool {
        let doc = &mut self.docs[index];
        let path = match (&doc.path, force_panel) {
            (Some(path), false) => path.clone(),
            // An unsaved document's panel starts in the default folder.
            _ => match file_dialog().set_file_name(doc.title()).save_file() {
                Some(path) => path,
                None => return false,
            },
        };
        let title = doc.title();
        match doc.save_to(path) {
            Ok(()) => true,
            Err(e) => {
                self.dialog = Some(Dialog::Error(tr_fill("Couldn’t save {}: {}", &[&title, &e.to_string()])));
                false
            }
        }
    }

    fn request_close(&mut self, tab: usize) {
        // The notes save themselves instead of asking.
        if self.docs[tab].notes && self.docs[tab].dirty && !self.save(tab, false) {
            self.closing = false; // the save failed; the error dialog says why
            return;
        }
        if self.docs[tab].dirty {
            self.active = tab;
            self.dialog = Some(Dialog::ConfirmClose { tab });
        } else {
            self.remove_tab(tab);
        }
    }

    fn remove_tab(&mut self, tab: usize) {
        self.docs.remove(tab);
        if self.docs.is_empty() && !self.closing {
            self.docs.push(Document::untitled());
        }
        if tab < self.active || self.active >= self.docs.len() {
            self.active = self.active.saturating_sub(1);
        }
    }

    /// While closing, close clean tabs and stop at the first dirty one to ask about it.
    fn continue_closing(&mut self) {
        if !self.closing || self.dialog.is_some() {
            return;
        }
        while let Some(last) = self.docs.last() {
            if last.dirty {
                self.request_close(self.docs.len() - 1);
                return;
            }
            self.docs.pop();
        }
    }

    // MARK: - Find & select

    fn find_next(&mut self, ctx: &egui::Context, forward: bool) {
        let doc = &self.docs[self.active];
        let from = if forward { doc.selection.end } else { doc.selection.start };
        match text_utils::find(&doc.text, &self.find.query, from, forward, self.find.match_case) {
            Some(range) => self.select(ctx, range),
            None => ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            )),
        }
    }

    /// Replaces the selection if it matches the query, then finds the next match.
    fn replace(&mut self, ctx: &egui::Context) {
        let (query, replacement, match_case) = (&self.find.query, &self.find.replacement, self.find.match_case);
        let doc = &mut self.docs[self.active];
        if text_utils::matches_at(&doc.text, &doc.selection, query, match_case) {
            let start = text_utils::char_to_byte(&doc.text, doc.selection.start);
            let end = text_utils::char_to_byte(&doc.text, doc.selection.end);
            doc.text.replace_range(start..end, replacement);
            doc.dirty = true;
            let after = doc.selection.start + replacement.chars().count();
            doc.selection = after..after;
        }
        self.find_next(ctx, true);
    }

    fn replace_all(&mut self) {
        let doc = &mut self.docs[self.active];
        let (text, count) = text_utils::replace_all(&doc.text, &self.find.query, &self.find.replacement, self.find.match_case);
        if count > 0 {
            doc.text = text;
            doc.dirty = true;
            doc.selection = 0..0;
        }
    }

    /// Selects `range` in the editor and gives it keyboard focus (egui only
    /// paints the selection of a focused text edit).
    fn select(&mut self, ctx: &egui::Context, range: Range<usize>) {
        let doc = self.doc();
        let id = editor_id(doc);
        let mut state = TextEditState::load(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(range.start), CCursor::new(range.end))));
        state.store(ctx, id);
        doc.selection = range;
        ctx.memory_mut(|memory| memory.request_focus(id));
        self.scroll_to_selection = true;
    }

    // MARK: - Window events

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter()
                .map(|f| f.path().to_path_buf())
                .filter(|path| !path.as_os_str().is_empty())
                .collect()
        });
        for path in dropped {
            self.open_path(path);
        }
    }

    fn update_window_title(&mut self, ctx: &egui::Context) {
        let Some(doc) = self.docs.get(self.active) else { return };
        let title = format!("{}{} — RustyText", if doc.shows_unsaved_mark() { "• " } else { "" }, doc.title());
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    // MARK: - Drawing

    fn find_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            ui.label(tr("Find:"));
            let field = ui.add(egui::TextEdit::singleline(&mut self.find.query).desired_width(220.0));
            if std::mem::take(&mut self.find.focus_query) {
                field.request_focus();
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if ui.button("◀").on_hover_text(tr("Previous")).clicked() {
                self.find_next(&ctx, false);
            }
            if ui.button("▶").on_hover_text(tr("Next")).clicked() || enter {
                self.find_next(&ctx, true);
            }
            ui.checkbox(&mut self.find.match_case, tr("Match case"));
            ui.toggle_value(&mut self.find.show_replace, tr("Replace…"));
            // Escape belongs to the shell while the terminal has the keyboard.
            let terminal_focused = ui.memory(|memory| memory.has_focus(self.ui_id("terminal_view")));
            let escape = !terminal_focused && ui.input(|i| i.key_pressed(Key::Escape));
            if ui.button(tr("Done")).clicked() || escape {
                self.find.visible = false;
            }
        });
        if self.find.show_replace {
            ui.horizontal(|ui| {
                ui.label(tr("With:"));
                ui.add(egui::TextEdit::singleline(&mut self.find.replacement).desired_width(220.0));
                if ui.button(tr("Replace")).clicked() {
                    self.replace(&ctx);
                }
                if ui.button(tr("Replace All")).clicked() {
                    self.replace_all();
                }
            });
        }
    }

    /// The text area, with a line-number gutter on its left.
    fn editor(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let soft_wrap = self.soft_wrap;
        let scroll_to_selection = std::mem::take(&mut self.scroll_to_selection);
        // While the find bar is open, every occurrence of its text is highlighted.
        let searched = (self.find.visible && !self.find.query.is_empty()).then(|| (self.find.query.clone(), self.find.match_case));
        let doc = self.doc();
        let id = editor_id(doc);
        let font = egui::FontId::monospace(14.0);

        // The gutter sits outside the scroll area so it stays put when scrolling sideways.
        let digit_width = ui.painter().layout_no_wrap("0".into(), font.clone(), egui::Color32::WHITE).size().x;
        let digits = text_utils::line_count(&doc.text).to_string().len().max(2);
        let full = ui.available_rect_before_wrap();
        let (gutter, text_area) = full.split_left_right_at_x(full.left() + digits as f32 * digit_width + 14.0);

        // egui moves the cursor on any mouse button; a right-click should keep the
        // selection (if there is one) for the context menu to act on.
        let right_clicking = ui.input(|i| {
            let button = egui::PointerButton::Secondary;
            i.pointer.button_pressed(button) || i.pointer.button_down(button) || i.pointer.button_released(button)
        });
        let kept_selection = (right_clicking && !doc.selection.is_empty()).then(|| doc.selection.clone());

        let scroll_area = if soft_wrap { egui::ScrollArea::vertical() } else { egui::ScrollArea::both() };
        let output = ui
            .scope_builder(egui::UiBuilder::new().max_rect(text_area), |ui| {
                scroll_area.id_salt(id).auto_shrink(false).show(ui, |ui| {
                    let wrap_width = if soft_wrap { ui.available_width() } else { f32::INFINITY };
                    // A place under the text for the highlights, which need the laid out text.
                    let highlights = ui.painter().add(egui::Shape::Noop);
                    let output = egui::TextEdit::multiline(&mut doc.text)
                        .id(id)
                        .code_editor()
                        .font(font.clone())
                        .frame(egui::Frame::NONE)
                        .desired_width(wrap_width)
                        .min_size(egui::vec2(0.0, text_area.height()))
                        .show(ui);
                    if let Some((query, match_case)) = &searched {
                        let shapes = match_highlights(ui, &output, &doc.text, query, *match_case, &doc.selection);
                        ui.painter().set(highlights, shapes);
                    }
                    if scroll_to_selection {
                        let cursor = output.galley.pos_from_cursor(CCursor::new(doc.selection.start));
                        ui.scroll_to_rect(cursor.translate(output.galley_pos.to_vec2()), Some(egui::Align::Center));
                    }
                    output
                })
            })
            .inner
            .inner;

        if output.response.response.changed() {
            doc.dirty = true;
        }
        // Right-click menu. The editor keeps its selection while the menu is open,
        // so the actions apply to the selected text, as from the menu bar.
        output.response.response.context_menu(|ui| context_menu(ui, actions));
        if let Some(range) = kept_selection {
            let mut state = output.state.clone();
            state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(range.start), CCursor::new(range.end))));
            state.store(ui.ctx(), id);
        } else if let Some(range) = output.cursor_range {
            let range = range.as_sorted_char_range();
            doc.selection = range.start.0..range.end.0;
        }

        // Number each row that starts a new line (soft-wrapped rows get no number).
        let painter = ui.painter_at(gutter);
        let rounded_left = egui::CornerRadius { nw: look::CARD_RADIUS - 1, sw: look::CARD_RADIUS - 1, ne: 0, se: 0 };
        painter.rect_filled(gutter, rounded_left, ui.visuals().faint_bg_color);
        painter.vline(gutter.right(), gutter.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
        let color = ui.visuals().weak_text_color();
        let mut line = 1;
        let mut starts_line = true;
        for row in &output.galley.rows {
            if starts_line {
                let y = output.galley_pos.y + row.pos.y;
                if y > gutter.bottom() {
                    break;
                }
                let pos = egui::pos2(gutter.right() - 6.0, y);
                painter.text(pos, egui::Align2::RIGHT_TOP, line.to_string(), font.clone(), color);
                line += 1;
            }
            starts_line = row.ends_with_newline;
        }
    }

    fn show_dialog(&mut self, ctx: &egui::Context) {
        // Take the dialog out of `self` so the closure below can freely read it
        // while we decide what to do; put it back if it stays open.
        let Some(mut dialog) = self.dialog.take() else { return };
        let mut choice = None;
        let titles: Vec<String> = self.docs.iter().map(Document::title).collect();

        let agent_server_error = self.agent_server_error.clone();
        let mut choose_folder = false;
        let mut failure = None;

        let modal = egui::Modal::new(self.ui_id("dialog")).show(ctx, |ui| {
            ui.set_width(if matches!(dialog, Dialog::Preferences) { 400.0 } else { 300.0 });
            match &mut dialog {
                Dialog::GoToLine { input } => {
                    ui.heading(tr("Go to Line"));
                    let field = ui.text_edit_singleline(input);
                    field.request_focus();
                    ui.horizontal(|ui| {
                        let enter = ui.input(|i| i.key_pressed(Key::Enter));
                        if ui.button(tr("Go")).clicked() || enter {
                            choice = Some(input.trim().parse().map_or(Choice::Cancel, Choice::GoTo));
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            choice = Some(Choice::Cancel);
                        }
                    });
                }
                Dialog::ConfirmClose { tab } => {
                    ui.heading(tr_fill("Save changes to “{}” before closing?", &[&titles[*tab]]));
                    ui.horizontal(|ui| {
                        if ui.button(tr("Save")).clicked() {
                            choice = Some(Choice::Save(*tab));
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            choice = Some(Choice::Cancel);
                        }
                        if ui.button(tr("Don’t Save")).clicked() {
                            choice = Some(Choice::Discard(*tab));
                        }
                    });
                }
                Dialog::ChangedOnDisk { doc, title } => {
                    ui.heading(tr_fill("“{}” was changed by another program", &[title]));
                    ui.label(tr("You have unsaved changes in it. Reloading replaces them with the file’s new text."));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr("Keep My Changes")).clicked() {
                            choice = Some(Choice::Cancel);
                        }
                        if ui.button(tr("Reload")).clicked() {
                            choice = Some(Choice::Reload(*doc));
                        }
                    });
                }
                Dialog::Error(message) => {
                    ui.label(message.as_str());
                    if ui.button(tr("OK")).clicked() {
                        choice = Some(Choice::Cancel);
                    }
                }
                Dialog::WordCount { of_selection, stats } => {
                    ui.heading(if *of_selection { tr("Word Count (selection)") } else { tr("Word Count") });
                    egui::Grid::new("word_count").num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
                        let rows = [
                            (tr("Words"), stats.words),
                            (tr("Characters"), stats.characters),
                            (tr("Characters (no spaces)"), stats.characters_no_spaces),
                            (tr("Lines"), stats.lines),
                            (tr("Paragraphs"), stats.paragraphs),
                        ];
                        for (label, value) in rows {
                            ui.label(label);
                            ui.label(egui::RichText::new(value.to_string()).monospace().strong());
                            ui.end_row();
                        }
                    });
                    if ui.button(tr("OK")).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        choice = Some(Choice::Cancel);
                    }
                }
                Dialog::Preferences => {
                    let current = settings::get();
                    let mut edited = current.clone();
                    ui.heading(tr("Preferences"));
                    ui.add_space(4.0);
                    egui::Grid::new("preferences").num_columns(2).spacing([20.0, 12.0]).show(ui, |ui| {
                        ui.label(tr("Theme"));
                        ui.horizontal(|ui| {
                            for theme in Theme::ALL {
                                ui.radio_value(&mut edited.theme, theme, tr(theme.label()));
                            }
                        });
                        ui.end_row();

                        ui.label(tr("Language"));
                        ui.horizontal(|ui| {
                            for language in Language::ALL {
                                ui.radio_value(&mut edited.language, language, language.name());
                            }
                        });
                        ui.end_row();

                        ui.label(tr("Default folder"));
                        ui.vertical(|ui| {
                            match &edited.default_folder {
                                Some(folder) => ui.add(egui::Label::new(folder.display().to_string()).wrap()),
                                None => ui.weak(tr("Not set")),
                            };
                            ui.horizontal(|ui| {
                                choose_folder = ui.button(tr("Choose…")).clicked();
                                let clear = egui::Button::new(tr("Clear"));
                                if ui.add_enabled(edited.default_folder.is_some(), clear).clicked() {
                                    edited.default_folder = None;
                                }
                            });
                            ui.weak(tr("Where the Open and Save panels start."));
                        });
                        ui.end_row();

                        ui.label(tr("Agents"));
                        ui.vertical(|ui| {
                            ui.checkbox(&mut edited.agent_server, tr("Let agents connect (MCP)"));
                            if edited.agent_server {
                                match &agent_server_error {
                                    Some(error) => ui.colored_label(ui.visuals().error_fg_color, error),
                                    None => ui.add(egui::Label::new(egui::RichText::new(mcp::url()).monospace()).selectable(true)),
                                };
                            }
                            ui.weak(tr("Agents can read and change your open documents."));
                        });
                        ui.end_row();
                    });
                    if edited != current {
                        failure = apply_settings(ui.ctx(), edited);
                    }
                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.button(tr("Done")).clicked() {
                            choice = Some(Choice::Cancel);
                        }
                    });
                }
            }
        });
        // The folder panel opens once the dialog is drawn, as it blocks until it is answered.
        if choose_folder && let Some(folder) = file_dialog().pick_folder() {
            failure = apply_settings(ctx, settings::Settings { default_folder: Some(folder), ..settings::get() });
        }
        if modal.should_close() && choice.is_none() {
            choice = Some(Choice::Cancel); // Escape or a click outside.
        }

        match choice {
            None => self.dialog = Some(dialog),
            Some(Choice::GoTo(line)) => {
                let range = text_utils::line_range(&self.doc().text, line);
                self.select(ctx, range);
            }
            Some(Choice::Save(tab)) => {
                if self.save(tab, false) {
                    self.remove_tab(tab);
                } else {
                    self.closing = false;
                }
            }
            Some(Choice::Discard(tab)) => self.remove_tab(tab),
            Some(Choice::Reload(id)) => {
                if let Some(doc) = self.docs.iter_mut().find(|doc| doc.id == id) {
                    match doc.reload() {
                        Ok(()) => agent_tools::store_cursor(ctx, doc),
                        Err(e) => failure = Some(tr_fill("Couldn’t open {}: {}", &[&doc.title(), &e.to_string()])),
                    }
                }
            }
            Some(Choice::Cancel) => self.closing = false,
        }
        if let Some(message) = failure {
            self.dialog = Some(Dialog::Error(message));
        }
    }
}

/// The rectangles to paint behind the occurrences of `query` that are in view.
fn match_highlights(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    text: &str,
    query: &str,
    match_case: bool,
    selection: &Range<usize>,
) -> Vec<egui::Shape> {
    let galley = &output.galley;
    // Only the rows in view are searched, so a long document costs no more than a short one.
    let visible = ui.clip_rect().translate(-output.galley_pos.to_vec2());
    let margin = query.chars().count();
    let first = galley.cursor_from_pos(egui::vec2(0.0, visible.top())).index.0.saturating_sub(margin);
    let last = galley.cursor_from_pos(egui::vec2(f32::MAX, visible.bottom())).index.0 + margin;

    let mut shapes = Vec::new();
    for found in text_utils::find_all(text, query, first..last, match_case) {
        let color = if found == *selection { CURRENT_MATCH_HIGHLIGHT } else { MATCH_HIGHLIGHT };
        let start = galley.pos_from_cursor(CCursor::new(found.start));
        let end = galley.pos_from_cursor(CCursor::new(found.end));
        let mut rows = Vec::new();
        if start.top() == end.top() {
            rows.push(egui::Rect::from_min_max(start.left_top(), end.left_bottom()));
        } else {
            // Soft wrap folded the line inside the occurrence: one rectangle per row,
            // from the characters' own places.
            let mut from = start;
            let mut to = start;
            for index in found.start + 1..=found.end {
                let next = galley.pos_from_cursor(CCursor::new(index));
                if next.top() != from.top() {
                    rows.push(egui::Rect::from_min_max(from.left_top(), egui::pos2(to.left() + to.height() * 0.6, to.bottom())));
                    from = next;
                }
                to = next;
            }
            rows.push(egui::Rect::from_min_max(from.left_top(), to.left_bottom()));
        }
        for row in rows.into_iter().filter(|row| row.width() > 0.0) {
            shapes.push(egui::Shape::rect_filled(row.translate(output.galley_pos.to_vec2()), 2.0, color));
        }
    }
    shapes
}

/// Makes `new` the current settings, switching the theme right away.
/// Returns the message to show if they couldn't be written to disk.
fn apply_settings(ctx: &egui::Context, new: settings::Settings) -> Option<String> {
    ctx.set_theme(new.theme.preference());
    settings::set(new).err().map(|e| tr_fill("Couldn’t save your preferences: {}", &[&e.to_string()]))
}

/// An Open or Save panel that starts in the default folder, if one is set.
fn file_dialog() -> rfd::FileDialog {
    match settings::default_folder() {
        Some(folder) => rfd::FileDialog::new().set_directory(folder),
        None => rfd::FileDialog::new(),
    }
}

/// The text pane's right-click menu.
fn context_menu(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    menu_item(ui, tr("Cut"), Action::Cut, actions);
    menu_item(ui, tr("Copy"), Action::Copy, actions);
    menu_item(ui, tr("Paste"), Action::Paste, actions);
    ui.separator();
    menu_item(ui, tr("Select All"), Action::SelectAll, actions);
    menu_item(ui, tr("Use Selection for Find"), Action::UseSelectionForFind, actions);
    ui.separator();
    text_tool_menus(ui, actions);
    ui.separator();
    menu_item(ui, tr("Word Count…"), Action::WordCount, actions);
}

/// Change Case, Lines and Whitespace submenus (Tools menu and right-click menu).
fn text_tool_menus(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    let submenus = [
        ("Change Case", TextTool::CASE),
        ("Lines", TextTool::LINES),
        ("Whitespace", TextTool::WHITESPACE),
    ];
    for (title, tools) in submenus {
        ui.menu_button(tr(title), |ui| {
            for &tool in tools {
                menu_item(ui, tr(tool.label()), Action::Tool(tool), actions);
            }
        });
    }
}

/// A menu item labelled with its keyboard shortcut.
fn menu_item(ui: &mut egui::Ui, label: &str, action: Action, actions: &mut Vec<Action>) {
    menu_item_enabled(ui, label, action, true, actions);
}

/// A menu item that is greyed out (and unclickable) unless `enabled`.
fn menu_item_enabled(ui: &mut egui::Ui, label: &str, action: Action, enabled: bool, actions: &mut Vec<Action>) {
    let shortcut = shortcut_for(action).map(|s| ui.ctx().format_shortcut(&s)).unwrap_or_default();
    if ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut)).clicked() {
        actions.push(action);
    }
}

/// Emojis as a grid of big, frameless buttons, 8 per row.
fn emoji_grid(ui: &mut egui::Ui, emojis: &[&'static str], actions: &mut Vec<Action>) {
    const COLUMNS: usize = 8;
    const CELL: f32 = 36.0;
    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
    egui::Grid::new("emoji_grid").spacing([2.0, 2.0]).show(ui, |ui| {
        for row in emojis.chunks(COLUMNS) {
            for &emoji in row {
                let button = egui::Button::new(egui::RichText::new(emoji).size(23.0))
                    .frame(false)
                    .min_size(egui::vec2(CELL, CELL));
                if ui.add(button).clicked() {
                    actions.push(Action::Insert(emoji));
                }
            }
            ui.end_row();
        }
    });
}

/// The native window (egui "viewport") that shows an extra `EditorWindow`.
fn viewport_id(window: &EditorWindow) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("window", window.id))
}

/// egui keeps each text edit's cursor and undo history under this id.
fn editor_id(doc: &Document) -> egui::Id {
    egui::Id::unique(("editor", doc.id))
}

impl EditorWindow {
    /// Draws this window for one frame. Returns the actions only `EditorApp`
    /// can handle (New Window, Quit).
    fn ui(&mut self, ui: &mut egui::Ui) -> Vec<Action> {
        let ctx = ui.ctx().clone();
        self.handle_dropped_files(&ctx);
        if let Some(range) = self.pending_select.take() {
            self.select(&ctx, range);
        }

        // Keyboard shortcuts, taken before the text editor can see the keys.
        let mut actions = Vec::new();
        if self.dialog.is_none() {
            ctx.input_mut(|input| {
                for (shortcut, action) in SHORTCUTS {
                    if input.consume_shortcut(shortcut) {
                        actions.push(*action);
                    }
                }
            });
        }

        look::window_border(ui);
        if !self.docs.is_empty() {
            // The window's own colour shows between the cards.
            ui.painter().rect_filled(ui.max_rect(), 0.0, look::palette(ui).window);
            // The window bar, then the cards: each panel holds one, a gap away from its neighbours.
            let bar_frame = egui::Frame::NONE.fill(look::bar::FILL);
            let drag_id = self.ui_id("title_bar_drag");
            egui::Panel::top(self.ui_id("title_bar")).frame(bar_frame).show_separator_line(false).exact_size(chrome::BAR_HEIGHT).show(ui, |ui| {
                chrome::bar_contents(ui, drag_id, |ui| self.window_bar(ui, &mut actions));
            });
            let panel = |panel: egui::Panel, frame: egui::Frame| panel.frame(frame).show_separator_line(false);
            panel(egui::Panel::top(self.ui_id("ribbon")), look::gutter(true, true, true, false)).show(ui, |ui| self.ribbon(ui, &mut actions));
            if self.find.visible {
                panel(egui::Panel::top(self.ui_id("find")), look::gutter(true, true, true, false)).show(ui, |ui| {
                    look::card(ui).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        self.find_bar(ui);
                    });
                });
            }
            if self.terminal.as_ref().is_some_and(Terminal::has_exited) {
                // The shell ended (e.g. `exit`): close the pane; reopening starts a new shell.
                self.terminal = None;
                self.terminal_visible = false;
            }
            if self.terminal_visible {
                let (panel_id, view_id) = (self.ui_id("terminal"), self.ui_id("terminal_view"));
                if let Some(terminal) = &mut self.terminal {
                    // At the very bottom, with no card, title or gap: the shell gets the window's whole width.
                    panel(egui::Panel::bottom(panel_id), egui::Frame::NONE)
                        .resizable(true)
                        .default_size(Terminal::height_for_rows(ui, TERMINAL_START_ROWS))
                        .min_size(80.0)
                        .show(ui, |ui| terminal.show(ui, view_id));
                }
            }
            // The status bar sits above the terminal, a gap away from it.
            panel(egui::Panel::bottom(self.ui_id("status")), look::gutter(true, true, false, true)).show(ui, |ui| self.status_card(ui));
            if self.glow_visible() {
                let text = &self.docs[self.active].text;
                let frame = look::gutter(false, true, true, true);
                glow::panel(self.ui_id("glow"), ui.available_width()).frame(frame).show_separator_line(false).show(ui, |ui| glow::show(ui, text));
            }
            egui::CentralPanel::no_frame().frame(look::gutter(true, true, true, true)).show(ui, |ui| {
                let page = look::card(ui).fill(look::palette(ui).page);
                page.show(ui, |ui| self.editor(ui, &mut actions));
            });
        }
        self.show_dialog(&ctx);

        if !actions.is_empty() {
            ctx.request_repaint();
        }
        let mut app_actions = Vec::new();
        for action in actions {
            match action {
                Action::NewWindow | Action::Quit => app_actions.push(action),
                _ => self.perform(action, &ctx),
            }
        }
        self.autosave_notes(&ctx);
        self.follow_files_on_disk(&ctx);
        self.continue_closing();
        self.update_window_title(&ctx);
        app_actions
    }
}

pub struct EditorApp {
    /// `windows[0]` is eframe's main window and always exists while the app runs.
    windows: Vec<EditorWindow>,
    next_window_id: u64,
    /// Every window is closing its tabs; the app exits once they're all gone.
    quitting: bool,
    /// Set right before exiting, so the main window's close request goes through.
    allow_close: bool,
    /// The main window's close button was clicked while other windows are open.
    confirm_quit: bool,
    /// Listens for agents while Preferences allow it.
    agent_server: Option<mcp::Server>,
    /// Why it couldn't start (the port is taken, usually).
    agent_server_error: Option<String>,
    /// Where the open files are remembered (see `session.rs`); nowhere in the tests.
    session_file: Option<PathBuf>,
    /// What that file says, to write it only when the open files change.
    saved_session: Option<String>,
}

impl EditorApp {
    /// Starts with the files that were open the last time, as `session_file` remembers
    /// them, then opens `paths`. Remembered files that are gone are passed over.
    pub fn restoring(session_file: PathBuf, paths: Vec<PathBuf>) -> Self {
        let mut app = Self::new(Vec::new());
        let remembered = session::load(&session_file).into_iter().filter(|window| window.files.iter().any(|file| file.is_file()));
        for (index, window) in remembered.enumerate() {
            if index > 0 {
                app.windows.push(EditorWindow::new(app.next_window_id, Vec::new()));
                app.next_window_id += 1;
            }
            app.windows[index].restore(window);
        }
        for path in paths {
            app.windows[0].open_path(path);
        }
        app.session_file = Some(session_file);
        app
    }

    /// Writes down which files are open, when that changed.
    fn remember_session(&mut self) {
        let Some(file) = &self.session_file else { return };
        // Quitting closes the tabs one by one: what was open before is what to remember.
        if self.quitting || self.windows.iter().any(|window| window.closing) {
            return;
        }
        let windows: Vec<session::Window> = self.windows.iter().map(EditorWindow::session).collect();
        let text = session::format(&windows);
        if self.saved_session.as_deref() != Some(&text) {
            let _ = session::save(file, &text); // not worth interrupting anyone if it fails
            self.saved_session = Some(text);
        }
    }

    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self {
            windows: vec![EditorWindow::new(0, paths)],
            next_window_id: 1,
            quitting: false,
            allow_close: false,
            confirm_quit: false,
            agent_server: None,
            agent_server_error: None,
            session_file: None,
            saved_session: None,
        }
    }

    /// Warns, in the main window, that closing it quits the app and its other windows.
    fn show_confirm_quit(&mut self, ctx: &egui::Context) {
        let others = self.windows.len() - 1;
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::unique("confirm_quit")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.heading(tr("Closing this window will quit RustyText"));
            let also = if others == 1 { tr("This also closes 1 other window.") } else { tr("This also closes the other windows.") };
            ui.label(format!("{also} {}", tr("You’ll be asked about any unsaved changes.")));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Quit")).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    choice = Some(true);
                }
                if ui.button(tr("Cancel")).clicked() {
                    choice = Some(false);
                }
            });
        });
        if modal.should_close() && choice.is_none() {
            choice = Some(false); // Escape or a click outside.
        }
        if let Some(quit) = choice {
            self.confirm_quit = false;
            if quit {
                self.quit();
            }
        }
    }

    fn quit(&mut self) {
        self.quitting = true;
        for window in &mut self.windows {
            window.closing = true;
        }
    }

    /// Someone pressed Cancel in a "Save changes?" dialog: keep every window open.
    fn cancel_quit(&mut self) {
        self.quitting = false;
        for window in &mut self.windows {
            window.closing = false;
            if window.docs.is_empty() {
                window.docs.push(Document::untitled());
                window.active = 0;
            }
        }
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // eframe exits when its main window closes, so closing it means quitting.
        // Cancel the close and quit our way (asking about unsaved tabs), after a
        // warning if other windows would close too.
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.windows.len() > 1 {
                self.confirm_quit = true;
            } else {
                self.quit();
            }
        }

        self.serve_agents(&ctx);

        // Files opened from the Finder while the app runs go to the main window, in front.
        let requested = crate::open_requests::take();
        if !requested.is_empty() {
            for path in requested {
                self.windows[0].open_path(path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        let notes_open = self.windows.iter().any(|window| window.docs.iter().any(|doc| doc.notes));
        for window in &mut self.windows {
            window.notes_open = notes_open;
            window.agent_server_error.clone_from(&self.agent_server_error);
        }

        let mut requests = Vec::new();
        let (main_window, extra_windows) = self.windows.split_first_mut().expect("main window always exists");
        requests.extend(main_window.ui(ui));
        for window in extra_windows {
            let viewport = viewport_id(window);
            let builder = title_bar::viewport(egui::ViewportBuilder::default())
                .with_title("RustyText")
                .with_inner_size(crate::WINDOW_SIZE)
                .with_drag_and_drop(true);
            // "Immediate" viewports are drawn right here, so the closure can borrow `window`.
            ctx.show_viewport_immediate(viewport, builder, |ui, _class| {
                if ui.input(|i| i.viewport().close_requested()) {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    window.closing = true;
                }
                requests.extend(window.ui(ui));
            });
        }

        if self.confirm_quit {
            self.show_confirm_quit(&ctx);
        }

        for request in requests {
            match request {
                Action::NewWindow => {
                    self.windows.push(EditorWindow::new(self.next_window_id, Vec::new()));
                    self.next_window_id += 1;
                }
                Action::Quit => self.quit(),
                _ => {}
            }
        }

        if self.quitting && self.windows.iter().any(|window| !window.closing) {
            self.cancel_quit();
        }

        // Extra windows go away once all their tabs are closed.
        let mut index = 0;
        self.windows.retain(|window| {
            let keep = index == 0 || !window.docs.is_empty();
            index += 1;
            keep
        });

        self.remember_session();

        // Only the main window is left and it has closed its tabs too: exit.
        if self.windows.len() == 1 && self.windows[0].docs.is_empty() {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Glow pane's background colour, used to find the pane among the drawn shapes.
    const GLOW_BACKGROUND: egui::Color32 = egui::Color32::from_rgb(0x1c, 0x1c, 0x1c);

    /// Draws `window` offscreen once per step, sending that step's mouse event,
    /// and returns the Glow pane's (left edge, width) after each frame.
    fn glow_pane_per_frame(window: &mut EditorWindow, steps: &[Option<egui::Event>]) -> Vec<(f32, f32)> {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let mut result = Vec::new();
        for (frame, event) in steps.iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(frame as f64 / 60.0),
                events: event.iter().cloned().collect(),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                window.ui(ui);
            });
            output.textures_delta.clear(); // nothing draws them offscreen
            let pane = output
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::Shape::Rect(rect) if rect.fill == GLOW_BACKGROUND => Some(rect.rect),
                    _ => None,
                })
                .max_by(|a, b| a.width().total_cmp(&b.width()));
            result.push(pane.map_or((0.0, 0.0), |rect| (rect.left(), rect.width())));
        }
        result
    }

    fn drag(from: f32, to: f32) -> Vec<Option<egui::Event>> {
        let at = |x: f32| egui::pos2(x, 300.0);
        let button = |x: f32, pressed: bool| egui::Event::PointerButton {
            pos: at(x),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let mut steps = vec![Some(egui::Event::PointerMoved(at(from))), Some(button(from, true))];
        for i in 1..=10 {
            steps.push(Some(egui::Event::PointerMoved(at(from + (to - from) * i as f32 / 10.0))));
        }
        steps.extend([Some(button(to, false)), None, None]);
        steps
    }

    #[test]
    fn right_click_opens_a_menu_and_keeps_the_selection() {
        let mut window = EditorWindow::new(0, Vec::new());
        window.docs[0].text = "hello world\nsecond line\n".to_owned();
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let at = egui::pos2(300.0, 200.0); // inside the text pane, below the text
        let right = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let steps = [vec![], vec![], vec![egui::Event::PointerMoved(at)], vec![right(true)], vec![right(false)], vec![]];
        for (frame, events) in steps.into_iter().enumerate() {
            if frame == 1 {
                window.select(&ctx, 0..5); // "hello"
            }
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(frame as f64 / 60.0),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                window.ui(ui);
            });
            output.textures_delta.clear();
        }
        assert!(egui::Popup::is_any_open(&ctx), "the context menu is open");
        assert_eq!(window.docs[0].selection, 0..5, "right-click kept the selection");
    }

    /// Draws `window` once offscreen.
    fn draw(window: &mut EditorWindow, ctx: &egui::Context) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            window.ui(ui);
        });
        output.textures_delta.clear();
    }

    fn editor_has_focus(window: &EditorWindow, ctx: &egui::Context) -> bool {
        let id = editor_id(&window.docs[window.active]);
        ctx.memory(|memory| memory.has_focus(id))
    }

    #[test]
    fn new_and_opened_documents_get_the_cursor() {
        let ctx = egui::Context::default();
        let mut window = EditorWindow::new(0, Vec::new());
        draw(&mut window, &ctx);
        assert!(editor_has_focus(&window, &ctx), "a new window's document");

        window.perform(Action::New, &ctx);
        draw(&mut window, &ctx);
        assert_eq!(window.active, 1);
        assert!(editor_has_focus(&window, &ctx), "after New");

        let path = std::env::temp_dir().join(format!("rusty-text-focus-{}.txt", std::process::id()));
        std::fs::write(&path, "some text\n").unwrap();
        window.open_path(path.clone());
        draw(&mut window, &ctx);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(window.active, 2);
        assert!(editor_has_focus(&window, &ctx), "after Open");
    }

    #[test]
    fn opening_a_markdown_file_shows_the_markdown_pane() {
        let dir = std::env::temp_dir();
        let text = dir.join(format!("rusty-text-pane-{}.txt", std::process::id()));
        let markdown = dir.join(format!("rusty-text-pane-{}.md", std::process::id()));
        std::fs::write(&text, "plain\n").unwrap();
        std::fs::write(&markdown, "# Title\n").unwrap();

        let mut window = EditorWindow::new(0, vec![text.clone()]);
        assert!(!window.glow, "a text file leaves the pane off");
        window.open_path(markdown.clone());
        assert!(window.glow_visible(), "a Markdown file turns it on");

        // Turned off by hand, it stays off when switching back to the tab.
        window.glow = false;
        window.open_path(text.clone());
        window.open_path(markdown.clone());
        assert!(!window.glow, "an already open file doesn't turn it back on");

        assert!(EditorWindow::new(1, vec![markdown.clone()]).glow_visible(), "also at startup");
        std::fs::remove_file(&text).unwrap();
        std::fs::remove_file(&markdown).unwrap();
    }

    #[test]
    fn a_file_changed_by_another_program_shows_in_its_tab() {
        let ctx = egui::Context::default();
        let dir = std::env::temp_dir();
        let clean = dir.join(format!("rusty-text-follow-clean-{}.txt", std::process::id()));
        let edited = dir.join(format!("rusty-text-follow-edited-{}.txt", std::process::id()));
        std::fs::write(&clean, "before\n").unwrap();
        std::fs::write(&edited, "before\n").unwrap();
        let mut window = EditorWindow::new(0, vec![edited.clone(), clean.clone()]);
        draw(&mut window, &ctx);
        assert!(window.dialog.is_none());

        window.docs[0].text = "my edit\n".to_owned();
        window.docs[0].dirty = true;
        std::fs::write(&clean, "after, by another program\n").unwrap();
        std::fs::write(&edited, "after, by another program\n").unwrap();
        window.check_files_on_disk(&ctx);

        // The tab with unsaved changes asks, and comes to the front; the other one waits.
        assert!(matches!(window.dialog, Some(Dialog::ChangedOnDisk { .. })));
        assert_eq!(window.active, 0);
        assert_eq!(window.docs[0].text, "my edit\n");
        assert_eq!(window.docs[1].text, "before\n");

        // Once answered (here: keep my changes), the tab without unsaved changes follows the file.
        window.dialog = None;
        window.check_files_on_disk(&ctx);
        assert!(window.dialog.is_none(), "asked once");
        assert_eq!(window.docs[0].text, "my edit\n");
        assert_eq!(window.docs[1].text, "after, by another program\n");

        std::fs::remove_file(&clean).unwrap();
        std::fs::remove_file(&edited).unwrap();
    }

    /// Draws `window` offscreen and returns how many rectangles of `color` it painted.
    fn count_highlights(window: &mut EditorWindow, ctx: &egui::Context, color: egui::Color32) -> usize {
        fn count(shape: &egui::Shape, color: egui::Color32) -> usize {
            match shape {
                egui::Shape::Rect(rect) => usize::from(rect.fill == color),
                egui::Shape::Vec(shapes) => shapes.iter().map(|shape| count(shape, color)).sum(),
                _ => 0,
            }
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            window.ui(ui);
        });
        output.textures_delta.clear(); // nothing draws them offscreen
        output.shapes.iter().map(|clipped| count(&clipped.shape, color)).sum()
    }

    #[test]
    fn the_find_bar_highlights_every_occurrence() {
        let ctx = egui::Context::default();
        let mut window = EditorWindow::new(0, Vec::new());
        window.docs[0].text = "one Two\ntwo three two\n".to_owned();
        assert_eq!(count_highlights(&mut window, &ctx, MATCH_HIGHLIGHT), 0, "no find bar, no highlight");

        window.find.visible = true;
        window.find.query = "two".to_owned();
        assert_eq!(count_highlights(&mut window, &ctx, MATCH_HIGHLIGHT), 3);
        assert_eq!(count_highlights(&mut window, &ctx, CURRENT_MATCH_HIGHLIGHT), 0, "none is selected yet");

        // Find Next selects one: it gets the stronger colour, the others keep theirs.
        window.find_next(&ctx, true);
        assert_eq!(window.docs[0].selection, 4..7);
        assert_eq!(count_highlights(&mut window, &ctx, CURRENT_MATCH_HIGHLIGHT), 1);
        assert_eq!(count_highlights(&mut window, &ctx, MATCH_HIGHLIGHT), 2);

        window.find.match_case = true;
        assert_eq!(count_highlights(&mut window, &ctx, MATCH_HIGHLIGHT), 2, "“Two” no longer counts");

        window.find.visible = false;
        assert_eq!(count_highlights(&mut window, &ctx, MATCH_HIGHLIGHT), 0, "gone with the find bar");
    }

    #[test]
    fn the_open_files_are_remembered_and_the_missing_ones_passed_over() {
        let dir = std::env::temp_dir().join(format!("rusty-text-session-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = |name: &str| dir.join(name);
        for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
            std::fs::write(file(name), name).unwrap();
        }
        let session_file = file("session");
        let remembered = [
            session::Window { files: vec![file("a.txt"), file("gone.txt"), file("b.txt")], active: 2 },
            session::Window { files: vec![file("gone too.txt")], active: 0 },
            session::Window { files: vec![file("c.txt")], active: 0 },
        ];
        std::fs::write(&session_file, session::format(&remembered)).unwrap();

        let titles = |window: &EditorWindow| window.docs.iter().map(Document::title).collect::<Vec<_>>();
        let mut app = EditorApp::restoring(session_file.clone(), vec![file("d.txt")]);
        assert_eq!(app.windows.len(), 2, "the window whose only file is gone doesn't come back");
        assert_eq!(titles(&app.windows[0]), ["a.txt", "b.txt", "d.txt"]);
        assert_eq!(app.windows[0].active, 2, "a file asked for now shows, rather than the remembered tab");
        assert!(app.windows[0].dialog.is_none(), "not a word about the missing file");
        assert_eq!(titles(&app.windows[1]), ["c.txt"]);

        let mut app_without_new_files = EditorApp::restoring(session_file.clone(), Vec::new());
        assert_eq!(app_without_new_files.windows[0].active, 1, "the tab that was showing shows again");
        app_without_new_files.remember_session();

        // What is open now is what the next start gets.
        app.remember_session();
        let expected = [
            session::Window { files: vec![file("a.txt"), file("b.txt"), file("d.txt")], active: 2 },
            session::Window { files: vec![file("c.txt")], active: 0 },
        ];
        assert_eq!(std::fs::read_to_string(&session_file).unwrap(), session::format(&expected));

        // Quitting closes the tabs one by one, and must not forget them.
        app.quit();
        app.windows[0].docs.clear();
        app.remember_session();
        assert_eq!(std::fs::read_to_string(&session_file).unwrap(), session::format(&expected));

        // Nothing but an untitled document: nothing to remember.
        let mut empty = EditorApp::restoring(session_file.clone(), Vec::new());
        empty.windows.truncate(1);
        empty.windows[0].docs = vec![Document::untitled()];
        empty.windows[0].active = 0;
        empty.remember_session();
        assert_eq!(std::fs::read_to_string(&session_file).unwrap(), "");
        assert_eq!(titles(&EditorApp::restoring(session_file.clone(), Vec::new()).windows[0]), ["untitled text"]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_notes_puts_them_first() {
        let mut window = EditorWindow::new(0, Vec::new());
        window.perform(Action::New, &egui::Context::default());
        window.perform(Action::OpenNotes, &egui::Context::default());
        assert_eq!(window.docs.len(), 3);
        assert!(window.docs[0].notes, "notes are the leftmost tab");
        assert_eq!(window.active, 0);
        assert!(window.docs[0].text.contains(&notes::date_line(notes::today())));

        // Choosing it again doesn't open a second copy.
        window.perform(Action::OpenNotes, &egui::Context::default());
        assert_eq!(window.docs.iter().filter(|doc| doc.notes).count(), 1);
    }

    #[test]
    fn open_notes_replaces_the_empty_starting_tab() {
        let mut window = EditorWindow::new(0, Vec::new());
        assert!(!window.docs[0].notes, "the notes don't open on their own");
        window.perform(Action::OpenNotes, &egui::Context::default());
        assert_eq!(window.docs.len(), 1);
        assert!(window.docs[0].notes);
    }

    #[test]
    fn preferences_open_from_the_shortcut_and_draw() {
        let ctx = egui::Context::default();
        let mut window = EditorWindow::new(0, Vec::new());
        draw(&mut window, &ctx);
        assert!(ctx.fonts_mut(|fonts| fonts.has_glyph(&egui::FontId::default(), '⚙')), "the gear can be drawn");

        assert_eq!(shortcut_for(Action::Preferences), Some(cmd(Key::Comma)));
        window.perform(Action::Preferences, &ctx);
        draw(&mut window, &ctx);
        assert!(matches!(window.dialog, Some(Dialog::Preferences)), "still open after a frame");
    }

    #[test]
    fn glow_pane_starts_at_half_width_and_resizes() {
        let path = std::env::temp_dir().join(format!("rusty-text-glow-{}.md", std::process::id()));
        std::fs::write(&path, "# Title\n\nSome *text*.\n").unwrap();
        let mut window = EditorWindow::new(0, vec![path.clone()]);
        window.glow = true;

        let start = glow_pane_per_frame(&mut window, &[None, None, None]);
        let (left, width) = *start.last().unwrap();
        eprintln!("start: {start:?}");
        std::fs::remove_file(&path).unwrap();
        assert!((width - 500.0).abs() < 20.0, "starts near half width, got {width}");

        let after = glow_pane_per_frame(&mut window, &drag(left, left + 200.0));
        eprintln!("drag: {after:?}");
        let (_, width) = *after.last().unwrap();
        assert!((width - 300.0).abs() < 20.0, "dragged 200px narrower, got {width}");
    }
}
