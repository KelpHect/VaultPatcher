//! App-wide preferences, per-game folders, profiles, updates and diagnostics.

use gpui::{AnyElement, App, ClipboardItem, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::input::Input;

use super::{confirm, open_folder, page_header, pick_paths};
use crate::core::backup;
use crate::mods::SdkStatus;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant, setting_row};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;

    // ---- appearance & sound
    let muted = state.settings.sound_muted;
    let music = state.settings.music;
    let sounds = crate::sound::available();
    let mute_ws = ws.clone();
    let music_ws = ws.clone();
    let appearance = ui::panel()
        .flex()
        .flex_col()
        .child(setting_row(
            "Theme",
            "Follows Windows: light or dark mode and your accent color, set in Personalization > Colors.",
            ui::button("set-colors", "Windows color settings", Some(Icon::Link), Variant::Secondary)
                .on_click(|_, _, cx| cx.open_url("ms-settings:colors"))
                .into_any_element(),
        ))
        .child(ui::divider())
        .child(setting_row(
            "Button sounds",
            if sounds { "Clicks and chimes from the game's own launcher files." } else { "Not available: no Borderlands launcher sounds found on this PC." },
            ui::toggle("set-sound", !muted && sounds)
                .when(!sounds, |d| d.opacity(0.4))
                .on_click(move |_, _, cx| mute_ws.update(cx, |ws, cx| ws.set_muted(!muted, cx)))
                .into_any_element(),
        ))
        .child(ui::divider())
        .child(setting_row(
            "Menu music",
            "Loops the launcher's music while Vault Patcher is open.",
            ui::toggle("set-music", music && !muted && sounds)
                .when(!sounds || muted, |d| d.opacity(0.4))
                .on_click(move |_, _, cx| music_ws.update(cx, |ws, cx| ws.set_music(!music, cx)))
                .into_any_element(),
        ));

    // ---- folders
    let install_ws = ws.clone();
    let config_ws = ws.clone();
    let reset_ws = ws.clone();
    let install_path = game.install.as_ref().map(|i| i.root.clone());
    let config_path = game.config_dir.clone();
    let folder_row = |id: &'static str, title: &'static str, value: String, path: Option<std::path::PathBuf>, on_pick: Box<dyn Fn(&mut App)>| {
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(16.))
            .py(px(10.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(ui::title(title))
                    .child(div().font_family(theme::font_mono()).text_size(px(12.)).text_color(theme::text_dim()).truncate().child(value)),
            )
            .when_some(path, |d, p| {
                d.child(
                    ui::icon_button(SharedString::from(format!("{id}-open")), Icon::Folder, theme::text_muted())
                        .tooltip(ui::tip("Open in Explorer"))
                        .on_click(move |_, _, cx| open_folder(&p, cx)),
                )
            })
            .child(ui::button(SharedString::from(format!("{id}-pick")), "Change…", None, Variant::Secondary).on_click(move |_, _, cx| on_pick(cx)))
    };
    let folders = ui::panel()
        .flex()
        .flex_col()
        .child(folder_row(
            "install",
            "Install folder",
            game.install.as_ref().map(|i| format!("{}  ({})", i.root.display(), i.store.label())).unwrap_or_else(|| "Not found".into()),
            install_path,
            Box::new(move |cx| {
                let ws = install_ws.clone();
                pick_paths(false, true, false, cx, move |p, cx| {
                    if let Some(p) = p.into_iter().next() {
                        ws.update(cx, |ws, cx| ws.set_manual_install(p, cx));
                    }
                });
            }),
        ))
        .child(ui::divider())
        .child(folder_row(
            "config",
            "Settings folder",
            game.config_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not found".into()),
            config_path,
            Box::new(move |cx| {
                let ws = config_ws.clone();
                pick_paths(false, true, false, cx, move |p, cx| {
                    if let Some(p) = p.into_iter().next() {
                        ws.update(cx, |ws, cx| ws.set_manual_config_dir(p, cx));
                    }
                });
            }),
        ))
        .child(ui::divider())
        .child(setting_row(
            "Auto-detect",
            "Forget custom folders and look in Steam, Epic and Documents\\My Games again.",
            ui::button("reset-paths", "Use auto-detect", Some(Icon::Refresh), Variant::Ghost)
                .on_click(move |_, _, cx| reset_ws.update(cx, |ws, cx| ws.clear_manual_paths(cx)))
                .into_any_element(),
        ));

    // ---- behavior
    let lock = state.settings.lock_configs_after_apply;
    let lock_ws = ws.clone();
    let keep = state.settings.max_backups.unwrap_or(30);
    let mut keep_chips = div().flex().gap(px(4.));
    for n in [10usize, 30, 100] {
        let ws = ws.clone();
        keep_chips = keep_chips.child(
            ui::chip(SharedString::from(format!("keep-{n}")), n.to_string(), keep == n)
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.update_settings(|s| s.max_backups = Some(n), cx))),
        );
    }
    let behavior = ui::panel()
        .flex()
        .flex_col()
        .child(setting_row(
            "Lock settings files after applying",
            "Marks the ini files read-only so nothing can revert them. The in-game options menu can't save while locked.",
            ui::toggle("set-lock", lock)
                .on_click(move |_, _, cx| lock_ws.update(cx, |ws, cx| ws.update_settings(|s| s.lock_configs_after_apply = !lock, cx)))
                .into_any_element(),
        ))
        .child(ui::divider())
        .child(setting_row(
            "Backups to keep",
            "Older automatic settings backups are pruned. Your original settings and exe/SDK backups are always kept.",
            keep_chips.into_any_element(),
        ));

    // ---- updates & diagnostics
    let app_update = state.updates.app.clone();
    let sdk_line = match (&game.sdk, state.updates.sdk.as_deref()) {
        (SdkStatus::Installed(v), Some(latest)) if v != latest => format!("Mod SDK {v} installed; {latest} is available (Mods page)."),
        (SdkStatus::Installed(v), _) => format!("Mod SDK {v} installed and up to date."),
        (_, Some(latest)) => format!("Latest mod SDK: {latest}."),
        _ => "Couldn't check for updates (offline?).".into(),
    };
    let diag_ws = ws.clone();
    let updates = ui::panel()
        .flex()
        .flex_col()
        .child(setting_row(
            format!("Vault Patcher {}", env!("CARGO_PKG_VERSION")),
            match &app_update {
                Some((tag, _)) => format!("Version {tag} is available."),
                None => format!("Up to date. {sdk_line}"),
            },
            match app_update {
                Some((_, page)) => ui::button("get-update", "Download", Some(Icon::Download), Variant::Primary)
                    .on_click(move |_, _, cx| cx.open_url(&page))
                    .into_any_element(),
                None => ui::button("releases", "Releases", Some(Icon::Link), Variant::Ghost)
                    .on_click(|_, _, cx| cx.open_url(&format!("https://github.com/{}/releases", crate::compare::IMAGE_REPO)))
                    .into_any_element(),
            },
        ))
        .child(ui::divider())
        .child(setting_row(
            "Copy diagnostics",
            "Copies paths, versions and install states (no personal files) for a bug report.",
            ui::button("copy-diag", "Copy", Some(Icon::Copy), Variant::Secondary)
                .on_click(move |_, _, cx| {
                    let text = crate::health::diagnostics(diag_ws.read(cx));
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    diag_ws.update(cx, |ws, cx| ws.toast(crate::workspace::ToastKind::Success, "Diagnostics copied to the clipboard", cx));
                })
                .into_any_element(),
        ))
        .child(ui::divider())
        .child(setting_row(
            "Report a problem",
            "Opens the issue tracker. Paste the diagnostics into your report.",
            ui::button("issues", "Open", Some(Icon::Bug), Variant::Ghost)
                .on_click(|_, _, cx| cx.open_url(&format!("https://github.com/{}/issues", crate::compare::IMAGE_REPO)))
                .into_any_element(),
        ));

    // ---- about
    let data_dir = backup::data_dir();
    let about = ui::panel().flex().flex_col().child(setting_row(
        "About",
        format!(
            "Vault Patcher {} · built with GPUI and gpui-component. Settings verified against live game files, PCGamingWiki, the Nvidia tweak guide, OpenBLCMM and the BLCMods wiki. Icons: Lucide (ISC). Not affiliated with Gearbox or 2K.",
            env!("CARGO_PKG_VERSION")
        ),
        ui::button("open-data", "Data folder", Some(Icon::Folder), Variant::Ghost)
            .on_click(move |_, _, cx| {
                std::fs::create_dir_all(&data_dir).ok();
                open_folder(&data_dir, cx)
            })
            .into_any_element(),
    ));

    div()
        .max_w(px(860.))
        .flex()
        .flex_col()
        .gap(px(22.))
        .child(page_header("App Settings", "Preferences for Vault Patcher itself.", vec![]))
        .child(section("Appearance and sound", appearance))
        .child(section(&format!("{} folders", def.name), folders))
        .child(section("Behavior", behavior))
        .child(profiles_section(ws, cx))
        .child(section("Updates and support", updates))
        .when(!def.comparisons.is_empty(), |d| {
            let ws = ws.clone();
            d.child(section(
                "Tools",
                ui::panel().flex().flex_col().child(setting_row(
                    "Comparison capture",
                    "Re-shoot the comparison screenshots on this PC (about 30 minutes; the game runs on its own).",
                    ui::button("open-capture", "Open", Some(Icon::Camera), Variant::Secondary)
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.navigate(crate::games::PageKind::Capture, cx)))
                        .into_any_element(),
                )),
            ))
        })
        .child(about)
        .into_any_element()
}

