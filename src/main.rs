//! RustyText, Rust + egui edition.

// Without this, Windows release builds open a console window behind the editor.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod document;
mod emoji;
mod glow;
mod i18n;
mod mcp;
mod notes;
mod open_requests;
mod settings;
mod terminal;
mod text_utils;
mod title_bar;
mod tools;

use std::path::PathBuf;

use eframe::egui;

/// The size a window opens at, in points: room for the editor and the Markdown pane side by side.
const WINDOW_SIZE: [f32; 2] = [1374.0, 800.0];

fn main() -> eframe::Result {
    settings::load();
    open_requests::install();
    // Files passed on the command line: `rusty-text foo.txt bar.rs`.
    // Skip argv[0] (the program itself) and flags like the `-psn_…` macOS may add.
    // With none, the window starts with an empty untitled document.
    let paths: Vec<PathBuf> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .collect();

    // eframe shows the egui logo in the Dock unless given an icon; this one is
    // baked into the binary at compile time.
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png"))
        .expect("bundled icon is a valid PNG");

    let options = eframe::NativeOptions {
        viewport: title_bar::viewport(egui::ViewportBuilder::default())
            .with_title("RustyText")
            .with_inner_size(WINDOW_SIZE)
            .with_drag_and_drop(true)
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "RustyText",
        options,
        Box::new(|creation_context| {
            // Everything 1pt larger than egui's defaults (menus, tabs, dialogs, status bar),
            // in both light and dark mode.
            creation_context.egui_ctx.set_theme(settings::get().theme.preference());
            creation_context.egui_ctx.all_styles_mut(|style| {
                for font in style.text_styles.values_mut() {
                    font.size += 1.0;
                }
            });
            // Files double-clicked in the Finder come after those of the command line.
            open_requests::wake(&creation_context.egui_ctx);
            let mut paths = paths;
            paths.extend(open_requests::take());
            Ok(Box::new(app::EditorApp::new(paths)))
        }),
    )
}
