//! Generic tweak page: renders every tweak in the nav item's categories.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::page_header;
use crate::games::NavItem;
use crate::theme::{self, Icon, Rarity};
use crate::tweaks::{Binding, Control, Impact, Tweak, Value};
use crate::ui::{self, Variant};
use crate::workspace::{TweakFilter, Workspace};

pub fn render(nav: &NavItem, ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let filter = state.tweak_filter;
    let categories = nav.categories;

    let reset_ws = ws.clone();
    let reset_button = ui::button("reset-page", "Page defaults", Some(Icon::Undo), Variant::Secondary)
        .on_click(move |_, _, cx| {
            reset_ws.update(cx, |ws, cx| ws.stage_defaults(categories, cx));
        })
        .into_any_element();

    let mut page = div().flex().flex_col().gap(px(26.)).child(page_header(
        nav.title,
        &format!(
            "{} — changes are staged first; nothing touches your files until you press Apply.",
            def.name
        ),
        vec![reset_button],
    ));

    page = page.child(filter_bar(ws, filter));

    if !game.config_found() {
        page = page.child(
            ui::card(theme::danger()).child(
                ui::card_body()
                    .p(px(16.))
                    .child(ui::body(format!(
                        "No config files found in {}. Launch {} once so it creates them, or set the folder in App Settings.",
                        game.config_dir
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "Documents\\My Games".into()),
                        def.name
                    ))),
            ),
        );
    }

    for &cat_id in categories {
        let Some(category) = def.category(cat_id) else { continue };
        let tweaks: Vec<&'static Tweak> = def
            .visible_tweaks()
            .filter(|t| t.category == cat_id)
            .filter(|t| match filter {
                TweakFilter::All => true,
                TweakFilter::Staged => game.pending.contains_key(t.id),
                TweakFilter::Modified => game.effective(t) != t.default.to_value(),
            })
            .collect();
        if tweaks.is_empty() && filter != TweakFilter::All {
            continue;
        }
        let mut list = ui::panel().flex().flex_col();
        for (i, tweak) in tweaks.iter().enumerate() {
            if i > 0 {
                list = list.child(div().h(px(1.)).bg(theme::line()));
            }
            list = list.child(tweak_row(tweak, ws, cx));
        }
        page = page.child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(ui::section_title(category.title, Some(category.blurb.into())))
                .child(list),
        );
    }
    page.into_any_element()
}

fn filter_bar(ws: &Entity<Workspace>, current: TweakFilter) -> impl IntoElement {
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(ui::label("Show").mr(px(4.)));
    for (filter, text) in [
        (TweakFilter::All, "Everything"),
        (TweakFilter::Modified, "Changed from default"),
        (TweakFilter::Staged, "Staged"),
    ] {
        let ws = ws.clone();
        row = row.child(
            ui::chip(SharedString::from(format!("filter-{text}")), text, current == filter).on_click(
                move |_, _, cx| {
                    ws.update(cx, |ws, cx| {
                        ws.tweak_filter = filter;
                        cx.notify();
                    })
                },
            ),
        );
    }
    row
}

fn impact_rarity(impact: Impact) -> Rarity {
    match impact {
        Impact::None => Rarity::Uncommon,
        Impact::Low => Rarity::Rare,
        Impact::Medium => Rarity::Epic,
        Impact::High => Rarity::Legendary,
    }
}

fn location(tweak: &Tweak, ws: &Workspace) -> String {
    match tweak.binding {
        Binding::Keys(keys, _) => {
            let k = keys[0];
            let file = ws
                .game()
                .def
                .ini_files
                .iter()
                .find(|(id, _)| *id == k.file)
                .map(|(_, name)| *name)
                .unwrap_or(k.file);
            let mirrored = if keys.len() > 1 { "  (+ launcher copy)" } else { "" };
            format!("{file}  ›  [{}]  ›  {}{mirrored}", k.section, k.key)
        }
        Binding::Custom { .. } => "Multi-key tweak — Vault Patcher edits several entries for you".into(),
    }
}

