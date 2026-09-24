//! App-wide preferences and per-game path overrides.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::{open_folder, page_header, pick_paths};
use crate::core::backup;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;

    // ---- paths
    let install_ws = ws.clone();
    let config_ws = ws.clone();
    let reset_ws = ws.clone();
    let paths = ui::panel()
        .p(px(18.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(ui::kv_row(
            "Install folder",
            game.install
                .as_ref()
                .map(|i| format!("{}  ({})", i.root.display(), i.store.label()))
                .unwrap_or_else(|| "Not found".into()),
        ))
        .child(ui::kv_row(
            "Config folder",
            game.config_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Not found".into()),
        ))
        .child(
            div()
                .pt(px(8.))
                .flex()
                .flex_wrap()
                .gap(px(10.))
                .child(
                    ui::button("set-install", "Choose install folder…", Some(Icon::Folder), Variant::Secondary)
                        .on_click(move |_, _, cx| {
                            let ws = install_ws.clone();
                            pick_paths(false, true, false, cx, move |p, cx| {
                                if let Some(p) = p.into_iter().next() {
                                    ws.update(cx, |ws, cx| ws.set_manual_install(p, cx));
                                }
                            });
                        }),
                )
                .child(
                    ui::button("set-config", "Choose config folder…", Some(Icon::Folder), Variant::Secondary)
                        .on_click(move |_, _, cx| {
                            let ws = config_ws.clone();
                            pick_paths(false, true, false, cx, move |p, cx| {
                                if let Some(p) = p.into_iter().next() {
                                    ws.update(cx, |ws, cx| ws.set_manual_config_dir(p, cx));
                                }
                            });
                        }),
                )
                .child(
                    ui::button("reset-paths", "Use auto-detect", Some(Icon::Refresh), Variant::Ghost)
                        .on_click(move |_, _, cx| reset_ws.update(cx, |ws, cx| ws.clear_manual_paths(cx))),
                ),
        );

    // ---- behavior
    let lock = state.settings.lock_configs_after_apply;
    let lock_ws = ws.clone();
    let keep = state.settings.max_backups.unwrap_or(30);
    let mut keep_chips = div().flex().gap(px(6.));
    for n in [10usize, 30, 100] {
        let ws = ws.clone();
        keep_chips = keep_chips.child(
            ui::chip(SharedString::from(format!("keep-{n}")), n.to_string(), keep == n).on_click(move |_, _, cx| {
                ws.update(cx, |ws, cx| ws.update_settings(|s| s.max_backups = Some(n), cx))
            }),
        );
    }
    let behavior = ui::panel()
        .flex()
        .flex_col()
        .child(setting_row(
            "Lock config files after applying",
            "Marks ini files read-only after every apply so nothing can revert them. The in-game options menu can't save while locked.",
            ui::toggle("set-lock", lock)
                .on_click(move |_, _, cx| {
                    lock_ws.update(cx, |ws, cx| ws.update_settings(|s| s.lock_configs_after_apply = !lock, cx))
                })
                .into_any_element(),
        ))
        .child(div().h(px(1.)).bg(theme::line()))
        .child(setting_row(
            "Config snapshots to keep",
            "Older automatic config snapshots are pruned. Exe and SDK snapshots are always kept.",
            keep_chips.into_any_element(),
        ));

    // ---- about
    let data_dir = backup::data_dir();
    let about = ui::panel()
        .p(px(18.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(ui::display("Vault Patcher", 30.))
        .child(ui::body(format!(
            "Version {} · Built with GPUI. Tweak data verified against live game files, PCGamingWiki, the Nvidia tweak guide, OpenBLCMM and the BLCMods wiki. Not affiliated with Gearbox or 2K.",
            env!("CARGO_PKG_VERSION")
        )))
        .child(ui::kv_row("Data folder", data_dir.display().to_string()))
        .child(
            div().pt(px(4.)).child(
                ui::button("open-data", "Open data folder", Some(Icon::Folder), Variant::Ghost).on_click(move |_, _, cx| {
                    std::fs::create_dir_all(&data_dir).ok();
                    open_folder(&data_dir, cx)
                }),
            ),
        );

    div()
        .flex()
        .flex_col()
        .gap(px(22.))
        .child(page_header("App Settings", "Preferences for Vault Patcher itself.", vec![]))
        .child(ui::section_title(format!("{} paths", def.short), None))
        .child(paths)
        .child(ui::section_title("Behavior", None))
        .child(behavior)
        .child(about)
        .into_any_element()
}

fn setting_row(title: &str, detail: &str, control: AnyElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .px(px(16.))
        .py(px(14.))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .font_family(theme::FONT_LABEL)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(15.))
                        .text_color(theme::text())
                        .child(title.to_string()),
                )
                .child(ui::body(detail.to_string())),
        )
        .child(control)
}
