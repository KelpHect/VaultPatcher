//! Comparison images: thumbnails under a setting, the full-size viewer, and
//! the Advanced-mode capture page.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ObjectFit, ParentElement, SharedString,
    Styled, StyledImage, Window, div, img, prelude::*, px, relative,
};

use super::{missing_notice, page_header};
use crate::compare;
use crate::games::willow;
use crate::theme::{self, Icon};
use crate::tweaks::Tweak;
use crate::ui::{self, Variant};
use crate::workspace::Workspace;

/// Thumbnails of a setting's captured images plus the Nvidia link, if any.
pub fn strip(tweak: &'static Tweak, ws: &Entity<Workspace>, cx: &App) -> Option<AnyElement> {
    let state = ws.read(cx);
    let def = state.game().def;
    let comparison = def.comparison(tweak.id)?;
    let images = compare::images(def.id, tweak);
    if images.is_empty() && comparison.link.is_none() {
        return None;
    }
    let mut row = div().flex().flex_wrap().items_end().gap(px(10.)).pt(px(4.));
    for (i, (label, path)) in images.into_iter().enumerate() {
        let ws = ws.clone();
        row = row.child(
            div()
                .id(SharedString::from(format!("thumb-{}-{i}", tweak.id)))
                .flex()
                .flex_col()
                .gap(px(4.))
                .cursor_pointer()
                .child(
                    div()
                        .w(px(150.))
                        .h(px(84.))
                        .border_1()
                        .border_color(theme::line())
                        .hover(|s| s.border_color(theme::text_muted()))
                        .child(img(path).size_full().object_fit(ObjectFit::Cover)),
                )
                .child(div().text_size(px(12.)).text_color(theme::text_dim()).child(label))
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.open_preview(tweak.id, i, cx))),
        );
    }
    if let Some(page) = comparison.link {
        let url = willow::nvidia_url(page);
        row = row.child(
            div()
                .id(SharedString::from(format!("nv-{}", tweak.id)))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(13.))
                .text_color(theme::echo())
                .cursor_pointer()
                .hover(|s| s.text_color(theme::text()))
                .child(ui::icon(Icon::Link).text_size(px(11.)))
                .child("Nvidia comparison")
                .on_click(move |_, _, cx| cx.open_url(&url)),
        );
    }
    Some(row.into_any_element())
}

/// Full-size viewer; flip between a setting's options to compare them.
pub fn lightbox(ws: &Entity<Workspace>, cx: &App) -> Option<AnyElement> {
    let state = ws.read(cx);
    let (tweak_id, index) = state.preview?;
    let def = state.game().def;
    let tweak = def.tweak(tweak_id)?;
    let images = compare::images(def.id, tweak);
    let (label, path) = images.get(index).cloned().or_else(|| images.first().cloned())?;

    let close_ws = ws.clone();
    let mut tabs = div().flex().flex_wrap().justify_center().gap(px(8.));
    for (i, (l, _)) in images.iter().enumerate() {
        let ws = ws.clone();
        tabs = tabs.child(
            ui::chip(SharedString::from(format!("lb-{i}")), l.clone(), i == index)
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| ws.open_preview(tweak_id, i, cx))),
        );
    }
    Some(
        div()
            .id("lightbox")
            .absolute()
            .inset_0()
            // Swallow every click so nothing underneath the overlay reacts.
            .occlude()
            .bg(theme::with_alpha(theme::bg_deep(), 0.94))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .p(px(32.))
            .on_click(move |_, _, cx| close_ws.update(cx, |ws, cx| ws.close_preview(cx)))
            .child(
                div()
                    .font_family(theme::FONT_LABEL)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(20.))
                    .text_color(theme::text())
                    .child(format!("{} — {label}", tweak.label)),
            )
            .child(
                div()
                    .flex_none()
                    .w(relative(0.9))
                    .h(relative(0.66))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    // Images keep their aspect ratio, so cap them to the box.
                    .child(img(path).max_w_full().max_h_full().object_fit(ObjectFit::Contain)),
            )
            .child(
                div()
                    .id("lightbox-tabs")
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(tabs),
            )
            .child(div().text_size(px(12.5)).text_color(theme::text_dim()).child("Click an option to flip between them · click anywhere else to close"))
            .into_any_element(),
    )
}

