//! Snapshot browser: restore or delete backups.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};

use super::{confirm, open_folder, page_header};
use crate::core::backup;
use crate::theme::{self, Icon};
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

pub fn render(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();

    let snap_ws = ws.clone();
    let folder = backup::data_dir().join("backups").join(game.def.id);
    let actions = vec![
        ui::button("bk-open", "Open folder", Some(Icon::Folder), Variant::Secondary)
            .on_click(move |_, _, cx| {
                std::fs::create_dir_all(&folder).ok();
                open_folder(&folder, cx)
            })
            .into_any_element(),
        ui::button("bk-snap", "Snapshot configs", Some(Icon::Save), Variant::Primary)
            .on_click(move |_, _, cx| snap_ws.update(cx, |ws, cx| ws.backup_configs_now(cx)))
            .into_any_element(),
    ];

    let mut list = ui::panel().flex().flex_col();
    if game.backups.is_empty() {
        list = list.child(div().p(px(20.)).child(ui::body(
            "No snapshots yet. One is created automatically before every apply, patch and SDK install.",
        )));
    }
    for (i, b) in game.backups.iter().enumerate() {
        if i > 0 {
            list = list.child(div().h(px(1.)).bg(theme::line()));
        }
        let restore_ws = ws.clone();
        let delete_ws = ws.clone();
        let label = b.label.clone();
        let files = b
            .files
            .iter()
            .map(|f| {
                let name = f.original.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if f.created { format!("{name} (new)") } else { name }
            })
            .collect::<Vec<_>>()
            .join(", ");
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .px(px(16.))
                .py(px(12.))
                .child(ui::icon(Icon::History).text_color(theme::echo()).text_size(px(18.)))
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
                                .items_center()
                                .child(
                                    div()
                                        .font_family(theme::FONT_LABEL)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(15.))
                                        .text_color(theme::text())
                                        .child(b.label.clone()),
                                )
                                .child(div().text_size(px(12.)).text_color(theme::text_dim()).child(b.created_at.clone())),
                        )
                        .child(
                            div()
                                .font_family(theme::FONT_MONO)
                                .text_size(px(11.5))
                                .text_color(theme::text_muted())
                                .child(files),
                        ),
                )
                .child(
                    ui::button(SharedString::from(format!("bk-restore-{i}")), "Restore", Some(Icon::Undo), Variant::Secondary)
                        .on_click(move |_, window, cx| {
                            let ws = restore_ws.clone();
                            confirm(
                                window,
                                cx,
                                &format!("Restore \"{label}\"?"),
                                "Overwrites the current files with this snapshot. Take a snapshot first if you want to keep the current state.",
                                "Restore",
                                move |cx| ws.update(cx, |ws, cx| ws.restore_backup(i, cx)),
                            );
                        }),
                )
                .child(
                    ui::icon_button(SharedString::from(format!("bk-del-{i}")), Icon::Delete, theme::danger())
                        .on_click(move |_, _, cx| delete_ws.update(cx, |ws, cx| ws.delete_backup(i, cx))),
                ),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(page_header(
            "Backups",
            &format!(
                "Snapshots of {}'s files taken before every change. The newest {} config snapshots are kept; exe and SDK snapshots are never pruned.",
                game.def.name,
                state.settings.max_backups.unwrap_or(30)
            ),
            actions,
        ))
        .child(list)
        .into_any_element()
}
