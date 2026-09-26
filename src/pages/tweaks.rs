//! Settings pages: a searchable list of compact rows on the left and a detail
//! pane on the right (full description, comparison slider, file location,
//! reset). Advanced tweak pages and Simple's Quick Settings share this view.

use gpui::{AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::Sizable as _;
use gpui_component::button::Button;
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};

use crate::games::NavItem;
use crate::theme::{self, Icon, Rarity};
use crate::tweaks::{Binding, Control, Impact, Tweak, Value};
use crate::ui::{self, Variant};
use crate::workspace::{TweakFilter, Workspace};

/// Width of the detail pane.
pub const DETAIL_W: f32 = 348.;

/// A titled group of rows.
pub struct Group {
    pub title: SharedString,
    pub blurb: SharedString,
    pub tweaks: Vec<&'static Tweak>,
    /// Shown between the title and the rows (e.g. quality presets).
    pub extra: Option<AnyElement>,
}

pub fn render(nav: &NavItem, ws: &Entity<Workspace>, window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let filter = state.tweak_filter;
    let query = state.search.trim().to_lowercase();
    let searching = !query.is_empty();
    // Searching looks across every category, not just this page's.
    let categories: Vec<&'static str> = if searching { def.categories.iter().map(|c| c.id).collect() } else { nav.categories.to_vec() };
    let groups: Vec<Group> = categories
        .iter()
        .filter_map(|cat_id| {
            let category = def.category(cat_id)?;
            let tweaks: Vec<&'static Tweak> = def
                .visible_tweaks()
                .filter(|t| t.category == *cat_id)
                .filter(|t| matches_query(t, category.title, &query))
                .filter(|t| match filter {
                    TweakFilter::All => true,
                    TweakFilter::Staged => game.pending.contains_key(t.id),
                    TweakFilter::Modified => game.effective(t) != t.default.to_value(),
                })
                .collect();
            (!tweaks.is_empty()).then(|| Group { title: category.title.into(), blurb: category.blurb.into(), tweaks, extra: None })
        })
        .collect();

    let categories_for_reset = nav.categories;
    let reset_count = def
        .visible_tweaks()
        .filter(|t| categories_for_reset.contains(&t.category) && game.effective(t) != t.default.to_value())
        .count();
    let reset_ws = ws.clone();
    let reset = ui::button("reset-page", "Reset page", Some(Icon::Undo), Variant::Ghost)
        .when(reset_count == 0, |b| b.opacity(0.4))
        .tooltip(ui::tip("Set everything on this page back to the game's defaults (as waiting changes)"))
        .on_click(move |_, window, cx| {
            if reset_count == 0 {
                return;
            }
            let ws = reset_ws.clone();
            super::confirm(
                window,
                cx,
                &format!("Reset {reset_count} setting(s) on this page to the game's defaults?"),
                "They're added to your waiting changes; nothing is written until you press Apply.",
                "Reset",
                move |cx| ws.update(cx, |ws, cx| ws.stage_defaults(categories_for_reset, cx)),
            );
        })
        .into_any_element();

    let mut filters = None;
    {
        let (wrap, segments) = ui::segmented(
            [(TweakFilter::All, "All"), (TweakFilter::Modified, "Changed"), (TweakFilter::Staged, "Waiting")]
                .iter()
                .map(|(f, t)| (SharedString::from(format!("filter-{t}")), SharedString::from(*t), *f == filter))
                .collect(),
        );
        let mut wrap = wrap;
        for (seg, f) in segments.into_iter().zip([TweakFilter::All, TweakFilter::Modified, TweakFilter::Staged]) {
            let ws = ws.clone();
            wrap = wrap.child(seg.on_click(move |_, _, cx| {
                ws.update(cx, |ws, cx| {
                    ws.tweak_filter = f;
                    cx.notify();
                })
            }));
        }
        filters.replace(wrap.into_any_element());
    }

    let subtitle = if searching {
        format!("Search results across all {} settings", def.visible_tweaks().count())
    } else {
        nav.categories.iter().filter_map(|c| def.category(c)).map(|c| c.title).collect::<Vec<_>>().join(" · ")
    };
    let notice = (!game.config_found()).then(|| {
        format!(
            "No config files found in {}. Launch {} once so it creates them, or set the folder in App Settings.",
            game.config_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Documents\\My Games".into()),
            def.name
        )
    });
    let title = if searching { "Search results" } else { nav.title };
    view(ws, title, &subtitle, groups, false, filters, Some(reset), notice, window, cx)
}

