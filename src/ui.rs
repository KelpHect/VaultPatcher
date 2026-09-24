//! Reusable widgets in the Vault Patcher style. Everything here is a plain
//! builder function so pages can compose them freely.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ContentMask, ElementId, FontWeight, IntoElement, MouseButton, ParentElement,
    PathBuilder, Pixels, Render, Rgba, SharedString, Stateful, Styled, Window, canvas, div,
    point, prelude::*, px, relative,
};

use crate::sound::{self, Sound};
use crate::theme::{self, Icon};

pub fn icon(icon: Icon) -> gpui::Div {
    div()
        .font_family(theme::FONT_ICON)
        .flex_none()
        .child(icon.glyph())
}

/// Big comic display heading. Reserve it for page titles and the hero —
/// used everywhere it stops being special.
pub fn display(text: impl Into<SharedString>, size: f32) -> gpui::Div {
    div()
        .font_family(theme::FONT_DISPLAY)
        .text_size(px(size))
        .line_height(px(size * 1.05))
        .text_color(theme::accent())
        .child(text.into())
}

/// Condensed uppercase label used for section titles and buttons.
pub fn label(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .font_family(theme::FONT_LABEL)
        .font_weight(FontWeight::BOLD)
        .text_size(px(14.))
        .text_color(theme::text_muted())
        .child(SharedString::from(text.into().to_uppercase()))
}

pub fn body(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(15.))
        .line_height(px(21.))
        .text_color(theme::text_muted())
        .child(text.into())
}

/// A quiet content surface. Most of the screen should be these.
pub fn panel() -> gpui::Div {
    div().bg(theme::panel()).border_1().border_color(theme::line())
}

/// The loud, cel-shaded surface (ink outline + hard shadow). One per page at
/// most, for the thing the page is about.
pub fn hero_panel() -> gpui::Div {
    div()
        .bg(theme::panel())
        .border_2()
        .border_color(theme::ink())
        .shadow(theme::comic_shadow(6.))
}

/// Panel with a colored accent strip on the left edge. Add content with
/// `card_body()` so long text wraps instead of overflowing.
pub fn card(accent: Rgba) -> gpui::Div {
    panel().flex().child(div().w(px(5.)).flex_none().bg(accent))
}

pub fn card_body() -> gpui::Div {
    div().flex_1().min_w_0()
}

pub fn badge(text: impl Into<SharedString>, color: Rgba) -> gpui::Div {
    div()
        .flex_none()
        .px(px(7.))
        .py(px(1.))
        .border_1()
        .border_color(theme::with_alpha(color, 0.8))
        .bg(theme::with_alpha(color, 0.14))
        .text_color(color)
        .font_family(theme::FONT_LABEL)
        .font_weight(FontWeight::BOLD)
        .text_size(px(12.))
        .child(SharedString::from(text.into().to_uppercase()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Secondary,
    Ghost,
}

pub fn button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    icon_glyph: Option<Icon>,
    variant: Variant,
) -> Stateful<gpui::Div> {
    let (bg, fg, hover) = match variant {
        Variant::Primary => (theme::accent(), theme::accent_ink(), theme::accent_hi()),
        Variant::Secondary => (theme::panel_hi(), theme::text(), theme::line()),
        Variant::Ghost => (gpui::transparent_black().into(), theme::text_muted(), theme::panel_hi()),
    };
    let primary = variant == Variant::Primary;
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .gap(px(7.))
        .h(px(34.))
        .px(px(14.))
        .bg(bg)
        .text_color(fg)
        // Only the primary action gets the comic ink + shadow treatment.
        .when(primary, |d| d.border_2().border_color(theme::ink()).shadow(theme::comic_shadow(3.)))
        .when(variant == Variant::Secondary, |d| d.border_1().border_color(theme::line()))
        .font_family(theme::FONT_LABEL)
        .font_weight(FontWeight::BOLD)
        .text_size(px(15.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .active(|s| s.mt(px(2.)).mb(px(-2.)))
        .when_some(icon_glyph, |d, i| d.child(icon(i).text_size(px(13.))))
        .child(SharedString::from(text.into().to_uppercase()))
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
        .when(primary, |d| {
            d.on_hover(|hovered, _, _| {
                if *hovered {
                    sound::play(Sound::Hover)
                }
            })
        })
}

/// A square icon-only button (window chrome, list actions).
pub fn icon_button(id: impl Into<ElementId>, glyph: Icon, tooltip_color: Rgba) -> Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .text_color(tooltip_color)
        .cursor_pointer()
        .hover(|s| s.bg(theme::panel_hi()))
        .child(icon(glyph).text_size(px(12.)))
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

pub fn toggle(id: impl Into<ElementId>, on: bool) -> Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .w(px(54.))
        .h(px(26.))
        .p(px(2.))
        .border_1()
        .border_color(if on { theme::echo() } else { theme::text_dim() })
        .bg(if on { theme::with_alpha(theme::echo(), 0.22) } else { theme::panel_lo().into() })
        .when(on, |d| d.justify_end())
        .cursor_pointer()
        .child(div().size(px(18.)).bg(if on { theme::echo() } else { theme::text_dim() }))
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// Marker type carried by slider drags so each slider only reacts to its own.
#[derive(Clone)]
pub struct SliderDrag(pub SharedString);

/// Invisible drag preview for drag interactions that draw their own feedback.
pub struct EmptyView;

impl Render for EmptyView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Horizontal slider. `fraction` is the current position in `0..=1` and
/// `on_change` receives the new fraction on click or drag.
pub fn slider(
    id: impl Into<SharedString>,
    fraction: f32,
    accent: Rgba,
    on_change: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let id: SharedString = id.into();
    let fraction = fraction.clamp(0., 1.);
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let on_change = Rc::new(on_change);
    let to_fraction = |x: Pixels, b: Bounds<Pixels>| -> f32 {
        if b.size.width <= px(0.) {
            return 0.;
        }
        ((x - b.left()) / b.size.width).clamp(0., 1.)
    };

    let drag_id = id.clone();
    let bounds_for_canvas = bounds.clone();
    let on_down = on_change.clone();
    let bounds_for_down = bounds.clone();

    div()
        .id(ElementId::Name(id.clone()))
        .relative()
        .flex_1()
        .min_w(px(120.))
        .h(px(26.))
        .cursor_pointer()
        .child(
            canvas(move |b, _, _| bounds_for_canvas.set(b), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        // track
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(8.))
                .h(px(10.))
                .bg(theme::bg_deep())
                .border_2()
                .border_color(theme::ink())
                .child(div().h_full().w(relative(fraction)).bg(theme::with_alpha(accent, 0.75))),
        )
        // knob
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(relative(fraction))
                .ml(px(-7.))
                .w(px(14.))
                .h(px(22.))
                .bg(theme::text())
                .border_2()
                .border_color(theme::ink()),
        )
        .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
            let f = to_fraction(ev.position.x, bounds_for_down.get());
            on_down(f, window, cx);
        })
        .on_drag(SliderDrag(drag_id.clone()), |_, _, _, cx| cx.new(|_| EmptyView))
        .on_drag_move::<SliderDrag>(move |ev, window, cx| {
            if ev.drag(cx).0 == drag_id {
                let f = to_fraction(ev.event.position.x, ev.bounds);
                on_change(f, window, cx);
            }
        })
        .into_any_element()
}

