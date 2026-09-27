//! App-wide preferences, per-game folders, profiles, updates and diagnostics.

use gpui::{AnyElement, App, ClipboardItem, Div, Entity, IntoElement, ParentElement, SharedString, Stateful, Styled, Window, div, prelude::*, px};
use gpui_component::button::Button;
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};

use super::{caption_style, clipped_with_tip, confirm, count, friendly_now, open_folder, page_header, pick_paths, short_path};
use crate::core::backup;
use crate::mods::SdkStatus;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant, setting_row};
use crate::workspace::{AppFrameRate, Workspace};

/// Settings page column width (the Gallery's SettingsPage max width).
const COLUMN: f32 = 1064.;

/// Id of the About expander in the workspace's expanded set.
const ABOUT: &str = "app-settings-about";

/// A clickable SettingsCard: the whole row is the button, with an action
/// glyph on the right (open-in-new for link-outs, a chevron for pages).
/// Vault Patcher's own frame rate (not the game's), with a warning when
/// "Match display" would mean more than 60 frames a second.
fn frame_rate_row(ws: &Entity<Workspace>, current: AppFrameRate, cx: &App) -> AnyElement {
    use crate::tweaks::{Opt, opt};
    static OPTIONS: [Opt; 4] = [opt("30", "30 fps"), opt("balanced", "Balanced"), opt("60", "60 fps"), opt("display", "Match display")];
    let hz = crate::win11::display_refresh_hz();
    let label = match current {
        AppFrameRate::Display => match hz {
            Some(hz) => format!("Match display ({hz} Hz)"),
            None => "Match display".to_string(),
        },
        AppFrameRate::Balanced => format!("Balanced ({} fps)", current.cap().unwrap_or(45)),
        other => format!("{} fps", other.cap().unwrap_or(0)),
    };
    let pick_ws = ws.clone();
    let select = crate::controls::select(
        "set-frame-rate",
        label,
        &OPTIONS,
        Some(current.id()),
        false,
        std::rc::Rc::new(move |v, _, cx| {
            if let Some(rate) = AppFrameRate::from_id(v) {
                pick_ws.update(cx, |ws, cx| ws.update_settings(|s| s.frame_rate = rate, cx));
            }
        }),
        cx,
    );
    let warn = (current == AppFrameRate::Display).then_some(hz).flatten().filter(|&hz| hz > 60);
    div()
        .flex()
        .flex_col()
        .child(setting_row(
            "App frame rate",
            "How smoothly Vault Patcher's own animations run. This is the app's frame rate, not the game's: it never affects how Borderlands runs. Balanced picks an even fraction of your display's refresh rate near 45 fps. Nothing is redrawn while nothing changes, and dragging or typing always follows your display.",
            select,
        ))
        .children(warn.map(|hz| {
            div().px(px(16.)).pb(px(12.)).child(ui::info_bar(
                ui::Severity::Warning,
                format!("{hz} Hz uses about {}x the CPU of Balanced", (hz as f32 / crate::workspace::balanced_fps(hz) as f32).round() as u32),
                format!("Vault Patcher would redraw {hz} times a second while animating, for no visible benefit in a settings app. Balanced or 60 fps is recommended."),
            ))
        }))
        .into_any_element()
}

