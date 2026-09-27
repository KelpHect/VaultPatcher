//! Mod manager: the Python SDK (install/update/remove) and installed mods.
//! Files can also be dropped anywhere on the window.

use gpui::{AnyElement, App, Entity, ExternalPaths, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};

use super::{caption_style, clipped_with_tip, confirm, missing_notice, open_folder, page_header, pick_paths};
use crate::mods::{ModKind, SdkStatus};
use crate::theme::{self, Icon, Rarity};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let Some(support) = def.mods else {
        return page_header("Mods", "Mod support for this game is coming soon.", vec![]).into_any_element();
    };
    let Some(install) = game.install.clone() else {
        return div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(page_header("Mods", "SDK mods and text mods for this game.", vec![]))
            .child(missing_notice("Game install not found", "Mods are installed into the game folder, so Vault Patcher needs to know where it is.", ws))
            .into_any_element();
    };

    let add_ws = ws.clone();
    let add = ui::button("mods-add", "Add mods…", Some(Icon::Add), Variant::Primary)
        .tooltip(ui::tip(".sdkmod, .zip, .blcm or .txt. You can also drop files onto the window."))
        .on_click(move |_, _, cx| {
            let ws = add_ws.clone();
            pick_paths(true, false, true, cx, move |paths, cx| ws.update(cx, |ws, cx| ws.install_mod_files(paths, cx)));
        })
        .into_any_element();
    let sdk_dir = install.root.join(support.sdk_mods_dir);
    let text_dir = install.root.join(support.text_mods_dir);
    // One DropDownButton for both folders.
    let open_folders = Button::new("mods-open")
        .ghost()
        .h(px(32.))
        .px(px(12.))
        .tooltip("Open a mods folder in Explorer")
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(14.))
                .text_color(theme::text())
                .child(ui::icon(Icon::Folder).text_color(theme::text()))
                .child("Open folder")
                .child(ui::icon(Icon::ChevronDown).size(px(12.)).text_color(theme::text_muted())),
        )
        .dropdown_menu(move |menu, _, _| {
            let (sdk_dir, text_dir) = (sdk_dir.clone(), text_dir.clone());
            menu.item(PopupMenuItem::new("SDK mods folder").on_click(move |_, _, cx| open_folder(&sdk_dir, cx)))
                .item(PopupMenuItem::new("Text mods folder").on_click(move |_, _, cx| open_folder(&text_dir, cx)))
        })
        .into_any_element();

    // ---- SDK
    let update = state.sdk_update_available().map(str::to_string);
    let (status_text, status_color) = match &game.sdk {
        SdkStatus::Installed(v) if update.is_some() => (format!("{v} · update available"), theme::warning()),
        SdkStatus::Installed(v) => (format!("{v} · up to date"), theme::success()),
        SdkStatus::Detected => ("Installed manually".to_string(), theme::success()),
        SdkStatus::Legacy => ("Old PythonSDK".to_string(), theme::warning()),
        SdkStatus::NotInstalled => ("Not installed".to_string(), theme::text_muted()),
    };
    let busy = state.busy.clone();
    let installed = matches!(game.sdk, SdkStatus::Installed(_) | SdkStatus::Detected);
    let install_ws = ws.clone();
    let zip_ws = ws.clone();
    let uninstall_ws = ws.clone();
    let redist = support.redist_url;
    let primary_label = match (&update, installed) {
        (Some(latest), _) => format!("Update to {latest}"),
        (None, true) => "Reinstall".to_string(),
        (None, false) => "Install".to_string(),
    };
    let mut sdk = ui::panel().flex().flex_col().child(
        div()
            .flex()
            .items_center()
            .gap(px(16.))
            .min_h(px(68.))
            .px(px(16.))
            .py(px(12.))
            .child(ui::icon(Icon::Package).size(px(20.)).text_color(status_color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().flex().flex_wrap().items_center().gap(px(8.)).child(ui::title(support.sdk_name)).child(ui::badge(status_text, status_color)))
                    .child(ui::caption(
                        "Adds a MODS menu in-game and runs Python mods. Needs the Visual C++ runtime. Text mods need Text Mod Loader (part of One-click setup).",
                    )),
            )
            .child(
                ui::button_if(busy.is_none(), "sdk-install", primary_label, Some(Icon::Download), if update.is_some() || !installed { Variant::Primary } else { Variant::Secondary })
                    .when(busy.is_some(), |d| d.tooltip(ui::tip("Wait for the current task to finish")))
                    .on_click(move |_, _, cx| {
                        install_ws.update(cx, |ws, cx| {
                            if ws.busy.is_none() {
                                ws.install_sdk(cx)
                            }
                        })
                    }),
            )
            .child(
                ui::icon_button("sdk-zip", Icon::FileUp, theme::text_muted()).tooltip(ui::tip("Install from a zip you downloaded")).on_click(move |_, _, cx| {
                    let ws = zip_ws.clone();
                    pick_paths(true, false, false, cx, move |paths, cx| {
                        if let Some(p) = paths.into_iter().next() {
                            ws.update(cx, |ws, cx| ws.install_sdk_from_zip(p, cx));
                        }
                    });
                }),
            )
            .child(ui::icon_button("sdk-redist", Icon::Link, theme::text_muted()).tooltip(ui::tip("Download the Visual C++ runtime")).on_click(move |_, _, cx| cx.open_url(redist)))
            .when(matches!(game.sdk, SdkStatus::Installed(_)), |d| {
                d.child(ui::icon_button("sdk-remove", Icon::Delete, theme::text_muted()).tooltip(ui::tip("Uninstall the SDK")).on_click(move |_, window, cx| {
                    let ws = uninstall_ws.clone();
                    confirm(window, cx, "Uninstall the SDK?", "Removes the files Vault Patcher installed. Your mods and their settings stay in sdk_mods.", "Uninstall", move |cx| {
                        ws.update(cx, |ws, cx| ws.uninstall_sdk(cx))
                    });
                }))
            }),
    );
    let legacy = matches!(game.sdk, SdkStatus::Legacy).then(|| {
        ui::info_bar(
            ui::Severity::Warning,
            "The old PythonSDK is installed",
            "It uses the Mods folder and doesn't run current mods. Installing the new SDK migrates compatible mods automatically.",
        )
    });
    if let Some(b) = busy {
        sdk = sdk.child(ui::divider()).child(
            div()
                .px(px(16.))
                .py(px(12.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(ui::caption(b))
                .child(ui::progress_bar("sdk-busy", None, theme::accent())),
        );
    }
    // HyperlinkButtons.
    let mut links = div().flex().flex_wrap().gap(px(4.));
    for (i, (text, url)) in support.links.iter().enumerate() {
        let url = *url;
        links = links.child(
            ui::focusable(div().id(SharedString::from(format!("link-{i}"))))
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(32.))
                .px(px(8.))
                .rounded(px(theme::RADIUS))
                .text_size(px(14.))
                .text_color(theme::accent_text())
                .cursor_pointer()
                .hover(|s| s.bg(theme::panel_hi()))
                .active(|s| s.bg(theme::panel_pressed()))
                .tooltip(ui::tip(url))
                .child(ui::icon(Icon::Link).size(px(12.)).text_color(theme::accent_text()))
                .child(*text)
                .on_click(move |_, _, cx| cx.open_url(url)),
        );
    }

    // ---- mods
    let mut list = ui::panel().flex().flex_col();
    let core = game.mods.iter().filter(|m| m.core).count();
    if core > 0 {
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .min_h(px(48.))
                .px(px(16.))
                .py(px(8.))
                .child(ui::icon(Icon::Lock).text_color(theme::text_muted()))
                .child(ui::title(format!("SDK core modules ({core})")))
                .child(ui::caption("Part of the mod loader; always on.")),
        );
    }
    let user_mods: Vec<_> = game.mods.iter().filter(|m| !m.core).collect();
    if user_mods.is_empty() {
        // A drop zone (the whole window takes drops; this one lights up).
        list = list.when(core > 0, |d| d.child(ui::divider())).child(
            div().p(px(8.)).child(
                div()
                    .id("mods-drop")
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .py(px(24.))
                    .px(px(16.))
                    .rounded(px(theme::RADIUS))
                    .border_1()
                    .border_dashed()
                    .border_color(theme::ink())
                    .drag_over::<ExternalPaths>(|s, _, _, _| s.border_color(theme::accent()))
                    .child(ui::icon(Icon::Upload).size(px(32.)).text_color(theme::text_muted()))
                    .child(ui::body("No mods of your own yet. Drop .sdkmod, .zip, .blcm or .txt files here, or use Add mods…").text_center()),
            ),
        );
    }
    for (i, m) in user_mods.into_iter().enumerate() {
        if i > 0 || core > 0 {
            list = list.child(ui::divider());
        }
        let kind_color = match m.kind {
            ModKind::SdkPackage | ModKind::SdkFolder => Rarity::Rare.color(),
            ModKind::TextMod => Rarity::Uncommon.color(),
        };
        let enabled = m.enabled;
        let toggle_ws = ws.clone();
        let toggle_path = m.path.clone();
        let (menu_ws, menu_path, menu_name) = (ws.clone(), m.path.clone(), m.name.clone());
        let path_text = m.path.display().to_string();
        let description = m.description.clone().unwrap_or_else(|| path_text.clone());
        let description_tip = match &m.description {
            Some(d) => format!("{d}\n{path_text}"),
            None => path_text,
        };
        list = list.child(
            div()
                .id(SharedString::from(format!("mod-row-{i}")))
                .flex()
                .items_center()
                .gap(px(12.))
                .min_h(px(56.))
                .px(px(16.))
                .py(px(8.))
                .hover(|s| s.bg(theme::panel_hi()))
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
                                .gap(px(8.))
                                .min_w_0()
                                // Names are short: let a long one wrap
                                // rather than clamp an inline item.
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .line_height(px(20.))
                                        .text_color(if enabled { theme::text() } else { theme::text_muted() })
                                        .child(m.name.clone()),
                                )
                                .when_some(m.version.clone(), |d, v| d.child(ui::caption(format!("v{v}")).flex_none()))
                                .child(ui::badge(m.kind.label(), kind_color)),
                        )
                        .child(caption_style(clipped_with_tip(format!("mod-desc-{i}"), description, description_tip))),
                )
                .child(
                    ui::toggle(SharedString::from(format!("mod-{i}")), enabled)
                        .tooltip(ui::tip(if enabled { "Disable (moves it aside, nothing is deleted)" } else { "Enable" }))
                        .on_click(move |_, _, cx| toggle_ws.update(cx, |ws, cx| ws.set_mod_enabled(toggle_path.clone(), !enabled, cx))),
                )
                .child({
                    let (ws, path, name) = (menu_ws.clone(), menu_path.clone(), menu_name.clone());
                    Button::new(SharedString::from(format!("mod-more-{i}")))
                        .ghost()
                        .w(px(32.))
                        .h(px(32.))
                        .flex_none()
                        .tooltip("More options")
                        .child(ui::icon(Icon::More).text_color(theme::text()))
                        .dropdown_menu(move |menu, _, _| mod_menu(menu, &ws, &path, &name, enabled))
                })
                .context_menu(move |menu, _, _| mod_menu(menu, &menu_ws, &menu_path, &menu_name, enabled)),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(page_header("Mods", "The community mod loader and your installed mods. Disabled mods are moved aside, never deleted.", vec![open_folders, add]))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(ui::section_title("Mod loader", None))
                .child(sdk)
                .children(legacy)
                .child(links),
        )
        .child(div().flex().flex_col().gap(px(8.)).child(ui::section_title("Installed mods", None)).child(list))
        .into_any_element()
}

/// Enable/disable, show in folder, delete: shared by the ⋮ button and right-click.
fn mod_menu(menu: PopupMenu, ws: &Entity<Workspace>, path: &std::path::Path, name: &str, enabled: bool) -> PopupMenu {
    let (ws1, ws2) = (ws.clone(), ws.clone());
    let (p1, p2, p3) = (path.to_path_buf(), path.to_path_buf(), path.to_path_buf());
    let name = name.to_string();
    menu.item(
        PopupMenuItem::new(if enabled { "Disable" } else { "Enable" })
            .on_click(move |_, _, cx| ws1.update(cx, |ws, cx| ws.set_mod_enabled(p1.clone(), !enabled, cx))),
    )
    .item(PopupMenuItem::new("Show in folder").on_click(move |_, _, cx| open_folder(&p2, cx)))
    .separator()
    .item(PopupMenuItem::new("Delete…").on_click(move |_, window, cx| {
        let ws = ws2.clone();
        let path = p3.clone();
        confirm(window, cx, &format!("Delete {name}?"), "This permanently removes the mod's files.", "Delete", move |cx| {
            ws.update(cx, |ws, cx| ws.remove_mod(path, cx))
        });
    }))
}
