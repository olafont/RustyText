# RustyText: user manual

RustyText is a small text editor for plain text files. It opens several files in tabs, finds
and replaces, tidies text (case, lines, whitespace), shows Markdown rendered beside what you
type, runs your shell in a pane under the editor, and keeps one note that is always there:
the Forever Note. AI agents can work on your open documents too, if you let them.

It runs on macOS, Windows and Linux. This manual writes shortcuts with ⌘, the macOS Command
key; on Windows and Linux use Ctrl instead.

**Contents**

1. [Getting started](#1-getting-started)
2. [The window](#2-the-window)
3. [Files](#3-files)
4. [Editing](#4-editing)
5. [Find and replace](#5-find-and-replace)
6. [Text tools](#6-text-tools)
7. [The Markdown pane](#7-the-markdown-pane)
8. [The terminal](#8-the-terminal)
9. [The Forever Note](#9-the-forever-note)
10. [Preferences](#10-preferences)
11. [Agents (MCP)](#11-agents-mcp)
12. [Keyboard shortcuts](#12-keyboard-shortcuts)
13. [Where RustyText keeps things](#13-where-rustytext-keeps-things)
14. [Limits](#14-limits)
15. [License](#15-license)

## 1. Getting started

### Install

RustyText is built from its source with [Rust](https://rustup.rs).

**macOS.** In the project folder:

    scripts/bundle.sh --install

This builds `RustyText.app` and copies it to `/Applications`. Run it again after updating
the source.

**Windows and Linux.** Build the program, then run it or put it wherever you keep programs:

    cargo build --release

The program is `target/release/rusty-text` (`rusty-text.exe` on Windows): a single file, with
no installer. On Linux the build needs a few system libraries first; the list is in
`.github/workflows/rust.yml`.

### First run

Open RustyText. It starts with one empty document, called *untitled text*, and the cursor in
it: type.

To open files as it starts, give them on the command line:

    rusty-text notes.txt src/main.rs

A file that does not exist yet opens empty and is created the first time you save it.

## 2. The window

From top to bottom:

- **Title bar** (macOS). After the red, yellow and green buttons come the app's name and the
  menus: File, Edit, Find, Text and Tools. At the right end are the name of the document you
  are in, with a • in front while it has unsaved changes, and the ⚙ button that opens
  [Preferences](#10-preferences). Drag an empty part of the bar to move the window;
  double-click it to zoom. On Windows and Linux the system's own title bar stays, and the
  menus are in a bar just under it.
- **Tabs.** One per open document. Click a tab to switch to it, × to close it. A • marks
  unsaved changes.
- **Find bar**, while you are searching ([Find and replace](#5-find-and-replace)).
- **Editor.** Your text, in a fixed-width font, with line numbers on the left.
- **Markdown pane**, on the right, when it is on ([The Markdown pane](#7-the-markdown-pane)).
- **Terminal**, under the editor, when it is on ([The terminal](#8-the-terminal)).
- **Status bar.** The cursor's line (L) and column (C), the number of lines and characters in
  the document, and its encoding.

### More than one window

File ▸ New Window (⇧⌘N) opens another window with its own tabs, find bar, Markdown pane and
terminal. A file is open in one tab at a time within a window.

Closing a window closes its tabs, asking about each one that has unsaved changes. The first
window is the app's main window: closing it quits RustyText, after a warning if other windows
are open. File ▸ Quit (⌘Q) does the same from any window.

## 3. Files

| To | Do |
| --- | --- |
| Start a new document | File ▸ New (⌘N) |
| Open files | File ▸ Open… (⌘O), and pick one or several; or drag files onto the window |
| Save | File ▸ Save (⌘S) |
| Save under another name | File ▸ Save As… (⇧⌘S) |
| Close the tab | File ▸ Close (⌘W), or the tab's × |

Opening a file that is already open switches to its tab. Opening a file while the only tab is
an untouched empty document replaces that tab.

Closing a tab with unsaved changes asks whether to **Save**, **Don't Save** or **Cancel**.
Cancel also stops a quit that was under way.

Saving writes to a temporary file first and then puts it in place of the old one, so a save
that fails never leaves you with half a file.

The Open panel, and the Save panel of a document that has never been saved, start in your
[default folder](#10-preferences) if you set one.

## 4. Editing

Type, select with the mouse or with Shift and the arrow keys, and use the Edit menu or the
usual shortcuts: Undo (⌘Z), Redo (⇧⌘Z), Cut (⌘X), Copy (⌘C), Paste (⌘V), Select All (⌘A).
Everything RustyText does to your text, including the text tools and what agents change, can
be undone.

**Right-click** in the text for a menu of the commands that act on the selection: Cut, Copy,
Paste, Select All, Use Selection for Find, the text tools and Word Count. Right-clicking keeps
the selection.

**Emoji.** Edit ▸ Emoji shows emoji by category (Smileys, Gestures, Symbols, Animals & Nature,
Food & Drink, Objects, Travel & Places). Click one to insert it at the cursor. They are drawn
in one colour in the editor; the file holds the real emoji.

**Go to a line.** Text ▸ Go to Line… (⌘J), type the line number, press Return. The line is
selected and scrolled into view.

**Soft wrap.** Text ▸ Soft Wrap folds long lines at the window's edge instead of scrolling
sideways. Folded lines keep one line number. It is off when a window opens.

## 5. Find and replace

Find ▸ Find… (⌘F) opens the find bar above the editor.

- Type what to look for and press Return, or click ▶, to select the next match; ◀ selects the
  previous one. Without the bar: Find Next (⌘G) and Find Previous (⇧⌘G).
- The search wraps around: after the last match comes the first.
- **Match case** makes capitals matter. Otherwise `note` also finds `Note` and `NOTE`.
- Find ▸ Use Selection for Find (⌘E) puts the selected text in the find field.
- If nothing matches, the selection stays where it is and the app asks for your attention
  (on macOS, its Dock icon bounces once).
- **Done**, or Esc, closes the bar.

Find ▸ Find & Replace… (⌥⌘F), or **Replace…** in the bar, adds a second row:

- **Replace** replaces the selected match with the text in *With*, and selects the next one.
- **Replace All** replaces every match in the document at once.

## 6. Text tools

The Tools menu changes the selected text, or the whole document when nothing is selected.

**Change Case**

| Tool | `the quick brown fox` becomes |
| --- | --- |
| UPPERCASE | `THE QUICK BROWN FOX` |
| lowercase | `the quick brown fox` |
| Title Case | `The Quick Brown Fox` |
| snake_case | `the_quick_brown_fox` |
| camelCase | `theQuickBrownFox` |

snake_case and camelCase work line by line, and split words at spaces, punctuation and
capital letters, so `helloWorld foo-bar` becomes `hello_world_foo_bar`.

**Lines**

| Tool | What it does |
| --- | --- |
| Sort Lines | Sorts the lines alphabetically, ignoring capitals |
| Reverse Lines | Puts the last line first |
| Remove Duplicate Lines | Keeps the first of each identical line |
| Remove Blank Lines | Removes lines that are empty or hold only spaces |

**Whitespace**

| Tool | What it does |
| --- | --- |
| Trim Trailing Whitespace | Removes spaces and tabs at the ends of lines |
| Tabs to Spaces | Replaces each tab with spaces, to tab stops every 4 columns |
| Leading Spaces to Tabs | Replaces each 4 spaces at the start of a line with a tab |
| Normalize Line Endings (LF) | Turns Windows (CRLF) and old Mac (CR) line endings into LF |

**Word Count.** Tools ▸ Word Count… counts words, characters, characters without spaces, lines
and paragraphs, in the selection if there is one, otherwise in the document. A paragraph is a
run of lines with no blank line between them.

## 7. The Markdown pane

Tools ▸ Markdown shows the document rendered, in a pane on the right of the editor, in the
style of the `glow` terminal app: headings, bold and italics, lists, quotes, code, tables and
links, on a dark background. It follows what you type.

- It is available for Markdown files: those ending in `.md`, `.markdown`, `.mdown` or `.mkd`.
  For other files the menu item is greyed out. Save a new document under such a name first.
- The pane starts at half the window's width. Drag its left edge to resize it.
- Images are not shown; their place and address are.
- Choose Tools ▸ Markdown again to hide it. While it is on, it shows whenever you switch to a
  Markdown tab and hides for the others.

## 8. The terminal

Tools ▸ Terminal (⌃`, with the Control key on every system) opens your shell in a pane under
the editor: your login shell on macOS and Linux, `cmd.exe` on Windows.

- The shell starts in the folder of the document you are in. For a document that has never
  been saved, it starts in your [default folder](#10-preferences), or your home folder.
- Click in the pane to type in it. Scroll up to see earlier output; typing brings you back
  to the bottom. Paste works as in the editor.
- Drag the pane's top edge to resize it. It opens five lines tall.
- ⌃` again hides the pane. The shell keeps running, with its history and whatever it was
  doing, until you close the window.
- When the shell ends (`exit`), the pane closes. Opening it again starts a new shell.

Each window has its own terminal.

## 9. The Forever Note

The Forever Note is one note that is always there for you: a running log, kept in a single
file. Open it with Tools ▸ Forever Note. Its tab is called **fn** and is always the leftmost.

- The first time you open it each day, a line with the date is added at the end
  (`--- Saturday, October 24, 2026 ---`), so each day's notes sit under their own heading.
- The cursor goes to the end, ready to type.
- It saves itself: a second after each change, and when its tab or the app closes. It never
  asks, and never shows the unsaved mark.
- It is open in one window at a time. While it is open somewhere, the menu item is hidden;
  go to its **fn** tab.

The note is an ordinary text file, at `~/.rustytext/notes`. You can read it, back it up or
search it with any other tool.

## 10. Preferences

Click ⚙ at the right end of the title bar, or choose File ▸ Preferences… (⌘,).

| Setting | Choices |
| --- | --- |
| Theme | **System** follows your system's light or dark mode; **Light**; **Dark** |
| Language | **English** or **Français**, for the menus, dialogs and messages |
| Default folder | Where the Open and Save panels start, and the terminal for an unsaved document. **Choose…** picks a folder; **Clear** goes back to none |
| Agents | **Let agents connect (MCP)**: see [Agents](#11-agents-mcp). Off until you turn it on |

Changes apply as you make them and are remembered. **Done**, Esc or a click outside closes
the dialog.

The Markdown pane and the terminal keep their own dark background in the light theme, like
the terminal apps they resemble.

## 11. Agents (MCP)

RustyText can let AI agents, such as Claude Code, read and change the documents you have
open, through the [Model Context Protocol](https://modelcontextprotocol.io) (MCP). You watch
their changes appear in the editor as they make them, and can undo them like your own.

### Turn it on

1. Open Preferences and tick **Let agents connect (MCP)**. The address appears under it:
   `http://127.0.0.1:47823/mcp`.
2. Tell your agent about that address. With Claude Code:

       claude mcp add --transport http rustytext http://127.0.0.1:47823/mcp

RustyText answers agents only while it is running. If the address is replaced by a message in
red, another program is using port 47823: quit it, then untick and tick the box again.

### What agents can do

| Tool | What it does |
| --- | --- |
| `list_documents` | Lists the open documents in every window: name, file, unsaved changes |
| `read_document` | Reads a document as it is in the editor, unsaved changes included; all of it or a range of lines |
| `get_selection` | Reads what you have selected, and where the cursor is |
| `open_file` | Opens a file in a new tab of the main window |
| `new_document` | Opens a new untitled tab, with text in it or not |
| `open_forever_note` | Opens the Forever Note |
| `edit_document` | Replaces one exact piece of text with another |
| `insert_text` | Inserts text at the cursor, at the start or at the end |
| `set_text` | Replaces a document's whole text |
| `save_document` | Saves a document, under a new name if asked |

An agent's edits are **not saved** until it calls `save_document` or you save: the tab shows
the unsaved mark meanwhile. The Forever Note is the exception, as it saves itself.

A tool that would need you to answer something, such as saving an untitled document without
a name, fails with a message to the agent instead of opening a panel.

### Who can connect

- Only programs running on your computer. RustyText listens on `127.0.0.1`, which other
  computers cannot reach.
- Not web pages. A page in your browser could try that address; such requests are refused.
- There is **no password**. While the setting is on, any program you run can read and change
  your open documents and save them. Turn it off when you are not using it.

## 12. Keyboard shortcuts

On Windows and Linux, read ⌘ as Ctrl and ⌥ as Alt.

| Shortcut | Command |
| --- | --- |
| ⌘N | New document |
| ⇧⌘N | New window |
| ⌘O | Open… |
| ⌘S | Save |
| ⇧⌘S | Save As… |
| ⌘W | Close the tab |
| ⌘Q | Quit |
| ⌘, | Preferences… |
| ⌘Z / ⇧⌘Z | Undo / Redo |
| ⌘X / ⌘C / ⌘V | Cut / Copy / Paste |
| ⌘A | Select All |
| ⌘F | Find… |
| ⌥⌘F | Find & Replace… |
| ⌘G / ⇧⌘G | Find Next / Find Previous |
| ⌘E | Use Selection for Find |
| ⌘J | Go to Line… |
| ⌃` | Show or hide the terminal |
| Esc | Close the find bar or a dialog |

The shortcuts are also shown beside each command in the menus.

## 13. Where RustyText keeps things

Both files are in the `.rustytext` folder of your home folder, and are plain text.

| File | Holds |
| --- | --- |
| `~/.rustytext/notes` | The Forever Note |
| `~/.rustytext/settings` | Your preferences, one `key = value` per line |

A settings file looks like this:

    theme = dark
    language = fr
    agents = off
    default_folder = /Users/you/Documents

You can edit it while RustyText is closed. Lines it does not understand are ignored. Delete
the file to go back to the defaults.

RustyText keeps nothing else: no list of recent files, and no memory of which tabs or windows
were open.

## 14. Limits

- **Plain text in UTF-8 only.** A file in another encoding, or one that is not text, is
  refused with a message rather than opened wrongly.
- **Finding without Match case** treats `a`–`z` and `A`–`Z` as the same letters, but not
  accented ones: `é` does not find `É`.
- **Emoji are drawn in one colour.**
- **The Markdown pane does not show images.**
- **Menus are inside the window**, on macOS too. The system menu bar only holds the standard
  application menu (About, Hide, Quit).
- **The window layout is not remembered**: soft wrap, the Markdown pane, the terminal and the
  pane sizes start afresh in each window.

## 15. License

Copyright © 2026 Olivier Lafont.

RustyText is free software, under the GNU General Public License version 3: see
[LICENSE](../LICENSE). It comes with no warranty. The works it is built from are listed in
[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).
