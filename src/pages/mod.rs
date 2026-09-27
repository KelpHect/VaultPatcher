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
pub(crate) mod tweaks;

use std::path::PathBuf;

use gpui::{
    AnyElement, App, Div, ElementId, Entity, FontWeight, IntoElement, ParentElement, PathPromptOptions, PromptLevel,
    SharedString, Stateful, Styled, Window, div, prelude::*, px,
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
    header(ui::display(title.to_string(), 28.).into_any_element(), subtitle, actions)
}

/// Page header whose title is a BreadcrumbBar: the parent page (a link back
/// to it) › this page.
pub(crate) fn breadcrumb_header(parent: &'static str, on_parent: impl Fn(&mut App) + 'static, title: &str, subtitle: &str) -> impl IntoElement {
    let crumbs = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            ui::focusable(div().id("breadcrumb-parent"))
                .rounded(px(theme::RADIUS))
                .font_family(theme::font_display())
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(28.))
                .line_height(px(36.))
                .text_color(theme::text_muted())
                .cursor_pointer()
                .hover(|s| s.text_color(theme::text()))
                .tooltip(ui::tip(format!("Back to {parent}")))
                .child(parent)
                .on_click(move |_, _, cx| on_parent(cx)),
        )
        .child(ui::icon(Icon::ChevronRight).size(px(16.)).text_color(theme::text_muted()))
        .child(ui::display(title.to_string(), 28.));
    header(crumbs.into_any_element(), subtitle, Vec::new())
}

fn header(title: AnyElement, subtitle: &str, actions: Vec<AnyElement>) -> impl IntoElement {
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
                .gap(px(4.))
                .child(title)
                .child(ui::body(subtitle.to_string()).max_w(px(760.))),
        )
        .child(div().flex_none().flex().gap(px(8.)).children(actions))
}

/// Shown when the game or its config can't be found: an Error InfoBar with a
/// "Locate game" action.
pub(crate) fn missing_notice(title: &str, detail: &str, ws: &Entity<Workspace>) -> AnyElement {
    let ws = ws.clone();
    ui::info_bar(ui::Severity::Error, title.to_string(), detail.to_string())
        .child(
            ui::button("missing-browse", "Locate game", Some(Icon::Folder), ui::Variant::Secondary)
                .tooltip(ui::tip("Pick the folder the game is installed in"))
                .on_click(move |_, _, cx| {
                    let ws = ws.clone();
                    pick_paths(false, true, false, cx, move |paths, cx| {
                        if let Some(p) = paths.into_iter().next() {
                            ws.update(cx, |ws, cx| ws.set_manual_install(p, cx));
                        }
                    });
                }),
        )
        .into_any_element()
}

/// Makes text one line that ends in "…" when it doesn't fit its width (from
/// the parent: a stretched `flex_col` child or a `flex_1().min_w_0()` item).
///
/// Not `truncate()`: that sets `whitespace_nowrap`, and gpui's text measure
/// cache ignores the truncation width for unwrapped text, so the first probe
/// wins (the whole text, clipped mid-glyph, or a lone "…"). Wrapping text
/// clamped to one line is measured again whenever its width changes.
pub(crate) fn one_line<E: Styled>(e: E) -> E {
    e.min_w_0().text_ellipsis().line_clamp(1)
}

