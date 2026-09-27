//! Settings pages: a searchable list of compact rows on the left and a detail
//! pane on the right (full description, comparison slider, file location,
//! reset). Advanced tweak pages and Simple's Quick settings share this view.

use std::rc::Rc;

use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};

use crate::controls;
use crate::games::{GameDef, NavItem, PageKind};
use crate::theme::{self, Icon};
use crate::tweaks::{Binding, Control, RangePair, Tweak, Value, order_range, with_unit};
use crate::ui::{self, Severity, Variant};
use crate::workspace::{TweakFilter, Workspace};

/// Width of the detail pane.
pub const DETAIL_W: f32 = 348.;
/// The detail pane's padding.
const DETAIL_PAD: f32 = 16.;
/// Width of a row's slider control: a 160px rail and an 80px readout
/// (wide enough for "10,000" and "Unlimited").
const ROW_SLIDER_W: f32 = 248.;

/// A titled group of rows.
pub struct Group {
    pub title: SharedString,
    pub blurb: SharedString,
    pub tweaks: Vec<&'static Tweak>,
    /// Shown between the title and the rows (e.g. quality presets).
    pub extra: Option<AnyElement>,
}

/// An InfoBar above the list (title, message), with a "Set folder" action.
pub struct Notice {
    pub title: String,
    pub message: String,
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
    let reset = ui::button_if(reset_count > 0, "reset-page", "Reset page", Some(Icon::Undo), Variant::Secondary)
        .tooltip(ui::tip(if reset_count > 0 {
            "Set everything on this page back to the game's defaults (as waiting changes)"
        } else {
            "Everything on this page is at the game's defaults"
        }))
        .on_click(move |_, window, cx| {
            let ws = reset_ws.clone();
            super::confirm(
                window,
                cx,
                &format!("Reset {} on this page to the game's defaults?", plural(reset_count, "setting")),
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
    let notice = (!game.config_found()).then(|| Notice {
        title: "Config files not found".into(),
        message: format!(
            "Nothing in {}. Launch {} once so it creates them, or set the folder in App settings.",
            game.config_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Documents\\My Games".into()),
            def.name
        ),
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

/// Quick settings reuses the search matcher.
pub(crate) fn search_filter(tweak: &Tweak, category: &str, query: &str) -> bool {
    matches_query(tweak, category, query)
}

/// "1 setting", "3 settings".
pub(crate) fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
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
    notice: Option<Notice>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    // The page gutter: 36 like the other pages, 24 in smaller windows.
    let gutter = px(if window.viewport_size().width >= px(1008.) { 36. } else { 24. });
    let (def, selected, search_input, query_empty) = {
        let state = ws.read(cx);
        let def = state.game().def;
        let visible: Vec<&'static Tweak> = groups.iter().flat_map(|g| g.tweaks.iter().copied()).collect();
        // Keep the selection if it's on screen; otherwise prefer a setting with pictures.
        let selected = state
            .selected_tweak
            .and_then(|id| visible.iter().copied().find(|t| t.id == id))
            .or_else(|| visible.iter().copied().find(|t| crate::compare::images(def.id, t).len() >= 2))
            .or_else(|| visible.first().copied());
        (def, selected, state.search_input.clone(), state.search.trim().is_empty())
    };

    let search = search_input.map(|input| {
        div()
            .w(px(280.))
            .flex_none()
            .child(controls::text_box(&input, Some(ui::icon(Icon::Search).size(px(14.)).text_color(theme::text_muted()).into_any_element()), window, cx))
    });

    // Title block, then search and filters: 116px on every settings page.
    let subtitle: SharedString = subtitle.to_string().into();
    let header = div()
        .flex_none()
        .h(px(116.))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(12.))
        .px(gutter)
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
                        .child(controls::caption_line("page-subtitle", subtitle.clone()).tooltip(ui::tip(subtitle))),
                )
                .children(action),
        )
        .child(div().h(px(32.)).flex().items_center().gap(px(12.)).children(search).child(div().flex_1()).children(filters));

    let mut list = div().flex().flex_col().gap(px(16.)).px(gutter).pt(px(16.)).pb(px(36.));
    if let Some(n) = notice {
        let ws = ws.clone();
        list = list.child(
            ui::info_bar(Severity::Error, n.title, n.message).child(
                ui::button("set-config-folder", "Set folder", Some(Icon::Folder), Variant::Secondary)
                    .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.navigate(PageKind::Settings, cx))),
            ),
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
                .child(ui::icon(Icon::Search).size(px(32.)).text_color(theme::text_muted()))
                .child(ui::body(if query_empty { "Nothing to show with this filter." } else { "No settings match your search." })),
        );
    }
    for group in groups {
        let mut rows = ui::panel().flex().flex_col();
        let mut first = true;
        for tweak in &group.tweaks {
            // Both ends of a range on this page make one RangeSlider row.
            let pair = def
                .range_of(tweak.id)
                .filter(|p| group.tweaks.iter().any(|t| t.id == p.min) && group.tweaks.iter().any(|t| t.id == p.max));
            let element = match pair {
                Some(p) if p.max == tweak.id => continue,
                Some(p) => range_row(p, selected.is_some_and(|s| s.id == p.min || s.id == p.max), ws, instant, cx),
                None => row(tweak, selected.is_some_and(|s| s.id == tweak.id), ws, instant, cx),
            };
            if !first {
                rows = rows.child(ui::divider());
            }
            first = false;
            rows = rows.child(element);
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

    let detail_pane = match selected {
        Some(t) => match def.range_of(t.id) {
            Some(pair) => range_detail(pair, ws, instant, window, cx),
            None => detail(t, ws, instant, window, cx),
        },
        None => div().p(px(DETAIL_PAD)).child(ui::body("Pick a setting to see what it does.")).into_any_element(),
    };

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
                .child(div().id("detail-scroll").size_full().overflow_y_scroll().child(detail_pane)),
        )
        .into_any_element()
}

// ---- rows ---------------------------------------------------------------------------

/// The shared row: 56px, 16/8 padding, status dot in the left padding, the
/// text, a reset button (on hover) and the control.
fn row_frame(
    id: SharedString,
    selected: bool,
    dot: Option<AnyElement>,
    text: gpui::Div,
    reset: Option<AnyElement>,
    control: AnyElement,
    on_select: impl Fn(&mut App) + 'static,
) -> AnyElement {
    div()
        .id(id.clone())
        .group(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(12.))
        .min_h(px(56.))
        .px(px(16.))
        .py(px(8.))
        .bg(if selected { theme::selected() } else { gpui::transparent_black().into() })
        .when(!selected, |d| d.hover(|s| s.bg(theme::panel_hi())))
        .cursor_pointer()
        .children(dot)
        .child(text)
        .children(reset)
        .child(div().flex_none().child(control))
        .on_click(move |_, _, cx| on_select(cx))
        .into_any_element()
}

/// Title (with picture/experimental glyphs and the FPS-cost meter) over a
/// one-line description that ends in an ellipsis and shows in full on hover.
fn row_text(id: &str, label: &'static str, description: &'static str, has_pictures: bool, tweak: Option<&'static Tweak>) -> gpui::Div {
    let experimental = tweak.is_some_and(|t| t.flags.experimental);
    let impact = tweak.map_or(crate::tweaks::Impact::None, |t| t.impact);
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
                .min_w_0()
                .child(ui::title(label).flex_1().min_w_0().line_clamp(1).text_ellipsis())
                .when(has_pictures, |d| {
                    d.child(
                        div()
                            .id(SharedString::from(format!("pic-{id}")))
                            .flex_none()
                            .child(ui::icon(Icon::Picture).size(px(12.)).text_color(theme::text_muted()))
                            .tooltip(ui::tip("Has comparison pictures")),
                    )
                })
                .when(experimental, |d| {
                    d.child(
                        div()
                            .id(SharedString::from(format!("exp-{id}")))
                            .flex_none()
                            .child(ui::icon(Icon::Warning).size(px(12.)).text_color(theme::warning()))
                            .tooltip(ui::tip("Experimental")),
                    )
                })
                .children(controls::cost_meter(SharedString::from(format!("cost-{id}")), impact)),
        )
        .child(controls::caption_line(SharedString::from(format!("desc-{id}")), first_sentence(description)).tooltip(ui::tip(description)))
}