fn matches_query(tweak: &Tweak, category: &str, query: &str) -> bool {
    query.is_empty()
        || query.split_whitespace().all(|word| {
            tweak.label.to_lowercase().contains(word)
                || tweak.description.to_lowercase().contains(word)
                || category.to_lowercase().contains(word)
                || tweak.id.contains(word)
                || match tweak.binding {
                    Binding::Keys(keys, _) => keys.iter().any(|k| k.key.to_lowercase().contains(word)),
                    Binding::Custom { .. } => false,
                }
        })
}

/// Quick Settings reuses the search matcher.
pub(crate) fn search_filter(tweak: &Tweak, category: &str, query: &str) -> bool {
    matches_query(tweak, category, query)
}

/// The shared two-pane layout.
#[allow(clippy::too_many_arguments)]
pub(crate) fn view(
    ws: &Entity<Workspace>,
    title: &str,
    subtitle: &str,
    groups: Vec<Group>,
    instant: bool,
    filters: Option<AnyElement>,
    action: Option<AnyElement>,
    notice: Option<String>,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let visible: Vec<&'static Tweak> = groups.iter().flat_map(|g| g.tweaks.iter().copied()).collect();
    // Keep the selection if it's on screen; otherwise prefer a setting with pictures.
    let selected = state
        .selected_tweak
        .and_then(|id| visible.iter().copied().find(|t| t.id == id))
        .or_else(|| visible.iter().copied().find(|t| crate::compare::images(def.id, t).len() >= 2))
        .or_else(|| visible.first().copied());

    let search = state.search_input.clone().map(|input| {
        div()
            .w(px(280.))
            .child(Input::new(&input).cleanable(true).prefix(ui::icon(Icon::Search).size(px(14.)).text_color(theme::text_dim())))
    });

    let header = div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(12.))
        .px(px(24.))
        .pt(px(18.))
        .pb(px(12.))
        .border_b_1()
        .border_color(theme::line())
        .child(
            div()
                .flex()
                .items_end()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(ui::display(title.to_string(), 28.))
                        .child(div().text_size(px(12.)).text_color(theme::text_dim()).truncate().child(subtitle.to_string())),
                )
                .children(action),
        )
        .child(div().flex().items_center().gap(px(10.)).children(search).child(div().flex_1()).children(filters));

    let mut list = div().flex().flex_col().gap(px(18.)).px(px(24.)).pt(px(16.)).pb(px(32.));
    if let Some(n) = notice {
        list = list.child(
            ui::card(theme::danger()).child(ui::card_body().p(px(12.)).flex().gap(px(10.)).child(ui::icon(Icon::Alert).text_color(theme::danger())).child(ui::body(n))),
        );
    }
    if groups.is_empty() {
        list = list.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .py(px(48.))
                .child(ui::icon(Icon::Search).size(px(28.)).text_color(theme::text_dim()))
                .child(ui::body(if state.search.trim().is_empty() { "Nothing to show with this filter." } else { "No settings match your search." })),
        );
    }
    for group in groups {
        let mut rows = ui::panel().flex().flex_col();
        for (i, tweak) in group.tweaks.iter().enumerate() {
            if i > 0 {
                rows = rows.child(ui::divider());
            }
            rows = rows.child(row(tweak, selected.is_some_and(|s| s.id == tweak.id), ws, instant, cx));
        }
        list = list.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(ui::section_title(group.title, (!group.blurb.is_empty()).then_some(group.blurb)))
                .children(group.extra)
                .child(rows),
        );
    }

    let scroll_id = SharedString::from(format!("list-{}-{title}", def.id));
    div()
        .size_full()
        .flex()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .child(header)
                .child(div().id(scroll_id).flex_1().min_h_0().overflow_y_scroll().child(list)),
        )
        .child(
            div()
                .w(px(DETAIL_W))
                .flex_none()
                .h_full()
                .bg(theme::panel_lo())
                .border_l_1()
                .border_color(theme::line())
                .child(
                    div()
                        .id("detail-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .child(match selected {
                            Some(t) => detail(t, ws, instant, cx),
                            None => div().p(px(20.)).child(ui::body("Pick a setting to see what it does.")).into_any_element(),
                        }),
                ),
        )
        .into_any_element()
}

