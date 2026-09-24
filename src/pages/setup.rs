//! Simple mode's home: one big button, and a short checklist of what it does.
//!
//! Layout rule for this page: one loud element (the hero with the upgrade
//! button); everything else is a calm, one-line-per-item list whose details
//! open on click.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, Rgba, SharedString, Styled,
    Window, div, prelude::*, px, relative,
};

use super::{confirm, missing_notice};
use crate::setup::{Component, Group, Status};
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::{StepState, Workspace};

const GROUPS: [Group; 4] = [Group::Essentials, Group::Performance, Group::Fixes, Group::Mods];

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let running = state.setup_running();

    let statuses: Vec<(&'static Component, Status)> =
        def.setup.iter().map(|c| (c, state.component_status(c))).collect();
    let installed = statuses.iter().filter(|(_, s)| s.is_active()).count();
    let to_install = statuses
        .iter()
        .filter(|(c, s)| game.setup_selected.contains(c.id) && !s.is_active() && !matches!(s, Status::Blocked(_)))
        .count();

    let mut page = div().flex().flex_col().gap(px(28.)).child(hero(state, installed, statuses.len(), to_install, ws));

    if game.install.is_none() {
        page = page.child(missing_notice(
            &format!("{} wasn't found", def.name),
            "Settings upgrades still work, but the renderer, memory patch and mods need the game folder.",
            ws,
        ));
    }

    // ---- the checklist
    let mut list = ui::panel().flex().flex_col();
    for (gi, group) in GROUPS.into_iter().enumerate() {
        let items: Vec<_> = statuses.iter().filter(|(c, _)| c.group == group).collect();
        if items.is_empty() {
            continue;
        }
        list = list.child(
            div()
                .px(px(20.))
                .pt(px(if gi == 0 { 16. } else { 22. }))
                .pb(px(6.))
                .child(ui::label(group.title()).text_color(theme::text_dim())),
        );
        for (c, status) in items {
            let expanded = state.setup_expanded.contains(c.id);
            let selected = game.setup_selected.contains(c.id);
            list = list.child(row(c, status, selected, expanded, running, def, ws));
        }
    }
    let restore_ws = ws.clone();
    list = list.child(
        div()
            .mt(px(10.))
            .px(px(20.))
            .py(px(14.))
            .border_t_1()
            .border_color(theme::line())
            .flex()
            .items_center()
            .gap(px(12.))
            .child(ui::body("Changed your mind? Everything above can be undone.").flex_1().text_color(theme::text_dim()))
            .child(
                ui::button("setup-restore", "Restore vanilla…", Some(Icon::Undo), Variant::Ghost).on_click(
                    move |_, window, cx| {
                        let ws = restore_ws.clone();
                        confirm(
                            window,
                            cx,
                            "Restore the vanilla game?",
                            "Removes every upgrade Vault Patcher installed and puts the affected settings back to the game's defaults. Your saves and your own mods aren't touched.",
                            "Restore",
                            move |cx| ws.update(cx, |ws, cx| ws.run_setup(true, cx)),
                        );
                    },
                ),
            ),
    );

    page = page.child(
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(ui::section_title(
                "What's included",
                Some("Untick anything you don't want. Click a line to see exactly what it does.".into()),
            ))
            .child(list),
    );
    page.into_any_element()
}

fn hero(state: &Workspace, installed: usize, total: usize, to_install: usize, ws: &Entity<Workspace>) -> impl IntoElement {
    let game = state.game();
    let def = game.def;
    let running = state.setup_running();
    let patch_ws = ws.clone();
    let play_ws = ws.clone();

    let done = !running && to_install == 0;
    let cta = if running {
        "Working…".to_string()
    } else if done {
        "All selected upgrades installed".to_string()
    } else {
        format!("Upgrade game  ·  {to_install}")
    };

    // Status: a live progress line while running, otherwise the install count.
    let status: AnyElement = match &state.setup_run {
        Some(run) if !run.finished => {
            let done = run.steps.iter().filter(|(_, s)| matches!(s, StepState::Done(_) | StepState::Failed(_))).count();
            let current = run
                .steps
                .iter()
                .find(|(_, s)| *s == StepState::Running)
                .and_then(|(id, _)| def.component(id))
                .map(|c| c.name)
                .unwrap_or("Preparing");
            let verb = if run.uninstall { "Removing" } else { "Installing" };
            progress(done as f32 / run.steps.len() as f32, theme::accent(), format!("{verb} {current}…  ({done}/{})", run.steps.len()))
        }
        Some(run) => {
            let failed: Vec<String> = run
                .steps
                .iter()
                .filter_map(|(id, s)| match s {
                    StepState::Failed(msg) => Some(format!("{}: {msg}", def.component(id).map_or(*id, |c| c.name))),
                    _ => None,
                })
                .collect();
            if failed.is_empty() {
                progress(installed as f32 / total.max(1) as f32, theme::success(), format!("Done — {installed} of {total} upgrades installed"))
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().text_size(px(14.)).text_color(theme::danger()).child(format!("{} step(s) didn't finish:", failed.len())))
                    .children(failed.into_iter().map(|f| div().text_size(px(13.)).text_color(theme::text_muted()).child(f)))
                    .into_any_element()
            }
        }
        None => progress(installed as f32 / total.max(1) as f32, theme::success(), format!("{installed} of {total} upgrades installed")),
    };

    ui::hero_panel()
        .flex()
        .flex_col()
        .child(div().h(px(10.)).w_full().bg(theme::accent()).child(ui::hazard_stripes(theme::ink(), 22.).size_full()))
        .child(
            div()
                .px(px(30.))
                .py(px(28.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(ui::label("One-click upgrade").text_color(theme::text_dim()))
                .child(ui::display(def.name, 50.))
                .child(
                    div()
                        .text_size(px(16.))
                        .line_height(px(24.))
                        .text_color(theme::text_muted())
                        .max_w(px(620.))
                        .child("Make it play like a modern Borderlands: smoother, sharper, fewer crashes and less busywork. One click, fully reversible."),
                )
                .child(
                    div()
                        .pt(px(12.))
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(14.))
                        .child(
                            // Once everything's installed, Play is the main action.
                            ui::button("setup-patch", cta, Some(Icon::Rocket), if done { Variant::Secondary } else { Variant::Primary })
                                .h(px(48.))
                                .px(px(26.))
                                .text_size(px(16.))
                                .when(running, |b| b.opacity(0.5))
                                .on_click(move |_, _, cx| {
                                    if !done {
                                        patch_ws.update(cx, |ws, cx| ws.run_setup(false, cx))
                                    }
                                }),
                        )
                        .when(game.install.is_some(), |d| {
                            d.child(
                                ui::button("setup-play", "Play", Some(Icon::Play), if done { Variant::Primary } else { Variant::Secondary })
                                    .h(px(48.))
                                    .px(px(22.))
                                    .text_size(px(16.))
                                    .on_click(move |_, _, cx| play_ws.update(cx, |ws, cx| ws.launch(cx))),
                            )
                        }),
                )
                .child(div().pt(px(6.)).max_w(px(520.)).child(status)),
        )
}

