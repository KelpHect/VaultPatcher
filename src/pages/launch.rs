//! Launch options: the switches the sidebar's Play button uses. Shown as a
//! section of the Overview page.

use gpui::{
    AnyElement, App, ClipboardItem, Entity, FontWeight, IntoElement, ParentElement, SharedString,
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
            list = list.child(div().h(px(1.)).bg(theme::line()));
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
                .py(px(12.))
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
                                .child(
                                    div()
                                        .font_family(theme::FONT_LABEL)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(15.))
                                        .text_color(theme::text())
                                        .child(arg.label),
                                )
                                .child(
                                    div()
                                        .font_family(theme::FONT_MONO)
                                        .text_size(px(13.))
                                        .text_color(theme::text_dim())
                                        .child(arg.arg),
                                ),
                        )
                        .child(ui::body(arg.description)),
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
            .p(px(18.))
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
                ui::button("launch-copy", "Copy", Some(Icon::Save), Variant::Secondary)
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()))),
            ),
    );
    page.into_any_element()
}
