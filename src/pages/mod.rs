//! Page renderers. Each page is a module with a `render` function that reads
//! the shared `Workspace` and wires its controls back to workspace methods.
//! To add a page: add a `PageKind` variant, a module here, and a match arm in
//! `render`; then list it in a game's `nav`.

mod backups;
pub(crate) mod compare;
mod launch;
mod mods;
mod overview;
mod patches;
mod presets;
mod quick;
mod settings;
mod setup;
mod tweaks;

use std::path::PathBuf;

use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, PathPromptOptions, PromptLevel, Styled,
    Window, div, prelude::*, px,
};

use crate::games::{NavItem, PageKind};
use crate::theme::{self, Icon};
use crate::ui;
use crate::workspace::Workspace;

pub fn render(nav: &NavItem, ws: &Entity<Workspace>, window: &mut Window, cx: &mut App) -> AnyElement {
    match nav.kind {
        PageKind::Overview => overview::render(ws, window, cx),
        PageKind::Tweaks(_) => tweaks::render(nav, ws, window, cx),
        PageKind::Presets => presets::render(ws, window, cx),
        PageKind::Patches => patches::render(ws, window, cx),
        PageKind::Mods => mods::render(ws, window, cx),
        PageKind::Launch => launch::render(ws, window, cx),
        PageKind::Backups => backups::render(ws, window, cx),
        PageKind::Settings => settings::render(ws, window, cx),
        PageKind::Setup => setup::render(ws, window, cx),
        PageKind::Quick => quick::render(ws, window, cx),
        PageKind::Capture => compare::capture_page(ws, window, cx),
    }
}

/// Standard page header: big title, subtitle, and optional right-side actions.
pub(crate) fn page_header(title: &str, subtitle: &str, actions: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .items_end()
        .gap(px(16.))
        .pb(px(14.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(ui::display(title.to_string(), 40.))
                .child(ui::body(subtitle.to_string()).max_w(px(720.))),
        )
        .child(div().flex().gap(px(10.)).children(actions))
}

/// Shown instead of page content when the game or its config can't be found.
pub(crate) fn missing_notice(title: &str, detail: &str, ws: &Entity<Workspace>) -> AnyElement {
    let ws = ws.clone();
    ui::card(theme::danger())
        .child(
            ui::card_body()
                .p(px(18.))
                .flex()
                .items_center()
                .gap(px(16.))
                .child(ui::icon(Icon::Warning).text_size(px(28.)).text_color(theme::danger()))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .font_family(theme::FONT_LABEL)
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_size(px(16.))
                                .text_color(theme::text())
                                .child(title.to_string()),
                        )
                        .child(ui::body(detail.to_string())),
                )
                .child(
                    ui::button("missing-browse", "Locate game", Some(Icon::Folder), ui::Variant::Primary)
                        .on_click(move |_, _, cx| {
                            let ws = ws.clone();
                            pick_paths(false, true, false, cx, move |paths, cx| {
                                if let Some(p) = paths.into_iter().next() {
                                    ws.update(cx, |ws, cx| ws.set_manual_install(p, cx));
                                }
                            });
                        }),
                ),
        )
        .into_any_element()
}

pub(crate) fn pick_paths(
    files: bool,
    directories: bool,
    multiple: bool,
    cx: &mut App,
    then: impl FnOnce(Vec<PathBuf>, &mut App) + 'static,
) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files,
        directories,
        multiple,
        prompt: None,
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = rx.await {
            cx.update(|cx| then(paths, cx)).ok();
        }
    })
    .detach();
}

/// Native confirmation dialog; `then` runs only if the user confirms.
pub(crate) fn confirm(
    window: &mut Window,
    cx: &mut App,
    message: &str,
    detail: &str,
    confirm_label: &'static str,
    then: impl FnOnce(&mut App) + 'static,
) {
    let rx = window.prompt(
        PromptLevel::Warning,
        message,
        Some(detail),
        &[confirm_label, "Cancel"],
        cx,
    );
    cx.spawn(async move |cx| {
        if let Ok(0) = rx.await {
            cx.update(|cx| then(cx)).ok();
        }
    })
    .detach();
}

pub(crate) fn open_folder(path: &std::path::Path, cx: &mut App) {
    if path.is_dir() {
        cx.open_with_system(path);
    } else if let Some(parent) = path.parent() {
        cx.open_with_system(parent);
    }
}
