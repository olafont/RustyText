//! Renders Markdown the way Charm's `glow` terminal app does, using the colours
//! of its default dark style (glamour's "dark" theme).
//!
//! pulldown-cmark turns the Markdown into a stream of events (`Start(Heading)`,
//! `Text("Hello")`, `End(Heading)`, …). `Renderer` walks that stream, keeping track
//! of the current style and nesting, and produces a list of `Block`s to draw.

use std::path::Path;

use eframe::egui::{self, Color32, FontId, Stroke};
use egui::text::{LayoutJob, TextFormat};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// glamour's dark theme uses xterm-256 colour numbers; these are their RGB values.
mod palette {
    use eframe::egui::Color32;

    pub const BACKGROUND: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1c);
    pub const TEXT: Color32 = Color32::from_rgb(0xd0, 0xd0, 0xd0); // 252
    pub const BOLD: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
    pub const H1_FG: Color32 = Color32::from_rgb(0xff, 0xff, 0x87); // 228
    pub const H1_BG: Color32 = Color32::from_rgb(0x5f, 0x5f, 0xff); // 63
    pub const HEADING: Color32 = Color32::from_rgb(0x00, 0xaf, 0xff); // 39
    pub const H6: Color32 = Color32::from_rgb(0x00, 0xaf, 0x5f); // 35
    pub const CODE_FG: Color32 = Color32::from_rgb(0xff, 0x5f, 0x5f); // 203
    pub const CODE_BG: Color32 = Color32::from_rgb(0x30, 0x30, 0x30); // 236
    pub const CODE_BLOCK_BG: Color32 = Color32::from_rgb(0x26, 0x26, 0x26); // 235
    pub const LINK: Color32 = Color32::from_rgb(0x00, 0x87, 0x87); // 30
    pub const LINK_TEXT: Color32 = Color32::from_rgb(0x00, 0xaf, 0x5f); // 35
    pub const DIM: Color32 = Color32::from_rgb(0x58, 0x58, 0x58); // 240
}

const FONT_SIZE: f32 = 14.0;

pub fn is_markdown(path: Option<&Path>) -> bool {
    path.and_then(Path::extension)
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ["md", "markdown", "mdown", "mkd"].contains(&ext.to_ascii_lowercase().as_str()))
}

/// Draws `markdown` filling the available space, on a terminal-like dark background.
/// The resizable pane on the right that shows the preview, starting at half of `available_width`.
pub fn panel(id: egui::Id, available_width: f32) -> egui::Panel {
    egui::Panel::right(id)
        .resizable(true)
        .default_size(available_width / 2.0)
        .min_size(200.0)
        .frame(egui::Frame::NONE)
}

pub fn show(ui: &mut egui::Ui, markdown: &str) {
    let font = FontId::monospace(FONT_SIZE);
    let char_width = ui.painter().layout_no_wrap("0".into(), font.clone(), Color32::WHITE).size().x;
    let blank_line = FONT_SIZE * 1.2;

    egui::Frame::new()
        .fill(palette::BACKGROUND)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                for (index, block) in parse(markdown, &font).into_iter().enumerate() {
                    if block.gap_before && index > 0 {
                        ui.add_space(blank_line);
                    }
                    ui.horizontal_top(|ui| {
                        ui.add_space(block.indent as f32 * char_width);
                        draw_block(ui, block.kind, &font, index);
                    });
                }
            });
        });
}

fn draw_block(ui: &mut egui::Ui, kind: BlockKind, font: &FontId, index: usize) {
    match kind {
        BlockKind::Text(job) => {
            ui.add(egui::Label::new(job).wrap());
        }
        BlockKind::Code(code) => {
            egui::Frame::new()
                .fill(palette::CODE_BLOCK_BG)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    egui::ScrollArea::horizontal().id_salt(("glow_code", index)).show(ui, |ui| {
                        let text = egui::RichText::new(code).font(font.clone()).color(palette::TEXT);
                        ui.add(egui::Label::new(text).extend());
                    });
                });
        }
        BlockKind::Rule => {
            ui.label(egui::RichText::new("--------").font(font.clone()).color(palette::DIM));
        }
        BlockKind::Table(rows) => {
            // Wide tables scroll sideways instead of widening the pane.
            egui::ScrollArea::horizontal().id_salt(("glow_table", index)).show(ui, |ui| {
                let text = egui::RichText::new(format_table(&rows)).font(font.clone()).color(palette::TEXT);
                ui.add(egui::Label::new(text).extend());
            });
        }
    }
}

