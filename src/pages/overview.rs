//! Advanced mode's landing page: the game banner, a health checklist with
//! one-click fixes, the numbers at a glance, and launch options.

use gpui::{AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};

use super::{clipped, clipped_caption, count, game_banner, missing_notice, open_folder};
use crate::core::binpatch::PatchState;
use crate::games::{PageKind, Support};
use crate::health::Level;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

/// Health and At a glance rows share one height so the two columns line up.
/// (Title + caption, 36px, plus 8px padding above and below, rounded up to
/// the grid.)
const ROW_H: f32 = 56.;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;

    let caption = match (&game.install, def.support) {
        (Some(i), Support::Full) => format!("{} · {}", i.store.label(), i.root.display()),
        (Some(i), Support::Preview) => format!("Preview support · {} · {}", i.store.label(), i.root.display()),
        (None, _) => def.tagline.to_string(),
    };
    let rescan_ws = ws.clone();
    let reapply_ws = ws.clone();
    let play_ws = ws.clone();
    let mut actions = vec![
        ui::button("ov-rescan", "Rescan", Some(Icon::Search), Variant::Secondary)
            .tooltip(ui::tip("Look for the game, its settings and mods again"))
            .on_click(move |_, _, cx| rescan_ws.update(cx, |ws, cx| ws.refresh_active(cx)))
            .into_any_element(),
        ui::button("ov-reapply", "Re-apply all", Some(Icon::Refresh), Variant::Secondary)
            .tooltip(ui::tip(
                "The game or its launcher rewrote your settings? Puts back every Vault Patcher setting and upgrade.",
            ))
            .on_click(move |_, _, cx| reapply_ws.update(cx, |ws, cx| ws.reapply_all(cx)))
            .into_any_element(),
    ];
    if game.install.is_some() {
        actions.push(
            ui::button("ov-play", "Play", Some(Icon::Play), Variant::Primary)
                .on_click(move |_, _, cx| play_ws.update(cx, |ws, cx| ws.launch_default(cx)))
                .into_any_element(),
        );
    }

    let mut page = div().flex().flex_col().gap(px(20.)).child(game_banner(state, caption, actions));

    if game.install.is_none() {
        page = page.child(missing_notice(
            &format!("{} wasn't found", def.name),
            &format!(
                "Checked your Steam libraries and the Epic Games Launcher. If it's installed elsewhere, pick the folder that contains {}. Settings still work without it.",
                def.exe
            ),
            ws,
        ));
    }

    // ---- health
    let checks = crate::health::checks(state);
    let problems = checks.iter().filter(|c| c.level <= Level::Warn).count();
    let mut health = ui::panel().flex().flex_col();
    for (i, check) in checks.iter().enumerate() {
        let (icon, color) = match check.level {
            Level::Error => (Icon::Alert, theme::danger()),
            Level::Warn => (Icon::Warning, theme::warning()),
            Level::Info => (Icon::Info, theme::echo()),
            Level::Ok => (Icon::CheckCircle, theme::success()),
        };
        // The first check is always the install one; when the game is
        // missing, the InfoBar above already offers "Locate game".
        let fixes: &[_] = if i == 0 && game.install.is_none() { &[] } else { &check.fixes };
        let fix = fixes.iter().enumerate().map(|(j, &(label, fix))| {
            let ws = ws.clone();
            ui::button(SharedString::from(format!("fix-{i}-{j}")), label, None, Variant::Secondary)
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.health_fix(fix, cx)))
        });
        health = health.when(i > 0, |d| d.child(ui::divider())).child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .min_h(px(ROW_H))
                .px(px(16.))
                .py(px(8.))
                .child(ui::icon(icon).text_color(color))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(clipped(format!("hc-title-{i}"), check.title.clone()).text_size(px(14.)).line_height(px(20.)).text_color(theme::text()))
                        .child(clipped_caption(format!("hc-detail-{i}"), check.detail.clone())),
                )
                .children(fix),
        );
    }

    // ---- at a glance
    let modified = def.visible_tweaks().filter(|t| game.current(t).is_some_and(|v| v != t.default.to_value())).count();
    let user_mods = game.mods.iter().filter(|m| !m.core).count();
    let enabled_mods = game.mods.iter().filter(|m| !m.core && m.enabled).count();
    let patches_on = game
        .exe
        .as_ref()
        .map(|e| def.patches.iter().filter(|p| e.patch_states.get(p.id) == Some(&PatchState::Patched)).count())
        .unwrap_or(0);
    let upgrades = def.setup.iter().filter(|c| state.component_status(c).is_active()).count();
    let config_dir = game.config_dir.clone().filter(|p| p.is_dir());
    let install_dir = game.install.as_ref().map(|i| i.root.clone());
    let stat = |id: &'static str, icon: Icon, value: String, label: &'static str, page: PageKind| {
        let ws = ws.clone();
        ui::focusable(div().id(id))
            .flex()
            .items_center()
            .gap(px(12.))
            .min_h(px(ROW_H))
            .px(px(16.))
            .py(px(8.))
            .cursor_pointer()
            .hover(|s| s.bg(theme::panel_hi()))
            .active(|s| s.bg(theme::panel_pressed()))
            .child(ui::icon(icon).text_color(theme::text_muted()))
            .child(div().flex_1().min_w_0().text_ellipsis().line_clamp(1).text_size(px(14.)).line_height(px(20.)).text_color(theme::text()).child(label))
            .child(div().flex_none().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::text()).child(value))
            .child(ui::icon(Icon::ChevronRight).size(px(12.)).text_color(theme::text_muted()))
            .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.navigate(page, cx)))
    };
    let first_settings = def.nav_items(state.mode()).find(|n| matches!(n.kind, PageKind::Tweaks(_))).map_or(PageKind::Presets, |n| n.kind);
    let mut glance = ui::panel()
        .flex()
        .flex_col()
        .child(stat("g-settings", Icon::Sliders, format!("{modified} / {}", def.visible_tweaks().count()), "Settings changed from default", first_settings))
        .child(ui::divider())
        .child(stat("g-waiting", Icon::Edit,game.pending.len().to_string(), "Changes waiting to apply", first_settings));
    if def.mods.is_some() {
        glance = glance
            .child(ui::divider())
            .child(stat("g-mods", Icon::Puzzle, format!("{enabled_mods} / {user_mods}"), "Mods enabled", PageKind::Mods));
    }
    if !def.patches.is_empty() {
        glance = glance
            .child(ui::divider())
            .child(stat("g-patches", Icon::Wrench, format!("{patches_on} / {}", def.patches.len()), "Exe patches active", PageKind::Patches));
    }
    glance = glance
        .child(ui::divider())
        .child(stat("g-upgrades", Icon::Rocket, format!("{upgrades} / {}", def.setup.len()), "One-click upgrades installed", PageKind::Setup))
        .child(ui::divider())
        .child(stat("g-backups", Icon::History, game.backups.len().to_string(), "Backups", PageKind::Backups));
    let folders = div()
        .flex()
        .gap(px(8.))
        .when_some(install_dir, |d, p| {
            d.child(ui::button("ov-open-install", "Game folder", Some(Icon::Folder), Variant::Ghost).on_click(move |_, _, cx| open_folder(&p, cx)))
        })
        .when_some(config_dir, |d, p| {
            d.child(ui::button("ov-open-config", "Settings folder", Some(Icon::Folder), Variant::Ghost).on_click(move |_, _, cx| open_folder(&p, cx)))
        });

    page = page.child(
        div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(20.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(360.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(ui::section_title(
                        "Health",
                        Some(if problems == 0 { "Everything looks good.".into() } else { format!("{} to look at", count(problems, "thing", "things")).into() }),
                    ))
                    .child(health),
            )
            .child(
                div()
                    .w(px(340.))
                    .flex_grow()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(ui::section_title("At a glance", Some("Click a line to go there.".into())))
                    .child(glance)
                    .child(folders),
            ),
    );

    page.child(super::launch::section(ws, cx)).into_any_element()
}
