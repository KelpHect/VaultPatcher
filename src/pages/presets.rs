//! One-click bundles of tweaks, presented as loot cards.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::page_header;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let def = state.game().def;

    let mut grid = div().flex().flex_wrap().gap(px(20.));
    for preset in def.presets {
        let color = preset.rarity.color();
        let count = if preset.values.is_empty() {
            def.visible_tweaks().count()
        } else {
            preset.values.iter().filter(|(id, _)| def.tweak(id).is_some()).count()
        };
        let ws = ws.clone();
        let mut chips = div().flex().flex_wrap().gap(px(5.));
        for (id, value) in preset.values.iter().take(8) {
            if let Some(t) = def.tweak(id) {
                chips = chips.child(
                    div()
                        .px(px(6.))
                        .py(px(1.))
                        .bg(theme::panel_lo())
                        .text_size(px(11.))
                        .text_color(theme::text_muted())
                        .child(format!("{}: {}", t.label, value.to_value().display(&t.control))),
                );
            }
        }
        if preset.values.len() > 8 {
            chips = chips.child(
                div()
                    .text_size(px(11.))
                    .text_color(theme::text_dim())
                    .child(format!("+{} more", preset.values.len() - 8)),
            );
        }

        grid = grid.child(
            ui::panel()
                .w(px(360.))
                .flex_grow()
                .flex()
                .flex_col()
                .border_color(theme::ink())
                .child(div().h(px(6.)).bg(color))
                .child(
                    div()
                        .p(px(18.))
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(ui::badge(preset.rarity.label(), color))
                                .child(ui::label(format!("{count} settings"))),
                        )
                        .child(
                            div()
                                .font_family(theme::FONT_DISPLAY)
                                .text_size(px(30.))
                                .text_color(color)
                                .child(preset.name),
                        )
                        .child(ui::body(preset.description))
                        .child(chips)
                        .child(div().flex_1())
                        .child(
                            div().pt(px(6.)).child(
                                ui::button(
                                    SharedString::from(format!("preset-{}", preset.id)),
                                    "Stage preset",
                                    Some(Icon::Add),
                                    Variant::Primary,
                                )
                                .on_click(move |_, _, cx| {
                                    ws.update(cx, |ws, cx| ws.stage_preset(preset, cx));
                                }),
                            ),
                        ),
                ),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(page_header(
            "Presets",
            "Stage a bundle of tweaks in one click, then fine-tune individual settings before applying. Presets stack: later ones override earlier ones.",
            vec![],
        ))
        .child(grid)
        .child(
            div()
                .font_family(theme::FONT_LABEL)
                .font_weight(FontWeight::NORMAL)
                .text_size(px(12.))
                .text_color(theme::text_dim())
                .child("Card color follows loot rarity: the rarer the card, the bigger the change."),
        )
        .into_any_element()
}
