//! Simple mode's home: the game banner with one big action, and a grouped
//! checklist of what it installs. Rows toggle on click; the chevron expands
//! the details.

use gpui::{AnyElement, App, Entity, IntoElement, MouseButton, ParentElement, SharedString, Styled, Window, div, prelude::*, px};

use super::{caption_style, clipped, confirm, count, game_banner, missing_notice};
use crate::setup::{Component, Group, Status};
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::{SetupGoal, StepState, Workspace};

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
    let reapply_ws = ws.clone();
    let main_action = if done {
        // Nothing to do: a status, not a button.
        div()
            .id("setup-done")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(12.))
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(theme::text())
            .tooltip(ui::tip("Everything you ticked is installed"))
            .child(ui::icon(Icon::CheckCircle).text_color(theme::success()))
            .child("All set")
            .into_any_element()
    } else {
        ui::button_if(
            !running,
            "setup-patch",
            if running { "Working…".to_string() } else { format!("Upgrade game ({to_install})") },
            Some(Icon::Bolt),
            Variant::Primary,
        )
        .tooltip(ui::tip(if running { "Setup is running" } else { "Installs everything ticked below. Every change is backed up." }))
        .on_click(move |_, _, cx| patch_ws.update(cx, |ws, cx| ws.run_setup(SetupGoal::Install, cx)))
        .into_any_element()
    };
    let mut actions = vec![
        main_action,
        ui::button_if(!running, "setup-reapply", "Re-apply all", Some(Icon::Refresh), Variant::Secondary)
            .tooltip(ui::tip(if running {
                "Wait for setup to finish"
            } else {
                "The game or its launcher rewrote your settings? Puts back every Vault Patcher setting and upgrade."
            }))
            .on_click(move |_, _, cx| reapply_ws.update(cx, |ws, cx| ws.reapply_all(cx)))
            .into_any_element(),
    ];
    if game.install.is_some() {
        actions.push(
            ui::button("setup-play", "Play", Some(Icon::Play), if done { Variant::Primary } else { Variant::Secondary })
                .on_click(move |_, _, cx| play_ws.update(cx, |ws, cx| ws.launch_default(cx)))
                .into_any_element(),
        );
    }
    let caption = format!("{installed} of {total} upgrades installed · smoother, sharper, fewer crashes, fully reversible");
    let mut page = div().flex().flex_col().gap(px(20.)).child(game_banner(state, caption, actions));

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
        if !run.finished {
            let current = run.steps.iter().find(|(_, s)| *s == StepState::Running).and_then(|(id, _)| def.component(id)).map(|c| c.name).unwrap_or("Preparing");
            let verb = match run.goal {
                SetupGoal::Uninstall => "Removing",
                SetupGoal::Install => "Installing",
                SetupGoal::Reapply => "Re-applying",
            };
            let fraction = finished_steps as f32 / run.steps.len().max(1) as f32;
            page = page.child(
                ui::panel()
                    .px(px(16.))
                    .py(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(ui::title(format!("{verb} {current}… ({finished_steps} of {})", run.steps.len())))
                    .child(ui::progress_bar("setup-progress", Some(fraction), theme::accent())),
            );
        } else if failed.is_empty() {
            let title = match run.goal {
                SetupGoal::Uninstall => "Restored to vanilla",
                SetupGoal::Reapply => "Everything's back in place",
                SetupGoal::Install => "Everything installed",
            };
            page = page.child(ui::info_bar(ui::Severity::Success, title, "Every change was backed up first, so you can undo it from Backups."));
        } else {
            page = page.child(ui::info_bar(
                ui::Severity::Error,
                format!("{} didn't finish", count(failed.len(), "step", "steps")),
                failed.join("\n"),
            ));
        }
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
            .child(ui::body("Changed your mind? Everything here can be undone in one go.").flex_1())
            .child(
                ui::button_if(!running, "setup-restore", "Restore vanilla…", Some(Icon::Undo), Variant::Ghost)
                    .tooltip(ui::tip(if running { "Wait for setup to finish" } else { "Remove everything Vault Patcher installed" }))
                    .on_click(move |_, window, cx| {
                        let ws = restore_ws.clone();
                        confirm(
                            window,
                            cx,
                            "Restore the vanilla game?",
                            "Removes every upgrade Vault Patcher installed and puts the affected settings back to the game's defaults. Your saves and your own mods aren't touched.",
                            "Restore",
                            move |cx| ws.update(cx, |ws, cx| ws.run_setup(SetupGoal::Uninstall, cx)),
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
    // Installed items show a success check instead of a checkbox (a locked
    // checked box read as "disabled"), so they need no "Installed" tag.
    let tag = match status {
        Status::Active(_) => None,
        Status::Partial => Some(ui::badge("Partly applied", theme::warning())),
        Status::Blocked(_) => Some(ui::badge("Unavailable", theme::danger())),
        Status::Missing if selected => Some(ui::badge("Will install", theme::accent_text())),
        Status::Missing => None,
    };
    let mark = if active {
        div()
            .id(SharedString::from(format!("installed-{}", c.id)))
            .size(px(20.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .tooltip(ui::tip("Installed"))
            .child(ui::icon(Icon::CheckCircle).size(px(20.)).text_color(theme::success()))
            .into_any_element()
    } else {
        ui::checkbox(checked, !can_toggle).into_any_element()
    };
    let note_color = match status {
        Status::Blocked(_) => theme::danger(),
        _ => theme::text_muted(),
    };
    let id = c.id;
    let toggle_ws = ws.clone();
    let expand_ws = ws.clone();
    let needs: Vec<&str> = c.requires.iter().map(|r| def.component(r).map_or(*r, |d| d.name)).collect();

    let header = div()
        .id(SharedString::from(format!("row-{id}")))
        .flex()
        .items_center()
        .gap(px(12.))
        .min_h(px(56.))
        .px(px(16.))
        .py(px(8.))
        .child(mark)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(if checked { theme::text() } else { theme::text_muted() })
                        .child(c.name),
                )
                .child(caption_style(clipped(format!("sum-{id}"), c.summary))),
        )
        .children(tag)
        // Expander toggle: ChevronDown collapsed, ChevronUp expanded, a 12px
        // glyph in a 32px subtle button.
        .child(
            ui::focusable(div().id(SharedString::from(format!("more-{id}"))))
                .size(px(32.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .cursor_pointer()
                .hover(|s| s.bg(theme::panel_hi()))
                .active(|s| s.bg(theme::panel_pressed()))
                .tooltip(ui::tip(if expanded { "Hide details" } else { "What does this do?" }))
                .child(ui::icon(if expanded { Icon::ChevronUp } else { Icon::ChevronDown }).size(px(12.)).text_color(theme::text_muted()))
                .on_mouse_down(MouseButton::Left, |_, _, _| crate::sound::play(crate::sound::Sound::Click))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    expand_ws.update(cx, |ws, cx| ws.toggle_expanded(id, cx))
                }),
        );
    // The row is a CheckBox when it can be toggled: a Tab stop with hover.
    let header = if can_toggle {
        ui::focusable_row(header)
            .cursor_pointer()
            .hover(|s| s.bg(theme::panel_hi()))
            .active(|s| s.bg(theme::panel_pressed()))
            .on_click(move |_, _, cx| {
                crate::sound::play(crate::sound::Sound::Click);
                toggle_ws.update(cx, |ws, cx| ws.toggle_component(id, cx));
            })
    } else {
        header
    };

    div().flex().flex_col().child(header).when(expanded, |d| {
        d.child(
            div()
                .border_t_1()
                .border_color(theme::line())
                .bg(theme::panel_lo())
                .pl(px(48.))
                .pr(px(16.))
                .py(px(16.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(ui::body(c.description).max_w(px(720.)))
                .when_some(
                    match status {
                        Status::Blocked(reason) => Some(reason.clone()),
                        Status::Active(Some(v)) => Some(format!("Installed version: {v}")),
                        _ => None,
                    },
                    |d, note| d.child(ui::caption(note).text_color(note_color)),
                )
                .when(!needs.is_empty(), |d| d.child(ui::caption(format!("Needs: {}", needs.join(", "))))),
        )
    })
}
