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
pub(crate) mod running;
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
        PageKind::Backups => backups::render(ws, window, cx),
        PageKind::Settings => settings::render(ws, window, cx),
        PageKind::Setup => setup::render(ws, window, cx),
        PageKind::Quick => quick::render(ws, window, cx),
        PageKind::Capture => compare::capture_page(ws, window, cx),
    }
}

/// Pages that lay out their own scrolling panes (list + detail).
pub fn fills_height(kind: PageKind) -> bool {
    matches!(kind, PageKind::Tweaks(_) | PageKind::Quick)
}

/// Standard page header: title, one-line subtitle, and right-side actions.
pub(crate) fn page_header(title: &str, subtitle: &str, actions: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .items_end()
        .gap(px(12.))
        .pb(px(4.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(ui::display(title.to_string(), 28.))
                .child(ui::body(subtitle.to_string()).max_w(px(760.))),
        )
        .child(div().flex().gap(px(8.)).children(actions))
}

/// Shown when the game or its config can't be found.
pub(crate) fn missing_notice(title: &str, detail: &str, ws: &Entity<Workspace>) -> AnyElement {
    let ws = ws.clone();
    ui::card(theme::danger())
        .child(
            ui::card_body()
                .p(px(14.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(ui::icon(Icon::Alert).size(px(20.)).text_color(theme::danger()))
                .child(div().flex_1().min_w_0().flex().flex_col().child(ui::title(title.to_string())).child(ui::body(detail.to_string())))
                .child(
                    ui::button("missing-browse", "Locate game", Some(Icon::Folder), ui::Variant::Secondary).on_click(move |_, _, cx| {
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

/// The game's Steam banner with its logo (or a plain title when the art
/// isn't on this PC), status line, and actions on the right.
pub(crate) fn game_banner(ws: &Workspace, caption: String, actions: Vec<AnyElement>) -> AnyElement {
    use gpui::{ObjectFit, StyledImage, img, linear_color_stop, linear_gradient};
    let game = ws.game();
    let def = game.def;
    let art = &game.art;
    let height = 188.;
    let title: AnyElement = match &art.logo {
        Some(logo) => img(logo.clone()).h(px(92.)).max_w(px(340.)).object_fit(ObjectFit::Contain).into_any_element(),
        None => ui::display(def.name, 40.).into_any_element(),
    };
    div()
        .relative()
        .h(px(height))
        .w_full()
        .rounded(px(theme::RADIUS_LG))
        .overflow_hidden()
        .border_1()
        .border_color(theme::line())
        .bg(theme::panel())
        .when_some(art.hero.clone(), |d, hero| d.child(img(hero).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)))
        // Darken the left and bottom so the logo and text stay readable.
        .child(
            div()
                .absolute()
                .inset_0()
                .bg(linear_gradient(90., linear_color_stop(theme::with_alpha(theme::bg_deep(), 0.92), 0.), linear_color_stop(theme::with_alpha(theme::bg_deep(), 0.15), 0.75))),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .bg(linear_gradient(180., linear_color_stop(theme::with_alpha(theme::bg_deep(), 0.), 0.45), linear_color_stop(theme::with_alpha(theme::bg_deep(), 0.85), 1.))),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .p(px(20.))
                .flex()
                .items_end()
                .gap(px(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(div().flex().child(title))
                        .child(div().text_size(px(12.)).text_color(theme::text_muted()).truncate().child(caption)),
                )
                .child(div().flex().gap(px(8.)).children(actions)),
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
