//! What surrounds the text: the window bar, the ribbon and the status card.
//!
//! - The **window bar** is the dark strip across the top: the app's name, the File menu,
//!   Save / Undo / Redo, the documents' tabs, and at the right end what is about the app
//!   (Preferences, Help, the version).
//! - The **ribbon** holds every command that acts on the document, as tools grouped under
//!   tabs. `TABS` is the whole of it: add a tool there and it shows, with its shortcut in its
//!   tooltip, and the command search finds it.

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Key, Layout, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

use super::{Action, EditorWindow, emoji_grid, menu_item, shortcut_for};
use crate::i18n::{tr, tr_fill};
use crate::look::{self, bar};
use crate::tools::TextTool;
use crate::{emoji, notes, text_utils, title_bar};

pub const BAR_HEIGHT: f32 = title_bar::HEIGHT;
const RIBBON_TAB_ROW: f32 = 30.0;
const TOOL_HEIGHT: f32 = 66.0;
const TOOL_MIN_WIDTH: f32 = 54.0;
const TOOL_LABEL_WIDTH: f32 = 86.0;
const CAPTION_HEIGHT: f32 = 18.0;
pub const MANUAL_URL: &str = "https://github.com/olafont/RustyText/blob/master/docs/USER_MANUAL.md";

// MARK: - The ribbon's contents

/// What a tool does when clicked.
#[derive(Clone, Copy)]
enum Run {
    Action(Action),
    /// Opens the emojis of this category of `emoji::CATEGORIES`.
    Emoji(usize),
}

struct Tool {
    /// A symbol, or a couple of letters, drawn large in the accent colour.
    icon: &'static str,
    label: &'static str,
    run: Run,
}

struct Group {
    name: &'static str,
    tools: &'static [Tool],
    /// The less used commands, in the menu under the group's name.
    more: &'static [Tool],
}

struct Tab {
    name: &'static str,
    groups: &'static [Group],
}

const fn tool(icon: &'static str, label: &'static str, action: Action) -> Tool {
    Tool { icon, label, run: Run::Action(action) }
}

const fn text_tool(icon: &'static str, tool: TextTool) -> Tool {
    Tool { icon, label: tool.label(), run: Run::Action(Action::Tool(tool)) }
}

const fn emojis(icon: &'static str, category: usize) -> Tool {
    Tool { icon, label: emoji::CATEGORIES[category].0, run: Run::Emoji(category) }
}

const TABS: &[Tab] = &[
    Tab {
        name: "Home",
        groups: &[
            Group {
                name: "Clipboard",
                tools: &[
                    tool("✂", "Cut", Action::Cut),
                    tool("🗐", "Copy", Action::Copy),
                    tool("📋", "Paste", Action::Paste),
                    tool("▣", "Select All", Action::SelectAll),
                ],
                more: &[],
            },
            Group {
                name: "Find",
                tools: &[
                    tool("🔍", "Find", Action::Find),
                    tool("🔁", "Replace", Action::FindReplace),
                    tool("⬆", "Previous", Action::FindPrevious),
                    tool("⬇", "Next", Action::FindNext),
                ],
                more: &[tool("🔎", "Use Selection for Find", Action::UseSelectionForFind)],
            },
            Group { name: "Go", tools: &[tool("↳", "Go to Line", Action::GoToLine)], more: &[] },
        ],
    },
    Tab {
        name: "Text",
        groups: &[
            Group {
                name: "Change Case",
                tools: &[text_tool("AA", TextTool::Upper), text_tool("aa", TextTool::Lower), text_tool("Aa", TextTool::Title)],
                more: &[text_tool("a_b", TextTool::Snake), text_tool("aB", TextTool::Camel)],
            },
            Group {
                name: "Lines",
                tools: &[text_tool("↕", TextTool::SortLines), text_tool("🔃", TextTool::ReverseLines)],
                more: &[text_tool("⊟", TextTool::RemoveDuplicateLines), text_tool("☐", TextTool::RemoveBlankLines)],
            },
            Group {
                name: "Whitespace",
                tools: &[text_tool("⏴", TextTool::TrimTrailingWhitespace), text_tool("⇄", TextTool::TabsToSpaces)],
                more: &[text_tool("⇆", TextTool::SpacesToTabs), text_tool("↲", TextTool::NormalizeLineEndings)],
            },
            Group { name: "Count", tools: &[tool("123", "Word Count", Action::WordCount)], more: &[] },
        ],
    },
    Tab {
        name: "Insert",
        groups: &[Group {
            name: "Emoji",
            tools: &[
                emojis("😀", 0),
                emojis("👍", 1),
                emojis("💖", 2),
                emojis("🐶", 3),
                emojis("🍎", 4),
                emojis("💻", 5),
                emojis("🚗", 6),
            ],
            more: &[],
        }],
    },
    Tab {
        name: "View",
        groups: &[
            Group {
                name: "Show",
                tools: &[
                    tool("M↓", "Markdown", Action::ToggleGlow),
                    tool(">_", "Terminal", Action::ToggleTerminal),
                    tool("↩", "Soft Wrap", Action::ToggleSoftWrap),
                ],
                more: &[],
            },
            Group { name: "Notes", tools: &[tool("📝", "Forever Note", Action::OpenNotes)], more: &[] },
            Group { name: "Window", tools: &[tool("🗖", "New Window", Action::NewWindow)], more: &[] },
        ],
    },
];