/// One compact row: status dot, name, one-line description, control.
fn row(tweak: &'static Tweak, selected: bool, ws: &Entity<Workspace>, instant: bool, cx: &App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let waiting = game.pending.contains_key(tweak.id);
    let value = game.effective(tweak);
    let changed = value != tweak.default.to_value();
    let has_pictures = crate::compare::images(game.def.id, tweak).len() >= 2;
    let select_ws = ws.clone();
    div()
        .id(SharedString::from(format!("row-{}", tweak.id)))
        .flex()
        .items_center()
        .gap(px(12.))
        .min_h(px(52.))
        .pl(px(10.))
        .pr(px(12.))
        .py(px(6.))
        .bg(if selected { theme::selected() } else { gpui::transparent_black().into() })
        .when(!selected, |d| d.hover(|s| s.bg(theme::panel_hi())))
        .cursor_pointer()
        .child(
            div()
                .size(px(6.))
                .flex_none()
                .rounded_full()
                .bg(if waiting {
                    theme::accent()
                } else if changed {
                    theme::text_dim()
                } else {
                    gpui::transparent_black().into()
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).text_color(theme::text()).child(tweak.label))
                        .when(has_pictures, |d| d.child(ui::icon(Icon::Picture).size(px(12.)).text_color(theme::text_dim())))
                        .when(tweak.flags.experimental, |d| d.child(ui::icon(Icon::Warning).size(px(12.)).text_color(theme::warning()))),
                )
                .child(div().text_size(px(12.)).text_color(theme::text_dim()).truncate().child(first_sentence(tweak.description))),
        )
        .child(div().flex_none().child(control(tweak, &value, ws, instant, true)))
        .on_click(move |_, _, cx| select_ws.update(cx, |ws, cx| ws.select_tweak(tweak.id, cx)))
        .into_any_element()
}

fn first_sentence(text: &str) -> &str {
    match text.find(". ") {
        Some(i) => &text[..=i],
        None => text,
    }
}

/// Everything about one setting.
fn detail(tweak: &'static Tweak, ws: &Entity<Workspace>, instant: bool, cx: &App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let value = game.effective(tweak);
    let default = tweak.default.to_value();
    let on_disk = game.current(tweak);
    let waiting = game.pending.get(tweak.id);
    let category = def.category(tweak.category).map_or("", |c| c.title);
    let advanced = state.mode() == crate::games::Mode::Advanced;

    let mut tags = div().flex().flex_wrap().gap(px(6.));
    if let Some(cost) = cost_tag(tweak.impact) {
        tags = tags.child(cost);
    }
    if tweak.flags.menu_managed {
        tags = tags.child(ui::badge("Also in the game's menu", theme::text_muted()));
    }
    if tweak.flags.experimental {
        tags = tags.child(ui::badge("Experimental", theme::warning()));
    }
    if matches!(on_disk, Some(Value::Unknown(_))) {
        tags = tags.child(ui::badge("Custom value in file", theme::echo()));
    }

    let values = div()
        .flex()
        .flex_col()
        .child(ui::kv_row("In your files", on_disk.as_ref().map_or("not set (default)".into(), |v| v.display(&tweak.control))))
        .child(ui::kv_row("Game default", default.display(&tweak.control)))
        .when_some(waiting, |d, w| d.child(ui::kv_row("Waiting to apply", w.display(&tweak.control)).text_color(theme::accent_text())));

    let reset_ws = ws.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .p(px(18.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(ui::label(category))
                .child(ui::display(tweak.label, 20.)),
        )
        .child(ui::body(tweak.description))
        .child(tags)
        .children(super::compare::inline_viewer(tweak, ws, DETAIL_W - 38., cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(12.))
                .rounded(px(theme::RADIUS_LG))
                .bg(theme::panel())
                .border_1()
                .border_color(theme::line())
                .child(ui::label(if instant { "Setting · saved instantly" } else { "Setting" }))
                .child(control(tweak, &value, ws, instant, false))
                .child(values)
                .when(value != default, |d| {
                    d.child(
                        ui::button(SharedString::from(format!("reset-{}", tweak.id)), "Reset to default", Some(Icon::Undo), Variant::Ghost)
                            .h(px(28.))
                            .on_click(move |_, _, cx| {
                                reset_ws.update(cx, |ws, cx| commit(ws, tweak, tweak.default.to_value(), instant, cx));
                            }),
                    )
                }),
        )
        .when(advanced, |d| {
            d.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(ui::label("Where it's stored"))
                    .child(div().font_family(theme::font_mono()).text_size(px(12.)).text_color(theme::text_muted()).child(location(tweak, state))),
            )
        })
        .into_any_element()
}

fn cost_tag(impact: Impact) -> Option<gpui::Div> {
    let (text, color) = match impact {
        Impact::None => return None,
        Impact::Low => ("Low FPS cost", Rarity::Uncommon.color()),
        Impact::Medium => ("Medium FPS cost", theme::warning()),
        Impact::High => ("High FPS cost", theme::danger()),
    };
    Some(ui::badge(text, color))
}

