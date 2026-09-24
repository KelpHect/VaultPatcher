//! Landing page for a game: detection status, health checks, quick actions.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, Rgba, SharedString, Styled,
    Window, div, prelude::*, px,
};

use super::{missing_notice, open_folder};
use crate::core::binpatch::PatchState;
use crate::games::{PageKind, Support};
use crate::mods::SdkStatus;
use crate::theme::{self, Icon, Rarity};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;

    let (support_text, support_rarity) = match def.support {
        Support::Full => ("Fully supported", Rarity::Legendary),
        Support::Preview => ("Preview support", Rarity::Rare),
    };

    let launch_ws = ws.clone();
    let rescan_ws = ws.clone();
    let hero = ui::hero_panel()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(div().h(px(12.)).w_full().bg(theme::accent()).child(ui::hazard_stripes(theme::ink(), 22.).size_full()))
        .child(
            div()
                .p(px(24.))
                .flex()
                .items_end()
                .gap(px(20.))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(ui::badge(support_text, support_rarity.color()))
                                .when_some(game.install.as_ref(), |d, i| d.child(ui::badge(i.store.label(), theme::echo()))),
                        )
                        .child(ui::display(def.name, 60.))
                        .child(ui::body(def.tagline).text_size(px(15.))),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(10.))
                        .child(
                            ui::button("ov-rescan", "Rescan", Some(Icon::Refresh), Variant::Secondary)
                                .on_click(move |_, _, cx| rescan_ws.update(cx, |ws, cx| ws.refresh_active(cx))),
                        )
                        .when(game.install.is_some(), |d| {
                            d.child(
                                ui::button("ov-launch", "Launch", Some(Icon::Play), Variant::Primary)
                                    .on_click(move |_, _, cx| launch_ws.update(cx, |ws, cx| ws.launch(cx))),
                            )
                        }),
                ),
        );

    let mut page = div().flex().flex_col().gap(px(22.)).child(hero);

    if game.install.is_none() {
        page = page.child(missing_notice(
            &format!("{} wasn't found", def.name),
            &format!(
                "Vault Patcher checked your Steam libraries and the Epic Games Launcher. If it's installed somewhere else, point to the game folder (the one containing {}). Config tweaks still work without it.",
                def.exe
            ),
            ws,
        ));
    }

    // Status cards
    let mut cards = div().flex().flex_wrap().gap(px(18.));

    let install_path = game.install.as_ref().map(|i| i.root.clone());
    cards = cards.child(stat_card(
        "Game install",
        Icon::Folder,
        if install_path.is_some() { theme::success() } else { theme::danger() },
        if install_path.is_some() { "Found" } else { "Not found" },
        install_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".into()),
        install_path.map(|p| {
            ui::button("ov-open-install", "Open folder", Some(Icon::Folder), Variant::Ghost)
                .on_click(move |_, _, cx| open_folder(&p, cx))
                .into_any_element()
        }),
    ));

    let files_total = game.config.files().count();
    let files_found = game.config.files().filter(|(_, f)| f.exists).count();
    let locked = state.configs_locked();
    let config_dir = game.config_dir.clone();
    cards = cards.child(stat_card(
        "Config files",
        Icon::Settings,
        if files_found > 0 { theme::success() } else { theme::danger() },
        &format!("{files_found} of {files_total} found{}", if locked { " · locked" } else { "" }),
        config_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".into()),
        config_dir.filter(|p| p.is_dir()).map(|p| {
            ui::button("ov-open-config", "Open folder", Some(Icon::Folder), Variant::Ghost)
                .on_click(move |_, _, cx| open_folder(&p, cx))
                .into_any_element()
        }),
    ));

    if let Some(support) = def.mods {
        let (text, color) = match &game.sdk {
            SdkStatus::Installed(v) => (format!("Installed ({v})"), theme::success()),
            SdkStatus::Detected => ("Installed".to_string(), theme::success()),
            SdkStatus::Legacy => ("Legacy SDK — update".to_string(), Rarity::Legendary.color()),
            SdkStatus::NotInstalled => ("Not installed".to_string(), theme::text_dim()),
        };
        let user_mods = game.mods.iter().filter(|m| !m.core).count();
        let enabled = game.mods.iter().filter(|m| !m.core && m.enabled).count();
        let nav_ws = ws.clone();
        cards = cards.child(stat_card(
            support.sdk_name,
            Icon::Puzzle,
            color,
            &text,
            format!("{user_mods} mod(s) installed, {enabled} enabled"),
            Some(
                ui::button("ov-mods", "Manage mods", Some(Icon::ChevronRight), Variant::Ghost)
                    .on_click(move |_, _, cx| nav_ws.update(cx, |ws, cx| ws.navigate(PageKind::Mods, cx)))
                    .into_any_element(),
            ),
        ));
    }

    let modified = def
        .visible_tweaks()
        .filter(|t| game.current(t).is_some_and(|v| v != t.default.to_value()))
        .count();
    let preset_ws = ws.clone();
    cards = cards.child(stat_card(
        "Tweaks",
        Icon::Sliders,
        if game.pending.is_empty() { theme::echo() } else { theme::accent() },
        &format!("{} staged", game.pending.len()),
        format!(
            "{modified} of {} settings differ from the game's defaults",
            def.visible_tweaks().count()
        ),
        Some(
            ui::button("ov-presets", "Presets", Some(Icon::Star), Variant::Ghost)
                .on_click(move |_, _, cx| preset_ws.update(cx, |ws, cx| ws.navigate(PageKind::Presets, cx)))
                .into_any_element(),
        ),
    ));

    if let Some(exe) = &game.exe {
        let applied = def
            .patches
            .iter()
            .filter(|p| exe.patch_states.get(p.id) == Some(&PatchState::Patched))
            .count();
        cards = cards.child(stat_card(
            "Executable",
            Icon::Code,
            theme::echo(),
            &format!("{:.1} MB", exe.size as f64 / 1_048_576.0),
            if def.patches.is_empty() {
                "No exe patches for this game".to_string()
            } else {
                format!("{applied} of {} patches active", def.patches.len())
            },
            None,
        ));
    }

    let latest = game.backups.first().map(|b| b.created_at.clone());
    let backup_ws = ws.clone();
    cards = cards.child(stat_card(
        "Backups",
        Icon::History,
        theme::echo(),
        &format!("{} snapshot(s)", game.backups.len()),
        latest.map(|d| format!("Latest: {d}")).unwrap_or_else(|| "Every apply creates one automatically".into()),
        Some(
            ui::button("ov-backups", "View", Some(Icon::ChevronRight), Variant::Ghost)
                .on_click(move |_, _, cx| backup_ws.update(cx, |ws, cx| ws.navigate(PageKind::Backups, cx)))
                .into_any_element(),
        ),
    ));

    page = page.child(cards);

    // Tips
    page = page.child(
        ui::card(theme::echo()).child(
            ui::card_body()
                .p(px(18.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(ui::label("How Vault Patcher works").text_color(theme::echo()))
                .child(ui::body("1. Pick settings on the tweak pages or stage a preset. Nothing is written yet."))
                .child(ui::body("2. Press Apply in the bar at the bottom. Every file is snapshotted first, and only the lines being changed are edited."))
                .child(ui::body("3. Changed your mind? Restore any snapshot from Backups, or reset single tweaks to their defaults.")),
        ),
    );

    page.into_any_element()
}

fn stat_card(
    title: &str,
    icon: Icon,
    color: Rgba,
    headline: &str,
    detail: String,
    action: Option<AnyElement>,
) -> impl IntoElement {
    ui::card(color).min_w(px(300.)).flex_1().child(
        ui::card_body()
            
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::icon(icon).text_color(color).text_size(px(15.)))
                    .child(ui::label(SharedString::from(title.to_string()))),
            )
            .child(
                div()
                    .font_family(theme::FONT_LABEL)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(20.))
                    .text_color(theme::text())
                    .child(headline.to_string()),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme::text_muted())
                    .font_family(theme::FONT_MONO)
                    .truncate()
                    .child(detail),
            )
            .children(action.map(|a| div().pt(px(4.)).flex().child(a))),
    )
}