/// One line of text that ends in "…" when it doesn't fit ([`one_line`]),
/// with the whole text in a tooltip. `id` must be unique among its siblings.
pub(crate) fn clipped(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Stateful<Div> {
    let text: SharedString = text.into();
    clipped_with_tip(id, text.clone(), text)
}

/// [`clipped`] with a different (usually longer) tooltip than the text.
pub(crate) fn clipped_with_tip(id: impl Into<SharedString>, text: impl Into<SharedString>, tip: impl Into<SharedString>) -> Stateful<Div> {
    one_line(div().id(ElementId::Name(id.into()))).tooltip(ui::tip(tip)).child(text.into())
}

/// Caption-size (12/16, secondary text) [`clipped`] line: row descriptions.
pub(crate) fn clipped_caption(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Stateful<Div> {
    caption_style(clipped(id, text))
}

/// Caption type (12/16, secondary text) on any element.
pub(crate) fn caption_style<E: Styled>(e: E) -> E {
    e.text_size(px(12.)).line_height(px(16.)).text_color(theme::text_muted())
}

/// A path shortened in the middle so the drive and the last folders both stay
/// visible: `C:\Users\…\sandbox\bl2`. Paths up to `max` characters are
/// returned as they are.
pub(crate) fn short_path(path: &std::path::Path, max: usize) -> String {
    let full = path.display().to_string();
    if full.chars().count() <= max {
        return full;
    }
    let sep = if full.contains('\\') { '\\' } else { '/' };
    let parts: Vec<&str> = full.split(sep).collect();
    if parts.len() <= 3 {
        return full;
    }
    let head = parts[..2].join(&sep.to_string());
    let budget = max.saturating_sub(head.chars().count() + 2);
    // Keep as many trailing folders as fit (always at least the last one).
    let mut tail: Vec<&str> = Vec::new();
    let mut used = 0;
    for part in parts[2..].iter().rev() {
        let len = part.chars().count() + usize::from(!tail.is_empty());
        if !tail.is_empty() && used + len > budget {
            break;
        }
        used += len;
        tail.insert(0, part);
    }
    if tail.len() == parts.len() - 2 {
        return full;
    }
    format!("{head}{sep}…{sep}{}", tail.join(&sep.to_string()))
}

/// "1 setting", "3 settings".
pub(crate) fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A stored "YYYY-MM-DD HH:MM[:SS]" local time as people say it: "Today,
/// 17:59", "Yesterday, 09:12", "25 Sep", or "25 Sep 2024" in other years.
/// Anything unparseable comes back unchanged.
pub(crate) fn friendly_time(stamp: &str, now: chrono::NaiveDateTime) -> String {
    use chrono::{Datelike, NaiveDateTime};
    let Some(at) = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"].iter().find_map(|f| NaiveDateTime::parse_from_str(stamp, f).ok()) else {
        return stamp.to_string();
    };
    match (now.date() - at.date()).num_days() {
        0 => format!("Today, {}", at.format("%H:%M")),
        1 => format!("Yesterday, {}", at.format("%H:%M")),
        _ if at.year() == now.year() => at.format("%-d %b").to_string(),
        _ => at.format("%-d %b %Y").to_string(),
    }
}

/// [`friendly_time`] against the local clock.
pub(crate) fn friendly_now(stamp: &str) -> String {
    friendly_time(stamp, chrono::Local::now().naive_local())
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
                        .child(clipped_caption("banner-caption", caption)),
                )
                .child(div().flex_none().flex().gap(px(8.)).children(actions)),
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

#[cfg(test)]
mod tests {
    use super::{count, friendly_time, short_path};
    use chrono::NaiveDateTime;
    use std::path::Path;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn friendly_times() {
        let now = at("2026-09-27 12:00:00");
        assert_eq!(friendly_time("2026-09-27 09:05:01", now), "Today, 09:05");
        assert_eq!(friendly_time("2026-09-26 23:59:59", now), "Yesterday, 23:59");
        assert_eq!(friendly_time("2026-09-25 00:28:43", now), "25 Sep");
        assert_eq!(friendly_time("2025-01-02 10:00", now), "2 Jan 2025");
        assert_eq!(friendly_time("original", now), "original");
    }

    #[test]
    fn short_paths_keep_both_ends() {
        let p = Path::new(r"C:\Users\someone\AppData\Local\Temp\claude\sandbox\bl2");
        let s = short_path(p, 30);
        assert!(s.starts_with(r"C:\Users\…\"), "{s}");
        assert!(s.ends_with(r"\sandbox\bl2"), "{s}");
        assert!(s.chars().count() <= 30, "{s}");
        assert_eq!(short_path(Path::new(r"D:\Games\BL2"), 30), r"D:\Games\BL2");
    }

    #[test]
    fn counts() {
        assert_eq!(count(1, "change", "changes"), "1 change");
        assert_eq!(count(3, "change", "changes"), "3 changes");
    }
}