/// One compact row: status dot, name, one-line description, control.
fn row(tweak: &'static Tweak, selected: bool, ws: &Entity<Workspace>, instant: bool, cx: &App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let waiting = game.pending.contains_key(tweak.id);
    let value = game.effective(tweak);
    let default = tweak.default.to_value();
    let changed = value != default;
    let has_pictures = crate::compare::images(game.def.id, tweak).len() >= 2;
    let id = SharedString::from(format!("row-{}", tweak.id));
    let reset = changed.then(|| {
        let ws = ws.clone();
        let label = default.display(&tweak.control);
        controls::reset_button(SharedString::from(format!("rr-{}", tweak.id)), id.clone(), format!("Reset to {label} (game default)"))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                ws.update(cx, |ws, cx| {
                    ws.select_tweak(tweak.id, cx);
                    commit(ws, tweak, default.clone(), instant, cx)
                });
            })
            .into_any_element()
    });
    let select_ws = ws.clone();
    row_frame(
        id,
        selected,
        controls::status_dot(SharedString::from(format!("dot-{}", tweak.id)), waiting, changed),
        row_text(tweak.id, tweak.label, tweak.description, has_pictures, Some(tweak)),
        reset,
        row_control(tweak, &value, ws, instant),
        move |cx| select_ws.update(cx, |ws, cx| ws.select_tweak(tweak.id, cx)),
    )
}

