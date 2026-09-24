//! Mod manager page: SDK install/uninstall and the installed mod list.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::{confirm, missing_notice, open_folder, page_header, pick_paths};
use crate::mods::{ModKind, SdkStatus};
use crate::theme::{self, Icon, Rarity};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let Some(support) = def.mods else {
        return div()
            .child(page_header("Mods", "Mod support for this game is coming soon.", vec![]))
            .into_any_element();
    };

    let mut page = div().flex().flex_col().gap(px(22.)).child(page_header(
        "Mods",
        "Install the community Python SDK, then drop in SDK mods (.sdkmod / .zip) and text mods (.blcm / .txt).",
        vec![],
    ));

    let Some(install) = game.install.clone() else {
        return page
            .child(missing_notice(
                "Game install not found",
                "Mods are installed into the game folder, so Vault Patcher needs to know where it is.",
                ws,
            ))
            .into_any_element();
    };

    // ---- SDK card
    let (status_text, status_color) = match &game.sdk {
        SdkStatus::Installed(v) => (format!("Installed · {v}"), theme::success()),
        SdkStatus::Detected => ("Installed (manually)".to_string(), theme::success()),
        SdkStatus::Legacy => ("Legacy PythonSDK found".to_string(), Rarity::Legendary.color()),
        SdkStatus::NotInstalled => ("Not installed".to_string(), theme::text_dim()),
    };
    let busy = state.busy.clone();
    let install_ws = ws.clone();
    let zip_ws = ws.clone();
    let uninstall_ws = ws.clone();
    let installed = matches!(game.sdk, SdkStatus::Installed(_) | SdkStatus::Detected);

    let mut sdk_actions = div().flex().flex_wrap().gap(px(10.)).child(
        ui::button(
            "sdk-install",
            if installed { "Update to latest" } else { "Install latest" },
            Some(Icon::Download),
            Variant::Primary,
        )
        .when(busy.is_some(), |d| d.opacity(0.5))
        .on_click(move |_, _, cx| {
            install_ws.update(cx, |ws, cx| {
                if ws.busy.is_none() {
                    ws.install_sdk(cx)
                }
            })
        }),
    );
    sdk_actions = sdk_actions.child(
        ui::button("sdk-zip", "Install from zip…", Some(Icon::Package), Variant::Secondary).on_click(move |_, _, cx| {
            let ws = zip_ws.clone();
            pick_paths(true, false, false, cx, move |paths, cx| {
                if let Some(p) = paths.into_iter().next() {
                    ws.update(cx, |ws, cx| ws.install_sdk_from_zip(p, cx));
                }
            });
        }),
    );
    if matches!(game.sdk, SdkStatus::Installed(_)) {
        sdk_actions = sdk_actions.child(
            ui::button("sdk-remove", "Uninstall", Some(Icon::Delete), Variant::Ghost).on_click(move |_, window, cx| {
                let ws = uninstall_ws.clone();
                confirm(
                    window,
                    cx,
                    "Uninstall the SDK?",
                    "Removes the files Vault Patcher installed. Your mods and their settings stay in sdk_mods.",
                    "Uninstall",
                    move |cx| ws.update(cx, |ws, cx| ws.uninstall_sdk(cx)),
                );
            }),
        );
    }
    let redist = support.redist_url;
    sdk_actions = sdk_actions.child(
        ui::button("sdk-redist", "VC++ runtime", Some(Icon::Link), Variant::Ghost)
            .on_click(move |_, _, cx| cx.open_url(redist)),
    );

    let mut links = div().flex().flex_wrap().gap(px(8.));
    for (i, (text, url)) in support.links.iter().enumerate() {
        let url = *url;
        links = links.child(
            ui::chip(SharedString::from(format!("link-{i}")), *text, false).on_click(move |_, _, cx| cx.open_url(url)),
        );
    }

    page = page.child(
        ui::card(status_color).child(
            ui::card_body()
                
                .p(px(18.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(ui::display(support.sdk_name, 28.))
                        .child(ui::badge(status_text, status_color)),
                )
                .child(ui::body(
                    "The SDK adds a MODS menu to the main menu, applies the classic console hex edits in memory, and runs Python mods. It needs the Visual C++ runtime. For text mods, also add the Text Mod Loader SDK mod, or use OpenBLCMM.",
                ))
                .when(matches!(game.sdk, SdkStatus::Legacy), |d| {
                    d.child(
                        div().text_size(px(12.5)).text_color(Rarity::Legendary.color()).child(
                            "The old PythonSDK (Mods folder) is installed. Installing the new SDK migrates compatible mods automatically.",
                        ),
                    )
                })
                .when_some(busy, |d, b| {
                    d.child(div().text_size(px(12.5)).text_color(theme::accent()).child(b))
                })
                .child(sdk_actions)
                .child(links),
        ),
    );

    // ---- Mod list
    let add_ws = ws.clone();
    let sdk_dir = install.root.join(support.sdk_mods_dir);
    let text_dir = install.root.join(support.text_mods_dir);
    let header = div()
        .flex()
        .items_end()
        .gap(px(10.))
        .child(div().flex_1().child(ui::section_title(
            "Installed mods",
            Some("Disabled mods are moved aside, not deleted. Core SDK modules are locked.".into()),
        )))
        .child(
            ui::button("mods-open-sdk", "sdk_mods", Some(Icon::Folder), Variant::Ghost)
                .on_click(move |_, _, cx| open_folder(&sdk_dir, cx)),
        )
        .child(
            ui::button("mods-open-text", "Binaries", Some(Icon::Folder), Variant::Ghost)
                .on_click(move |_, _, cx| open_folder(&text_dir, cx)),
        )
        .child(
            ui::button("mods-add", "Add mods…", Some(Icon::Add), Variant::Primary).on_click(move |_, _, cx| {
                let ws = add_ws.clone();
                pick_paths(true, false, true, cx, move |paths, cx| {
                    ws.update(cx, |ws, cx| ws.install_mod_files(paths, cx));
                });
            }),
        );

    let mut list = ui::panel().flex().flex_col();
    if game.mods.is_empty() {
        list = list.child(div().p(px(20.)).child(ui::body(
            "No mods yet. Use Add mods… to install .sdkmod, .zip, .blcm or .txt files.",
        )));
    }
    for (i, m) in game.mods.iter().enumerate() {
        if i > 0 {
            list = list.child(div().h(px(1.)).bg(theme::line()));
        }
        let kind_color = match m.kind {
            ModKind::SdkPackage | ModKind::SdkFolder => Rarity::Rare.color(),
            ModKind::TextMod => Rarity::Uncommon.color(),
        };
        let toggle_ws = ws.clone();
        let remove_ws = ws.clone();
        let enabled = m.enabled;
        let name = m.name.clone();
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(14.))
            .px(px(16.))
            .py(px(12.))
            .when(!m.enabled, |d| d.opacity(0.6))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .font_family(theme::FONT_LABEL)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(15.))
                                    .text_color(theme::text())
                                    .child(m.name.clone()),
                            )
                            .when_some(m.version.clone(), |d, v| {
                                d.child(div().text_size(px(12.)).text_color(theme::text_dim()).child(format!("v{v}")))
                            })
                            .child(ui::badge(m.kind.label(), kind_color))
                            .when(m.core, |d| d.child(ui::badge("Core", Rarity::Pearlescent.color()))),
                    )
                    .when_some(m.description.clone(), |d, desc| d.child(ui::body(desc)))
                    .child(
                        div()
                            .font_family(theme::FONT_MONO)
                            .text_size(px(11.))
                            .text_color(theme::text_dim())
                            .child(m.path.display().to_string()),
                    ),
            );
        if !m.core {
            row = row
                .child(
                    ui::toggle(SharedString::from(format!("mod-{i}")), enabled)
                        .on_click(move |_, _, cx| toggle_ws.update(cx, |ws, cx| ws.set_mod_enabled(i, !enabled, cx))),
                )
                .child(
                    ui::icon_button(SharedString::from(format!("mod-del-{i}")), Icon::Delete, theme::danger()).on_click(
                        move |_, window, cx| {
                            let ws = remove_ws.clone();
                            confirm(
                                window,
                                cx,
                                &format!("Delete {name}?"),
                                "This permanently removes the mod's files.",
                                "Delete",
                                move |cx| ws.update(cx, |ws, cx| ws.remove_mod(i, cx)),
                            );
                        },
                    ),
                );
        }
        list = list.child(row);
    }

    page = page.child(div().flex().flex_col().gap(px(10.)).child(header).child(list));
    page.into_any_element()
}