/// The commands of the File menu, which the command search also finds.
const FILE_COMMANDS: &[(&str, Action)] = &[
    ("New", Action::New),
    ("Open…", Action::Open),
    ("New Window", Action::NewWindow),
    ("Close", Action::CloseTab),
    ("Save", Action::Save),
    ("Save As…", Action::SaveAs),
    ("Preferences…", Action::Preferences),
    ("Quit", Action::Quit),
];

/// The symbols the window bar draws as icons.
const BAR_SAVE: &str = "💾";
const BAR_UNDO: &str = "⟲";
const BAR_REDO: &str = "⟳";

/// The interface texts that reach `tr` through the tables above (for the translation test).
#[cfg(test)]
pub fn texts() -> Vec<&'static str> {
    let mut texts: Vec<&'static str> = FILE_COMMANDS.iter().map(|(label, _)| *label).collect();
    for tab in TABS {
        texts.push(tab.name);
        for group in tab.groups {
            texts.push(group.name);
            texts.extend(group.tools.iter().chain(group.more).map(|tool| tool.label));
        }
    }
    texts
}

/// The ribbon's own state: the tab that shows, and the command search.
#[derive(Default)]
pub struct Ribbon {
    tab: usize,
    search: String,
    search_open: bool,
    /// Put the keyboard in the search field on the next frame (⇧⌘P).
    pub focus_search: bool,
}

impl EditorWindow {
    // MARK: - Window bar