fn link_row(
    id: &'static str,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    glyph: Icon,
    on_click: impl Fn(&mut App) + 'static,
) -> Stateful<Div> {
    ui::focusable_row(div().id(id))
        .flex()
        .items_center()
        .gap(px(16.))
        .min_h(px(68.))
        .px(px(16.))
        .py(px(12.))
        .cursor_pointer()
        .hover(|s| s.bg(theme::panel_hi()))
        .active(|s| s.bg(theme::panel_pressed()))
        .child(div().flex_1().min_w_0().flex().flex_col().child(ui::title(title)).child(ui::caption(description)))
        .child(ui::icon(glyph).size(px(if glyph == Icon::Link { 13. } else { 12. })).text_color(theme::text_muted()))
        .on_click(move |_, _, cx| on_click(cx))
}

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
        .child(link_row(
            "set-colors",
            "Theme",
            format!(
                "Follows Windows: {} mode and your accent color. Change them in Personalization > Colors.",
                if theme::is_dark() { "dark" } else { "light" }
            ),
            Icon::Link,
            |cx| cx.open_url("ms-settings:colors"),
        ))
        .child(ui::divider())
        .child(setting_row(
            "Button sounds",
            if sounds { "Clicks and chimes from the game's own launcher files." } else { "Not available: no Borderlands launcher sounds found on this PC." },
            ui::toggle_if(sounds, "set-sound", !muted && sounds)
                .when(!sounds, |d| d.tooltip(ui::tip("No launcher sounds found on this PC")))
                .on_click(move |_, _, cx| mute_ws.update(cx, |ws, cx| ws.set_muted(!muted, cx)))
                .into_any_element(),
        ))
        .child(ui::divider())
        .child(setting_row(
            "Menu music",
            "Loops the launcher's music while Vault Patcher is open.",
            ui::toggle_if(sounds && !muted, "set-music", music && !muted && sounds)
                .when(!sounds || muted, |d| {
                    d.tooltip(ui::tip(if sounds { "Turn on button sounds first" } else { "No launcher sounds found on this PC" }))
                })
                .on_click(move |_, _, cx| music_ws.update(cx, |ws, cx| ws.set_music(!music, cx)))
                .into_any_element(),
        ))
        .child(ui::divider())
        .child(frame_rate_row(ws, state.settings.frame_rate, cx));

    // ---- folders
    let install_ws = ws.clone();
    let config_ws = ws.clone();
    let reset_ws = ws.clone();
    let install_path = game.install.as_ref().map(|i| i.root.clone());
    let config_path = game.config_dir.clone();
    let folder_row = |id: &'static str, title: &'static str, value: (String, String), path: Option<std::path::PathBuf>, on_pick: Box<dyn Fn(&mut App)>| {
        let (shown, full) = value;
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .min_h(px(68.))
            .px(px(16.))
            .py(px(12.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(ui::title(title))
                    .child(caption_style(clipped_with_tip(format!("{id}-path"), shown, full)).font_family(theme::font_mono())),
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
    let not_found = || ("Not found".to_string(), "Not found".to_string());
    let folders = ui::panel()
        .flex()
        .flex_col()
        .child(folder_row(
            "install",
            "Install folder",
            game.install
                .as_ref()
                .map(|i| (format!("{}  ({})", short_path(&i.root, 64), i.store.label()), format!("{}  ({})", i.root.display(), i.store.label())))
                .unwrap_or_else(not_found),
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
            game.config_dir.as_ref().map(|p| (short_path(p, 64), p.display().to_string())).unwrap_or_else(not_found),
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
            "Search again",
            "Look for every game's install and settings folders again: after installing or moving a game, or plugging in a drive.",
            ui::button_if(!state.searching, "search-again", if state.searching { "Searching\u{2026}" } else { "Search again" }, Some(Icon::Search), Variant::Secondary)
                .on_click({
                    let ws = ws.clone();
                    move |_, _, cx| ws.update(cx, |ws, cx| ws.search_again(cx))
                })
                .into_any_element(),
        ))
        .child(ui::divider())
        .child(setting_row(
            "Auto-detect",
            "Forget custom folders and look in Steam, Epic and Documents\\My Games again.",
            ui::button("reset-paths", "Use auto-detect", Some(Icon::Refresh), Variant::Secondary)
                .on_click(move |_, _, cx| reset_ws.update(cx, |ws, cx| ws.clear_manual_paths(cx)))
                .into_any_element(),
        ));

    // ---- behavior
    let lock = state.settings.lock_configs_after_apply;
    let lock_ws = ws.clone();
    let keep = state.settings.max_backups.unwrap_or(30);
    let keep_ws = ws.clone();
    // ComboBox: pick one of a few values.
    let keep_box = Button::new("keep-backups")
        .outline()
        .dropdown_caret(true)
        .label(count(keep, "backup", "backups"))
        .h(px(32.))
        .min_w(px(160.))
        .dropdown_menu(move |mut menu, _, _| {
            for n in [10usize, 30, 100] {
                let ws = keep_ws.clone();
                menu = menu.item(
                    PopupMenuItem::new(count(n, "backup", "backups"))
                        .checked(keep == n)
                        .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.update_settings(|s| s.max_backups = Some(n), cx))),
                );
            }
            menu
        });
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
            keep_box.into_any_element(),
        ))
        .child(ui::divider())
        .child({
            let dir = backup::data_dir();
            setting_row(
                "Where Vault Patcher keeps its data",
                format!("Settings, backups, profiles and cached images: {}", dir.display()),
                ui::button("open-data", "Open folder", Some(Icon::Folder), Variant::Secondary)
                    .on_click(move |_, _, cx| {
                        std::fs::create_dir_all(&dir).ok();
                        open_folder(&dir, cx)
                    })
                    .into_any_element(),
            )
        });

    // ---- updates & diagnostics
    let app_update = state.updates.app.clone();
    let sdk_line = match (&game.sdk, state.latest_sdk()) {
        (SdkStatus::Installed(v), Some(latest)) if v != latest => format!("Mod SDK {v} installed; {latest} is available (Mods page)."),
        (SdkStatus::Installed(v), _) => format!("Mod SDK {v} installed and up to date."),
        (_, Some(latest)) => format!("Latest mod SDK: {latest}."),
        _ => "Couldn't check for updates (offline?).".into(),
    };
    let diag_ws = ws.clone();
    let version_title = format!("Vault Patcher {}", env!("CARGO_PKG_VERSION"));
    let version_row = match app_update {
        Some((tag, page)) => setting_row(
            version_title,
            format!("Version {tag} is available."),
            ui::button("get-update", "Download", Some(Icon::Download), Variant::Primary)
                .on_click(move |_, _, cx| cx.open_url(&page))
                .into_any_element(),
        )
        .into_any_element(),
        None => link_row("releases", version_title, format!("Up to date. {sdk_line} Opens the release notes."), Icon::Link, |cx| {
            cx.open_url(&format!("https://github.com/{}/releases", crate::compare::IMAGE_REPO))
        })
        .into_any_element(),
    };
    let updates = ui::panel()
        .flex()
        .flex_col()
        .child(version_row)
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
        .child(link_row("issues", "Report a problem", "Opens the issue tracker. Paste the diagnostics into your report.", Icon::Link, |cx| {
            cx.open_url(&format!("https://github.com/{}/issues", crate::compare::IMAGE_REPO))
        }));

    // ---- about: a SettingsExpander
    let about_open = state.setup_expanded.contains(ABOUT);
    let about_ws = ws.clone();
    let about = ui::panel()
        .flex()
        .flex_col()
        .child(
            ui::focusable_row(div().id("about-header"))
                .flex()
                .items_center()
                .gap(px(16.))
                .min_h(px(68.))
                .px(px(16.))
                .py(px(12.))
                .cursor_pointer()
                .hover(|s| s.bg(theme::panel_hi()))
                .active(|s| s.bg(theme::panel_pressed()))
                .child(ui::icon(Icon::Info).text_color(theme::text_muted()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(ui::title("About Vault Patcher"))
                        .child(ui::caption(format!("Version {} · not affiliated with Gearbox or 2K", env!("CARGO_PKG_VERSION")))),
                )
                .child(div().size(px(32.)).flex().flex_none().items_center().justify_center().child(
                    ui::icon(if about_open { Icon::ChevronUp } else { Icon::ChevronDown }).size(px(12.)).text_color(theme::text_muted()),
                ))
                .on_click(move |_, _, cx| about_ws.update(cx, |ws, cx| ws.toggle_expanded(ABOUT, cx))),
        )
        .when(about_open, |d| {
            let row = |title: &'static str, text: &'static str| {
                div().pl(px(48.)).pr(px(16.)).py(px(12.)).flex().flex_col().child(ui::title(title)).child(ui::caption(text))
            };
            d.child(
                div()
                    .border_t_1()
                    .border_color(theme::line())
                    .bg(theme::panel_lo())
                    .flex()
                    .flex_col()
                    .child(row("Built with", "GPUI and gpui-component."))
                    .child(ui::divider())
                    .child(row(
                        "Sources",
                        "Settings verified against live game files, PCGamingWiki, the Nvidia tweak guide, OpenBLCMM and the BLCMods wiki.",
                    ))
                    .child(ui::divider())
                    .child(row(
                        "Icons and fonts",
                        "Icons are Segoe Fluent Icons from Windows. Where Segoe UI isn't installed, text falls back to the bundled Noto Sans (SIL Open Font License).",
                    )),
            )
        });

    div()
        .w_full()
        .max_w(px(COLUMN))
        .flex()
        .flex_col()
        .gap(px(24.))
        .child(page_header("App settings", "Preferences for Vault Patcher itself.", vec![]))
        .child(section("Appearance and sound", appearance))
        .child(section(&format!("{} folders", def.name), folders))
        .child(section("Behavior", behavior))
        .child(profiles_section(ws, cx))
        .child(section("Updates and support", updates))
        .when(!def.comparisons.is_empty(), |d| {
            let ws = ws.clone();
            d.child(section(
                "Tools",
                ui::panel().flex().flex_col().child(link_row(
                    "open-capture",
                    "Comparison capture",
                    "Re-shoot the comparison screenshots on this PC (about 30 minutes; the game runs on its own).",
                    Icon::ChevronRight,
                    move |cx| ws.update(cx, |ws, cx| ws.navigate(crate::games::PageKind::Capture, cx)),
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
                ui::button("profile-import", "Import…", Some(Icon::FileUp), Variant::Secondary)
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
                .child(ui::body("No profiles yet. Save one to switch between looks (e.g. \"Streaming\" and \"Max quality\") or to share your setup.")),
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
                .min_h(px(56.))
                .px(px(16.))
                .py(px(8.))
                .child(ui::icon(Icon::Profile).text_color(theme::text_muted()).mr(px(4.)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(ui::title(entry.name.clone()))
                        .child(caption_style(
                            div()
                                .id(SharedString::from(format!("pt-{file_name}")))
                                .tooltip(ui::tip(format!("Saved {}", entry.created)))
                                .child(format!("{} · saved {}", count(entry.count, "setting", "settings"), friendly_now(&entry.created))),
                        )),
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