/// Lays a table out as text, like a terminal would: padded columns and box-drawing lines.
fn format_table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|c| rows.iter().filter_map(|row| row.get(c)).map(|cell| cell.chars().count()).max().unwrap_or(0))
        .collect();
    let line = |row: &Vec<String>| -> String {
        widths
            .iter()
            .enumerate()
            .map(|(c, width)| {
                let cell = row.get(c).map_or("", String::as_str);
                format!("{cell}{}", " ".repeat(width - cell.chars().count()))
            })
            .collect::<Vec<_>>()
            .join(" │ ")
    };
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        out.push(line(row));
        if i == 0 {
            out.push(widths.iter().map(|w| "─".repeat(*w)).collect::<Vec<_>>().join("─┼─"));
        }
    }
    out.join("\n")
}

struct Block {
    kind: BlockKind,
    /// Left indent, in character columns.
    indent: usize,
    /// Glow separates blocks with a blank line, except items of a tight list.
    gap_before: bool,
}

enum BlockKind {
    Text(LayoutJob),
    Code(String),
    Rule,
    Table(Vec<Vec<String>>),
}

struct ListState {
    /// `Some(n)` for numbered lists: the number of the next item.
    next_number: Option<u64>,
}

#[derive(Default)]
struct Renderer {
    font: FontId,
    blocks: Vec<Block>,
    job: LayoutJob,
    previous_was_tight_item: bool,

    // Inline styles; counters because they can nest (**bold *and italic***).
    strong: usize,
    emphasis: usize,
    strikethrough: usize,
    /// Destination of the link being written; glow prints it after the link text.
    link: Option<String>,
    heading: Option<HeadingLevel>,

    // Block nesting.
    quote_depth: usize,
    lists: Vec<ListState>,
    /// Bullet or number waiting to be written before the item's first text.
    item_prefix: Option<String>,
    code_block: Option<String>,
    table: Option<Vec<Vec<String>>>,
    cell: String,
}

fn parse(markdown: &str, font: &FontId) -> Vec<Block> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let mut renderer = Renderer { font: font.clone(), ..Default::default() };
    for event in Parser::new_ext(markdown, options) {
        renderer.event(event);
    }
    renderer.flush(false);
    renderer.blocks
}