    /// The dark strip across the top of the window.
    pub(super) fn window_bar(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let visuals = ui.visuals_mut();
        visuals.override_text_color = Some(bar::TEXT);
        visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        visuals.widgets.hovered.weak_bg_fill = bar::PILL;
        visuals.widgets.active.weak_bg_fill = bar::PILL;
        visuals.widgets.open.weak_bg_fill = bar::PILL;
        visuals.widgets.active.bg_stroke = Stroke::NONE;
        ui.spacing_mut().item_spacing.x = 4.0;

        // The app's logo and name.
        let logo = self.logo.get_or_insert_with(|| {
            let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icon-256.png")).expect("bundled icon is a valid PNG");
            let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
            ui.ctx().load_texture("logo", image, egui::TextureOptions::LINEAR)
        });
        ui.add(egui::Image::new((logo.id(), vec2(18.0, 18.0))));
        ui.label(egui::RichText::new("RustyText").font(look::semibold(look::LARGE)));
        ui.add_space(4.0);
        look::divider(ui, 16.0, bar::DIVIDER);

        // The only menu: what concerns files and the app.
        ui.menu_button(egui::RichText::new(tr("File")).size(look::TAB), |ui| {
            ui.visuals_mut().override_text_color = None;
            for (index, (label, action)) in FILE_COMMANDS.iter().enumerate() {
                if matches!(index, 3 | 6 | 7) {
                    ui.separator();
                }
                menu_item(ui, tr(label), *action, actions);
            }
        });
        for (icon, label, action) in [(BAR_SAVE, "Save", Action::Save), (BAR_UNDO, "Undo", Action::Undo), (BAR_REDO, "Redo", Action::Redo)] {
            if bar_icon(ui, icon, &tip(ui, tr(label), action)).clicked() {
                actions.push(action);
            }
        }
        ui.add_space(4.0);
        look::divider(ui, 16.0, bar::DIVIDER);
        ui.add_space(4.0);

        // The documents' tabs, in what is left before the right end.
        const RIGHT_END: f32 = 132.0;
        let mut close = None;
        let scroll_to_active = std::mem::replace(&mut self.bar_tab_shown, self.active) != self.active;
        egui::ScrollArea::horizontal()
            .id_salt(self.ui_id("bar_tabs"))
            .max_width((ui.available_width() - RIGHT_END).max(60.0))
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for index in 0..self.docs.len() {
                        let doc = &self.docs[index];
                        // The Forever Note's tab is just "fn", whatever its file is called.
                        let title = if doc.notes { "fn".to_owned() } else { doc.title() };
                        let title = format!("{}{}", if doc.shows_unsaved_mark() { "• " } else { "" }, title);
                        let tab = document_tab(ui, &title, index == self.active);
                        if tab.close {
                            close = Some(index);
                        } else if tab.response.clicked() {
                            self.active = index;
                        }
                        if index == self.active && scroll_to_active {
                            tab.response.scroll_to_me(None);
                        }
                    }
                    if bar_icon(ui, "+", &tip(ui, tr("New"), Action::New)).clicked() {
                        actions.push(Action::New);
                    }
                });
            });
        if let Some(index) = close {
            self.request_close(index);
        }

        // The right end: what is about the app rather than the document.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).size(look::VERY_SMALL).color(bar::DIM));
            ui.add_space(4.0);
            if bar_icon(ui, "?", tr("Help")).clicked() {
                actions.push(Action::Help);
            }
            if bar_icon(ui, "⚙", &tip(ui, tr("Preferences"), Action::Preferences)).clicked() {
                actions.push(Action::Preferences);
            }
        });
    }

    // MARK: - Ribbon

    /// The card under the window bar: the tabs and the command search, then the tools.
    pub(super) fn ribbon(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let palette = look::palette(ui);
        look::card(ui).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            let tab_row = ui.horizontal(|ui| {
                ui.set_height(RIBBON_TAB_ROW - 4.0);
                for (index, tab) in TABS.iter().enumerate() {
                    if ribbon_tab(ui, tr(tab.name), index == self.ribbon.tab).clicked() {
                        self.ribbon.tab = index;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| self.command_search(ui, actions));
            });
            let line = tab_row.response.rect.bottom() + 1.0;
            ui.painter().hline(ui.max_rect().x_range(), line, Stroke::new(1.0, palette.divider));
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                for group in TABS[self.ribbon.tab.min(TABS.len() - 1)].groups {
                    self.ribbon_group(ui, group, actions);
                    ui.add_space(4.0);
                    look::divider(ui, TOOL_HEIGHT + CAPTION_HEIGHT - 8.0, palette.divider);
                    ui.add_space(4.0);
                }
            });
        });
    }

    fn ribbon_group(&self, ui: &mut Ui, group: &Group, actions: &mut Vec<Action>) {
        let palette = look::palette(ui);
        ui.vertical(|ui| {
            let tools = ui.horizontal(|ui| {
                for tool in group.tools {
                    let (enabled, checked) = self.tool_state(tool);
                    let response = ribbon_tool(ui, tool, enabled, checked);
                    match tool.run {
                        Run::Action(action) => {
                            if response.on_hover_text(tip(ui, tr(tool.label), action)).clicked() {
                                actions.push(action);
                            }
                        }
                        Run::Emoji(category) => {
                            egui::Popup::menu(&response).show(|ui| emoji_grid(ui, emoji::CATEGORIES[category].1, actions));
                        }
                    }
                }
            });

            // The group's name, centred under its tools; it opens the less used commands.
            let width = tools.response.rect.width();
            let has_more = !group.more.is_empty();
            let sense = if has_more { Sense::click() } else { Sense::hover() };
            let (rect, response) = ui.allocate_exact_size(vec2(width, CAPTION_HEIGHT), sense);
            let color = if has_more && response.hovered() { palette.text } else { palette.dim };
            let text = ui.painter().layout_no_wrap(tr(group.name).to_owned(), look::regular(look::SMALL), color);
            let chevron_room = if has_more { 12.0 } else { 0.0 };
            let left = rect.center().x - (text.size().x + chevron_room) / 2.0;
            let text_width = text.size().x;
            ui.painter().galley(pos2(left, rect.center().y - text.size().y / 2.0), text, color);
            if has_more {
                look::chevron_down(ui, pos2(left + text_width + 8.0, rect.center().y + 1.0), color);
                egui::Popup::menu(&response).show(|ui| {
                    for tool in group.more {
                        if let Run::Action(action) = tool.run {
                            menu_item(ui, tr(tool.label), action, actions);
                        }
                    }
                });
            }
        });
    }

    /// Whether a tool can be used now, and whether what it switches is on.
    fn tool_state(&self, tool: &Tool) -> (bool, bool) {
        match tool.run {
            Run::Action(Action::ToggleGlow) => (self.is_markdown(), self.glow_visible()),
            Run::Action(Action::ToggleTerminal) => (true, self.terminal_visible),
            Run::Action(Action::ToggleSoftWrap) => (true, self.soft_wrap),
            Run::Action(Action::OpenNotes) => (!self.notes_open && notes::path().is_some(), false),
            _ => (true, false),
        }
    }

    /// The search field at the right end of the ribbon's tabs, and the commands it finds.
    fn command_search(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let palette = look::palette(ui);
        // Laid out right to left: the key cap first, then the field.
        let shortcut = shortcut_for(Action::CommandSearch).map(|shortcut| ui.ctx().format_shortcut(&shortcut)).unwrap_or_default();
        egui::Frame::new()
            .stroke(Stroke::new(1.0, palette.card_border))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(egui::Margin::symmetric(4, 1))
            .show(ui, |ui| ui.label(egui::RichText::new(shortcut).size(look::VERY_SMALL).color(palette.dim)));
        let field_id = self.ui_id("command_search");
        let field = ui.add(
            egui::TextEdit::singleline(&mut self.ribbon.search)
                .id(field_id)
                .hint_text(format!("🔍 {}", tr("Search commands")))
                .desired_width(190.0),
        );
        if std::mem::take(&mut self.ribbon.focus_search) {
            field.request_focus();
        }
        if field.gained_focus() || field.changed() {
            self.ribbon.search_open = true;
        }
        if !self.ribbon.search_open || self.ribbon.search.trim().is_empty() {
            return;
        }

        let found = find_commands(self.ribbon.search.trim());
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let escape = ui.input(|i| i.key_pressed(Key::Escape));
        let mut chosen = if enter { found.first().map(|(_, action)| *action) } else { None };
        let list = egui::Area::new(self.ui_id("command_search_list"))
            .order(egui::Order::Foreground)
            .fixed_pos(field.rect.left_bottom() + vec2(0.0, 4.0))
            .show(ui.ctx(), |ui| {
                look::card(ui).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                    ui.set_width(field.rect.width().max(230.0));
                    if found.is_empty() {
                        ui.label(egui::RichText::new(tr("No command found")).color(palette.dim));
                    }
                    for (label, action) in found.iter().take(8) {
                        let shortcut = shortcut_for(*action).map(|shortcut| ui.ctx().format_shortcut(&shortcut)).unwrap_or_default();
                        let item = egui::Button::new(*label).shortcut_text(shortcut).frame_when_inactive(false);
                        if ui.add_sized([ui.available_width(), 22.0], item).clicked() {
                            chosen = Some(*action);
                        }
                    }
                });
            });
        let clicked_elsewhere = ui.input(|i| i.pointer.any_click())
            && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !list.response.rect.contains(pos) && !field.rect.contains(pos));
        if let Some(action) = chosen {
            actions.push(action);
            self.ribbon.search.clear();
        }
        if chosen.is_some() || escape || enter || clicked_elsewhere {
            self.ribbon.search_open = false;
        }
    }

    // MARK: - Status card

    /// The card at the bottom of the window: where the cursor is, and how long the text is.
    pub(super) fn status_card(&mut self, ui: &mut Ui) {
        let palette = look::palette(ui);
        look::card(ui).inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let doc = self.doc();
            let (line, column) = text_utils::line_col(&doc.text, doc.selection.end);
            let lines = text_utils::line_count(&doc.text);
            let chars = doc.text.chars().count();
            let numbers = [line, column, lines, chars].map(|number| number.to_string());
            let status = tr_fill("L: {}  C: {}    {} lines  {} chars    UTF-8", &numbers.each_ref().map(String::as_str));
            ui.label(egui::RichText::new(status).monospace().size(look::SMALL).color(palette.dim));
        });
    }
}

