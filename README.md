# RustyText

A small text editor, in Rust with [egui](https://github.com/emilk/egui)/eframe.
Runs on macOS, Windows and Linux.

    cargo run -- [file ...]        # debug build
    cargo run --release -- [file ...]
    cargo test

How to use it: [docs/USER_MANUAL.md](docs/USER_MANUAL.md).

## Features

- On macOS the app draws its title bar: the traffic lights, the app's name, then the menus
- Tabs: New / Open / Close (⌘N ⌘O ⌘W), New Window (⇧⌘N), Save / Save As (⌘S ⇧⌘S),
  unsaved-changes prompts, drag and drop files onto the window
- Files changed by another program reload on their own (it asks first if you have unsaved changes)
- Line-number gutter, monospaced font, soft wrap
- Find & Replace (⌘F, ⌥⌘F, ⌘G, ⇧⌘G, ⌘E), Go to Line (⌘J)
- Status bar: line, column, line count, char count
- Right-click menu in the text: cut/copy/paste, text tools, word count
- Edit ▸ Emoji: emoji grids by category
- Tools: Change Case / Lines / Whitespace transforms (undoable), Word Count…
- Tools ▸ Markdown: rendered Markdown pane beside the editor (`.md` files), shown when you open one
- Tools ▸ Terminal (⌃`): your shell in a pane under the editor
- Forever Note: a running log that is always there (below)
- Preferences (the ⚙ at the right of the title bar, or ⌘,): theme, language and default folder (below)
- Agents: an MCP server, so that AI agents can read and edit your open documents (below)

## Forever Note

Tools ▸ Forever Note opens your forever note, `~/.rustytext/forevernote`, as the leftmost tab: a running
log. The first time you open them each day, a date line (`--- Saturday, October 24, 2026 ---`)
is added at the end, and the cursor goes to the end, ready to type. The notes
save themselves a second after each change, and when their tab or the app closes.

## Preferences

The ⚙ button at the right end of the title bar (or File ▸ Preferences…, ⌘,) sets:

- Theme: System (follows the system's light or dark mode), Light or Dark
- Language: English or Français
- Default folder: where the Open and Save panels start, and the terminal for an unsaved document
- Agents: let agents connect (off until you turn it on)

Changes apply at once and are kept in `~/.rustytext/settings`, a small `key = value` text file.

## Agents (MCP)

With "Let agents connect" on in Preferences, RustyText is an [MCP](https://modelcontextprotocol.io)
server while it runs, at `http://127.0.0.1:47823/mcp` (Streamable HTTP). For example, with Claude Code:

    claude mcp add --transport http rustytext http://127.0.0.1:47823/mcp

The agent then works on the documents open in the app, and you see its changes as they happen:

- `list_documents`, `read_document`, `get_selection`
- `open_file`, `new_document`, `open_forever_note`
- `edit_document`, `insert_text`, `set_text` (not saved to disk until `save_document`)
- `save_document`

Only programs on your computer can connect: the server listens on 127.0.0.1 and turns away
requests that come from a web page. There is no password, so any program you run can use it
while it is on.

## Install as an app (macOS)

    scripts/bundle.sh --install

Builds `RustyText.app` (with the icon from `assets/icon.png`) into `/Applications`.
`scripts/bundle.sh --target x86_64-apple-darwin` builds it for Intel Macs instead (and
`aarch64-apple-darwin` for Apple silicon), into `target/RustyText.app`.
The app declares itself an editor of Markdown and plain text files, so the Finder offers it
under *Open with* for `.md` and `.txt` files; Get Info ▸ Open with ▸ Change All… makes it their
default app.
Re-run after code changes. `assets/icon-256.png` is the copy compiled into the binary.
Both PNGs are rendered from `assets/icon.svg`: run `scripts/render-icon.sh` after changing it
(needs Google Chrome).

## Windows build (from macOS)

    rustup target add x86_64-pc-windows-msvc
    cargo install --locked cargo-xwin
    scripts/build-windows.sh

Produces `target/x86_64-pc-windows-msvc/release/rusty-text.exe`, a single file with no
installer. The first run downloads the Windows SDK and asks you to accept Microsoft's license.

## CI and releases

`.github/workflows/rust.yml` tests and builds for every push: macOS for Apple silicon and for
Intel, Windows and Linux. Each binary is attached to the run as an artifact. (The Intel build
is made on an Apple silicon machine, so the tests run for Apple silicon only.)

`.github/workflows/release.yml` builds the downloads of a release when a version tag is pushed,
and attaches them to it: `RustyText.app` for Apple silicon and for Intel, and the Windows and
Linux programs, each with the manual, the license and the notices.

    git tag v0.2.0 && git push origin v0.2.0

## Layout

- `src/main.rs` — parses arguments, opens the window
- `src/title_bar.rs` — the app-drawn title bar on macOS: traffic lights, app name, menus (tested)
- `src/app.rs` — UI: title bar and menus, tabs, editor + line-number gutter, find bar, status bar, dialogs
- `src/open_requests.rs` — files macOS asks the app to open (Finder double-click, Dock drop)
- `src/document.rs` — one open file: load, save, dirty flag
- `src/notes.rs` — your notes file: where it lives and the daily date line (tested)
- `src/settings.rs` — your preferences: theme, language, default folder, and their file (tested)
- `src/mcp.rs` — the MCP server: HTTP and JSON-RPC, hands tool calls to the app (tested)
- `src/app/agent_tools.rs` — the tools agents can call, and what they do to the documents (tested)
- `src/i18n.rs` — the French translation of the interface (tested)
- `src/text_utils.rs` — line/column math, find, replace (pure functions, tested)
- `src/emoji.rs` — the emoji list shown in the Emoji menu
- `src/tools.rs` — Tools menu: case, line and whitespace transforms, word count (tested)
- `src/glow.rs` — Tools ▸ Markdown: renders Markdown like the `glow` terminal app (tested)
- `src/terminal.rs` — Tools ▸ Terminal: your shell in a PTY, drawn from a vt100 screen (tested)

## Limitations

- Menus are drawn inside the window (egui doesn't use the macOS menu bar).
- Case-insensitive find only folds ASCII letters.
- Emojis render in black and white: egui has no colour emoji support.
- The binary is ~14 MB: egui, its renderer and fonts are compiled in.

## License

Copyright © 2026 Olivier Lafont.

This program is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License version 3, as published by the Free Software Foundation. It is
distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See [LICENSE](LICENSE).

Anyone who distributes it, changed or not, must do so under the same license and with its
source code. Third-party code and designs are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
