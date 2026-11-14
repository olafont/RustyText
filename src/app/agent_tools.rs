//! The tools agents can call through the MCP server (see `mcp.rs`): what each
//! is called, what it takes, and what it does to the open documents.
//!
//! Everything here runs on the UI thread, between two frames, so a tool sees
//! and changes the documents exactly as a menu command would.

use std::ops::Range;
use std::path::PathBuf;

use eframe::egui;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use serde_json::{Value, json};

use super::{Dialog, EditorApp, EditorWindow, editor_id};
use crate::document::Document;
use crate::i18n::tr_fill;
use crate::{mcp, settings, text_utils};

/// The tools as MCP's `tools/list` describes them: a name, a description for
/// the agent to read, and a JSON Schema of the arguments.
pub fn definitions() -> Value {
    let document_id = json!({
        "type": "integer",
        "description": "The document's id, from list_documents. Leave out for the active tab of the main window.",
    });
    let tool = |name: &str, description: &str, properties: Value, required: &[&str]| {
        json!({
            "name": name,
            "description": description,
            "inputSchema": { "type": "object", "properties": properties, "required": required },
        })
    };
    json!([
        tool(
            "list_documents",
            "Lists the documents open in RustyText, in every window: id, title, file path, \
             whether it is the active tab of its window, and whether it has unsaved changes.",
            json!({}),
            &[],
        ),
        tool(
            "read_document",
            "Returns the text of an open document as it is in the editor now, including unsaved changes. \
             Give start_line and/or end_line (1-based, inclusive) to read only part of it.",
            json!({
                "document_id": document_id,
                "start_line": { "type": "integer", "minimum": 1 },
                "end_line": { "type": "integer", "minimum": 1 },
            }),
            &[],
        ),
        tool(
            "get_selection",
            "Returns what the user has selected in a document, and where the selection (or the cursor) is.",
            json!({ "document_id": document_id }),
            &[],
        ),
        tool(
            "open_file",
            "Opens a file in a new tab of the main window and makes it the active tab, or switches to its \
             tab if it is already open. A file that does not exist yet opens empty and is created on save.",
            json!({ "path": { "type": "string", "description": "Absolute path of the file." } }),
            &["path"],
        ),
        tool(
            "new_document",
            "Opens a new untitled tab in the main window, optionally filled with text.",
            json!({ "text": { "type": "string" } }),
            &[],
        ),
        tool(
            "open_forever_note",
            "Opens the Forever Note, the user's running log that saves itself, and returns its document id. \
             The first time it is opened each day, a line with the date is added at the end.",
            json!({}),
            &[],
        ),
        tool(
            "edit_document",
            "Replaces old_text with new_text in a document. old_text must match exactly, and only once \
             unless replace_all is true. The change shows in the editor at once and is not saved to disk.",
            json!({
                "document_id": document_id,
                "old_text": { "type": "string" },
                "new_text": { "type": "string" },
                "replace_all": { "type": "boolean", "description": "Replace every occurrence. Default: false." },
            }),
            &["old_text", "new_text"],
        ),
        tool(
            "insert_text",
            "Inserts text into a document. `at` is \"cursor\" (the default: where the cursor is, replacing \
             the selection), \"start\" or \"end\".",
            json!({
                "document_id": document_id,
                "text": { "type": "string" },
                "at": { "type": "string", "enum": ["cursor", "start", "end"] },
            }),
            &["text"],
        ),
        tool(
            "set_text",
            "Replaces the whole text of a document. Prefer edit_document for small changes.",
            json!({ "document_id": document_id, "text": { "type": "string" } }),
            &["text"],
        ),
        tool(
            "save_document",
            "Saves a document to disk. Give `path` to save it under a new name (needed for an untitled document).",
            json!({
                "document_id": document_id,
                "path": { "type": "string", "description": "Absolute path to save to." },
            }),
            &[],
        ),
    ])
}

fn text_argument<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, String> {
    arguments.get(name).and_then(Value::as_str).ok_or_else(|| format!("`{name}` is required and must be a string."))
}

fn absolute_path(arguments: &Value) -> Result<Option<PathBuf>, String> {
    match arguments.get("path").filter(|path| !path.is_null()) {
        None => Ok(None),
        Some(path) => {
            let path = PathBuf::from(path.as_str().ok_or("`path` must be a string.")?);
            if path.is_absolute() { Ok(Some(path)) } else { Err("`path` must be an absolute path.".to_owned()) }
        }
    }
}

/// A 1-based line number argument.
fn line_argument(arguments: &Value, name: &str) -> Result<Option<usize>, String> {
    match arguments.get(name).filter(|line| !line.is_null()) {
        None => Ok(None),
        Some(line) => match line.as_u64() {
            Some(line) if line >= 1 => Ok(Some(line as usize)),
            _ => Err(format!("`{name}` must be a line number, starting at 1.")),
        },
    }
}