pub(crate) fn tweak_row(tweak: &'static Tweak, ws: &Entity<Workspace>, cx: &App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let staged = game.pending.contains_key(tweak.id);
    let value = game.effective(tweak);
    let default = tweak.default.to_value();
    let on_disk = game.current(tweak);

    let rarity = impact_rarity(tweak.impact);
    let mut badges = div()
        .flex()
        .flex_wrap()
        .gap(px(6.))
        .child(ui::badge(tweak.impact.label(), rarity.color()));
    if staged {
        badges = badges.child(ui::badge("Staged", theme::accent()));
    }
    if tweak.flags.menu_managed {
        badges = badges.child(ui::badge("Also in game menu", theme::echo()));
    }
    if tweak.flags.experimental {
        badges = badges.child(ui::badge("Experimental", theme::danger()));
    }
    if matches!(on_disk, Some(Value::Unknown(_))) && !staged {
        badges = badges.child(ui::badge("Custom value", Rarity::Pearlescent.color()));
    }

    let info = div()
        .flex_1()
        .min_w(px(240.))
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div()
                .flex()
                .items_center()
                .flex_wrap()
                .gap(px(10.))
                .child(
                    div()
                        .font_family(theme::FONT_LABEL)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(15.5))
                        .text_color(theme::text())
                        .child(tweak.label),
                )
                .child(badges),
        )
        .child(ui::body(tweak.description))
        .children(super::compare::strip(tweak, ws, cx))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(14.))
                .text_size(px(11.))
                .text_color(theme::text_dim())
                .child(div().font_family(theme::FONT_MONO).child(location(tweak, state)))
                .child(format!("Default: {}", default.display(&tweak.control))),
        );

    let reset_ws = ws.clone();
    let reset = ui::icon_button(SharedString::from(format!("reset-{}", tweak.id)), Icon::Undo, theme::text_dim())
        .when(value == default, |d| d.invisible())
        .on_click(move |_, _, cx| {
            reset_ws.update(cx, |ws, cx| ws.stage(tweak, tweak.default.to_value(), cx));
        });

    div()
        .flex()
        .child(div().w(px(4.)).flex_none().bg(if staged { theme::accent() } else { theme::panel() }))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(18.))
                .px(px(16.))
                .py(px(14.))
                .child(info)
                .child(
                    div()
                        .w(px(380.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(8.))
                        .child(control(tweak, &value, ws, false))
                        .child(reset),
                ),
        )
        .into_any_element()
}

pub(crate) fn control(tweak: &'static Tweak, value: &Value, ws: &Entity<Workspace>, instant: bool) -> AnyElement {
    match tweak.control {
        Control::Toggle => {
            let on = matches!(value, Value::Bool(true));
            let ws = ws.clone();
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .font_family(theme::FONT_LABEL)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.))
                        .text_color(if on { theme::accent() } else { theme::text_dim() })
                        .child(if on { "ON" } else { "OFF" }),
                )
                .child(
                    ui::toggle(SharedString::from(format!("t-{}", tweak.id)), on).on_click(move |_, _, cx| {
                        ws.update(cx, |ws, cx| commit(ws, tweak, Value::Bool(!on), instant, cx));
                    }),
                )
                .into_any_element()
        }
        Control::Slider { min, max, step, .. } => {
            let n = match value {
                Value::Num(n) => *n,
                _ => tweak.default.to_value().as_num().unwrap_or(min),
            };
            let fraction = ((n - min) / (max - min)) as f32;
            let slide_ws = ws.clone();
            let minus_ws = ws.clone();
            let plus_ws = ws.clone();
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::slider(
                    format!("s-{}", tweak.id),
                    fraction,
                    theme::accent(),
                    move |f, _, cx| {
                        let v = min + (max - min) * f as f64;
                        slide_ws.update(cx, |ws, cx| commit(ws, tweak, Value::Num(v), instant, cx));
                    },
                ))
                .child(
                    div()
                        .w(px(78.))
                        .h(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme::bg_deep())
                        .border_2()
                        .border_color(theme::ink())
                        .font_family(theme::FONT_MONO)
                        .text_size(px(12.))
                        .text_color(theme::text())
                        .child(value.display(&tweak.control)),
                )
                .child(
                    ui::chip(SharedString::from(format!("m-{}", tweak.id)), "−", false).on_click(move |_, _, cx| {
                        minus_ws.update(cx, |ws, cx| commit(ws, tweak, Value::Num(n - step), instant, cx));
                    }),
                )
                .child(
                    ui::chip(SharedString::from(format!("p-{}", tweak.id)), "+", false).on_click(move |_, _, cx| {
                        plus_ws.update(cx, |ws, cx| commit(ws, tweak, Value::Num(n + step), instant, cx));
                    }),
                )
                .into_any_element()
        }
        Control::Choice(options) => {
            let mut chips = div().flex().flex_wrap().justify_end().gap(px(6.));
            for o in options {
                let selected = matches!(value, Value::Choice(v) if *v == o.value);
                let ws = ws.clone();
                let v = o.value;
                chips = chips.child(
                    ui::chip(SharedString::from(format!("c-{}-{}", tweak.id, o.value)), o.label, selected)
                        .on_click(move |_, _, cx| {
                            ws.update(cx, |ws, cx| commit(ws, tweak, Value::Choice(v), instant, cx));
                        }),
                );
            }
            if let Value::Unknown(raw) = value {
                chips = chips.child(ui::badge(format!("Now: {raw}"), Rarity::Pearlescent.color()));
            }
            chips.into_any_element()
        }
    }
}

/// Advanced mode stages edits; Simple mode saves them right away.
fn commit(ws: &mut Workspace, tweak: &'static Tweak, value: Value, instant: bool, cx: &mut gpui::Context<Workspace>) {
    if instant {
        ws.set_now(tweak, value, cx);
    } else {
        ws.stage(tweak, value, cx);
    }
}
