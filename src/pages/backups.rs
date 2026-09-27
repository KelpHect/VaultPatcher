//! Backup history: restore or delete snapshots. In Simple mode it doubles as
//! the app's undo.

use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, px};

use super::{caption_style, clipped, confirm, friendly_now, open_folder, page_header};
use crate::core::backup::{self, Backup};
use crate::games::Mode;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

/// "WillowEngine.ini, WillowEngine.ini (launcher copy), …"
fn file_list(b: &Backup) -> String {
    b.files
        .iter()
        .map(|f| {
            let name = f.original.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let launcher = f
                .original
                .parent()
                .and_then(|p| p.file_name())
                .is_some_and(|d| d.eq_ignore_ascii_case("LauncherConfig"));
            match (launcher, f.created) {
                (true, _) => format!("{name} (launcher copy)"),
                (false, true) => format!("{name} (new)"),
                _ => name,
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let simple = state.mode() == Mode::Simple;

    let snap_ws = ws.clone();
    let folder = backup::data_dir().join("backups").join(game.def.id);
    let actions = vec![
        ui::button("bk-open", "Open folder", Some(Icon::Folder), Variant::Secondary)
            .on_click(move |_, _, cx| {
                std::fs::create_dir_all(&folder).ok();
                open_folder(&folder, cx)
            })
            .into_any_element(),
        ui::button("bk-snap", "Back up now", Some(Icon::Save), Variant::Secondary)
            .on_click(move |_, _, cx| snap_ws.update(cx, |ws, cx| ws.backup_configs_now(cx)))
            .into_any_element(),
    ];

    // Original settings pinned first, then newest-first with consecutive
    // repeats of the same action collapsed into one row.
    let mut ordered: Vec<&Backup> = game.backups.iter().filter(|b| backup::is_original(b)).collect();
    ordered.extend(game.backups.iter().filter(|b| !backup::is_original(b)));
    let mut groups: Vec<(&Backup, usize)> = Vec::new();
    for b in ordered {
        match groups.last_mut() {
            Some((first, n)) if first.label == b.label && !backup::is_original(b) => *n += 1,
            _ => groups.push((b, 1)),
        }
    }

    let mut list = ui::panel().flex().flex_col();
    if groups.is_empty() {
        list = list.child(div().px(px(16.)).py(px(20.)).child(ui::body(
            "Nothing here yet. Vaulter saves your files automatically before every change it makes.",
        )));
    }
    for (i, (b, repeats)) in groups.into_iter().enumerate() {
        if i > 0 {
            list = list.child(ui::divider());
        }
        let original = backup::is_original(b);
        let restore_ws = ws.clone();
        let delete_ws = ws.clone();
        let label = b.label.clone();
        let restore_dir = b.dir.clone();
        let delete_dir = b.dir.clone();
        let delete_label = b.label.clone();
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .min_h(px(56.))
                .px(px(16.))
                .py(px(8.))
                .child(
                    ui::icon(if original { Icon::Star } else { Icon::History })
                        .text_color(if original { theme::accent_text() } else { theme::text_muted() }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .items_baseline()
                                .min_w_0()
                                // Labels are short; a long one wraps.
                                .child(div().min_w_0().child(ui::title(b.label.clone())))
                                .child(
                                    caption_style(div().id(SharedString::from(format!("bk-time-{i}"))).flex_none())
                                        .tooltip(ui::tip(b.created_at.clone()))
                                        .child(friendly_now(&b.created_at)),
                                )
                                .when(repeats > 1, |d| {
                                    d.child(
                                        caption_style(div().id(SharedString::from(format!("bk-similar-{i}"))).flex_none())
                                            .tooltip(ui::tip("Older backups of the same action are grouped here; this row restores the newest"))
                                            .child(format!("({repeats} similar)")),
                                    )
                                }),
                        )
                        .child(caption_style(clipped(format!("bk-files-{i}"), file_list(b)))),
                )
                // A quiet (subtle) button: a page of equal-weight Standard
                // buttons read as a wall of calls to action.
                .child(
                    ui::button(SharedString::from(format!("bk-restore-{i}")), "Restore", Some(Icon::Undo), Variant::Ghost)
                        .tooltip(ui::tip("Put these files back (the current ones are backed up first)"))
                        .on_click(move |_, window, cx| {
                            let ws = restore_ws.clone();
                            let dir = restore_dir.clone();
                            confirm(
                                window,
                                cx,
                                &format!("Restore \"{label}\"?"),
                                "Your current files are backed up first, so you can undo this from here.",
                                "Restore",
                                move |cx| ws.update(cx, |ws, cx| ws.restore_backup(dir, cx)),
                            );
                        }),
                )
                .child(if original {
                    ui::icon_button_if(false, SharedString::from(format!("bk-del-{i}")), Icon::Delete, theme::text_muted())
                        .tooltip(ui::tip("Your original settings are always kept"))
                        .into_any_element()
                } else {
                    ui::icon_button(SharedString::from(format!("bk-del-{i}")), Icon::Delete, theme::text_muted())
                        .tooltip(ui::tip("Delete this backup"))
                        .on_click(move |_, window, cx| {
                            let ws = delete_ws.clone();
                            let dir = delete_dir.clone();
                            confirm(
                                window,
                                cx,
                                &format!("Delete the backup \"{delete_label}\"?"),
                                "This can't be undone.",
                                "Delete",
                                move |cx| ws.update(cx, |ws, cx| ws.delete_backup(dir, cx)),
                            );
                        })
                        .into_any_element()
                }),
        );
    }

    // The page is titled like its nav item in both modes; Simple mode's
    // subtitle says it is the undo history.
    div()
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(page_header(
            "Backups",
            if simple {
                "Your undo history. Every change Vaulter makes is backed up first, and your original settings are kept forever. Restoring anything backs up the current files first."
            } else {
                "Every change Vaulter makes is backed up first. Your original settings are kept forever; restoring anything backs up the current files first."
            },
            actions,
        ))
        .child(list)
        .into_any_element()
}
