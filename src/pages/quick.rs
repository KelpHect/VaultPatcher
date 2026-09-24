//! Simple mode's settings page: a short, friendly list that saves instantly.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::page_header;
use super::tweaks::control;
use crate::theme::{self, Icon};
use crate::ui;
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;

    let mut page = div().flex().flex_col().gap(px(26.)).child(page_header(
        "Quick Settings",
        "The settings that matter most, saved as soon as you change them. Want every knob? Switch to Advanced in the title bar.",
        vec![],
    ));
    if !game.config_found() {
        page = page.child(
            ui::card(theme::danger()).child(ui::card_body().p(px(16.)).child(ui::body(format!(
                "{} hasn't created its settings files yet. Launch it once, then come back.",
                def.name
            )))),
        );
    }

    for section in def.quick {
        let mut block = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::section_title(section.title, Some(section.blurb.into())));

        if !section.quality_presets.is_empty() {
            let mut row = div().flex().flex_wrap().gap(px(12.));
            for id in section.quality_presets {
                let Some(preset) = def.presets.iter().find(|p| p.id == *id) else { continue };
                let ws = ws.clone();
                let color = preset.rarity.color();
                let current = !preset.values.is_empty()
                    && preset.values.iter().all(|(id, v)| def.tweak(id).is_none_or(|t| game.effective(t) == v.to_value()));
                row = row.child(
                    ui::panel()
                        .id(SharedString::from(format!("q-preset-{id}")))
                        .w(px(220.))
                        .flex_grow()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::panel_hi()))
                        .child(div().h(px(5.)).bg(color))
                        .child(
                            div()
                                .p(px(14.))
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(div().font_family(theme::FONT_DISPLAY).text_size(px(24.)).text_color(color).child(preset.name))
                                        .when(current, |d| d.child(ui::badge("Current", theme::success()))),
                                )
                                .child(div().text_size(px(13.)).text_color(theme::text_muted()).child(preset.description)),
                        )
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.preset_now(preset, cx))),
                );
            }
            block = block.child(row);
        }

        let mut list = ui::panel().flex().flex_col();
        let mut first = true;
        for id in section.tweaks {
            let Some(tweak) = def.tweak(id) else { continue };
            if !first {
                list = list.child(div().h(px(1.)).bg(theme::line()));
            }
            first = false;
            let value = game.effective(tweak);
            if let Some(cards) = super::compare::choice_cards(tweak, ws, cx) {
                list = list.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .px(px(18.))
                        .py(px(16.))
                        .child(
                            div()
                                .font_family(theme::FONT_LABEL)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(16.))
                                .text_color(theme::text())
                                .child(tweak.label),
                        )
                        .child(ui::body(tweak.description))
                        .child(cards),
                );
                continue;
            }
            list = list.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(18.))
                    .px(px(18.))
                    .py(px(14.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(240.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .font_family(theme::FONT_LABEL)
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(16.))
                                            .text_color(theme::text())
                                            .child(tweak.label),
                                    )
                                    .when(game.pending.contains_key(tweak.id), |d| {
                                        d.child(ui::icon(Icon::Save).text_size(px(12.)).text_color(theme::accent()))
                                    }),
                            )
                            .child(ui::body(tweak.description))
                            .children(super::compare::strip(tweak, ws, cx)),
                    )
                    .child(
                        div()
                            .w(px(400.))
                            .flex_none()
                            .flex()
                            .justify_end()
                            .child(control(tweak, &value, ws, true)),
                    ),
            );
        }
        page = page.child(block.child(list));
    }
    page.into_any_element()
}