// MARK: - Widgets

/// A label with its keyboard shortcut, for a tooltip: "Save (⌘S)".
fn tip(ui: &Ui, label: &str, action: Action) -> String {
    let label = label.trim_end_matches('…');
    match shortcut_for(action) {
        Some(shortcut) => format!("{label} ({})", ui.ctx().format_shortcut(&shortcut)),
        None => label.to_owned(),
    }
}

/// The commands whose name contains `query`, whatever the case, those starting with it first.
fn find_commands(query: &str) -> Vec<(&'static str, Action)> {
    let query = query.to_lowercase();
    let mut found: Vec<(&'static str, Action)> = Vec::new();
    let tools = TABS.iter().flat_map(|tab| tab.groups).flat_map(|group| group.tools.iter().chain(group.more));
    let tools = tools.filter_map(|tool| match tool.run {
        Run::Action(action) => Some((tr(tool.label), action)),
        Run::Emoji(_) => None,
    });
    for (label, action) in FILE_COMMANDS.iter().map(|(label, action)| (tr(label), *action)).chain(tools) {
        if label.to_lowercase().contains(&query) && !found.iter().any(|(_, other)| *other == action) {
            found.push((label, action));
        }
    }
    found.sort_by_key(|(label, _)| !label.to_lowercase().starts_with(&query));
    found
}

/// An icon button of the window bar.
fn bar_icon(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 5.0, bar::PILL);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, look::regular(15.0), bar::TEXT);
    response.on_hover_text(tooltip)
}