/// Replaces `range` (in chars) of the document's text, and moves the selection
/// so that it stays on the same text: after the change if it was after it,
/// at the end of the new text if it was inside what got replaced.
fn replace_range(doc: &mut Document, range: Range<usize>, new_text: &str) {
    let start = text_utils::char_to_byte(&doc.text, range.start);
    let end = text_utils::char_to_byte(&doc.text, range.end);
    doc.text.replace_range(start..end, new_text);
    doc.dirty = true;

    let new_len = new_text.chars().count();
    let selection = &mut doc.selection;
    if selection.start >= range.end {
        selection.start = selection.start - range.len() + new_len;
        selection.end = selection.end - range.len() + new_len;
    } else if selection.end > range.start || selection.start == range.start {
        *selection = range.start + new_len..range.start + new_len;
    }
}

/// Tells egui where the document's cursor now is. The editor keeps its own
/// copy, which would otherwise win and still point at the old place.
pub(super) fn store_cursor(ctx: &egui::Context, doc: &Document) {
    let id = editor_id(doc);
    let mut state = TextEditState::load(ctx, id).unwrap_or_default();
    let (start, end) = (CCursor::new(doc.selection.start), CCursor::new(doc.selection.end));
    state.cursor.set_char_range(Some(CCursorRange::two(start, end)));
    state.store(ctx, id);
}

/// A position in a document as agents and the status bar count it.
fn position(text: &str, char_index: usize) -> Value {
    let (line, column) = text_utils::line_col(text, char_index);
    json!({ "line": line, "column": column })
}

impl EditorWindow {
    /// The message of the error dialog an action just raised, taken off the screen:
    /// the agent gets the message instead of the user.
    fn take_error(&mut self) -> Option<String> {
        match self.dialog.take() {
            Some(Dialog::Error(message)) => Some(message),
            other => {
                self.dialog = other;
                None
            }
        }
    }
}

impl EditorApp {
    /// Starts or stops the agent server to match Preferences, then answers the agents' calls.
    pub(super) fn serve_agents(&mut self, ctx: &egui::Context) {
        if !settings::agent_server() {
            self.agent_server = None;
            self.agent_server_error = None;
            return;
        }
        if self.agent_server.is_none() && self.agent_server_error.is_none() {
            let wake = {
                let ctx = ctx.clone();
                move || ctx.request_repaint()
            };
            match mcp::Server::start(mcp::PORT, definitions(), wake) {
                Ok(server) => self.agent_server = Some(server),
                Err(e) => {
                    let port = mcp::PORT.to_string();
                    self.agent_server_error = Some(tr_fill("Couldn’t listen on port {}: {}", &[&port, &e.to_string()]));
                }
            }
        }
        let calls: Vec<mcp::Call> = self.agent_server.iter().flat_map(mcp::Server::calls).collect();
        if !calls.is_empty() {
            ctx.request_repaint(); // show what the tools changed
        }
        for call in calls {
            let result = self.run_agent_tool(&call.tool, &call.arguments, ctx);
            call.respond(result);
        }
    }

    /// Where a tool's document is: (window, tab).
    fn locate(&self, arguments: &Value) -> Result<(usize, usize), String> {
        let Some(id) = arguments.get("document_id").filter(|id| !id.is_null()) else {
            let main = &self.windows[0];
            return if main.docs.is_empty() { Err("No document is open.".to_owned()) } else { Ok((0, main.active)) };
        };
        let id = id.as_u64().ok_or("`document_id` must be a number from list_documents.")?;
        self.windows
            .iter()
            .enumerate()
            .find_map(|(window, editor)| editor.docs.iter().position(|doc| doc.id == id).map(|tab| (window, tab)))
            .ok_or_else(|| format!("No open document has the id {id}. Call list_documents."))
    }

    fn document(&mut self, arguments: &Value) -> Result<&mut Document, String> {
        let (window, tab) = self.locate(arguments)?;
        Ok(&mut self.windows[window].docs[tab])
    }

