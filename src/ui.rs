//! Reusable widgets. Metrics follow Windows 11 / Fluent: 32px buttons, 4px
//! radius, 13–14px body text, sentence-case labels. Everything here is a plain
//! builder function so pages can compose them freely.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, FontWeight, IntoElement, MouseButton, ParentElement, Pixels, Render, Rgba,
    SharedString, Stateful, Styled, Window, canvas, div, prelude::*, px, relative, svg,
};

use crate::sound::{self, Sound};
use crate::theme::{self, Icon};

/// A 16px line icon; size with `.size(px(..))`, color with `.text_color(..)`.
pub fn icon(icon: Icon) -> gpui::Svg {
    svg().path(icon.path()).size(px(16.)).flex_none().text_color(theme::text_muted())
}

/// Page and game titles (condensed display face).
pub fn display(text: impl Into<SharedString>, size: f32) -> gpui::Div {
    div()
        .font_family(theme::FONT_TITLE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(size))
        .line_height(px(size * 1.15))
        .text_color(theme::text())
        .child(text.into())
}

/// Small uppercase group header ("ESSENTIALS", "DISPLAY").
pub fn label(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(11.5))
        .text_color(theme::text_dim())
        .child(SharedString::from(text.into().to_uppercase()))
}

pub fn body(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(theme::text_muted())
        .child(text.into())
}

/// Row/control title text.
pub fn title(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(14.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::text())
        .child(text.into())
}

/// A grouped surface (Settings-style card). Most content lives in these.
pub fn panel() -> gpui::Div {
    div()
        .bg(theme::panel())
        .border_1()
        .border_color(theme::line())
        .rounded(px(theme::RADIUS_LG))
        .overflow_hidden()
}

/// Panel with a thin colored strip on the left edge. Add content with
/// `card_body()` so long text wraps instead of overflowing.
pub fn card(accent: Rgba) -> gpui::Div {
    panel().flex().child(div().w(px(3.)).flex_none().bg(accent))
}

pub fn card_body() -> gpui::Div {
    div().flex_1().min_w_0()
}

/// 1px divider between rows in a panel.
pub fn divider() -> gpui::Div {
    div().h(px(1.)).flex_none().bg(theme::line())
}

/// A small colored tag.
pub fn badge(text: impl Into<SharedString>, color: Rgba) -> gpui::Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(px(20.))
        .px(px(7.))
        .rounded(px(theme::RADIUS))
        .bg(theme::with_alpha(color, 0.15))
        .text_color(color)
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(11.5))
        .child(text.into())
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
    let (bg, fg, hover, border): (gpui::Hsla, Rgba, Rgba, gpui::Hsla) = match variant {
        Variant::Primary => (theme::accent().into(), theme::accent_ink(), theme::accent_hi(), gpui::transparent_black()),
        Variant::Secondary => (theme::panel_hi().into(), theme::text(), theme::line(), theme::ink().into()),
        Variant::Ghost => (gpui::transparent_black(), theme::text(), theme::panel_hi(), gpui::transparent_black()),
    };
    let pressed = if variant == Variant::Primary { theme::accent_pressed() } else { theme::ink() };
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .h(px(32.))
        .px(px(12.))
        .rounded(px(theme::RADIUS))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_color(fg)
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(13.5))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .active(move |s| s.bg(pressed))
        .when_some(icon_glyph, |d, i| d.child(icon(i).size(px(15.)).text_color(fg)))
        .child(text.into())
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// A square icon-only button (toolbars, list actions). Give it a tooltip.
pub fn icon_button(id: impl Into<ElementId>, glyph: Icon, color: Rgba) -> Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded(px(theme::RADIUS))
        .cursor_pointer()
        .hover(|s| s.bg(theme::panel_hi()))
        .child(icon(glyph).size(px(15.)).text_color(color))
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// Attaches a plain-text tooltip.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| gpui_component::tooltip::Tooltip::new(text.clone()).build(window, cx)
}

