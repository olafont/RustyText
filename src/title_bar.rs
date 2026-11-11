//! The app's own title bar, on macOS: the OS title bar is hidden and the window's
//! content runs under it, so the strip the OS would leave empty holds the app's
//! name, the menus and the document's name. The OS still draws its traffic
//! lights over the bar's left end (`TRAFFIC_LIGHTS`), and keeps the window's frame,
//! corners and shadow.
//!
//! Empty space in the bar moves the window (with the OS's own move loop) and a
//! double-click there zooms it, as on a native title bar. On Windows and Linux the
//! OS title bar is kept and `integrated()` is false.
//!
//! The approach is Marine Data Simulator's (`crates/app/src/ui/title_bar.rs`).

use eframe::egui::{self, Color32, Id, PointerButton, Rect, Sense, Ui, UiBuilder, ViewportBuilder, ViewportCommand, pos2};

/// Room for the macOS traffic lights at the bar's left end.
pub const TRAFFIC_LIGHTS: f32 = 76.0;

pub const HEIGHT: f32 = 32.0;

/// The bar's colour: a shade away from the window's, as an OS title bar is.
pub fn fill(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(17, 18, 20) } else { Color32::from_rgb(226, 229, 234) }
}

/// True when the app draws the title bar (macOS).
pub const fn integrated() -> bool {
    cfg!(target_os = "macos")
}

/// The window settings that hide the OS title bar and run the content under it.
pub fn viewport(builder: ViewportBuilder) -> ViewportBuilder {
    if !integrated() {
        return builder;
    }
    builder.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false)
}

/// Lays out the bar in `bar`: the drag area first (so the widgets added by `contents`,
/// which are registered later, take their clicks), then `contents` in the space right
/// of the traffic lights. `id` tells the bars of different windows apart.
pub fn show<R>(ui: &mut Ui, bar: Rect, id: Id, contents: impl FnOnce(&mut Ui) -> R) -> R {
    drag_area(ui, bar, id);
    let inner = Rect::from_min_max(pos2(bar.left() + TRAFFIC_LIGHTS, bar.top()), bar.right_bottom());
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.scope_builder(UiBuilder::new().max_rect(inner).layout(layout), contents).inner
}

/// The bar's background: a press-and-drag on empty space moves the window, a double-click zooms.
fn drag_area(ui: &Ui, rect: Rect, id: Id) {
    let ctx = ui.ctx();
    let response = ui.interact(rect, id, Sense::click_and_drag());
    // egui hands a drag that starts on a click-only button to the drag-sensing background
    // below it, so remember whether the press itself landed on bare background.
    if ctx.input(|i| i.pointer.any_pressed()) {
        let bare = ctx.viewport(|viewport| viewport.hits.click.is_some_and(|widget| widget.id == response.id));
        ctx.data_mut(|data| data.insert_temp(response.id, bare));
    }
    if response.double_clicked() {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    } else if response.drag_started_by(PointerButton::Primary) && ctx.data(|data| data.get_temp(response.id).unwrap_or(false)) {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Pos2, RawInput, ViewportId, vec2};

    /// Frames of a press-drag-release from `from` to `to` over a bar with one button
    /// at its start; the viewport commands they sent.
    fn drag(from: Pos2, to: Pos2) -> Vec<ViewportCommand> {
        let ctx = egui::Context::default();
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let frames = [
            vec![Event::PointerMoved(from)],
            vec![button(from, true)],
            vec![Event::PointerMoved(to)],
            vec![Event::PointerMoved(to)],
            vec![button(to, false)],
        ];
        let mut commands = vec![];
        for (index, events) in frames.into_iter().enumerate() {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 400.0))),
                time: Some(index as f64 * 0.05),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                let bar = Rect::from_min_size(Pos2::ZERO, vec2(800.0, HEIGHT));
                show(ui, bar, Id::unique("bar"), |ui| ui.button("Open"));
            });
            output.textures_delta.clear();
            commands.extend(output.viewport_output.remove(&ViewportId::ROOT).map(|viewport| viewport.commands).unwrap_or_default());
        }
        commands
    }

    #[test]
    fn empty_space_moves_the_window_and_buttons_do_not() {
        let moves = |commands: &Vec<ViewportCommand>| {
            commands.iter().filter(|command| matches!(command, ViewportCommand::StartDrag)).count()
        };
        assert_eq!(moves(&drag(pos2(500.0, 15.0), pos2(560.0, 40.0))), 1, "drag from empty space");
        let on_button = pos2(TRAFFIC_LIGHTS + 12.0, 15.0);
        assert_eq!(moves(&drag(on_button, pos2(TRAFFIC_LIGHTS + 80.0, 40.0))), 0, "drag from the button");
    }
}