fn first_sentence(text: &str) -> &str {
    match text.find(". ") {
        Some(i) => &text[..=i],
        None => text,
    }
}

// ---- controls -----------------------------------------------------------------------

fn num_or_default(tweak: &Tweak, value: &Value) -> f64 {
    match (value, tweak.control) {
        (Value::Num(n), _) => *n,
        (_, Control::Slider { min, .. }) => tweak.default.to_value().as_num().unwrap_or(min),
        _ => 0.,
    }
}

/// The slider of a numeric setting (`suffix` keeps row and detail apart).
fn slider(tweak: &'static Tweak, n: f64, ws: &Entity<Workspace>, instant: bool, suffix: &str) -> AnyElement {
    let Control::Slider { min, max, step, recommended, .. } = tweak.control else { return div().into_any_element() };
    let span = (max - min).max(f64::EPSILON);
    let to_value = move |f: f32| tweak.clamp(Value::Num(min + span * f as f64));
    let fraction = |v: f64| ((v - min) / span) as f32;
    let steps = (step > 0.).then(|| (span / step).round()).filter(|s| *s >= 1. && *s <= 1000.).map(|s| s as u32);
    let select_ws = ws.clone();
    let slide_ws = ws.clone();
    let spec = ui::SliderSpec {
        id: slider_id(tweak, suffix),
        fraction: fraction(n),
        steps,
        default: tweak.default.to_value().as_num().map(fraction),
        recommended: recommended.map(|(lo, hi)| (fraction(lo), fraction(hi))),
        accent: theme::accent(),
    };
    ui::slider(
        spec,
        move |_, cx| select_ws.update(cx, |ws, cx| ws.select_tweak(tweak.id, cx)),
        move |f| to_value(f).display(&tweak.control).into(),
        move |f, _, cx| slide_ws.update(cx, |ws, cx| commit(ws, tweak, to_value(f), instant, cx)),
    )
}

fn slider_id(tweak: &Tweak, suffix: &str) -> SharedString {
    SharedString::from(format!("s-{}-{suffix}", tweak.id))
}

/// What the slider shows now: the dragged position while dragging.
fn shown_value(tweak: &'static Tweak, value: &Value, suffix: &str) -> Value {
    let Control::Slider { min, max, .. } = tweak.control else { return value.clone() };
    ui::slider_preview(&slider_id(tweak, suffix))
        .map(|f| tweak.clamp(Value::Num(min + (max - min) * f as f64)))
        .unwrap_or_else(|| value.clone())
}

fn readout(tweak: &Tweak, shown: &Value) -> gpui::Div {
    let Control::Slider { unit, .. } = tweak.control else { return div() };
    match shown {
        Value::Num(n) if tweak.control.slider_label(*n).is_some() => controls::number_readout(tweak.control.number_text(*n), ""),
        Value::Num(n) => controls::number_readout(tweak.control.number_text(*n), unit),
        other => controls::number_readout(other.display(&tweak.control), ""),
    }
}

/// Picks an option and selects the row.
fn pick(tweak: &'static Tweak, ws: &Entity<Workspace>, instant: bool) -> controls::OnPick {
    let ws = ws.clone();
    Rc::new(move |v, _, cx| {
        ws.update(cx, |ws, cx| {
            ws.select_tweak(tweak.id, cx);
            commit(ws, tweak, Value::Choice(v), instant, cx)
        })
    })
}

