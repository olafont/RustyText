//! How the app looks: its fonts, text sizes and colours, and the "cards" its parts are drawn on.
//!
//! The window is a pale background on which everything else sits as a rounded card with a
//! hairline border and a soft shadow: the ribbon, the editor, the bars. The
//! window bar across the top is a dark navy strip in both themes.
//!
//! The interface font is IBM Plex Sans, in four weights, and the monospace font IBM Plex Mono.
//! egui picks a font by family, not by weight, so each weight is a family of its own
//! (`medium()`, `semibold()`, `bold()`); Regular is the default family.

use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Stroke, TextStyle};

// The four text sizes, in points.
pub const VERY_SMALL: f32 = 10.0;
pub const SMALL: f32 = 11.0;
pub const MEDIUM: f32 = 12.0;
pub const LARGE: f32 = 14.0;
/// The ribbon's tabs and the window bar's menus: one point above `MEDIUM`.
pub const TAB: f32 = 13.0;

/// The space between two cards, and between a card and the window's edge.
pub const GAP: i8 = 6;
pub const CARD_RADIUS: u8 = 8;

const MEDIUM_FAMILY: &str = "plex-medium";
const SEMIBOLD_FAMILY: &str = "plex-semibold";
const BOLD_FAMILY: &str = "plex-bold";

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Light emphasis: the selected tab, text on buttons.
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM_FAMILY.into()))
}

/// Titles: panel titles, the app's name, the active document's tab.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD_FAMILY.into()))
}

/// Headings inside a dialog.
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD_FAMILY.into()))
}

/// The colours of the current theme.
pub struct Palette {
    /// The window, behind the cards.
    pub window: Color32,
    pub card: Color32,
    pub card_border: Color32,
    /// Where text is written: the editor.
    pub page: Color32,
    pub text: Color32,
    pub dim: Color32,
    /// The one accent colour: the selected tab, the active marker, the tools' icons.
    pub accent: Color32,
    /// Behind what is selected or switched on.
    pub accent_soft: Color32,
    pub divider: Color32,
    pub hover: Color32,
}

/// The window bar, the same in both themes.
pub mod bar {
    use super::Color32;

    pub const FILL: Color32 = Color32::from_rgb(35, 40, 56);
    pub const TEXT: Color32 = Color32::from_rgb(240, 242, 247);
    pub const DIM: Color32 = Color32::from_rgb(150, 158, 176);
    /// Behind the active document's tab, and a button under the pointer.
    pub const PILL: Color32 = Color32::from_rgb(58, 65, 88);
    pub const DIVIDER: Color32 = Color32::from_rgb(72, 80, 104);
}

const LIGHT: Palette = Palette {
    window: Color32::from_rgb(226, 231, 238),
    card: Color32::from_rgb(247, 248, 250),
    card_border: Color32::from_rgb(208, 214, 223),
    page: Color32::from_rgb(255, 255, 255),
    text: Color32::from_rgb(31, 39, 51),
    dim: Color32::from_rgb(104, 114, 130),
    accent: Color32::from_rgb(40, 104, 214),
    accent_soft: Color32::from_rgb(219, 232, 252),
    divider: Color32::from_rgb(218, 223, 230),
    hover: Color32::from_rgb(234, 238, 244),
};

const DARK: Palette = Palette {
    window: Color32::from_rgb(20, 23, 29),
    card: Color32::from_rgb(34, 38, 46),
    card_border: Color32::from_rgb(54, 60, 71),
    page: Color32::from_rgb(26, 29, 36),
    text: Color32::from_rgb(228, 232, 239),
    dim: Color32::from_rgb(140, 149, 165),
    accent: Color32::from_rgb(98, 160, 250),
    accent_soft: Color32::from_rgb(38, 56, 86),
    divider: Color32::from_rgb(52, 58, 68),
    hover: Color32::from_rgb(46, 51, 61),
};

pub fn palette(ui: &egui::Ui) -> &'static Palette {
    if ui.visuals().dark_mode { &DARK } else { &LIGHT }
}

/// A card: what every part of the window is drawn on.
pub fn card(ui: &egui::Ui) -> Frame {
    let palette = palette(ui);
    Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0, palette.card_border))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .shadow(egui::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(if ui.visuals().dark_mode { 70 } else { 22 }) })
}

/// The frame of a panel that holds one card: it only keeps the card `GAP` away from its
/// neighbours. `sides` says where (left, right, top, bottom), so that two panels side by side
/// are one gap apart, not two.
pub fn gutter(left: bool, right: bool, top: bool, bottom: bool) -> Frame {
    let side = |on: bool| if on { GAP } else { 0 };
    Frame::NONE.inner_margin(Margin { left: side(left), right: side(right), top: side(top), bottom: side(bottom) })
}