/// Advanced mode: capture comparison screenshots on this PC.
pub fn capture_page(ws: &Entity<Workspace>, _window: &mut Window, cx: &mut App) -> AnyElement {
    let state = ws.read(cx);
    let game = state.game();
    let def = game.def;
    let mut page = div().flex().flex_col().gap(px(24.)).child(page_header(
        "Comparison Capture",
        "Takes a screenshot of every option of the listed settings, so Quick Settings can show what each one looks like. Your settings are backed up first and restored when it finishes.",
        vec![],
    ));
    if game.install.is_none() {
        return page.child(missing_notice("Game install not found", "Capturing runs the game.", ws)).into_any_element();
    }

    let targets: Vec<&'static Tweak> = def.comparisons.iter().filter(|c| c.capture).filter_map(|c| def.tweak(c.tweak)).collect();
    let total_shots: usize = targets.iter().map(|t| compare::capture_values(t).len()).sum();
    let have: usize = targets.iter().map(|t| compare::images(def.id, t).len()).sum();

    // ---- how it works
    page = page.child(
        ui::panel().p(px(20.)).flex().flex_col().gap(px(8.))
            .child(ui::section_title("How it works", None))
            .child(ui::body(format!("1. For each of the {total_shots} shots, Vault Patcher writes that setting and launches the game straight into your chosen save (using the Quick Startup mod).")))
            .child(ui::body("2. A small helper mod waits for the world to load, hides the HUD, takes a screenshot and quits."))
            .child(ui::body("3. Screenshots are resized and stored with Vault Patcher; your settings and game folder are restored afterwards."))
            .child(ui::body(format!("It takes roughly {} minutes and the game will take over the screen — start it when you're away from the PC. Stand somewhere scenic when you save, since every shot is taken where the save loads.", (total_shots as u32 * (state.capture_settle + 30)).div_ceil(60)))),
    );

    // ---- options
    let saves = state.capture_saves();
    let chosen = state.capture_save.clone().or_else(|| saves.first().cloned());
    let mut save_row = div().flex().flex_wrap().gap(px(8.));
    for s in saves.iter().take(8) {
        let ws = ws.clone();
        let name = s.clone();
        save_row = save_row.child(
            ui::chip(SharedString::from(format!("save-{s}")), s.clone(), chosen.as_deref() == Some(s))
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| {
                    ws.capture_save = Some(name.clone());
                    cx.notify();
                })),
        );
    }
    if saves.is_empty() {
        save_row = save_row.child(ui::body("No save files found."));
    }
    let mut settle_row = div().flex().gap(px(8.));
    for secs in [15u32, 25, 40] {
        let ws = ws.clone();
        settle_row = settle_row.child(
            ui::chip(SharedString::from(format!("settle-{secs}")), format!("{secs} s"), state.capture_settle == secs)
                .on_click(move |_, _, cx| ws.update(cx, |ws, cx| {
                    ws.capture_settle = secs;
                    cx.notify();
                })),
        );
    }
    page = page.child(
        ui::panel().p(px(20.)).flex().flex_col().gap(px(14.))
            .child(div().flex().flex_col().gap(px(6.)).child(ui::label("Save to load")).child(save_row))
            .child(div().flex().flex_col().gap(px(6.)).child(ui::label("Wait after loading (lets textures stream in)")).child(settle_row))
            .child(div().flex().flex_col().gap(px(6.)).child(ui::label("Settings captured")).child(ui::body(
                targets.iter().map(|t| t.label).collect::<Vec<_>>().join(" · "),
            ))),
    );

    // ---- run / progress
    let running = state.capture_running();
    let start_ws = ws.clone();
    let cancel_ws = ws.clone();
    let mut run = ui::panel().p(px(20.)).flex().flex_col().gap(px(12.)).child(
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(ui::body(format!("{have} of {total_shots} images captured so far.")).flex_1())
            .child(if running {
                ui::button("capture-cancel", "Stop after this shot", Some(Icon::Close), Variant::Secondary)
                    .on_click(move |_, _, cx| cancel_ws.update(cx, |ws, cx| ws.cancel_capture(cx)))
                    .into_any_element()
            } else {
                ui::button("capture-start", "Start capture", Some(Icon::Camera), Variant::Primary)
                    .on_click(move |_, _, cx| start_ws.update(cx, |ws, cx| ws.start_capture(cx)))
                    .into_any_element()
            }),
    );
    if let Some(progress) = state.capture.as_ref().and_then(|p| p.lock().ok().map(|p| p.clone())) {
        let fraction = if progress.total == 0 { 0. } else { progress.done as f32 / progress.total as f32 };
        run = run
            .child(div().h(px(6.)).w_full().bg(theme::panel_lo()).child(div().h_full().w(relative(fraction)).bg(theme::accent())))
            .child(div().text_size(px(13.)).text_color(theme::text_dim()).child(if progress.finished {
                "Finished — settings restored.".to_string()
            } else {
                format!("Shot {} of {}: {}", progress.done + 1, progress.total, progress.current)
            }))
            .children(progress.log.iter().rev().take(8).map(|l| div().text_size(px(12.5)).text_color(theme::text_muted()).child(l.clone())));
    }
    page.child(run).into_any_element()
}