/// Segmented for a few short options, a ComboBox otherwise (the same rule
/// in rows and the detail pane).
fn choice(tweak: &'static Tweak, value: &Value, ws: &Entity<Workspace>, instant: bool, fill: bool) -> AnyElement {
    let Control::Choice(options) = tweak.control else { return div().into_any_element() };
    let suffix = if fill { "detail" } else { "row" };
    let on_pick = pick(tweak, ws, instant);
    if controls::use_segmented(options) {
        let (wrap, segments) = controls::segmented(
            options
                .iter()
                .map(|o| {
                    let id = SharedString::from(format!("c-{}-{}-{suffix}", tweak.id, o.value));
                    (id.into(), SharedString::from(o.label), matches!(value, Value::Choice(v) if *v == o.value))
                })
                .collect(),
            fill,
        );
        let mut wrap = wrap;
        for (seg, o) in segments.into_iter().zip(options.iter()) {
            let on_pick = on_pick.clone();
            let v = o.value;
            wrap = wrap.child(seg.on_click(move |_, window, cx| {
                cx.stop_propagation();
                on_pick(v, window, cx)
            }));
        }
        return div()
            .id(SharedString::from(format!("cw-{}-{suffix}", tweak.id)))
            .flex()
            .flex_col()
            .gap(px(4.))
            .when(fill, |d| d.w_full())
            .child(wrap)
            .when_some(if let Value::Unknown(raw) = value { Some(raw.clone()) } else { None }, |d, raw| {
                d.child(ui::badge(format!("Custom: {raw}"), theme::echo()))
            })
            .on_click(|_, _, cx| cx.stop_propagation())
            .into_any_element();
    }
    let current = match value {
        Value::Unknown(raw) => format!("Custom: {raw}"),
        v => v.display(&tweak.control),
    };
    let selected = if let Value::Choice(v) = value { Some(*v) } else { None };
    div()
        .when(fill, |d| d.w_full().flex().flex_col())
        .child(controls::combo_box(SharedString::from(format!("dd-{}-{suffix}", tweak.id)), current, options, selected, on_pick))
        .into_any_element()
}

fn toggle(tweak: &'static Tweak, value: &Value, ws: &Entity<Workspace>, instant: bool, suffix: &str) -> AnyElement {
    let on = matches!(value, Value::Bool(true));
    let ws = ws.clone();
    ui::toggle(SharedString::from(format!("t-{}-{suffix}", tweak.id)), on)
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            ws.update(cx, |ws, cx| {
                ws.select_tweak(tweak.id, cx);
                commit(ws, tweak, Value::Bool(!on), instant, cx)
            });
        })
        .into_any_element()
}

/// The in-row control.
fn row_control(tweak: &'static Tweak, value: &Value, ws: &Entity<Workspace>, instant: bool) -> AnyElement {
    match tweak.control {
        Control::Toggle => toggle(tweak, value, ws, instant, "row"),
        Control::Slider { .. } => div()
            .id(SharedString::from(format!("sw-{}", tweak.id)))
            .w(px(ROW_SLIDER_W))
            .flex()
            .items_center()
            .gap(px(8.))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(slider(tweak, num_or_default(tweak, value), ws, instant, "row"))
            .child(readout(tweak, &shown_value(tweak, value, "row")).w(px(80.)))
            .into_any_element(),
        Control::Choice(_) => choice(tweak, value, ws, instant, false),
    }
}