fn section(title: &str, body: impl IntoElement) -> impl IntoElement {
    div().flex().flex_col().gap(px(8.)).child(ui::section_title(title.to_string(), None)).child(body)
}

/// Save the current settings under a name; load, export, import, delete.
pub(crate) fn profiles_section(ws: &Entity<Workspace>, cx: &App) -> AnyElement {
    let state = ws.read(cx);
    let entries = state.game().profiles.clone();
    let advanced = state.mode() == crate::games::Mode::Advanced;

    let save_ws = ws.clone();
    let import_ws = ws.clone();
    let mut panel = ui::panel().flex().flex_col().child(
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(16.))
            .py(px(12.))
            .children(state.profile_input.clone().map(|input| div().flex_1().min_w(px(200.)).child(Input::new(&input).w_full())))
            .child(
                ui::button("profile-save", "Save current settings", Some(Icon::Save), Variant::Secondary).on_click(move |_, window, cx| {
                    let Some(input) = save_ws.read(cx).profile_input.clone() else { return };
                    let name = input.read(cx).value().to_string();
                    let saved = save_ws.update(cx, |ws, cx| ws.save_profile(&name, cx));
                    if saved {
                        input.update(cx, |i, cx| i.set_value("", window, cx));
                    }
                }),
            )
            .child(
                ui::button("profile-import", "Import…", Some(Icon::FileUp), Variant::Ghost)
                    .tooltip(ui::tip("Add a profile file someone shared with you"))
                    .on_click(move |_, _, cx| {
                        let ws = import_ws.clone();
                        pick_paths(true, false, false, cx, move |paths, cx| {
                            if let Some(p) = paths.into_iter().next() {
                                ws.update(cx, |ws, cx| ws.import_profile(&p, cx));
                            }
                        });
                    }),
            ),
    );
    if entries.is_empty() {
        panel = panel.child(ui::divider()).child(
            div()
                .px(px(16.))
                .py(px(12.))
                .child(ui::body("No profiles yet. Save one to switch between looks (e.g. \"Streaming\" and \"Max quality\") or to share your setup.").text_color(theme::text_dim())),
        );
    }
    for entry in entries {
        let load_ws = ws.clone();
        let export_ws = ws.clone();
        let delete_ws = ws.clone();
        let (load_path, export_path, delete_path) = (entry.path.clone(), entry.path.clone(), entry.path.clone());
        let file_name = entry.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let name = entry.name.clone();
        panel = panel.child(ui::divider()).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(16.))
                .py(px(8.))
                .child(ui::icon(Icon::Profile).text_color(theme::text_dim()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(ui::title(entry.name.clone()))
                        .child(div().text_size(px(12.)).text_color(theme::text_dim()).child(format!("{} settings · saved {}", entry.count, entry.created))),
                )
                .child(
                    ui::button(SharedString::from(format!("pl-{file_name}")), "Load", None, Variant::Secondary)
                        .tooltip(ui::tip(if advanced { "Adds the profile's values to your waiting changes" } else { "Applies the profile now (backed up first)" }))
                        .on_click(move |_, _, cx| load_ws.update(cx, |ws, cx| ws.load_profile(&load_path, cx))),
                )
                .child(
                    ui::icon_button(SharedString::from(format!("pe-{file_name}")), Icon::FileDown, theme::text_muted())
                        .tooltip(ui::tip("Export to a file"))
                        .on_click(move |_, _, cx| {
                            let ws = export_ws.clone();
                            let from = export_path.clone();
                            let dir = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_default();
                            let rx = cx.prompt_for_new_path(&dir, Some(&file_name_for(&from)));
                            cx.spawn(async move |cx| {
                                if let Ok(Ok(Some(to))) = rx.await {
                                    cx.update(|cx| ws.update(cx, |ws, cx| ws.export_profile(&from, &to, cx))).ok();
                                }
                            })
                            .detach();
                        }),
                )
                .child(
                    ui::icon_button(SharedString::from(format!("pd-{file_name}")), Icon::Delete, theme::text_muted())
                        .tooltip(ui::tip("Delete"))
                        .on_click(move |_, window, cx| {
                            let ws = delete_ws.clone();
                            let path = delete_path.clone();
                            confirm(window, cx, &format!("Delete the profile \"{name}\"?"), "This only removes the saved profile; your settings aren't changed.", "Delete", move |cx| {
                                ws.update(cx, |ws, cx| ws.delete_profile(&path, cx))
                            });
                        }),
                ),
        );
    }
    section("Profiles", panel).into_any_element()
}

fn file_name_for(path: &std::path::Path) -> String {
    format!("vault-patcher-{}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "profile.json".into()))
}
