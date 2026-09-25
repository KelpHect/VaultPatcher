//! Launch options: the switches the sidebar's Play button uses. Shown as a
//! section of the Overview page.

use gpui::{
    AnyElement, App, ClipboardItem, Entity, IntoElement, ParentElement, SharedString,
    Styled, div, prelude::*, px,
};


use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn section(ws: &Entity<Workspace>, cx: &App) -> AnyElement {
    let state = ws.read(cx);
    let def = state.game().def;
    let args = state.launch_args();
    let command_line = args.join(" ");

    let mut page = div().flex().flex_col().gap(px(12.)).child(ui::section_title(
        "Launch options",
        Some("Switches the Play button starts the game with. Steam still handles DRM and the overlay.".into()),
    ));

    let mut list = ui::panel().flex().flex_col();
    for (i, arg) in def.launch_args.iter().enumerate() {
        if i > 0 {
            list = list.child(ui::divider());
        }
        let on = args.iter().any(|a| a == arg.arg);
        let ws = ws.clone();
        let name = arg.arg;
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(16.))
                .px(px(16.))
                .py(px(9.))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(ui::title(arg.label))
                                .child(
                                    div()
                                        .font_family(theme::FONT_MONO)
                                        .text_size(px(13.))
                                        .text_color(theme::text_dim())
                                        .child(arg.arg),
                                ),
                        )
                        .child(div().text_size(px(12.)).text_color(theme::text_dim()).child(arg.description)),
                )
                .child(
                    ui::toggle(SharedString::from(format!("arg-{i}")), on)
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.toggle_launch_arg(name, cx))),
                ),
        );
    }

    let copy_text = command_line.clone();
    page = page.child(list).child(
        ui::panel()
            .p(px(16.))
            .flex()
            .items_center()
            .gap(px(14.))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ui::label("Steam launch options"))
                    .child(
                        div()
                            .px(px(10.))
                            .py(px(8.))
                            .bg(theme::bg_deep())
                            .border_1()
                            .border_color(theme::line())
                            .font_family(theme::FONT_MONO)
                            .text_size(px(13.))
                            .text_color(theme::text())
                            .child(if command_line.is_empty() { "(none)".to_string() } else { command_line }),
                    )
                    .child(ui::body(
                        "Paste into Steam › Properties › Launch Options to use these switches when launching from Steam too.",
                    )),
            )
            .child(
                ui::button("launch-copy", "Copy", Some(Icon::Copy), Variant::Secondary)
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()))),
            ),
    );
    page.into_any_element()
}
