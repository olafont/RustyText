//! A terminal pane: your login shell running in a pseudo-terminal (PTY).
//!
//! - `portable-pty` starts the shell and gives us its input and output streams.
//! - A background thread reads the shell's output and feeds it to a `vt100`
//!   parser, which keeps the screen (a grid of styled characters and a cursor).
//! - Each frame, `show()` draws that grid and sends keystrokes back to the shell.
//!
//! The parser is shared between the reader thread and the UI, so it lives in an
//! `Arc<Mutex<…>>`: `Arc` lets both own it, `Mutex` lets only one use it at a time.

use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, FontId, Key, Modifiers};
use egui::text::{LayoutJob, TextFormat};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

const FONT_SIZE: f32 = 14.0;
/// Space between the pane's edge and the text.
const PADDING: f32 = 6.0;
const SCROLLBACK_LINES: usize = 5000;
const BACKGROUND: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1c);
const FOREGROUND: Color32 = Color32::from_rgb(0xd0, 0xd0, 0xd0);

pub struct Terminal {
    parser: Arc<Mutex<vt100::Parser>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    /// Set by the reader thread when the shell exits (e.g. after `exit`).
    exited: Arc<AtomicBool>,
    rows: u16,
    cols: u16,
}

impl Terminal {
    /// Starts the user's login shell in `cwd`.
    pub fn spawn(cwd: &Path, ctx: egui::Context) -> Result<Self, String> {
        let (rows, cols) = (24, 80);
        let pair = native_pty_system()
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("couldn’t open a terminal: {e}"))?;

        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        if cfg!(windows) {
            // Windows' pseudo-console starts by asking where the cursor is (ESC[6n) and
            // shows nothing until it gets the answer. The answer must arrive before the
            // shell starts; a later one is taken as typed keys.
            writer.write_all(b"\x1b[1;1R").and_then(|()| writer.flush()).map_err(|e| e.to_string())?;
        }

        let mut command = CommandBuilder::new_default_prog();
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(command).map_err(|e| format!("couldn’t start the shell: {e}"))?;
        drop(pair.slave); // the shell has its own handle now

        let parser = Arc::new(Mutex::new(vt100::Parser::new(nonzero(rows), nonzero(cols), SCROLLBACK_LINES)));
        let exited = Arc::new(AtomicBool::new(false));

