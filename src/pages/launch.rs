//! Launch options: the switches the sidebar's Play button uses. Shown as a
//! section of the Overview page.

use gpui::{AnyElement, App, ClipboardItem, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px};

use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

/// A launch argument as inline code: 12px mono in a subtle pill.
fn code(text: &'static str) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(theme::RADIUS))
        .bg(theme::control_alt())
        .border_1()
        .border_color(theme::card_stroke())
        .font_family(theme::font_mono())
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(theme::text_muted())
        .child(text)
}

pub fn section(ws: &Entity<Workspace>, cx: &App) -> AnyElement {
    let state = ws.read(cx);
    let def = state.game().def;
    let args = state.launch_args();
    let command_line = args.join(" ");

    let mut page = div().flex().flex_col().gap(px(8.)).child(ui::section_title(
        "Launch options",
        Some("Switches the Play button starts the game with. Steam still handles DRM and the overlay.".into()),
    ));

    // SettingsCards: title with the argument, description, toggle.
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
                .min_h(px(68.))
                .px(px(16.))
                .py(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().flex().flex_wrap().items_center().gap(px(8.)).child(ui::title(arg.label)).child(code(arg.arg)))
                        .child(ui::caption(arg.description)),
                )
                .child(
                    ui::toggle(SharedString::from(format!("arg-{i}")), on)
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.toggle_launch_arg(name, cx))),
                ),
        );
    }

    // A read-only TextBox with its Copy button.
    let copy_text = command_line.clone();
    let empty = command_line.is_empty();
    page = page.child(list).child(
        ui::panel().px(px(16.)).py(px(12.)).flex().flex_col().gap(px(8.)).child(ui::title("Steam launch options")).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h(px(32.))
                        .flex()
                        .items_center()
                        .px(px(12.))
                        .py(px(6.))
                        .rounded(px(theme::RADIUS))
                        .bg(theme::control())
                        .border_1()
                        .border_color(theme::control_stroke())
                        .font_family(theme::font_mono())
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(if empty { theme::text_muted() } else { theme::text() })
                        .child(if empty { "(none)".to_string() } else { command_line }),
                )
                .child(
                    ui::button_if(!empty, "launch-copy", "Copy", Some(Icon::Copy), Variant::Secondary)
                        .tooltip(ui::tip(if empty { "Turn on a launch option first" } else { "Copy to the clipboard" }))
                        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()))),
                ),
        )
        .child(ui::caption("Paste into Steam › Properties › Launch Options to use these switches when launching from Steam too.")),
    );
    page.into_any_element()
}
