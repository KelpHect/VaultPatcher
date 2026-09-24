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

/// For settings with captured images: the options *are* the pictures.
/// Click a card to choose it (saved instantly in Simple mode); "Compare"
/// opens the side-by-side viewer.
pub fn choice_cards(tweak: &'static Tweak, ws: &Entity<Workspace>, cx: &App) -> Option<AnyElement> {
    let state = ws.read(cx);
    let game = state.game();
    let images = compare::images(game.def.id, tweak);
    let values = compare::captured_values(game.def.id, tweak);
    if images.len() < 2 || images.len() != values.len() {
        return None;
    }
    let current = game.effective(tweak);
    let simple = state.mode() == crate::games::Mode::Simple;
    let selected_index = values.iter().position(|v| *v == current).unwrap_or(0);
    let mut grid = div().flex().flex_wrap().gap(px(12.));
    for (i, ((label, path), value)) in images.into_iter().zip(values).enumerate() {
        let selected = value == current;
        let ws = ws.clone();
        grid = grid.child(
            div()
                .id(SharedString::from(format!("card-{}-{i}", tweak.id)))
                .w(px(178.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(3.))
                .border_2()
                .border_color(if selected { theme::accent() } else { gpui::transparent_black().into() })
                .cursor_pointer()
                .hover(|s| s.bg(theme::panel_hi()))
                .child(div().w(px(168.)).h(px(94.)).child(img(path).size_full().object_fit(ObjectFit::Cover)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(3.))
                        .pb(px(2.))
                        .font_family(theme::FONT_LABEL)
                        .font_weight(if selected { FontWeight::BOLD } else { FontWeight::SEMIBOLD })
                        .text_size(px(14.5))
                        .text_color(if selected { theme::text() } else { theme::text_muted() })
                        .when(selected, |d| d.child(ui::icon(Icon::Check).text_size(px(11.)).text_color(theme::accent())))
                        // Constrained so long names wrap instead of spilling into the next card.
                        .child(div().flex_1().min_w(px(0.)).child(label)),
                )
                .on_mouse_down(gpui::MouseButton::Left, |_, _, _| crate::sound::play(crate::sound::Sound::Click))
                .on_click(move |_, _, cx| {
                    let value = value.clone();
                    ws.update(cx, |ws, cx| {
                        if simple {
                            ws.set_now(tweak, value, cx);
                        } else {
                            ws.stage(tweak, value, cx);
                        }
                    })
                }),
        );
    }
    let compare_ws = ws.clone();
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(grid)
            .child(
                div()
                    .id(SharedString::from(format!("compare-{}", tweak.id)))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(14.))
                    .text_color(theme::echo())
                    .cursor_pointer()
                    .hover(|s| s.text_color(theme::text()))
                    .child("⇆ Compare side by side")
                    .on_click(move |_, _, cx| compare_ws.update(cx, |ws, cx| ws.open_preview(tweak.id, selected_index, cx))),
            )
            .into_any_element(),
    )
}

/// Marker for divider drags in the comparison viewer.
#[derive(Clone)]
struct DividerDrag;