        // Reading blocks until the shell prints something, so it gets its own thread.
        let (thread_parser, thread_exited) = (Arc::clone(&parser), Arc::clone(&exited));
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        thread_parser.lock().unwrap().process(&buffer[..n]);
                        ctx.request_repaint();
                    }
                }
            }
            thread_exited.store(true, Ordering::Relaxed);
            ctx.request_repaint();
        });

        Ok(Self { parser, master: pair.master, writer, child, exited, rows, cols })
    }

    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::Relaxed)
    }

    fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Tells the shell (and the parser) how many rows and columns fit.
    fn resize(&mut self, rows: u16, cols: u16) {
        if (rows, cols) == (self.rows, self.cols) {
            return;
        }
        self.rows = rows;
        self.cols = cols;
        let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        self.parser.lock().unwrap().set_size(nonzero(rows), nonzero(cols));
    }

    /// Pane height that fits `rows` lines of text, for the pane's starting size.
    pub fn height_for_rows(ui: &egui::Ui, rows: u16) -> f32 {
        cell_size(ui).y * f32::from(rows) + 2.0 * PADDING
    }

    /// Draws the terminal in the available space and handles its keyboard input.
    pub fn show(&mut self, ui: &mut egui::Ui, id: egui::Id) {
        let font = FontId::monospace(FONT_SIZE);
        let cell = cell_size(ui);
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, BACKGROUND);

        // Claim the whole area: a panel remembers its size as the space its contents
        // used, so without this a resized terminal pane would shrink back to its minimum.
        ui.advance_cursor_after_rect(rect);
        let response = ui.interact(rect, id, egui::Sense::click());
        if response.clicked() {
            response.request_focus();
        }
        let focused = response.has_focus();
        if focused {
            // Keep Tab, arrows and Escape for the shell instead of moving focus.
            let filter = egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
            ui.memory_mut(|memory| memory.set_focus_lock_filter(id, filter));
            self.handle_input(ui);
        }

        let inner = rect.shrink(PADDING);
        let rows = ((inner.height() / cell.y).floor() as u16).max(1);
        let cols = ((inner.width() / cell.x).floor() as u16).max(2);
        self.resize(rows, cols);

        // Mouse wheel scrolls back through earlier output.
        let wheel = ui.input(|i| i.smooth_scroll_delta.y);
        let mut parser = self.parser.lock().unwrap();
        if response.hovered() && wheel != 0.0 {
            let lines = (wheel / cell.y).round() as isize;
            let current = parser.screen().scrollback() as isize;
            parser.screen_mut().set_scrollback((current + lines).max(0) as usize);
        }

        let screen = parser.screen();
        let painter = ui.painter_at(rect);
        for row in 0..rows {
            let job = row_layout(screen, row, cols, &font);
            let galley = painter.layout_job(job);
            painter.galley(inner.min + egui::vec2(0.0, row as f32 * cell.y), galley, FOREGROUND);
        }

        // Block cursor: solid when focused, an outline otherwise.
        if !screen.hide_cursor() && screen.scrollback() == 0 {
            let (row, col) = screen.cursor_position();
            let pos = inner.min + egui::vec2(col as f32 * cell.x, row as f32 * cell.y);
            let cursor = egui::Rect::from_min_size(pos, cell);
            if focused {
                painter.rect_filled(cursor, 0.0, FOREGROUND.gamma_multiply(0.6));
            } else {
                painter.rect_stroke(cursor, 0.0, (1.0, FOREGROUND), egui::StrokeKind::Inside);
            }
        }
    }

    /// Turns this frame's keyboard events into the bytes a terminal sends.
    fn handle_input(&mut self, ui: &egui::Ui) {
        let (app_cursor, bracketed_paste) = {
            let parser = self.parser.lock().unwrap();
            (parser.screen().application_cursor(), parser.screen().bracketed_paste())
        };
        let events = ui.input(|i| i.events.clone());
        for event in events {
            let bytes: Vec<u8> = match event {
                egui::Event::Text(text) => text.into_bytes(),
                egui::Event::Paste(text) if bracketed_paste => format!("\x1b[200~{text}\x1b[201~").into_bytes(),
                egui::Event::Paste(text) => text.into_bytes(),
                egui::Event::Key { key, pressed: true, modifiers, .. } => match key_bytes(key, modifiers, app_cursor) {
                    Some(bytes) => bytes,
                    None => continue,
                },
                _ => continue,
            };
            // Typing jumps back to the live screen, like a real terminal.
            self.parser.lock().unwrap().screen_mut().set_scrollback(0);
            self.write(&bytes);
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
        // Collect the exit status, or the dead shell lingers as a "zombie" process.
        let _ = self.child.wait();
    }
}

/// The size of one character cell in the terminal font.
fn cell_size(ui: &egui::Ui) -> egui::Vec2 {
    ui.painter().layout_no_wrap("W".into(), FontId::monospace(FONT_SIZE), Color32::WHITE).size()
}

fn nonzero(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n.max(1)).expect("at least 1")
}

/// Bytes for keys that don't produce text: Enter, arrows, Ctrl+letter, …
fn key_bytes(key: Key, modifiers: Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    // Ctrl+A … Ctrl+Z are bytes 1 … 26 (Ctrl+C = 3 interrupts, Ctrl+D = 4 ends input).
    if modifiers.ctrl && !modifiers.command {
        let name = key.name();
        if name.len() == 1 && name.as_bytes()[0].is_ascii_alphabetic() {
            return Some(vec![name.as_bytes()[0].to_ascii_uppercase() - b'A' + 1]);
        }
    }
    if modifiers.command {
        return None; // ⌘ shortcuts belong to the app
    }
    let arrow = |letter: char| {
        let prefix = if app_cursor { "\x1bO" } else { "\x1b[" };
        Some(format!("{prefix}{letter}").into_bytes())
    };
    let text = |s: &str| Some(s.as_bytes().to_vec());
    match key {
        Key::Enter => text("\r"),
        Key::Backspace => text("\x7f"),
        Key::Tab if modifiers.shift => text("\x1b[Z"),
        Key::Tab => text("\t"),
        Key::Escape => text("\x1b"),
        Key::ArrowUp => arrow('A'),
        Key::ArrowDown => arrow('B'),
        Key::ArrowRight => arrow('C'),
        Key::ArrowLeft => arrow('D'),
        Key::Home => text("\x1b[H"),
        Key::End => text("\x1b[F"),
        Key::PageUp => text("\x1b[5~"),
        Key::PageDown => text("\x1b[6~"),
        Key::Delete => text("\x1b[3~"),
        _ => None,
    }
}