impl Renderer {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some(code) = &mut self.code_block {
                    code.push_str(&text);
                } else if self.table.is_some() {
                    self.cell.push_str(&text);
                } else {
                    self.push(&text, self.format());
                }
            }
            Event::Code(code) => {
                if self.table.is_some() {
                    self.cell.push_str(&code);
                } else {
                    let format = TextFormat {
                        color: palette::CODE_FG,
                        background: palette::CODE_BG,
                        ..self.format()
                    };
                    self.push(&format!(" {code} "), format);
                }
            }
            Event::SoftBreak => self.push(" ", self.format()),
            Event::HardBreak => self.push("\n", self.format()),
            Event::Rule => {
                self.flush(false);
                self.add_block(BlockKind::Rule, false);
            }
            Event::TaskListMarker(done) => self.push(if done { "[✓] " } else { "[ ] " }, self.format()),
            Event::Html(html) | Event::InlineHtml(html) => {
                let format = TextFormat { color: palette::DIM, ..self.format() };
                self.push(html.trim_end(), format);
            }
            Event::FootnoteReference(name) => self.push(&format!("[^{name}]"), self.format()),
            Event::InlineMath(math) | Event::DisplayMath(math) => self.push(&math, self.format()),
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.flush(true),
            Tag::Heading { level, .. } => {
                self.flush(false);
                self.heading = Some(level);
                // Unlike glow, no "##" prefixes: levels differ by size and colour instead.
                if level == HeadingLevel::H1 {
                    self.push(" ", self.format());
                }
            }
            Tag::BlockQuote(_) => {
                self.flush(false);
                self.quote_depth += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush(false);
                self.code_block = Some(String::new());
            }
            Tag::List(start) => {
                self.flush(true);
                if self.lists.is_empty() {
                    self.previous_was_tight_item = false;
                }
                self.lists.push(ListState { next_number: start });
            }
            Tag::Item => {
                self.flush(true);
                let bullet = match self.lists.last_mut().and_then(|list| list.next_number.as_mut()) {
                    Some(number) => {
                        *number += 1;
                        format!("{}. ", *number - 1)
                    }
                    None => "• ".to_owned(),
                };
                self.item_prefix = Some(bullet);
            }
            Tag::Table(_) => {
                self.flush(false);
                self.table = Some(Vec::new());
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.push(Vec::new());
                }
            }
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strikethrough += 1,
            Tag::Link { dest_url, .. } => self.link = Some(dest_url.to_string()),
            Tag::Image { dest_url, .. } => {
                self.push("Image: ", self.format());
                self.link = Some(dest_url.to_string());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(false),
            TagEnd::Heading(level) => {
                if level == HeadingLevel::H1 {
                    self.push(" ", self.format());
                }
                self.flush(false);
                self.heading = None;
            }
            TagEnd::BlockQuote(_) => {
                self.flush(false);
                self.quote_depth -= 1;
            }
            TagEnd::CodeBlock => {
                if let Some(code) = self.code_block.take() {
                    self.add_block(BlockKind::Code(code.trim_end_matches('\n').to_owned()), false);
                }
            }
            TagEnd::List(_) => {
                self.flush(true);
                self.lists.pop();
            }
            TagEnd::Item => self.flush(true),
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.cell);
                if let Some(row) = self.table.as_mut().and_then(|table| table.last_mut()) {
                    row.push(cell.trim().to_owned());
                }
            }
            TagEnd::Table => {
                if let Some(rows) = self.table.take() {
                    self.add_block(BlockKind::Table(rows), false);
                }
            }
            TagEnd::Emphasis => self.emphasis -= 1,
            TagEnd::Strong => self.strong -= 1,
            TagEnd::Strikethrough => self.strikethrough -= 1,
            TagEnd::Link => self.end_link(" "),
            TagEnd::Image => self.end_link(" → "),
            _ => {}
        }
    }

    fn end_link(&mut self, separator: &str) {
        if let Some(url) = self.link.take() {
            self.push(separator, self.format());
            let format = TextFormat {
                color: palette::LINK,
                underline: Stroke::new(1.0, palette::LINK),
                ..self.format()
            };
            self.push(&url, format);
        }
    }

    /// The text style for the current nesting of headings, bold, links, etc.
    fn format(&self) -> TextFormat {
        let mut format = TextFormat {
            font_id: self.font.clone(),
            color: palette::TEXT,
            ..Default::default()
        };
        match self.heading {
            Some(HeadingLevel::H1) => {
                format.color = palette::H1_FG;
                format.background = palette::H1_BG;
            }
            Some(HeadingLevel::H6) => format.color = palette::H6,
            Some(_) => format.color = palette::HEADING,
            None => {}
        }
        format.font_id.size *= match self.heading {
            Some(HeadingLevel::H1 | HeadingLevel::H2) => 1.3,
            Some(HeadingLevel::H3) => 1.15,
            _ => 1.0,
        };
        if self.link.is_some() {
            format.color = palette::LINK_TEXT;
        }
        // egui's monospace font has no bold face, so bold is shown brighter.
        if self.strong > 0 && self.heading.is_none() {
            format.color = palette::BOLD;
        }
        format.italics = self.emphasis > 0;
        if self.strikethrough > 0 {
            format.strikethrough = Stroke::new(1.0, format.color);
        }
        format
    }

    /// Appends text to the current block, starting it with any quote bars and bullet.
    fn push(&mut self, text: &str, format: TextFormat) {
        if self.job.text.is_empty() {
            let plain = TextFormat { font_id: self.font.clone(), color: palette::DIM, ..Default::default() };
            for _ in 0..self.quote_depth {
                self.job.append("│ ", 0.0, plain.clone());
            }
            if let Some(bullet) = self.item_prefix.take() {
                self.job.append(&bullet, 0.0, TextFormat { color: palette::TEXT, ..plain });
            }
        }
        self.job.append(text, 0.0, format);
    }

    /// Ends the current text block, if any. `tight` marks an item of a tight list,
    /// which glow draws without a blank line before it.
    fn flush(&mut self, tight: bool) {
        if self.job.text.is_empty() {
            return;
        }
        let job = std::mem::take(&mut self.job);
        self.add_block(BlockKind::Text(job), tight);
    }

    fn add_block(&mut self, kind: BlockKind, tight: bool) {
        let gap_before = !(tight && self.previous_was_tight_item);
        let indent = 2 * self.lists.len().saturating_sub(1);
        self.blocks.push(Block { kind, indent, gap_before });
        self.previous_was_tight_item = tight;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws the Glow pane in an offscreen 1000×700 window for each of `pointer`'s
    /// steps (None = no mouse event), and returns the pane's width after each frame.
    fn pane_widths(markdown: &str, pointer: &[Option<egui::Event>]) -> Vec<f32> {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let mut widths = Vec::new();
        for (frame, event) in pointer.iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(frame as f64 / 60.0),
                events: event.iter().cloned().collect(),
                ..Default::default()
            };
            let mut width = 0.0;
            let mut output = ctx.run_ui(input, |ui| {
                let response = panel(egui::Id::unique("glow"), ui.available_width()).show(ui, |ui| show(ui, markdown));
                width = response.response.rect.width();
                egui::CentralPanel::default().show(ui, |_| {});
            });
            output.textures_delta.clear(); // nothing draws them offscreen
            widths.push(width);
        }
        widths
    }

    fn drag_from_to(from: f32, to: f32) -> Vec<Option<egui::Event>> {
        let at = |x: f32| egui::pos2(x, 300.0);
        let button = |x: f32, pressed: bool| egui::Event::PointerButton {
            pos: at(x),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut steps = vec![None, None, Some(egui::Event::PointerMoved(at(from))), Some(button(from, true))];
        for i in 1..=10 {
            steps.push(Some(egui::Event::PointerMoved(at(from + (to - from) * i as f32 / 10.0))));
        }
        steps.extend([Some(button(to, false)), None, None]);
        steps
    }

    #[test]
    fn pane_starts_at_half_width_and_can_be_dragged_narrower() {
        let wide_table = format!("| {} | b |\n|---|---|\n| 1 | 2 |\n", "x".repeat(150));
        let wide_code = format!("```\n{}\n```\n", "y".repeat(150));
        let long_word = "z".repeat(150);
        for markdown in ["# Title\n\nSome text.\n", &wide_table, &wide_code, &long_word] {
            let widths = pane_widths(markdown, &drag_from_to(500.0, 700.0));
            assert!((widths[1] - 500.0).abs() < 2.0, "starts at half width, got {} for {markdown:.20}", widths[1]);
            let last = *widths.last().unwrap();
            assert!((last - 300.0).abs() < 5.0, "dragged to 300 wide, got {last} for {markdown:.20}");
        }
    }

    fn texts(markdown: &str) -> Vec<String> {
        parse(markdown, &FontId::monospace(FONT_SIZE))
            .into_iter()
            .map(|block| match block.kind {
                BlockKind::Text(job) => job.text,
                BlockKind::Code(code) => format!("[code] {code}"),
                BlockKind::Rule => "--------".to_owned(),
                BlockKind::Table(rows) => format_table(&rows),
            })
            .collect()
    }

    #[test]
    fn detects_markdown_files() {
        assert!(is_markdown(Some(Path::new("README.md"))));
        assert!(is_markdown(Some(Path::new("notes.MARKDOWN"))));
        assert!(!is_markdown(Some(Path::new("main.rs"))));
        assert!(!is_markdown(None));
    }

    #[test]
    fn renders_like_glow() {
        let md = "# Title\n\n## Sub\n\nSome `code` and a [link](https://x.io).\n\n- one\n- two\n  1. nested\n\n> quoted\n\n```\nfn main() {}\n```\n\n---\n";
        assert_eq!(
            texts(md),
            [
                " Title ",
                "Sub",
                "Some  code  and a link https://x.io.",
                "• one",
                "• two",
                "1. nested",
                "│ quoted",
                "[code] fn main() {}",
                "--------",
            ]
        );
    }

    #[test]
    fn formats_tables() {
        let md = "| Name | Qty |\n|---|---|\n| apple | 3 |\n";
        assert_eq!(texts(md), ["Name  │ Qty\n──────┼────\napple │ 3  "]);
    }
}
