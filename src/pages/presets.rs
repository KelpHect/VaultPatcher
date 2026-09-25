//! One-click bundles of settings, plus the user's own saved profiles.

use gpui::{AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};

use super::page_header;
use crate::theme;
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;

    let mut list = ui::panel().flex().flex_col();
    for (i, preset) in def.presets.iter().enumerate() {
        let color = preset.rarity.color();
        let count = if preset.values.is_empty() {
            def.visible_tweaks().count()
        } else {
            preset.values.iter().filter(|(id, _)| def.tweak(id).is_some()).count()
        };
        // Waiting changes count, so a just-loaded preset shows as current.
        let current = !preset.values.is_empty()
            && preset.values.iter().all(|(id, v)| def.tweak(id).is_none_or(|t| game.effective(t) == v.to_value()));
        let summary: Vec<String> = preset
            .values
            .iter()
            .filter_map(|(id, v)| def.tweak(id).map(|t| format!("{}: {}", t.label, v.to_value().display(&t.control))))
            .collect();
        let ws = ws.clone();
        list = list.when(i > 0, |d| d.child(ui::divider())).child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .px(px(16.))
                .py(px(12.))
                .child(div().w(px(3.)).h(px(40.)).flex_none().rounded_full().bg(color))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(div().text_size(px(14.5)).font_weight(FontWeight::SEMIBOLD).child(preset.name))
                                .child(ui::badge(preset.rarity.label(), color))
                                .when(current, |d| d.child(ui::badge("Current", theme::success()))),
                        )
                        .child(ui::body(preset.description))
                        .when(!summary.is_empty(), |d| {
                            d.child(div().text_size(px(12.)).text_color(theme::text_dim()).truncate().child(summary.join(" · ")))
                        }),
                )
                .child(div().flex_none().text_size(px(12.)).text_color(theme::text_dim()).child(format!("{count} settings")))
                .child(
                    ui::button(SharedString::from(format!("preset-{}", preset.id)), "Load", None, Variant::Secondary)
                        .tooltip(ui::tip("Adds these values to your waiting changes; nothing is written until Apply"))
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.stage_preset(preset, cx))),
                ),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap(px(18.))
        .child(page_header(
            "Presets",
            "Load a bundle of settings, fine-tune, then Apply. Loading replaces any changes still waiting. The tag shows how big a change it is.",
            vec![],
        ))
        .child(list)
        .child(super::settings::profiles_section(ws, cx))
        .into_any_element()
}