struct DocumentTab {
    response: Response,
    /// Its × was clicked.
    close: bool,
}

/// A document's tab in the window bar: the active one bold on a pill, with a × to close it.
fn document_tab(ui: &mut Ui, title: &str, active: bool) -> DocumentTab {
    let font = if active { look::semibold(look::TAB) } else { look::regular(look::TAB) };
    let text = ui.painter().layout_no_wrap(title.to_owned(), font, bar::TEXT);
    let close_room = if active { 20.0 } else { 0.0 };
    let size = vec2(text.size().x.min(220.0) + 20.0 + close_room, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let color = if active || response.hovered() { bar::TEXT } else { bar::DIM };
    if active {
        ui.painter().rect_filled(rect, 6.0, bar::PILL);
    }
    let text_clip = Rect::from_min_max(rect.min, pos2(rect.right() - close_room - 6.0, rect.bottom()));
    ui.painter().with_clip_rect(text_clip.intersect(ui.clip_rect())).galley_with_override_text_color(
        pos2(rect.left() + 10.0, rect.center().y - text.size().y / 2.0),
        text,
        color,
    );
    let mut close = false;
    if active {
        let cross = Rect::from_center_size(pos2(rect.right() - 14.0, rect.center().y), vec2(16.0, 16.0));
        let cross_response = ui.interact(cross, response.id.with("close"), Sense::click()).on_hover_text(tr("Close"));
        if cross_response.hovered() {
            ui.painter().rect_filled(cross, 4.0, bar::DIVIDER);
        }
        ui.painter().text(cross.center(), Align2::CENTER_CENTER, "×", look::regular(look::LARGE), bar::TEXT);
        close = cross_response.clicked();
    }
    DocumentTab { response: response.on_hover_text(title), close }
}

/// A tab of the ribbon: plain text, the selected one in the accent colour on a soft pill.
fn ribbon_tab(ui: &mut Ui, name: &str, selected: bool) -> Response {
    let palette = look::palette(ui);
    let font = if selected { look::medium(look::TAB) } else { look::regular(look::TAB) };
    let text = ui.painter().layout_no_wrap(name.to_owned(), font, palette.text);
    let (rect, response) = ui.allocate_exact_size(vec2(text.size().x + 22.0, 24.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 6.0, palette.accent_soft);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, palette.hover);
    }
    let color = if selected { palette.accent } else { palette.text };
    ui.painter().galley_with_override_text_color(rect.center() - text.size() / 2.0, text, color);
    response
}