/// The detail pane's control: full width; sliders get the rail on its own
/// line and an editable NumberBox below.
fn detail_control(tweak: &'static Tweak, value: &Value, ws: &Entity<Workspace>, instant: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let Control::Slider { min, step, recommended, .. } = tweak.control else {
        return match tweak.control {
            Control::Toggle => toggle(tweak, value, ws, instant, "detail"),
            _ => choice(tweak, value, ws, instant, true),
        };
    };
    let control = tweak.control;
    let n = num_or_default(tweak, value);
    let shown = shown_value(tweak, value, "detail").as_num().unwrap_or(n);
    let default = tweak.default.to_value().as_num();
    // Ordinal sliders (a handful of whole steps) label every tick.
    let ordinal = match tweak.control {
        Control::Slider { max, .. } if step >= 1. && ((max - min) / step).round() <= 5. => Some(((max - min) / step).round() as usize),
        _ => None,
    };
    let ticks = ordinal.map(|count| {
        let labels = (0..=count).map(|i| crate::tweaks::format_number(min + step * i as f64, 0)).collect();
        controls::step_labels(labels, Some(((shown - min) / step).round() as usize))
    });
    let box_ws = ws.clone();
    let number = controls::number_box(
        format!("nb-{}", tweak.id),
        n,
        control,
        Rc::new(move |v, _, cx| box_ws.update(cx, |ws, cx| commit(ws, tweak, tweak.clamp(Value::Num(v)), instant, cx))),
        window,
        cx,
    );
    let mut notes = Vec::new();
    if let Some(d) = default.filter(|d| (d - n).abs() > 1e-9) {
        notes.push(format!("Default {}", control.slider_text(d)));
    }
    if let Some((lo, hi)) = recommended {
        notes.push(format!("Recommended {}–{}", control.number_text(lo), control.slider_text(hi)));
    }
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().w_full().flex().child(slider(tweak, n, ws, instant, "detail")))
        .children(ticks)
        .child(
            div()
                .mt(px(8.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(div().w(px(128.)).flex_none().child(number))
                .child(div().flex_1().min_w_0().flex().flex_col().children(notes.into_iter().map(ui::caption))),
        )
        .into_any_element()
}

// ---- detail ---------------------------------------------------------------------------

/// The detail pane's frame: category, title, description, tags, then `body`.
fn detail_frame(category: &str, title: &str, description: &str, tags: gpui::Div, body: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(DETAIL_PAD))
        .child(div().flex().flex_col().gap(px(2.)).child(ui::label(category.to_string())).child(ui::display(title.to_string(), 20.)))
        .child(ui::body(description.to_string()))
        .child(tags)
        .child(body)
}

/// The "Setting" card: an in-page card (radius 4) with the control, the
/// values table and a Standard "Reset to default" button.
fn setting_card(instant: bool, control: AnyElement, values: gpui::Div, reset: AnyElement) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(theme::RADIUS))
        .bg(theme::panel())
        .border_1()
        .border_color(theme::card_stroke())
        .child(ui::label("Setting"))
        .child(control)
        .when(instant, |d| d.child(ui::caption("Saved as you change it")))
        .child(values)
        .child(div().flex().child(reset))
}

fn reset_default_button(id: String, enabled: bool, on_reset: impl Fn(&mut App) + 'static) -> AnyElement {
    ui::button_if(enabled, SharedString::from(id), "Reset to default", Some(Icon::Undo), Variant::Secondary)
        .when(!enabled, |b| b.tooltip(ui::tip("Already at the game's default")))
        .on_click(move |_, _, cx| on_reset(cx))
        .into_any_element()
}

