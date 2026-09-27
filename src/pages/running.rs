//! Shown in place of the page while the selected game is running: editing
//! settings then is pointless (the game rewrites its config files on exit),
//! so this offers to force-close the game or tuck Vaulter away in a
//! quiet background mode until the game exits.

use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, Styled, Window, div, prelude::*, px};

use super::confirm;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let name = def.name;
    let closing = state.busy.is_some();

    let minimize_ws = ws.clone();
    let close_ws = ws.clone();
    let dismiss_ws = ws.clone();

    let card = ui::panel()
        .w(px(540.))
        .max_w_full()
        .p(px(32.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(16.))
        .child(crate::app::game_icon(game.art.icon.clone(), def.short, 64.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .child(ui::display(name, 28.))
                .child(ui::badge("Running", theme::success())),
        )
        .child(
            ui::body(format!(
                "Settings are on hold while you play. {} rewrites its config files when it exits, so anything changed now would be lost.",
                def.short
            ))
            .text_center()
            .max_w(px(440.)),
        )
        .child(
            div()
                .pt(px(8.))
                .flex()
                .gap(px(8.))
                .child(
                    ui::button("running-minimize", "Minimize to background", Some(Icon::Minimize), Variant::Primary)
                        .tooltip(ui::tip("Mutes Vaulter and checks in every 10 seconds. It comes back when you quit the game."))
                        .on_click(move |_, window, cx| {
                            if minimize_ws.update(cx, |ws, cx| ws.enter_background(cx)) {
                                window.minimize_window();
                            }
                        }),
                )
                .child(
                    ui::button_if(!closing, "running-close", "Force close", Some(Icon::Close), Variant::Secondary)
                        .tooltip(ui::tip(if closing { "Wait for the current task to finish".to_string() } else { format!("Ends {name} right away") }))
                        .on_click(move |_, window, cx| {
                            if closing {
                                return;
                            }
                            let ws = close_ws.clone();
                            confirm(
                                window,
                                cx,
                                &format!("Force close {name}?"),
                                "Anything since your last save or checkpoint will be lost. Quitting from the game's menu is safer.",
                                "Force close",
                                move |cx| ws.update(cx, |ws, cx| ws.force_close_game(cx)),
                            );
                        }),
                ),
        )
        .child(
            ui::button("running-dismiss", "Keep working anyway", None, Variant::Ghost)
                .text_color(theme::text_muted())
                .tooltip(ui::tip("Hide this screen until the game closes"))
                .on_click(move |_, _, cx| dismiss_ws.update(cx, |ws, cx| ws.dismiss_running_screen(cx))),
        );

    div()
        .id("running-screen")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .items_center()
        .justify_center()
        .p(px(32.))
        .child(card)
        .into_any_element()
}