fn location(tweak: &Tweak, ws: &Workspace) -> String {
    match tweak.binding {
        Binding::Keys(keys, _) => {
            let k = keys[0];
            let file = ws.game().def.ini_files.iter().find(|(id, _)| *id == k.file).map(|(_, name)| *name).unwrap_or(k.file);
            let mirrored = if keys.len() > 1 { "\n+ launcher copy" } else { "" };
            format!("{file}\n[{}]\n{}{mirrored}", k.section, k.key)
        }
        Binding::Custom { .. } => "Several entries, edited together".into(),
    }
}

/// The control for a setting. `compact` is the in-row version.
pub(crate) fn control(tweak: &'static Tweak, value: &Value, ws: &Entity<Workspace>, instant: bool, compact: bool) -> AnyElement {
    match tweak.control {
        Control::Toggle => {
            let on = matches!(value, Value::Bool(true));
            let ws = ws.clone();
            ui::toggle(SharedString::from(format!("t-{}-{compact}", tweak.id)), on)
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    ws.update(cx, |ws, cx| {
                        ws.select_tweak(tweak.id, cx);
                        commit(ws, tweak, Value::Bool(!on), instant, cx)
                    });
                })
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
            let readout = div()
                .w(px(64.))
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .bg(theme::panel_lo())
                .border_1()
                .border_color(theme::line())
                .font_family(theme::font_mono())
                .text_size(px(12.))
                .text_color(theme::text())
                .child(value.display(&tweak.control));
            div()
                .id(SharedString::from(format!("sw-{}-{compact}", tweak.id)))
                .flex()
                .items_center()
                .gap(px(8.))
                .when(compact, |d| d.w(px(210.)))
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(ui::slider(format!("s-{}-{compact}", tweak.id), fraction, theme::accent(), move |f, _, cx| {
                    let v = min + (max - min) * f as f64;
                    slide_ws.update(cx, |ws, cx| commit(ws, tweak, Value::Num(v), instant, cx));
                }))
                .child(readout)
                .when(!compact, |d| {
                    d.child(
                        ui::icon_button(SharedString::from(format!("m-{}", tweak.id)), Icon::Minus, theme::text_muted())
                            .on_click(move |_, _, cx| minus_ws.update(cx, |ws, cx| commit(ws, tweak, Value::Num(n - step), instant, cx))),
                    )
                    .child(
                        ui::icon_button(SharedString::from(format!("p-{}", tweak.id)), Icon::Add, theme::text_muted())
                            .on_click(move |_, _, cx| plus_ws.update(cx, |ws, cx| commit(ws, tweak, Value::Num(n + step), instant, cx))),
                    )
                })
                .into_any_element()
        }
        Control::Choice(options) => {
            let label_len: usize = options.iter().map(|o| o.label.len()).sum();
            let short = options.len() <= 3 && label_len <= 24;
            if short || !compact {
                // Segmented (row) or wrapped chips (detail pane).
                let mut chips = div()
                    .id(SharedString::from(format!("cw-{}-{compact}", tweak.id)))
                    .flex()
                    .when(!compact, |d| d.flex_wrap())
                    .gap(px(4.))
                    .on_click(|_, _, cx| cx.stop_propagation());
                for o in options {
                    let selected = matches!(value, Value::Choice(v) if *v == o.value);
                    let ws = ws.clone();
                    let v = o.value;
                    chips = chips.child(ui::chip(SharedString::from(format!("c-{}-{}-{compact}", tweak.id, o.value)), o.label, selected).on_click(
                        move |_, _, cx| {
                            ws.update(cx, |ws, cx| {
                                ws.select_tweak(tweak.id, cx);
                                commit(ws, tweak, Value::Choice(v), instant, cx)
                            });
                        },
                    ));
                }
                if let Value::Unknown(raw) = value {
                    chips = chips.child(ui::badge(format!("Custom: {raw}"), theme::echo()));
                }
                return chips.into_any_element();
            }
            // Long lists: a dropdown.
            let current = match value {
                Value::Unknown(raw) => format!("Custom: {raw}"),
                v => v.display(&tweak.control),
            };
            let ws = ws.clone();
            let value = value.clone();
            Button::new(SharedString::from(format!("dd-{}", tweak.id)))
                .outline()
                .dropdown_caret(true)
                .label(current)
                .small()
                .h(px(28.))
                .min_w(px(150.))
                .dropdown_menu(move |mut menu, _, _| {
                    for o in options {
                        let ws = ws.clone();
                        let v = o.value;
                        menu = menu.item(
                            PopupMenuItem::new(o.label)
                                .checked(matches!(value, Value::Choice(c) if c == o.value))
                                .on_click(move |_, _, cx| {
                                    ws.update(cx, |ws, cx| {
                                        ws.select_tweak(tweak.id, cx);
                                        commit(ws, tweak, Value::Choice(v), instant, cx)
                                    })
                                }),
                        );
                    }
                    menu
                })
                .into_any_element()
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