/// File, section and key, in monospace (they're identifiers).
fn where_stored(rows: Vec<(&'static str, String)>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(ui::label("Where it's stored"))
        .map(|d| {
            if rows.is_empty() {
                d.child(ui::caption("Several entries, edited together"))
            } else {
                d.children(rows.into_iter().map(|(k, v)| ui::kv_row(k, v)))
            }
        })
}

/// Everything about one setting.
fn detail(tweak: &'static Tweak, ws: &Entity<Workspace>, instant: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let (value, default, on_disk, waiting, category, advanced, stored) = {
        let state = ws.read(cx);
        let game = state.game();
        (
            game.effective(tweak),
            tweak.default.to_value(),
            game.current(tweak),
            game.pending.get(tweak.id).cloned(),
            game.def.category(tweak.category).map_or("", |c| c.title),
            state.mode() == crate::games::Mode::Advanced,
            location(tweak, state),
        )
    };

    let mut tags = div().flex().flex_wrap().gap(px(6.));
    if let Some(cost) = controls::cost_badge(tweak.impact) {
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
        .child(controls::kv_row_text("In your files", on_disk.as_ref().map_or("Not set (default)".into(), |v| v.display(&tweak.control)), theme::text()))
        .child(controls::kv_row_text("Game default", default.display(&tweak.control), theme::text()))
        .when_some(waiting, |d, w| d.child(controls::kv_row_text("Waiting to apply", w.display(&tweak.control), theme::accent_text())));

    let viewer = super::compare::inline_viewer(tweak, ws, DETAIL_W - DETAIL_PAD * 2. - 1., cx);
    let control = detail_control(tweak, &value, ws, instant, window, cx);
    let reset_ws = ws.clone();
    let reset = reset_default_button(format!("reset-{}", tweak.id), value != default, move |cx| {
        reset_ws.update(cx, |ws, cx| commit(ws, tweak, tweak.default.to_value(), instant, cx))
    });
    detail_frame(
        category,
        tweak.label,
        tweak.description,
        tags,
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .children(viewer)
            .child(setting_card(instant, control, values, reset))
            .when(advanced, |d| d.child(where_stored(stored))),
    )
    .into_any_element()
}

// ---- ranges ---------------------------------------------------------------------------

/// Both ends of a range, with the shared rail's scale.
struct Range {
    lo: &'static Tweak,
    hi: &'static Tweak,
    /// The rail runs from `start` over `span` in steps of `step`.
    start: f64,
    span: f64,
    step: f64,
}

impl Range {
    fn of(pair: &RangePair, def: &GameDef) -> Option<Range> {
        let (lo, hi) = (def.tweak(pair.min)?, def.tweak(pair.max)?);
        let (Control::Slider { min: a, max: b, step: s1, .. }, Control::Slider { min: c, max: d, step: s2, .. }) = (lo.control, hi.control) else {
            return None;
        };
        let start = a.min(c);
        let step = if s1 > 0. && s2 > 0. { s1.min(s2) } else { s1.max(s2) };
        Some(Range { lo, hi, start, span: (b.max(d) - start).max(f64::EPSILON), step })
    }

    fn fraction(&self, v: f64) -> f32 {
        ((v - self.start) / self.span) as f32
    }

    fn value(&self, f: f32) -> f64 {
        self.start + self.span * f as f64
    }

    fn bounds(&self, t: &Tweak) -> (f32, f32) {
        match t.control {
            Control::Slider { min, max, .. } => (self.fraction(min), self.fraction(max)),
            _ => (0., 1.),
        }
    }

    /// "22–62 fps".
    fn text(&self, lo: f64, hi: f64) -> String {
        let unit = match self.hi.control {
            Control::Slider { unit, .. } => unit,
            _ => "",
        };
        format!("{}–{}", self.lo.control.number_text(lo), with_unit(self.hi.control.number_text(hi), unit))
    }
}

fn range_values(range: &Range, ws: &Entity<Workspace>, cx: &App) -> (f64, f64) {
    let game = ws.read(cx).game();
    (num_or_default(range.lo, &game.effective(range.lo)), num_or_default(range.hi, &game.effective(range.hi)))
}

/// The RangeSlider for a pair; also returns the text it shows now.
fn range_control(range: &Range, ws: &Entity<Workspace>, instant: bool, suffix: &str, cx: &App) -> (AnyElement, String) {
    let (lo, hi) = range_values(range, ws, cx);
    let id = SharedString::from(format!("range-{}-{suffix}", range.lo.id));
    let (shown_lo, shown_hi) = controls::range_preview(&id).map_or((lo, hi), |(l, h)| {
        (range.lo.control.clamp_num(range.value(l)), range.hi.control.clamp_num(range.value(h)))
    });
    let text = range.text(shown_lo, shown_hi);
    let steps = (range.step > 0.).then(|| (range.span / range.step).round() as u32);
    let (lo_t, hi_t, start, span) = (range.lo, range.hi, range.start, range.span);
    let select_ws = ws.clone();
    let commit_ws = ws.clone();
    let element = controls::range_slider(
        controls::RangeSpec {
            id,
            lo: range.fraction(lo),
            hi: range.fraction(hi),
            steps,
            lo_bounds: range.bounds(lo_t),
            hi_bounds: range.bounds(hi_t),
        },
        Rc::new(move |_, cx| select_ws.update(cx, |ws, cx| ws.select_tweak(lo_t.id, cx))),
        Rc::new(move |f| hi_t.control.slider_text(hi_t.control.clamp_num(start + span * f as f64)).into()),
        Rc::new(move |l, h, moved_lo, _, cx| {
            let (l, h) = (lo_t.control.clamp_num(start + span * l as f64), hi_t.control.clamp_num(start + span * h as f64));
            let (l, h) = order_range(l, h, moved_lo);
            commit_ws.update(cx, |ws, cx| {
                ws.select_tweak(lo_t.id, cx);
                if moved_lo {
                    commit(ws, lo_t, Value::Num(l), instant, cx)
                } else {
                    commit(ws, hi_t, Value::Num(h), instant, cx)
                }
            });
        }),
    );
    (element, text)
}

/// One row for both ends of a range.
fn range_row(pair: &'static RangePair, selected: bool, ws: &Entity<Workspace>, instant: bool, cx: &App) -> AnyElement {
    let def = ws.read(cx).game().def;
    let Some(range) = Range::of(pair, def) else { return div().into_any_element() };
    let (waiting, changed, both_default) = {
        let game = ws.read(cx).game();
        let waiting = game.pending.contains_key(range.lo.id) || game.pending.contains_key(range.hi.id);
        let changed = [range.lo, range.hi].iter().any(|t| game.effective(t) != t.default.to_value());
        (waiting, changed, (range.lo.default.to_value(), range.hi.default.to_value()))
    };
    let id = SharedString::from(format!("row-{}", pair.min));
    let (slider, text) = range_control(&range, ws, instant, "row", cx);
    let reset = changed.then(|| {
        let ws = ws.clone();
        let (lo_t, hi_t) = (range.lo, range.hi);
        let (lo_d, hi_d) = both_default.clone();
        let label = range.text(lo_d.as_num().unwrap_or(0.), hi_d.as_num().unwrap_or(0.));
        controls::reset_button(SharedString::from(format!("rr-{}", pair.min)), id.clone(), format!("Reset to {label} (game default)"))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                ws.update(cx, |ws, cx| {
                    ws.select_tweak(lo_t.id, cx);
                    commit(ws, lo_t, lo_d.clone(), instant, cx);
                    commit(ws, hi_t, hi_d.clone(), instant, cx);
                });
            })
            .into_any_element()
    });
    let select_ws = ws.clone();
    let lo_id = range.lo.id;
    row_frame(
        id,
        selected,
        controls::status_dot(SharedString::from(format!("dot-{}", pair.min)), waiting, changed),
        row_text(pair.min, pair.label, pair.description, false, Some(range.hi)),
        reset,
        div()
            .id(SharedString::from(format!("rw-{}", pair.min)))
            .w(px(ROW_SLIDER_W + 24.))
            .flex()
            .items_center()
            .gap(px(8.))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(div().flex_1().min_w(px(160.)).flex().child(slider))
            .child(controls::number_readout(text, ""))
            .into_any_element(),
        move |cx| select_ws.update(cx, |ws, cx| ws.select_tweak(lo_id, cx)),
    )
}