/// Sets the fonts, the text sizes and the colours of both themes. Call once, at startup.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, regular(VERY_SMALL)),
            (TextStyle::Body, regular(MEDIUM)),
            (TextStyle::Button, regular(MEDIUM)),
            (TextStyle::Heading, bold(LARGE)),
            (TextStyle::Monospace, FontId::monospace(MEDIUM)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(7.0, 3.0);
        style.spacing.menu_margin = Margin::same(6);
    });
    ctx.style_mut_of(egui::Theme::Light, |style| colours(&mut style.visuals, &LIGHT));
    ctx.style_mut_of(egui::Theme::Dark, |style| colours(&mut style.visuals, &DARK));
}

fn colours(visuals: &mut egui::Visuals, palette: &Palette) {
    visuals.panel_fill = palette.window;
    visuals.window_fill = palette.card;
    visuals.window_stroke = Stroke::new(1.0, palette.card_border);
    visuals.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    visuals.menu_corner_radius = CornerRadius::same(CARD_RADIUS);
    visuals.extreme_bg_color = palette.page;
    visuals.faint_bg_color = palette.hover;
    visuals.hyperlink_color = palette.accent;
    visuals.selection.bg_fill = palette.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    let radius = CornerRadius::same(6);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = radius;
        widget.fg_stroke.color = palette.text;
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.divider);
    visuals.widgets.inactive.weak_bg_fill = palette.hover;
    visuals.widgets.inactive.bg_fill = palette.hover;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.hovered.weak_bg_fill = palette.accent_soft;
    visuals.widgets.hovered.bg_fill = palette.accent_soft;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    visuals.widgets.active.weak_bg_fill = palette.accent_soft;
    visuals.widgets.active.bg_fill = palette.accent_soft;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);
    visuals.widgets.open.weak_bg_fill = palette.accent_soft;
    visuals.widgets.open.bg_stroke = Stroke::NONE;
}

fn fonts() -> FontDefinitions {
    // egui's own fonts stay, after Plex: they have the symbols and emojis Plex doesn't.
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add("plex-sans", include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf"));
    add(MEDIUM_FAMILY, include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-Medium.ttf"));
    add(SEMIBOLD_FAMILY, include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-SemiBold.ttf"));
    add(BOLD_FAMILY, include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-Bold.ttf"));
    add("plex-mono", include_bytes!("../assets/fonts/ibm-plex/IBMPlexMono-Regular.ttf"));

    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "plex-sans".to_owned());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "plex-mono".to_owned());
    for weight in [MEDIUM_FAMILY, SEMIBOLD_FAMILY, BOLD_FAMILY] {
        let mut family = vec![weight.to_owned()];
        family.extend(fallbacks.iter().cloned());
        fonts.families.insert(FontFamily::Name(weight.into()), family);
    }
    fonts
}

/// The radius of the window's own corners: macOS rounds them, and the border follows.
const WINDOW_RADIUS: u8 = if cfg!(target_os = "macos") { 16 } else { 0 };

/// A thin line around the whole window, over everything else, so that the window's edge shows
/// where a dark part of it (the window bar, the terminal) meets a dark desktop. Not drawn
/// in full screen, where the window has no edge.
pub fn window_border(ui: &egui::Ui) {
    if ui.input(|input| input.viewport().fullscreen == Some(true)) {
        return;
    }
    let layer = egui::LayerId::new(egui::Order::Foreground, ui.scope_id().with("window_border"));
    let painter = ui.ctx().layer_painter(layer);
    let stroke = Stroke::new(1.0, bar::DIVIDER);
    painter.rect_stroke(ui.ctx().content_rect(), CornerRadius::same(WINDOW_RADIUS), stroke, egui::StrokeKind::Inside);
}

/// A small chevron pointing down: "this opens a menu".
pub fn chevron_down(ui: &egui::Ui, center: egui::Pos2, color: Color32) {
    let stroke = Stroke::new(1.3, color);
    ui.painter().line_segment([center + egui::vec2(-3.0, -1.5), center + egui::vec2(0.0, 1.5)], stroke);
    ui.painter().line_segment([center + egui::vec2(0.0, 1.5), center + egui::vec2(3.0, -1.5)], stroke);
}

/// A thin vertical line between two groups, in a row of height `height`.
pub fn divider(ui: &mut egui::Ui, height: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, height), egui::Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(1.0, color));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_weight_has_its_font_and_the_symbol_fallbacks() {
        let fonts = fonts();
        for family in [FontFamily::Proportional, FontFamily::Monospace, medium(1.0).family, semibold(1.0).family, bold(1.0).family] {
            let names = &fonts.families[&family];
            assert!(names[0].starts_with("plex-"), "{family:?} starts with Plex");
            assert!(names.len() > 1, "{family:?} falls back to egui's fonts for symbols");
            assert!(names.iter().all(|name| fonts.font_data.contains_key(name)), "{family:?} names fonts that exist");
        }
    }

    #[test]
    fn the_look_installs_on_a_fresh_context() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.label(egui::RichText::new("Title").font(semibold(LARGE)));
            assert_eq!(ui.style().text_styles[&TextStyle::Body].size, MEDIUM);
        });
        output.textures_delta.clear(); // nothing draws them offscreen
    }
}