/// On/off switch (40×20).
pub fn toggle(id: impl Into<ElementId>, on: bool) -> Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .w(px(40.))
        .h(px(20.))
        .px(px(3.))
        .rounded_full()
        .bg(if on { theme::accent() } else { theme::panel_lo() })
        .border_1()
        .border_color(if on { theme::accent() } else { theme::ink() })
        .when(on, |d| d.justify_end())
        .cursor_pointer()
        .child(div().size(px(12.)).rounded_full().bg(if on { theme::accent_ink() } else { theme::text_muted() }))
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// 16px checkbox.
pub fn checkbox(checked: bool, locked: bool) -> gpui::Div {
    div()
        .size(px(18.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::RADIUS))
        .border_1()
        .border_color(if checked { gpui::transparent_black() } else { theme::ink().into() })
        .bg(match (checked, locked) {
            (true, true) => theme::success(),
            (true, false) => theme::accent(),
            _ => theme::panel_lo(),
        })
        .when(checked, |d| d.child(icon(Icon::Check).size(px(13.)).text_color(theme::accent_ink())))
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
        .h(px(20.))
        .cursor_pointer()
        .child(
            canvas(move |b, _, _| bounds_for_canvas.set(b), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(8.))
                .h(px(4.))
                .rounded_full()
                .bg(theme::ink())
                .child(div().h_full().rounded_full().w(relative(fraction)).bg(accent)),
        )
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(relative(fraction))
                .ml(px(-8.))
                .size(px(16.))
                .rounded_full()
                .bg(theme::text())
                .border_2()
                .border_color(theme::bg()),
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

/// One option of a segmented choice; wraps onto multiple rows when needed.
pub fn chip(id: impl Into<ElementId>, text: impl Into<SharedString>, selected: bool) -> Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex_none()
        .px(px(10.))
        .h(px(28.))
        .flex()
        .items_center()
        .rounded(px(theme::RADIUS))
        .border_1()
        .border_color(if selected { theme::accent() } else { theme::line() })
        .bg(if selected { theme::selected() } else { theme::panel_lo() })
        .text_color(if selected { theme::text() } else { theme::text_muted() })
        .font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
        .text_size(px(13.))
        .cursor_pointer()
        .when(!selected, |d| d.hover(|s| s.bg(theme::panel_hi()).text_color(theme::text())))
        .child(text.into())
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// Joined segmented control: `items` are (id, label, selected).
pub fn segmented(items: Vec<(SharedString, SharedString, bool)>) -> (gpui::Div, Vec<Stateful<gpui::Div>>) {
    let wrap = div()
        .flex()
        .flex_none()
        .items_center()
        .p(px(2.))
        .gap(px(2.))
        .rounded(px(theme::RADIUS + 2.))
        .bg(theme::panel_lo())
        .border_1()
        .border_color(theme::line());
    let segments = items
        .into_iter()
        .map(|(id, text, selected)| {
            div()
                .id(ElementId::Name(id))
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(26.))
                .px(px(10.))
                .rounded(px(theme::RADIUS))
                .text_size(px(13.))
                .font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                .bg(if selected { theme::panel_hi() } else { gpui::transparent_black().into() })
                .text_color(if selected { theme::text() } else { theme::text_muted() })
                .cursor_pointer()
                .when(!selected, |d| d.hover(|s| s.text_color(theme::text())))
                .child(text)
                .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
        })
        .collect();
    (wrap, segments)
}

/// Heading above a group of rows.
pub fn section_title(title: impl Into<SharedString>, subtitle: Option<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
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
        .py(px(3.))
        .child(
            div()
                .w(px(120.))
                .flex_none()
                .text_size(px(12.5))
                .text_color(theme::text_dim())
                .child(key.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_family(theme::FONT_MONO)
                .text_size(px(12.5))
                .text_color(theme::text())
                .child(value.into()),
        )
}

/// A settings row: title + description on the left, control on the right.
pub fn setting_row(title_text: impl Into<SharedString>, description: impl Into<SharedString>, control: AnyElement) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .min_h(px(56.))
        .px(px(16.))
        .py(px(10.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(title(title_text))
                .child(body(description).text_color(theme::text_dim())),
        )
        .child(div().flex_none().child(control))
}