fn progress(fraction: f32, color: Rgba, text: String) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .h(px(6.))
                .w_full()
                .bg(theme::panel_lo())
                .child(div().h_full().w(relative(fraction.clamp(0., 1.))).bg(color)),
        )
        .child(div().text_size(px(13.)).text_color(theme::text_dim()).child(text))
        .into_any_element()
}

fn row(
    c: &'static Component,
    status: &Status,
    selected: bool,
    expanded: bool,
    running: bool,
    def: &'static crate::games::GameDef,
    ws: &Entity<Workspace>,
) -> impl IntoElement {
    let active = status.is_active();
    let blocked = matches!(status, Status::Blocked(_));
    let checked = active || (selected && !blocked);
    let can_toggle = !active && !blocked && !running;
    let (pill, pill_color) = match status {
        Status::Active(_) => (Some("Installed"), theme::success()),
        Status::Partial => (Some("Partly applied"), theme::accent()),
        Status::Blocked(_) => (Some("Unavailable"), theme::danger()),
        Status::Missing => (None, theme::text_dim()),
    };
    let id = c.id;
    let toggle_ws = ws.clone();
    let expand_ws = ws.clone();
    let needs: Vec<&str> = c.requires.iter().map(|r| def.component(r).map_or(*r, |d| d.name)).collect();

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .id(SharedString::from(format!("row-{id}")))
                .flex()
                .items_center()
                .gap(px(14.))
                .px(px(20.))
                .py(px(11.))
                .when(can_toggle, |d| d.cursor_pointer().hover(|s| s.bg(theme::panel_hi())))
                .child(
                    checkbox(checked, !can_toggle),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_baseline()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_none()
                                .font_family(theme::FONT_LABEL)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(15.5))
                                .text_color(if checked || active { theme::text() } else { theme::text_dim() })
                                .child(c.name),
                        )
                        .child(div().min_w_0().truncate().text_size(px(13.5)).text_color(theme::text_dim()).child(c.summary)),
                )
                .when_some(pill, |d, p| {
                    d.child(div().flex_none().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(pill_color).child(p))
                })
                .child(
                    div()
                        .id(SharedString::from(format!("more-{id}")))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .px(px(8.))
                        .py(px(4.))
                        .text_size(px(13.))
                        .text_color(if expanded { theme::text() } else { theme::text_dim() })
                        .hover(|s| s.bg(theme::line()).text_color(theme::text()))
                        .child(if expanded { "Less" } else { "Details" })
                        .child(ui::icon(Icon::ChevronRight).text_size(px(9.)))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            expand_ws.update(cx, |ws, cx| ws.toggle_expanded(id, cx))
                        }),
                )
                .on_click(move |_, _, cx| {
                    if can_toggle {
                        crate::sound::play(crate::sound::Sound::Click);
                        toggle_ws.update(cx, |ws, cx| ws.toggle_component(id, cx));
                    }
                }),
        )
        .when(expanded, |d| {
            d.child(
                div()
                    .pl(px(56.))
                    .pr(px(28.))
                    .pb(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ui::body(c.description).max_w(px(700.)))
                    .when_some(
                        match status {
                            Status::Blocked(reason) => Some(reason.clone()),
                            Status::Active(Some(v)) => Some(format!("Installed version: {v}")),
                            _ => None,
                        },
                        |d, note| d.child(div().text_size(px(13.)).text_color(pill_color).child(note)),
                    )
                    .when(!needs.is_empty(), |d| {
                        d.child(div().text_size(px(13.)).text_color(theme::text_dim()).child(format!("Needs: {}", needs.join(", "))))
                    }),
            )
        })
}

fn checkbox(checked: bool, locked: bool) -> impl IntoElement {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if checked { gpui::transparent_black().into() } else { theme::text_dim() })
        .bg(match (checked, locked) {
            (true, true) => theme::success(),
            // Neutral, so the upgrade button stays the only yellow thing.
            (true, false) => theme::text_muted(),
            _ => gpui::transparent_black().into(),
        })
        .when(checked, |d| d.child(ui::icon(Icon::Check).text_size(px(11.)).text_color(theme::accent_ink())))
}
