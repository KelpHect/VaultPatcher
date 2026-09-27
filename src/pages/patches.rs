//! Executable patches plus config-file locking.

use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
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
        "Exe patches",
        "Signature-based patches for the game executable. Each is verified against the exe before it's offered, and the exe is backed up before every change.",
        vec![],
    ));

    let sdk_installed = matches!(game.sdk, SdkStatus::Installed(_) | SdkStatus::Detected);
    if sdk_installed {
        page = page.child(ui::info_bar(
            ui::Severity::Info,
            "The Python SDK is installed",
            "It applies the console and array-limit edits in memory at startup, so you don't need the hex edits below.",
        ));
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
            let mut list = ui::panel().flex().flex_col();
            for (pi, patch) in def.patches.iter().enumerate() {
                let st = exe.patch_states.get(patch.id).copied().unwrap_or(PatchState::Unsupported);
                let (status, color) = match st {
                    PatchState::Patched => ("Active", theme::success()),
                    PatchState::Unpatched => ("Not applied", theme::text_muted()),
                    PatchState::Unsupported => ("Unsupported exe build", theme::danger()),
                };
                let action = match (st, patch.revertible) {
                    (PatchState::Unpatched, _) => {
                        let ws = ws.clone();
                        let id = patch.id;
                        Some(
                            ui::button(
                                SharedString::from(format!("patch-{id}")),
                                "Apply",
                                Some(Icon::Wrench),
                                if sdk_installed && !matches!(patch.kind, crate::patches::ExePatchKind::LargeAddressAware) {
                                    Variant::Secondary
                                } else {
                                    Variant::Primary
                                },
                            )
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
                list = list.when(pi > 0, |d| d.child(ui::divider())).child(
                    div()
                        .min_h(px(68.))
                        .px(px(16.))
                        .py(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(16.))
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
                                        .flex_wrap()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(ui::title(quote_code(patch.name)))
                                        .child(ui::badge(status, color)),
                                )
                                .child(ui::body(quote_code(patch.description)))
                                .when_some(patch.note, |d, n| d.child(ui::caption(quote_code(n)))),
                        )
                        .children(action),
                );
            }
            if def.patches.is_empty() {
                list = list.child(div().p(px(16.)).child(ui::body("No executable patches are needed for this game.")));
            }
            page = page.child(list);
        }
    }

    // Config locking: a SettingsCard with the lock state as its icon.
    let locked = state.configs_locked();
    let lock_ws = ws.clone();
    page = page.child(
        div().flex().flex_col().gap(px(8.)).child(ui::section_title("Settings files", None)).child(
            ui::panel()
                .min_h(px(68.))
                .px(px(16.))
                .py(px(12.))
                .flex()
                .items_center()
                .gap(px(16.))
                .child(ui::icon(if locked { Icon::Lock } else { Icon::Unlock }).text_color(if locked { theme::accent_text() } else { theme::text_muted() }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(ui::title("Lock settings files"))
                        .child(ui::caption(
                            "Marks the ini files read-only so the game, its launcher, or a co-op host can't revert your tweaks. The in-game options menu then shows \"Failed to save your settings\", so unlock before changing settings in-game.",
                        )),
                )
                .child(
                    ui::toggle("lock-configs", locked)
                        .tooltip(ui::tip(if locked { "The ini files are read-only now" } else { "The ini files are writable now" }))
                        .on_click(move |_, _, cx| lock_ws.update(cx, |ws, cx| ws.set_config_lock(!locked, cx))),
                ),
        ),
    );

    page.into_any_element()
}

/// Shows `code` spans in patch copy as quoted words ("the “set” command"):
/// backticks would render literally.
fn quote_code(text: &str) -> String {
    let mut open = false;
    text.chars()
        .map(|c| match c {
            '`' => {
                open = !open;
                if open { '\u{201C}' } else { '\u{201D}' }
            }
            c => c,
        })
        .collect()
}
