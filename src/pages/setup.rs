//! Simple mode's home: the game banner with one big action, and a grouped
//! checklist of what it installs. Rows toggle on click; "Details" expands.

use gpui::{AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px, relative};

use super::{confirm, game_banner, missing_notice};
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

    let statuses: Vec<(&'static Component, Status)> = def.setup.iter().map(|c| (c, state.component_status(c))).collect();
    let installed = statuses.iter().filter(|(_, s)| s.is_active()).count();
    let total = statuses.len();
    let to_install = statuses
        .iter()
        .filter(|(c, s)| game.setup_selected.contains(c.id) && !s.is_active() && !matches!(s, Status::Blocked(_)))
        .count();
    let done = !running && to_install == 0;

    // ---- banner
    let patch_ws = ws.clone();
    let play_ws = ws.clone();
    let cta = if running {
        "Working…".to_string()
    } else if done {
        "All set".to_string()
    } else {
        format!("Upgrade game ({to_install})")
    };
    let mut actions = vec![
        ui::button("setup-patch", cta, Some(if done { Icon::CheckCircle } else { Icon::Rocket }), if done { Variant::Secondary } else { Variant::Primary })
            .h(px(36.))
            .px(px(16.))
            .when(running || done, |b| b.opacity(if running { 0.6 } else { 1. }))
            .tooltip(ui::tip(if done { "Everything you ticked is installed" } else { "Installs everything ticked below. Every change is backed up." }))
            .on_click(move |_, _, cx| {
                if !done {
                    patch_ws.update(cx, |ws, cx| ws.run_setup(false, cx))
                }
            })
            .into_any_element(),
    ];
    if game.install.is_some() {
        actions.push(
            ui::button("setup-play", "Play", Some(Icon::Play), if done { Variant::Primary } else { Variant::Secondary })
                .h(px(36.))
                .px(px(16.))
                .on_click(move |_, _, cx| play_ws.update(cx, |ws, cx| ws.launch(cx)))
                .into_any_element(),
        );
    }
    let caption = format!("{installed} of {total} upgrades installed · smoother, sharper, fewer crashes, fully reversible");
    let mut page = div().flex().flex_col().gap(px(18.)).child(game_banner(state, caption, actions));

    // ---- progress / result
    if let Some(run) = &state.setup_run {
        let finished_steps = run.steps.iter().filter(|(_, s)| matches!(s, StepState::Done(_) | StepState::Failed(_))).count();
        let failed: Vec<String> = run
            .steps
            .iter()
            .filter_map(|(id, s)| match s {
                StepState::Failed(msg) => Some(format!("{}: {msg}", def.component(id).map_or(*id, |c| c.name))),
                _ => None,
            })
            .collect();
        let (text, color, fraction) = if !run.finished {
            let current = run.steps.iter().find(|(_, s)| *s == StepState::Running).and_then(|(id, _)| def.component(id)).map(|c| c.name).unwrap_or("Preparing");
            let verb = if run.uninstall { "Removing" } else { "Installing" };
            (format!("{verb} {current}… ({finished_steps}/{})", run.steps.len()), theme::accent(), finished_steps as f32 / run.steps.len().max(1) as f32)
        } else if failed.is_empty() {
            (if run.uninstall { "Restored to vanilla.".to_string() } else { "Done. Everything installed.".to_string() }, theme::success(), 1.)
        } else {
            (format!("{} step(s) didn't finish", failed.len()), theme::danger(), 1.)
        };
        page = page.child(
            ui::panel()
                .p(px(14.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(color).child(text))
                .child(div().h(px(4.)).w_full().rounded_full().bg(theme::panel_lo()).child(div().h_full().rounded_full().w(relative(fraction)).bg(color)))
                .children(failed.into_iter().map(|f| div().text_size(px(12.5)).text_color(theme::text_muted()).child(f))),
        );
    }

    if game.install.is_none() {
        page = page.child(missing_notice(
            &format!("{} wasn't found", def.name),
            "Settings upgrades still work, but the renderer, memory patch and mods need the game folder.",
            ws,
        ));
    }

    // ---- checklist
    for group in GROUPS {
        let items: Vec<_> = statuses.iter().filter(|(c, _)| c.group == group).collect();
        if items.is_empty() {
            continue;
        }
        let mut list = ui::panel().flex().flex_col();
        for (i, (c, status)) in items.into_iter().enumerate() {
            let expanded = state.setup_expanded.contains(c.id);
            let selected = game.setup_selected.contains(c.id);
            list = list.when(i > 0, |d| d.child(ui::divider())).child(row(c, status, selected, expanded, running, def, ws));
        }
        page = page.child(div().flex().flex_col().gap(px(8.)).child(ui::label(group.title())).child(list));
    }
    let restore_ws = ws.clone();
    page = page.child(
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .pt(px(4.))
            .child(ui::body("Changed your mind? Everything here can be undone in one go.").flex_1().text_color(theme::text_dim()))
            .child(
                ui::button("setup-restore", "Restore vanilla…", Some(Icon::Undo), Variant::Ghost)
                    .tooltip(ui::tip("Remove everything Vault Patcher installed"))
                    .on_click(move |_, window, cx| {
                        let ws = restore_ws.clone();
                        confirm(
                            window,
                            cx,
                            "Restore the vanilla game?",
                            "Removes every upgrade Vault Patcher installed and puts the affected settings back to the game's defaults. Your saves and your own mods aren't touched.",
                            "Restore",
                            move |cx| ws.update(cx, |ws, cx| ws.run_setup(true, cx)),
                        );
                    }),
            ),
    );
    page.into_any_element()
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
    let tag = match status {
        Status::Active(_) => Some(ui::badge("Installed", theme::success())),
        Status::Partial => Some(ui::badge("Partly applied", theme::warning())),
        Status::Blocked(_) => Some(ui::badge("Unavailable", theme::danger())),
        Status::Missing if selected => Some(ui::badge("Will install", theme::accent())),
        Status::Missing => None,
    };
    let note_color = match status {
        Status::Blocked(_) => theme::danger(),
        _ => theme::text_dim(),
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
                .gap(px(12.))
                .px(px(14.))
                .py(px(9.))
                .when(can_toggle, |d| d.cursor_pointer().hover(|s| s.bg(theme::panel_hi())))
                .child(ui::checkbox(checked, !can_toggle))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).text_color(if checked { theme::text() } else { theme::text_muted() }).child(c.name))
                        .child(div().text_size(px(12.)).text_color(theme::text_dim()).truncate().child(c.summary)),
                )
                .children(tag)
                .child(
                    ui::icon_button(SharedString::from(format!("more-{id}")), if expanded { Icon::ChevronDown } else { Icon::ChevronRight }, theme::text_dim())
                        .tooltip(ui::tip(if expanded { "Hide details" } else { "What does this do?" }))
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
                    .pl(px(44.))
                    .pr(px(24.))
                    .pb(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ui::body(c.description).max_w(px(720.)))
                    .when_some(
                        match status {
                            Status::Blocked(reason) => Some(reason.clone()),
                            Status::Active(Some(v)) => Some(format!("Installed version: {v}")),
                            _ => None,
                        },
                        |d, note| d.child(div().text_size(px(12.5)).text_color(note_color).child(note)),
                    )
                    .when(!needs.is_empty(), |d| d.child(div().text_size(px(12.5)).text_color(theme::text_dim()).child(format!("Needs: {}", needs.join(", "))))),
            )
        })
}