    /// Runs one tool. `Ok` is its output for the agent; `Err` says what went wrong.
    pub(super) fn run_agent_tool(&mut self, tool: &str, arguments: &Value, ctx: &egui::Context) -> Result<String, String> {
        match tool {
            "list_documents" => {
                let mut documents = Vec::new();
                for (index, window) in self.windows.iter().enumerate() {
                    for (tab, doc) in window.docs.iter().enumerate() {
                        documents.push(json!({
                            "document_id": doc.id,
                            "title": doc.title(),
                            "path": doc.path.as_ref().map(|path| path.display().to_string()),
                            "window": index + 1,
                            "active_tab": tab == window.active,
                            "unsaved_changes": doc.dirty,
                            "forever_note": doc.notes,
                            "lines": text_utils::line_count(&doc.text),
                        }));
                    }
                }
                Ok(json!({ "documents": documents }).to_string())
            }
            "read_document" => {
                let first = line_argument(arguments, "start_line")?;
                let last = line_argument(arguments, "end_line")?;
                let doc = self.document(arguments)?;
                if first.is_none() && last.is_none() {
                    return Ok(doc.text.clone());
                }
                let (first, last) = (first.unwrap_or(1), last.unwrap_or(usize::MAX));
                if last < first {
                    return Err("`end_line` is before `start_line`.".to_owned());
                }
                Ok(doc.text.split_inclusive('\n').skip(first - 1).take(last - first + 1).collect())
            }
            "get_selection" => {
                let doc = self.document(arguments)?;
                Ok(json!({
                    "document_id": doc.id,
                    "title": doc.title(),
                    "selected_text": doc.selected_text(),
                    "start": position(&doc.text, doc.selection.start),
                    "end": position(&doc.text, doc.selection.end),
                })
                .to_string())
            }
            "open_file" => {
                let path = absolute_path(arguments)?.ok_or("`path` is required.")?;
                let main = &mut self.windows[0];
                main.open_path(path.clone());
                if let Some(message) = main.take_error() {
                    return Err(message);
                }
                let doc = &main.docs[main.active];
                Ok(json!({ "document_id": doc.id, "title": doc.title() }).to_string())
            }
            "new_document" => {
                let main = &mut self.windows[0];
                main.docs.push(Document::untitled());
                main.active = main.docs.len() - 1;
                let doc = &mut main.docs[main.active];
                if let Some(text) = arguments.get("text").and_then(Value::as_str) {
                    doc.text = text.to_owned();
                    doc.dirty = !text.is_empty();
                }
                let id = doc.id;
                main.pending_select = Some(0..0);
                Ok(json!({ "document_id": id }).to_string())
            }
            "open_forever_note" => {
                let already_open = self.windows.iter().flat_map(|window| &window.docs).find(|doc| doc.notes);
                if let Some(doc) = already_open {
                    return Ok(json!({ "document_id": doc.id }).to_string());
                }
                let main = &mut self.windows[0];
                main.open_notes();
                if let Some(message) = main.take_error() {
                    return Err(message);
                }
                match main.docs.first().filter(|doc| doc.notes) {
                    Some(doc) => Ok(json!({ "document_id": doc.id }).to_string()),
                    None => Err("The Forever Note has no place to live: no home folder was found.".to_owned()),
                }
            }
            "edit_document" => {
                let old_text = text_argument(arguments, "old_text")?;
                let new_text = text_argument(arguments, "new_text")?;
                let replace_all = arguments.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
                if old_text.is_empty() {
                    return Err("`old_text` is empty. Use insert_text to add text.".to_owned());
                }
                let doc = self.document(arguments)?;
                let found: Vec<usize> = doc.text.match_indices(old_text).map(|(byte, _)| byte).collect();
                match found.len() {
                    0 => return Err("`old_text` was not found in the document. It must match exactly.".to_owned()),
                    1 => {}
                    count if !replace_all => {
                        return Err(format!(
                            "`old_text` appears {count} times. Add surrounding text to make it unique, or set replace_all."
                        ));
                    }
                    _ => {}
                }
                // Last one first, so the earlier positions stay right.
                let old_len = old_text.chars().count();
                for byte in found.iter().rev() {
                    let start = text_utils::byte_to_char(&doc.text, *byte);
                    replace_range(doc, start..start + old_len, new_text);
                }
                store_cursor(ctx, doc);
                Ok(format!("Replaced {} occurrence(s) in {}.", found.len(), doc.title()))
            }
            "insert_text" => {
                let text = text_argument(arguments, "text")?;
                let at = arguments.get("at").and_then(Value::as_str).unwrap_or("cursor");
                let doc = self.document(arguments)?;
                let range = match at {
                    "cursor" => doc.selection.clone(),
                    "start" => 0..0,
                    "end" => {
                        let end = doc.text.chars().count();
                        end..end
                    }
                    _ => return Err("`at` must be \"cursor\", \"start\" or \"end\".".to_owned()),
                };
                replace_range(doc, range, text);
                store_cursor(ctx, doc);
                Ok(format!("Inserted {} character(s) into {}.", text.chars().count(), doc.title()))
            }
            "set_text" => {
                let text = text_argument(arguments, "text")?;
                let doc = self.document(arguments)?;
                let all = 0..doc.text.chars().count();
                replace_range(doc, all, text);
                store_cursor(ctx, doc);
                Ok(format!("Replaced the text of {}.", doc.title()))
            }
            "save_document" => {
                let path = absolute_path(arguments)?;
                let (window, tab) = self.locate(arguments)?;
                let editor = &mut self.windows[window];
                let saved = match path {
                    Some(path) => editor.docs[tab].save_to(path).map_err(|e| e.to_string()),
                    None if editor.docs[tab].path.is_none() => {
                        return Err("This document has never been saved. Give `path` to say where.".to_owned());
                    }
                    None if editor.save(tab, false) => Ok(()),
                    None => Err(editor.take_error().unwrap_or_else(|| "The document could not be saved.".to_owned())),
                };
                saved?;
                let doc = &editor.docs[tab];
                Ok(format!("Saved {}.", doc.path.as_ref().map_or_else(|| doc.title(), |path| path.display().to_string())))
            }
            _ => Err(format!("Unknown tool: {tool}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(text: &str) -> EditorApp {
        let mut app = EditorApp::new(Vec::new());
        app.windows[0].docs[0].text = text.to_owned();
        app
    }

    fn run(app: &mut EditorApp, tool: &str, arguments: Value) -> Result<String, String> {
        app.run_agent_tool(tool, &arguments, &egui::Context::default())
    }

    #[test]
    fn every_described_tool_exists() {
        let mut app = app_with("");
        for tool in definitions().as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            if name == "open_forever_note" {
                continue; // would open your real notes
            }
            let outcome = run(&mut app, name, json!({}));
            assert!(!outcome.is_err_and(|message| message.starts_with("Unknown tool")), "{name} is not handled");
        }
    }

    #[test]
    fn lists_and_reads_documents() {
        let mut app = app_with("one\ntwo\nthree\n");
        let id = app.windows[0].docs[0].id;

        let list: Value = serde_json::from_str(&run(&mut app, "list_documents", json!({})).unwrap()).unwrap();
        assert_eq!(list["documents"][0]["document_id"], id);
        assert_eq!(list["documents"][0]["active_tab"], true);

        assert_eq!(run(&mut app, "read_document", json!({})).unwrap(), "one\ntwo\nthree\n");
        let lines = json!({ "document_id": id, "start_line": 2, "end_line": 3 });
        assert_eq!(run(&mut app, "read_document", lines).unwrap(), "two\nthree\n");
        assert!(run(&mut app, "read_document", json!({ "document_id": 0 })).is_err());
    }

    #[test]
    fn edits_must_match_exactly_once() {
        let mut app = app_with("red green red");
        let edit = |old: &str, all: bool| json!({ "old_text": old, "new_text": "blue", "replace_all": all });

        assert!(run(&mut app, "edit_document", edit("purple", false)).is_err());
        assert!(run(&mut app, "edit_document", edit("red", false)).unwrap_err().contains("2 times"));
        assert_eq!(app.windows[0].docs[0].text, "red green red", "a refused edit changes nothing");

        run(&mut app, "edit_document", edit("green", false)).unwrap();
        run(&mut app, "edit_document", edit("red", true)).unwrap();
        let doc = &app.windows[0].docs[0];
        assert_eq!(doc.text, "blue blue blue");
        assert!(doc.dirty);
    }

    #[test]
    fn the_selection_follows_its_text_through_an_edit() {
        let mut app = app_with("aaa bbb ccc");
        app.windows[0].docs[0].selection = 8..11; // "ccc"
        run(&mut app, "edit_document", json!({ "old_text": "aaa", "new_text": "a" })).unwrap();
        let doc = &app.windows[0].docs[0];
        assert_eq!(doc.selected_text(), "ccc");

        run(&mut app, "insert_text", json!({ "text": "!" })).unwrap();
        run(&mut app, "insert_text", json!({ "text": ">", "at": "start" })).unwrap();
        run(&mut app, "insert_text", json!({ "text": "<", "at": "end" })).unwrap();
        assert_eq!(app.windows[0].docs[0].text, ">a bbb !<");
    }

    #[test]
    fn opens_and_saves_files() {
        let path = std::env::temp_dir().join(format!("rusty-text-agent-{}.txt", std::process::id()));
        std::fs::write(&path, "before\n").unwrap();
        let mut app = app_with("");

        assert!(run(&mut app, "open_file", json!({ "path": "relative.txt" })).is_err());
        assert!(run(&mut app, "save_document", json!({})).unwrap_err().contains("never been saved"));

        run(&mut app, "open_file", json!({ "path": path })).unwrap();
        run(&mut app, "set_text", json!({ "text": "after\n" })).unwrap();
        run(&mut app, "save_document", json!({})).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "after\n");
        assert!(!app.windows[0].docs[0].dirty);

        let new: Value = serde_json::from_str(&run(&mut app, "new_document", json!({ "text": "hi" })).unwrap()).unwrap();
        assert_eq!(app.windows[0].docs[1].id, new["document_id"]);
        std::fs::remove_file(&path).unwrap();
    }
}
