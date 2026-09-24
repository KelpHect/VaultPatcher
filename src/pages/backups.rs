//! Backup history: restore or delete snapshots. Named "History" in Simple
//! mode, where it acts as the app's undo.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::{confirm, open_folder, page_header};
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
    let mut ordered: Vec<&Backup> = game.backups.iter().filter(|b| b.label == backup::ORIGINAL_LABEL).collect();
    ordered.extend(game.backups.iter().filter(|b| b.label != backup::ORIGINAL_LABEL));
    let mut groups: Vec<(&Backup, usize)> = Vec::new();
    for b in ordered {
        match groups.last_mut() {
            Some((first, n)) if first.label == b.label && b.label != backup::ORIGINAL_LABEL => *n += 1,
            _ => groups.push((b, 1)),
        }
    }

    let mut list = ui::panel().flex().flex_col();
    if groups.is_empty() {
        list = list.child(div().p(px(20.)).child(ui::body(
            "Nothing here yet. Vault Patcher saves your files automatically before every change it makes.",
        )));
    }
    for (i, (b, repeats)) in groups.into_iter().enumerate() {
        if i > 0 {
            list = list.child(div().h(px(1.)).bg(theme::line()));
        }
        let original = b.label == backup::ORIGINAL_LABEL;
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
                .gap(px(14.))
                .px(px(18.))
                .py(px(13.))
                .child(
                    ui::icon(if original { Icon::Star } else { Icon::History })
                        .text_color(if original { theme::accent() } else { theme::text_dim() })
                        .text_size(px(17.)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .flex()
                                .gap(px(10.))
                                .items_baseline()
                                .child(
                                    div()
                                        .font_family(theme::FONT_LABEL)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(16.))
                                        .text_color(theme::text())
                                        .child(b.label.clone()),
                                )
                                .child(div().text_size(px(13.)).text_color(theme::text_dim()).child(b.created_at.clone()))
                                .when(repeats > 1, |d| {
                                    d.child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(theme::text_dim())
                                            .child(format!("· newest of {repeats}")),
                                    )
                                }),
                        )
                        .child(div().text_size(px(13.)).text_color(theme::text_muted()).child(file_list(b))),
                )
                .child(
                    ui::button(SharedString::from(format!("bk-restore-{i}")), "Restore", Some(Icon::Undo), Variant::Secondary)
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
                .when(!original, |d| {
                    d.child(
                        ui::icon_button(SharedString::from(format!("bk-del-{i}")), Icon::Delete, theme::text_dim())
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
                            }),
                    )
                }),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(page_header(
            if simple { "History" } else { "Backups" },
            "Every change Vault Patcher makes is backed up first. Your original settings are kept forever; restoring anything backs up the current files first.",
            actions,
        ))
        .child(list)
        .into_any_element()
}