/// One screen row as a LayoutJob, with a run of text per change of style.
fn row_layout(screen: &vt100::Screen, row: u16, cols: u16, font: &FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else { continue };
        if cell.is_wide_continuation() {
            continue;
        }
        let mut fg = color(cell.fgcolor(), FOREGROUND, cell.bold());
        let mut bg = color(cell.bgcolor(), Color32::TRANSPARENT, false);
        if cell.inverse() {
            (fg, bg) = (if bg == Color32::TRANSPARENT { BACKGROUND } else { bg }, fg);
        }
        if cell.dim() {
            fg = fg.gamma_multiply(0.6);
        }
        let format = TextFormat {
            font_id: font.clone(),
            color: fg,
            background: bg,
            italics: cell.italic(),
            underline: if cell.underline() { egui::Stroke::new(1.0, fg) } else { egui::Stroke::NONE },
            ..Default::default()
        };
        let contents = if cell.has_contents() { cell.contents() } else { " " };
        job.append(contents, 0.0, format);
    }
    job
}

/// A vt100 colour as RGB, using the standard xterm 256-colour palette.
fn color(color: vt100::Color, default: Color32, bold: bool) -> Color32 {
    const BASIC: [(u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00), (0xcd, 0x31, 0x31), (0x0d, 0xbc, 0x79), (0xe5, 0xe5, 0x10),
        (0x24, 0x72, 0xc8), (0xbc, 0x3f, 0xbc), (0x11, 0xa8, 0xcd), (0xe5, 0xe5, 0xe5),
        (0x66, 0x66, 0x66), (0xf1, 0x4c, 0x4c), (0x23, 0xd1, 0x8b), (0xf5, 0xf5, 0x43),
        (0x3b, 0x8e, 0xea), (0xd6, 0x70, 0xd6), (0x29, 0xb8, 0xdb), (0xff, 0xff, 0xff),
    ];
    match color {
        vt100::Color::Default if bold => Color32::WHITE,
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        vt100::Color::Idx(i) => {
            // egui's monospace font has no bold face, so bold uses the bright colours.
            let i = if bold && i < 8 { i + 8 } else { i };
            match i {
                0..=15 => {
                    let (r, g, b) = BASIC[i as usize];
                    Color32::from_rgb(r, g, b)
                }
                16..=231 => {
                    let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
                    let i = i - 16;
                    Color32::from_rgb(level(i / 36), level((i / 6) % 6), level(i % 6))
                }
                232..=255 => {
                    let gray = 8 + 10 * (i - 232);
                    Color32::from_rgb(gray, gray, gray)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_keys_to_terminal_bytes() {
        assert_eq!(key_bytes(Key::Enter, Modifiers::NONE, false), Some(b"\r".to_vec()));
        assert_eq!(key_bytes(Key::C, Modifiers::CTRL, false), Some(vec![3]));
        assert_eq!(key_bytes(Key::ArrowUp, Modifiers::NONE, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(key_bytes(Key::ArrowUp, Modifiers::NONE, true), Some(b"\x1bOA".to_vec()));
        assert_eq!(key_bytes(Key::S, Modifiers::COMMAND, false), None);
    }

    #[test]
    fn maps_256_colours() {
        assert_eq!(color(vt100::Color::Idx(1), FOREGROUND, true), Color32::from_rgb(0xf1, 0x4c, 0x4c));
        assert_eq!(color(vt100::Color::Idx(196), FOREGROUND, false), Color32::from_rgb(255, 0, 0));
        assert_eq!(color(vt100::Color::Idx(232), FOREGROUND, false), Color32::from_rgb(8, 8, 8));
    }

    #[test]
    fn runs_a_command_in_a_real_shell() {
        // End-to-end: spawn the shell, run a command, and find its output on screen.
        let ctx = egui::Context::default();
        let mut terminal = Terminal::spawn(&std::env::temp_dir(), ctx).unwrap();
        // The expected text only appears if the shell ran the command (the typed command
        // is echoed too, but unexpanded). Windows' default shell is cmd.exe, not a Unix shell.
        let (command, expected): (&[u8], &str) = if cfg!(windows) {
            (b"echo rusty-%OS%-text\r", "rusty-Windows_NT-text")
        } else {
            (b"echo rusty-$((40+2))-text\r", "rusty-42-text")
        };
        terminal.write(command);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut screen = String::new();
        while std::time::Instant::now() < deadline {
            screen = terminal.parser.lock().unwrap().screen().contents();
            if screen.contains(expected) {
                assert!(!screen.contains("1;1R"), "the cursor reply reached the shell as typing:\n{screen}");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("command output never appeared; screen:\n{screen}");
    }
}