/// Side-by-side viewer: two options of a setting stacked in one frame with a
/// draggable divider; the option chips below choose what's on each side.
pub fn lightbox(ws: &Entity<Workspace>, window: &Window, cx: &App) -> Option<AnyElement> {
    let state = ws.read(cx);
    let preview = state.preview?;
    let def = state.game().def;
    let tweak = def.tweak(preview.tweak)?;
    let images = compare::images(def.id, tweak);
    if images.is_empty() {
        return None;
    }
    let pick = |i: usize| images.get(i).or_else(|| images.first()).cloned().expect("non-empty");
    let (left_label, left_path) = pick(preview.left);
    let (right_label, right_path) = pick(preview.right);

    // Fit a 16:9 frame into the window, leaving room for the controls.
    let viewport = window.viewport_size();
    let max_w = f32::from(viewport.width) * 0.86;
    let max_h = f32::from(viewport.height) - 260.;
    let frame_w = max_w.min(max_h * 16. / 9.).max(320.);
    let frame_h = frame_w * 9. / 16.;
    let split = preview.split.clamp(0., 1.);

    let bounds = std::rc::Rc::new(std::cell::Cell::new(gpui::Bounds::<gpui::Pixels>::default()));
    let to_split = |x: gpui::Pixels, b: gpui::Bounds<gpui::Pixels>| {
        if b.size.width <= px(0.) { 0.5 } else { ((x - b.left()) / b.size.width).clamp(0., 1.) }
    };
    let down_ws = ws.clone();
    let drag_ws = ws.clone();
    let bounds_down = bounds.clone();
    let bounds_canvas = bounds.clone();

    let tag = |text: String, right: bool| {
        div()
            .absolute()
            .top(px(12.))
            .when(right, |d| d.right(px(12.)))
            .when(!right, |d| d.left(px(12.)))
            .px(px(10.))
            .py(px(4.))
            .bg(theme::with_alpha(theme::bg_deep(), 0.8))
            .font_family(theme::FONT_LABEL)
            .font_weight(FontWeight::BOLD)
            .text_size(px(13.))
            .text_color(theme::text())
            .child(text)
    };

    let frame = div()
        .id("compare-frame")
        .relative()
        .flex_none()
        .w(px(frame_w))
        .h(px(frame_h))
        .overflow_hidden()
        .border_2()
        .border_color(theme::ink())
        .cursor(gpui::CursorStyle::ResizeLeftRight)
        .child(gpui::canvas(move |b, _, _| bounds_canvas.set(b), |_, _, _, _| {}).absolute().size_full())
        // Right option fills the frame…
        .child(img(right_path).absolute().top_0().left_0().w(px(frame_w)).h(px(frame_h)).object_fit(ObjectFit::Cover))
        // …and the left option is revealed up to the divider.
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .w(px(frame_w * split))
                .overflow_hidden()
                .child(img(left_path).w(px(frame_w)).h(px(frame_h)).object_fit(ObjectFit::Cover)),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(frame_w * split - 1.5))
                .w(px(3.))
                .bg(theme::accent()),
        )
        .child(
            div()
                .absolute()
                .top(px(frame_h / 2. - 18.))
                .left(px(frame_w * split - 18.))
                .size(px(36.))
                .flex()
                .items_center()
                .justify_center()
                .bg(theme::accent())
                .border_2()
                .border_color(theme::ink())
                .text_color(theme::accent_ink())
                .font_family(theme::FONT_LABEL)
                .font_weight(FontWeight::BOLD)
                .text_size(px(14.))
                .child("◀▶"),
        )
        .child(tag(left_label.clone(), false))
        .child(tag(right_label.clone(), true))
        .on_mouse_down(gpui::MouseButton::Left, move |ev, _, cx| {
            cx.stop_propagation();
            let s = to_split(ev.position.x, bounds_down.get());
            down_ws.update(cx, |ws, cx| ws.set_preview(|p| p.split = s, cx));
        })
        .on_click(|_, _, cx| cx.stop_propagation())
        .on_drag(DividerDrag, |_, _, _, cx| cx.new(|_| ui::EmptyView))
        .on_drag_move::<DividerDrag>(move |ev, _, cx| {
            let s = to_split(ev.event.position.x, ev.bounds);
            drag_ws.update(cx, |ws, cx| ws.set_preview(|p| p.split = s, cx));
        });

    let chips = |side_right: bool| {
        let mut row = div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(6.))
            .child(ui::label(if side_right { "Right" } else { "Left" }).mr(px(4.)));
        for (i, (label, _)) in images.iter().enumerate() {
            let ws = ws.clone();
            let selected = if side_right { preview.right == i } else { preview.left == i };
            row = row.child(
                ui::chip(SharedString::from(format!("lb-{}-{i}", if side_right { "r" } else { "l" })), label.clone(), selected)
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        ws.update(cx, |ws, cx| {
                            ws.set_preview(|p| if side_right { p.right = i } else { p.left = i }, cx)
                        })
                    }),
            );
        }
        row
    };

    let close_ws = ws.clone();
    Some(
        div()
            .id("lightbox")
            .absolute()
            .inset_0()
            .occlude()
            .bg(theme::with_alpha(theme::bg_deep(), 0.95))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .p(px(24.))
            .on_click(move |_, _, cx| close_ws.update(cx, |ws, cx| ws.close_preview(cx)))
            .child(
                div()
                    .font_family(theme::FONT_DISPLAY)
                    .text_size(px(30.))
                    .text_color(theme::accent())
                    .child(tweak.label),
            )
            .child(frame)
            .child(
                div()
                    .id("lightbox-controls")
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(28.))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(chips(false))
                    .child(chips(true)),
            )
            .child({
                let values = compare::captured_values(def.id, tweak);
                let simple = state.mode() == crate::games::Mode::Simple;
                let mut row = div().id("lightbox-use").flex().gap(px(12.)).on_click(|_, _, cx| cx.stop_propagation());
                for (side, index, label) in [("left", preview.left, &left_label), ("right", preview.right, &right_label)] {
                    let Some(value) = values.get(index).cloned() else { continue };
                    let ws = ws.clone();
                    row = row.child(
                        ui::button(
                            SharedString::from(format!("use-{side}")),
                            format!("Use {label}"),
                            Some(Icon::Check),
                            if side == "right" { ui::Variant::Primary } else { ui::Variant::Secondary },
                        )
                        .on_click(move |_, _, cx| {
                            let value = value.clone();
                            ws.update(cx, |ws, cx| {
                                if simple {
                                    ws.set_now(tweak, value, cx);
                                } else {
                                    ws.stage(tweak, value, cx);
                                }
                                ws.close_preview(cx);
                            })
                        }),
                    );
                }
                row
            })
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(theme::text_dim())
                    .child("Drag across the image to compare · ← → switch the right side · Esc closes"),
            )
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
        save_row = save_row.child(ui::body("No save files yet — play the game once and save somewhere scenic first."));
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
    let no_saves = saves.is_empty();
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
                ui::button(
                    "capture-start",
                    if have >= total_shots { "Recapture" } else { "Start capture" },
                    Some(Icon::Camera),
                    if have >= total_shots { Variant::Secondary } else { Variant::Primary },
                )
                .when(no_saves, |b| b.opacity(0.4))
                .on_click(move |_, _, cx| {
                    if !no_saves {
                        start_ws.update(cx, |ws, cx| ws.start_capture(cx))
                    }
                })
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