/// The detail pane for a range: the RangeSlider full width, then a
/// NumberBox for each end.
fn range_detail(pair: &'static RangePair, ws: &Entity<Workspace>, instant: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let def = ws.read(cx).game().def;
    let Some(range) = Range::of(pair, def) else { return div().into_any_element() };
    let (lo, hi) = range_values(&range, ws, cx);
    let (on_disk, waiting, category, advanced, stored) = {
        let state = ws.read(cx);
        let game = state.game();
        let disk = |t: &Tweak| game.current(t).as_ref().and_then(Value::as_num).unwrap_or_else(|| num_or_default(t, &t.default.to_value()));
        let on_disk = (game.current(range.lo).is_some() || game.current(range.hi).is_some()).then(|| range.text(disk(range.lo), disk(range.hi)));
        let pending = |t: &Tweak| game.pending.get(t.id).and_then(Value::as_num);
        let waiting = (pending(range.lo).is_some() || pending(range.hi).is_some())
            .then(|| range.text(pending(range.lo).unwrap_or(lo), pending(range.hi).unwrap_or(hi)));
        (
            on_disk,
            waiting,
            def.category(range.lo.category).map_or("", |c| c.title),
            state.mode() == crate::games::Mode::Advanced,
            {
                // Both ends live side by side: one file and section, two keys.
                let mut rows = location(range.lo, state);
                rows.extend(location(range.hi, state).into_iter().filter(|(k, _)| *k == "Key"));
                rows
            },
        )
    };
    let (lo_d, hi_d) = (num_or_default(range.lo, &range.lo.default.to_value()), num_or_default(range.hi, &range.hi.default.to_value()));
    let at_default = (lo - lo_d).abs() < 1e-9 && (hi - hi_d).abs() < 1e-9;

    let mut tags = div().flex().flex_wrap().gap(px(6.));
    if let Some(cost) = controls::cost_badge(range.lo.impact.max(range.hi.impact)) {
        tags = tags.child(cost);
    }
    let values = div()
        .flex()
        .flex_col()
        .child(controls::kv_row_text("In your files", on_disk.unwrap_or_else(|| "Not set (default)".into()), theme::text()))
        .child(controls::kv_row_text("Game default", range.text(lo_d, hi_d), theme::text()))
        .when_some(waiting, |d, w| d.child(controls::kv_row_text("Waiting to apply", w, theme::accent_text())));

    let (slider, _) = range_control(&range, ws, instant, "detail", cx);
    let end_box = |t: &'static Tweak, n: f64, caption: &'static str, window: &mut Window, cx: &mut App| {
        let ws = ws.clone();
        div().flex_1().min_w_0().flex().flex_col().gap(px(4.)).child(ui::caption(caption)).child(controls::number_box(
            format!("nb-{}", t.id),
            n,
            t.control,
            Rc::new(move |v, _, cx| ws.update(cx, |ws, cx| commit(ws, t, t.clamp(Value::Num(v)), instant, cx))),
            window,
            cx,
        ))
    };
    let min_box = end_box(range.lo, lo, "Minimum", window, cx);
    let max_box = end_box(range.hi, hi, "Maximum", window, cx);
    let control = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(div().w_full().flex().child(slider))
        .child(div().flex().gap(px(12.)).child(min_box).child(max_box))
        .into_any_element();
    let reset_ws = ws.clone();
    let (lo_t, hi_t) = (range.lo, range.hi);
    let reset = reset_default_button(format!("reset-{}", pair.min), !at_default, move |cx| {
        reset_ws.update(cx, |ws, cx| {
            commit(ws, lo_t, lo_t.default.to_value(), instant, cx);
            commit(ws, hi_t, hi_t.default.to_value(), instant, cx);
        })
    });
    detail_frame(
        category,
        pair.label,
        pair.description,
        tags,
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(setting_card(instant, control, values, reset))
            .when(advanced, |d| d.child(where_stored(stored))),
    )
    .into_any_element()
}