/// A tool of the ribbon: a large icon over its name, on one line or two.
fn ribbon_tool(ui: &mut Ui, tool: &Tool, enabled: bool, checked: bool) -> Response {
    let palette = look::palette(ui);
    let mut job = egui::text::LayoutJob::simple(tr(tool.label).to_owned(), look::regular(look::MEDIUM), palette.text, TOOL_LABEL_WIDTH);
    job.halign = Align::Center;
    let label = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let width = (label.size().x + 12.0).max(TOOL_MIN_WIDTH);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(width, TOOL_HEIGHT), sense);
    if checked {
        ui.painter().rect_filled(rect, 6.0, palette.accent_soft);
    } else if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 6.0, palette.hover);
    }
    let (icon_color, label_color) = if enabled { (palette.accent, palette.text) } else { (palette.divider, palette.dim) };
    // Letters ("AA", ">_") are drawn in the interface font, smaller than the symbols.
    let lettered = tool.icon.chars().all(|c| c.is_ascii() || c == '↓');
    let font = if lettered { look::semibold(16.0) } else { look::regular(22.0) };
    ui.painter().text(pos2(rect.center().x, rect.top() + 17.0), Align2::CENTER_CENTER, tool.icon, font, icon_color);
    ui.painter().galley_with_override_text_color(pos2(rect.center().x, rect.top() + 31.0), label, label_color);
    response
}

/// The window bar's frame and place: on macOS it is the title bar, with the traffic lights at
/// its left end; elsewhere it sits under the OS title bar.
pub fn bar_contents<R>(ui: &mut Ui, drag_id: egui::Id, contents: impl FnOnce(&mut Ui) -> R) -> R {
    let bar = ui.max_rect();
    if title_bar::integrated() {
        title_bar::show(ui, bar, drag_id, contents)
    } else {
        let inner = Rect::from_min_max(pos2(bar.left() + 10.0, bar.top()), bar.right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)), contents).inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_can_be_drawn() {
        let ctx = egui::Context::default();
        look::install(&ctx);
        let mut icons = vec![BAR_SAVE, BAR_UNDO, BAR_REDO, "⚙", "?", "+", "×", "🔍"];
        let tools = TABS.iter().flat_map(|tab| tab.groups).flat_map(|group| group.tools.iter().chain(group.more));
        icons.extend(tools.map(|tool| tool.icon));
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let missing: Vec<char> = icons
                .iter()
                .flat_map(|icon| icon.chars())
                .filter(|&c| !ui.fonts_mut(|fonts| fonts.has_glyph(&look::regular(look::LARGE), c)))
                .collect();
            assert!(missing.is_empty(), "no glyph for {missing:?}");
        });
        output.textures_delta.clear(); // nothing draws them offscreen
    }

    #[test]
    fn the_command_search_finds_tools_and_file_commands() {
        let labels = |query: &str| find_commands(query).into_iter().map(|(label, _)| label).collect::<Vec<_>>();
        assert_eq!(labels("sort"), ["Sort Lines"]);
        assert_eq!(labels("SAVE"), ["Save", "Save As…"]);
        assert_eq!(labels("go")[0], "Go to Line", "a name that starts with the text comes first");
        assert!(labels("zzz").is_empty());
        assert!(labels("smileys").is_empty(), "emoji categories are not commands");
    }

    #[test]
    fn every_tool_has_a_place_and_a_name() {
        for tab in TABS {
            assert!(!tab.groups.is_empty(), "{} has groups", tab.name);
            for group in tab.groups {
                assert!(!group.tools.is_empty(), "{} has tools", group.name);
                assert!(group.tools.len() <= 7, "{} fits in the ribbon", group.name);
            }
        }
        assert!(texts().contains(&"Word Count"));
    }
}
