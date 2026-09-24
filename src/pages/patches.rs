//! Executable patches plus config-file locking.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::{confirm, missing_notice, page_header};
use crate::core::binpatch::PatchState;
use crate::mods::SdkStatus;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;

    let mut page = div().flex().flex_col().gap(px(20.)).child(page_header(
        "Exe Patches",
        "Signature-based patches for the game executable. Each is verified against the exe before it's offered, and the exe is backed up before every change.",
        vec![],
    ));

    let sdk_installed = matches!(game.sdk, SdkStatus::Installed(_) | SdkStatus::Detected);
    if sdk_installed {
        page = page.child(
            ui::card(theme::echo()).child(
                ui::card_body().p(px(14.)).child(ui::body(
                    "The Python SDK is installed: it applies the console and array-limit edits in memory at startup, so you don't need the hex edits below.",
                )),
            ),
        );
    }

    match &game.exe {
        None => {
            page = page.child(missing_notice(
                "Executable not found",
                &format!("Patches need the game's install folder ({}).", def.exe),
                ws,
            ));
        }
        Some(exe) => {
            let mut list = div().flex().flex_col().gap(px(14.));
            for patch in def.patches {
                let st = exe.patch_states.get(patch.id).copied().unwrap_or(PatchState::Unsupported);
                let (status, color) = match st {
                    PatchState::Patched => ("Active", theme::success()),
                    PatchState::Unpatched => ("Not applied", theme::text_dim()),
                    PatchState::Unsupported => ("Unsupported exe build", theme::danger()),
                };
                let action = match (st, patch.revertible) {
                    (PatchState::Unpatched, _) => {
                        let ws = ws.clone();
                        let id = patch.id;
                        Some(
                            ui::button(SharedString::from(format!("patch-{id}")), "Apply", Some(Icon::Wrench), Variant::Primary)
                                .on_click(move |_, window, cx| {
                                    let ws = ws.clone();
                                    confirm(
                                        window,
                                        cx,
                                        "Patch the game executable?",
                                        "Close the game first. The current exe is backed up and can be restored from Backups. Steam's \"Verify integrity\" also undoes patches.",
                                        "Patch",
                                        move |cx| ws.update(cx, |ws, cx| ws.set_exe_patch(id, true, cx)),
                                    );
                                })
                                .into_any_element(),
                        )
                    }
                    (PatchState::Patched, true) => {
                        let ws = ws.clone();
                        let id = patch.id;
                        Some(
                            ui::button(SharedString::from(format!("patch-{id}")), "Revert", Some(Icon::Undo), Variant::Secondary)
                                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.set_exe_patch(id, false, cx)))
                                .into_any_element(),
                        )
                    }
                    _ => None,
                };
                list = list.child(
                    ui::card(patch.rarity.color()).child(
                        ui::card_body()
                            
                            .p(px(16.))
                            .flex()
                            .items_center()
                            .gap(px(18.))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.))
                                            .child(
                                                div()
                                                    .font_family(theme::FONT_LABEL)
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(16.))
                                                    .text_color(theme::text())
                                                    .child(patch.name),
                                            )
                                            .child(ui::badge(status, color)),
                                    )
                                    .child(ui::body(patch.description))
                                    .when_some(patch.note, |d, n| {
                                        d.child(div().text_size(px(12.)).text_color(theme::text_dim()).child(n))
                                    }),
                            )
                            .children(action),
                    ),
                );
            }
            if def.patches.is_empty() {
                list = list.child(ui::body("No executable patches are needed for this game."));
            }
            page = page.child(list);
        }
    }

    // Config locking
    let locked = state.configs_locked();
    let lock_ws = ws.clone();
    page = page.child(
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::section_title(
                "Config lock",
                Some("Marks the ini files read-only so the game, its launcher, or a co-op host can't revert your tweaks. The in-game options menu then shows \"Failed to save your settings\" — unlock before changing settings in-game.".into()),
            ))
            .child(
                ui::panel().p(px(16.)).flex().items_center().gap(px(14.))
                    .child(ui::icon(if locked { Icon::Lock } else { Icon::Unlock }).text_size(px(22.)).text_color(if locked { theme::accent() } else { theme::text_dim() }))
                    .child(div().flex_1().child(ui::body(if locked { "Config files are locked (read-only)." } else { "Config files are writable." })))
                    .child(
                        ui::toggle("lock-configs", locked)
                            .on_click(move |_, _, cx| lock_ws.update(cx, |ws, cx| ws.set_config_lock(!locked, cx))),
                    ),
            ),
    );

    page.into_any_element()
}