/// (label, value) rows naming the file, section and key a setting lives in;
/// empty for settings that edit several entries.
fn location(tweak: &Tweak, ws: &Workspace) -> Vec<(&'static str, String)> {
    let Binding::Keys(keys, _) = tweak.binding else { return Vec::new() };
    let file_name = |id: &'static str| ws.game().def.ini_files.iter().find(|(f, _)| *f == id).map_or(id, |(_, name)| *name);
    let k = keys[0];
    let mut rows = vec![("File", file_name(k.file).to_string()), ("Section", format!("[{}]", k.section)), ("Key", k.key.to_string())];
    if let Some(copy) = keys.get(1) {
        rows.push(("Also written to", file_name(copy.file).to_string()));
    }
    rows
}

/// Keeps a range's ends in order when one of them is set on its own.
fn ordered(ws: &Workspace, tweak: &'static Tweak, value: Value) -> Value {
    let game = ws.game();
    let (Some(pair), Value::Num(n)) = (game.def.range_of(tweak.id), &value) else { return value };
    let is_min = pair.min == tweak.id;
    let partner = game.def.tweak(if is_min { pair.max } else { pair.min });
    let Some(other) = partner.and_then(|p| game.effective(p).as_num()) else { return value };
    let (lo, hi) = if is_min { order_range(*n, other, true) } else { order_range(other, *n, false) };
    Value::Num(if is_min { lo } else { hi })
}

/// Advanced mode stages edits; Simple mode saves them right away.
fn commit(ws: &mut Workspace, tweak: &'static Tweak, value: Value, instant: bool, cx: &mut gpui::Context<Workspace>) {
    let value = ordered(ws, tweak, value);
    if instant {
        ws.set_now(tweak, value, cx);
    } else {
        ws.stage(tweak, value, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{first_sentence, plural};

    #[test]
    fn counts_are_pluralised() {
        assert_eq!(plural(1, "setting"), "1 setting");
        assert_eq!(plural(0, "setting"), "0 settings");
        assert_eq!(plural(12, "setting"), "12 settings");
    }

    #[test]
    fn rows_show_the_first_sentence() {
        assert_eq!(first_sentence("One. Two."), "One.");
        assert_eq!(first_sentence("Only one"), "Only one");
    }
}