/// Segmented option chips; wraps onto multiple rows when needed.
pub fn chip(id: impl Into<ElementId>, text: impl Into<SharedString>, selected: bool) -> Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex_none()
        .px(px(10.))
        .h(px(28.))
        .flex()
        .items_center()
        .border_1()
        .border_color(if selected { theme::text_dim() } else { theme::line() })
        // Selected options are a raised surface with a yellow underline;
        // solid yellow is reserved for actions (buttons).
        .when(selected, |d| d.border_b_2().border_color(theme::accent()))
        .bg(if selected { theme::panel_hi() } else { theme::panel_lo() })
        .text_color(if selected { theme::text() } else { theme::text_muted() })
        .font_family(theme::FONT_LABEL)
        .font_weight(if selected { FontWeight::BOLD } else { FontWeight::SEMIBOLD })
        .text_size(px(14.5))
        .cursor_pointer()
        .when(!selected, |d| d.hover(|s| s.bg(theme::panel_hi()).text_color(theme::text())))
        .child(text.into())
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// Diagonal hazard stripes painted across the element's bounds.
pub fn hazard_stripes(color: Rgba, spacing: f32) -> gpui::Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                let h = bounds.size.height;
                let stripe = px(spacing / 2.);
                let mut x = bounds.left() - h;
                while x < bounds.right() {
                    let mut path = PathBuilder::fill();
                    path.move_to(point(x, bounds.bottom()));
                    path.line_to(point(x + stripe, bounds.bottom()));
                    path.line_to(point(x + stripe + h, bounds.top()));
                    path.line_to(point(x + h, bounds.top()));
                    path.close();
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                    x += px(spacing);
                }
            });
        },
    )
}

/// Title row used at the top of each page section: plain, readable, calm.
pub fn section_title(title: impl Into<SharedString>, subtitle: Option<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(
            div()
                .font_family(theme::FONT_LABEL)
                .font_weight(FontWeight::BOLD)
                .text_size(px(23.))
                .text_color(theme::text())
                .child(title.into()),
        )
        .when_some(subtitle, |d, s| d.child(body(s).text_color(theme::text_dim())))
}

pub fn kv_row(key: impl Into<SharedString>, value: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .gap(px(12.))
        .items_start()
        .py(px(4.))
        .child(label(key).w(px(130.)).flex_none().pt(px(2.)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_family(theme::FONT_MONO)
                .text_size(px(13.5))
                .text_color(theme::text())
                .child(value.into()),
        )
}
