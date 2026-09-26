//! Reusable widgets, drawn to the Windows 11 (WinUI 3) control specs: 32px
//! buttons and inputs, 4px control corners, the Segoe UI Variable type ramp,
//! sentence-case labels. Everything here is a plain builder function so pages
//! can compose them freely.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, ElementId, FontWeight, IntoElement, MouseButton,
    ParentElement, PathBuilder, Pixels, Render, Rgba, SharedString, Stateful, Styled, Window, canvas, div, point,
    prelude::*, px, relative, svg,
};

use crate::sound::{self, Sound};
use crate::theme::{self, Icon};

// ---- motion ---------------------------------------------------------------------

/// A CSS-style cubic-bezier easing curve through (0,0), (x1,y1), (x2,y2), (1,1).
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> impl Fn(f32) -> f32 + Clone {
    move |t: f32| {
        let t = t.clamp(0., 1.);
        let bez = |a: f32, b: f32, s: f32| {
            let u = 1. - s;
            3. * u * u * s * a + 3. * u * s * s * b + s * s * s
        };
        // Solve x(s) = t by bisection; 20 steps is far below a pixel.
        let (mut lo, mut hi) = (0f32, 1f32);
        for _ in 0..20 {
            let mid = (lo + hi) / 2.;
            if bez(x1, x2, mid) < t { lo = mid } else { hi = mid }
        }
        bez(y1, y2, (lo + hi) / 2.)
    }
}

/// Fluent's "fast out, slow in" decelerate curve for things arriving.
pub fn ease_decelerate() -> impl Fn(f32) -> f32 + Clone {
    cubic_bezier(0., 0., 0., 1.)
}

/// The page-refresh and navigation-pill curve (WinUI's entrance spline).
pub fn ease_entrance() -> impl Fn(f32) -> f32 + Clone {
    cubic_bezier(0.1, 0.9, 0.2, 1.)
}

thread_local! {
    /// Controls the user just flipped, so only they animate (not every
    /// control that mounts when a page opens).
    static CHANGED: RefCell<Vec<(SharedString, Instant)>> = const { RefCell::new(Vec::new()) };
}

/// Remembers that the control `id` changed state just now.
pub fn mark_changed(id: &SharedString) {
    CHANGED.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|(_, at)| at.elapsed() < Duration::from_secs(1));
        c.push((id.clone(), Instant::now()));
    });
}

fn changed_recently(id: &SharedString) -> bool {
    theme::motion()
        && CHANGED.with(|c| c.borrow().iter().any(|(i, at)| i == id && at.elapsed() < Duration::from_millis(400)))
}

/// Page refresh (WinUI's EntranceNavigationTransition): the content rises
/// `offset` px (140 for pages) over 300 ms as it fades in.
/// `key` must change whenever the content should play the entrance again.
pub fn entrance(key: impl Into<SharedString>, offset: f32, content: impl IntoElement) -> AnyElement {
    let key: SharedString = key.into();
    if !theme::motion() {
        return div().w_full().min_w_0().flex().flex_col().child(content).into_any_element();
    }
    let ease = ease_entrance();
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .relative()
        .child(content)
        .with_animation(ElementId::Name(key), Animation::new(Duration::from_millis(300)).with_easing(ease), move |d, t| {
            d.top(px(offset * (1. - t))).opacity((t * 2.).min(1.))
        })
        .into_any_element()
}

// ---- type ramp ------------------------------------------------------------------

/// A 16px Segoe Fluent icon; size with `.size(px(..))`, color with `.text_color(..)`.
pub fn icon(icon: Icon) -> gpui::Svg {
    svg().path(icon.path()).size(px(16.)).flex_none().text_color(theme::text_muted())
}

/// Page titles and headings in Segoe UI Variable Display: 28 is Title, 20
/// Subtitle, 40 Title Large.
pub fn display(text: impl Into<SharedString>, size: f32) -> gpui::Div {
    let line = if size >= 40. { size * 1.3 } else if size >= 24. { size * 1.29 } else { size * 1.4 };
    div()
        .font_family(theme::font_display())
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(size))
        .line_height(px(line.round()))
        .text_color(theme::text())
        .child(text.into())
}

/// Group header (Body Strong), as used above settings groups and nav sections.
pub fn label(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(14.))
        .line_height(px(20.))
        .text_color(theme::text())
        .child(text.into())
}

/// Body text (14/20).
pub fn body(text: impl Into<SharedString>) -> gpui::Div {
    div().text_size(px(14.)).line_height(px(20.)).text_color(theme::text_muted()).child(text.into())
}

/// Caption text (12/16), for descriptions under a row title.
pub fn caption(text: impl Into<SharedString>) -> gpui::Div {
    div().text_size(px(12.)).line_height(px(16.)).text_color(theme::text_muted()).child(text.into())
}

/// Row/control title text (Body, 14/20).
pub fn title(text: impl Into<SharedString>) -> gpui::Div {
    div().text_size(px(14.)).line_height(px(20.)).text_color(theme::text()).child(text.into())
}

// ---- surfaces -------------------------------------------------------------------

/// A grouped surface (Settings-style card). Most content lives in these.
pub fn panel() -> gpui::Div {
    div()
        .bg(theme::panel())
        .border_1()
        .border_color(theme::card_stroke())
        .rounded(px(theme::RADIUS))
        .overflow_hidden()
}

/// Panel with a thin colored strip on the left edge (severity marker). Add
/// content with `card_body()` so long text wraps.
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

/// A small tinted tag (InfoBadge-like).
pub fn badge(text: impl Into<SharedString>, color: Rgba) -> gpui::Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(px(20.))
        .px(px(8.))
        .rounded(px(theme::RADIUS))
        .bg(theme::with_alpha(color, if theme::is_dark() { 0.16 } else { 0.12 }))
        .text_color(color)
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(12.))
        .child(text.into())
}

/// InfoBar severities, kept as a set for pages to pick from.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}

impl Severity {
    /// (background, icon color, icon) of an InfoBar.
    pub fn style(self) -> (Rgba, Rgba, Icon) {
        match self {
            Severity::Info => (theme::info_bg(), theme::accent(), Icon::Info),
            Severity::Success => (theme::success_bg(), theme::success(), Icon::CheckCircle),
            Severity::Warning => (theme::warning_bg(), theme::warning(), Icon::Warning),
            Severity::Error => (theme::danger_bg(), theme::danger(), Icon::Alert),
        }
    }
}

// ---- buttons --------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Accent button: the one main action.
    Primary,
    /// Standard button.
    Secondary,
    /// Subtle button: transparent until hovered.
    Ghost,
}

pub fn button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    icon_glyph: Option<Icon>,
    variant: Variant,
) -> Stateful<gpui::Div> {
    let transparent: Rgba = gpui::transparent_black().into();
    let (bg, fg, hover, pressed, border) = match variant {
        Variant::Primary => (theme::accent(), theme::accent_ink(), theme::accent_hi(), theme::accent_pressed(), transparent),
        Variant::Secondary => (theme::control(), theme::text(), theme::control_hover(), theme::control_pressed(), theme::control_stroke()),
        Variant::Ghost => (transparent, theme::text(), theme::panel_hi(), theme::panel_pressed(), transparent),
    };
    let pressed_fg = if variant == Variant::Primary { fg } else { theme::text_muted() };
    focusable(div().id(id.into()))
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
        .text_size(px(14.))
        .line_height(px(20.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .active(move |s| s.bg(pressed).text_color(pressed_fg))
        .when_some(icon_glyph, |d, i| d.child(icon(i).text_color(fg)))
        .child(text.into())
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// A square icon-only subtle button (toolbars, list actions). Give it a tooltip.
pub fn icon_button(id: impl Into<ElementId>, glyph: Icon, color: Rgba) -> Stateful<gpui::Div> {
    focusable(div().id(id.into()))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(32.))
        .rounded(px(theme::RADIUS))
        .cursor_pointer()
        .hover(|s| s.bg(theme::panel_hi()))
        .active(|s| s.bg(theme::panel_pressed()))
        .child(icon(glyph).text_color(color))
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// Makes a control a Tab stop with the Windows focus visual: a 2px ring over
/// a 1px inner ring, just outside the control, shown only after keyboard
/// navigation. Enter and Space then click it.
pub fn focusable(control: Stateful<gpui::Div>) -> Stateful<gpui::Div> {
    control.tab_index(0).focus_visible(|s| s.outline(px(2.), theme::focus_stroke(), px(1.), Some(theme::focus_stroke_inner().into())))
}

/// Attaches a plain-text tooltip.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| gpui_component::tooltip::Tooltip::new(text.clone()).build(window, cx)
}

// ---- selection controls ---------------------------------------------------------

/// ToggleSwitch (40×20). The knob slides when the user flips it.
pub fn toggle(id: impl Into<SharedString>, on: bool) -> Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let knob = div().absolute().top(px(3.)).size(px(12.)).rounded_full().bg(if on { theme::accent_ink() } else { theme::text_muted() });
    // Knob travel is 20px between 3px insets of the 40px track (inside its border).
    let knob: AnyElement = if changed_recently(&id) {
        knob.with_animation(
            ElementId::Name(format!("{id}-knob-{on}").into()),
            Animation::new(Duration::from_millis(367)).with_easing(ease_decelerate()),
            move |d, t| d.left(px(if on { 3. + 20. * t } else { 23. - 20. * t })),
        )
        .into_any_element()
    } else {
        knob.left(px(if on { 23. } else { 3. })).into_any_element()
    };
    let mark_id = id.clone();
    focusable(div().id(ElementId::Name(id)))
        .relative()
        .flex_none()
        .w(px(40.))
        .h(px(20.))
        .rounded_full()
        .bg(if on { theme::accent() } else { theme::control_alt() })
        .border_1()
        .border_color(if on { theme::accent() } else { theme::ink() })
        .cursor_pointer()
        .hover(move |s| if on { s.bg(theme::accent_hi()) } else { s.bg(theme::control_hover()) })
        .child(knob)
        .on_mouse_down(MouseButton::Left, move |_, _, _| {
            mark_changed(&mark_id);
            sound::play(Sound::Click)
        })
}

/// CheckBox glyph box (20px).
pub fn checkbox(checked: bool, locked: bool) -> gpui::Div {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::RADIUS))
        .border_1()
        .border_color(if checked { gpui::transparent_black() } else { theme::ink().into() })
        .bg(match (checked, locked) {
            (true, true) => theme::text_disabled(),
            (true, false) => theme::accent(),
            _ => theme::control_alt(),
        })
        .when(checked, |d| d.child(icon(Icon::Check).size(px(12.)).text_color(theme::accent_ink())))
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

thread_local! {
    /// Where each slider's thumb is while it's being dragged, by slider id.
    /// The value is only committed when the drag ends.
    static SLIDER_PREVIEW: RefCell<Vec<(SharedString, f32)>> = const { RefCell::new(Vec::new()) };
}

fn set_preview(id: &SharedString, fraction: Option<f32>) {
    SLIDER_PREVIEW.with(|p| {
        let mut p = p.borrow_mut();
        p.retain(|(i, _)| i != id);
        if let Some(f) = fraction {
            p.push((id.clone(), f));
        }
    });
}

/// The fraction a slider is being dragged to, if it's being dragged now.
pub fn slider_preview(id: &str) -> Option<f32> {
    SLIDER_PREVIEW.with(|p| p.borrow().iter().find(|(i, _)| i == id).map(|(_, f)| *f))
}

/// What a slider shows and how it moves.
pub struct SliderSpec {
    pub id: SharedString,
    /// Current position in `0..=1`.
    pub fraction: f32,
    /// Number of steps between the ends, for snapping and the keyboard;
    /// tick marks show when there are few.
    pub steps: Option<u32>,
    /// Where the game's default sits, drawn as a notch on the rail.
    pub default: Option<f32>,
    pub accent: Rgba,
}

/// WinUI Slider: a 4px rail with an 18px thumb (its accent core grows on
/// hover, shrinks while pressed), tick marks for coarse steps, a notch at the
/// default, and the value in a tooltip above the thumb while dragging.
/// Dragging previews live; `on_commit` receives the snapped fraction once,
/// on release. Arrow keys step, Page Up/Down move ten steps, Home/End jump
/// to the ends. `format` labels a fraction for the tooltip.
pub fn slider(
    spec: SliderSpec,
    format: impl Fn(f32) -> SharedString + 'static,
    on_commit: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let SliderSpec { id, fraction, steps, default, accent } = spec;
    let steps = steps.filter(|s| *s > 0);
    let snap = move |f: f32| -> f32 {
        let f = f.clamp(0., 1.);
        match steps {
            Some(n) => (f * n as f32).round() / n as f32,
            None => f,
        }
    };
    let dragging = slider_preview(&id);
    let shown = snap(dragging.unwrap_or(fraction));
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let on_commit = Rc::new(on_commit);
    let to_fraction = move |x: Pixels, b: Bounds<Pixels>| -> f32 {
        if b.size.width <= px(0.) {
            return 0.;
        }
        snap((x - b.left()) / b.size.width)
    };

    let bounds_for_canvas = bounds.clone();
    let bounds_for_down = bounds.clone();
    let (down_id, move_id, up_id, out_id, key_id) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
    let (commit_up, commit_out, commit_key) = (on_commit.clone(), on_commit.clone(), on_commit);
    let end_drag = move |id: &SharedString, commit: &Rc<dyn Fn(f32, &mut Window, &mut App)>, window: &mut Window, cx: &mut App| {
        if let Some(f) = slider_preview(id) {
            set_preview(id, None);
            commit(f, window, cx);
            window.refresh();
        }
    };
    let end_up = end_drag.clone();
    let commit_up: Rc<dyn Fn(f32, &mut Window, &mut App)> = commit_up;
    let commit_out: Rc<dyn Fn(f32, &mut Window, &mut App)> = commit_out;

    // Tick marks under the rail when there are few enough to read.
    let ticks = steps.filter(|n| *n <= 12).map(|n| {
        let mut row = div().absolute().left(px(9.)).right(px(9.)).top(px(22.)).h(px(4.));
        for i in 0..=n {
            row = row.child(div().absolute().left(relative(i as f32 / n as f32)).ml(px(-0.5)).w(px(1.)).h_full().bg(theme::ink()));
        }
        row
    });

    div()
        .id(ElementId::Name(id.clone()))
        .group(id.clone())
        .relative()
        .flex_1()
        .min_w(px(120.))
        .h(px(32.))
        .cursor_pointer()
        .tab_index(0)
        .focus_visible(|s| s.outline(px(2.), theme::focus_stroke(), px(1.), Some(theme::focus_stroke_inner().into())))
        .rounded(px(theme::RADIUS))
        .child(canvas(move |b, _, _| bounds_for_canvas.set(b), |_, _, _, _| {}).absolute().size_full())
        // The rail spans between the thumb's centers at either end.
        .child(
            div()
                .absolute()
                .left(px(9.))
                .right(px(9.))
                .top(px(14.))
                .h(px(4.))
                .rounded(px(2.))
                .bg(theme::ink())
                .child(div().h_full().rounded(px(2.)).w(relative(shown)).bg(accent))
                .when_some(default, |d, f| {
                    d.child(
                        div()
                            .absolute()
                            .left(relative(f.clamp(0., 1.)))
                            .ml(px(-1.))
                            .top(px(-3.))
                            .w(px(2.))
                            .h(px(10.))
                            .rounded(px(1.))
                            .bg(theme::text_dim()),
                    )
                }),
        )
        .children(ticks)
        .child(
            div().absolute().left(px(9.)).right(px(9.)).top(px(7.)).h(px(18.)).child(
                div()
                    .absolute()
                    .left(relative(shown))
                    .ml(px(-9.))
                    .size(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(theme::control_solid())
                    .border_1()
                    .border_color(theme::control_stroke())
                    .shadow(theme::shadow_card())
                    .child(
                        div()
                            .size(px(if dragging.is_some() { 10. } else { 12. }))
                            .rounded_full()
                            .bg(accent)
                            .when(dragging.is_none(), |d| d.group_hover(id.clone(), |s| s.size(px(14.)))),
                    )
                    // The value, above the thumb, while dragging.
                    .when(dragging.is_some(), |d| {
                        d.child(
                            div().absolute().bottom(px(26.)).left(px(-40.)).w(px(98.)).flex().justify_center().child(
                                div()
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(theme::RADIUS))
                                    .backdrop_blur(px(theme::ACRYLIC_BLUR))
                                    .bg(theme::acrylic())
                                    .border_1()
                                    .border_color(theme::flyout_stroke())
                                    .shadow(theme::shadow())
                                    .text_size(px(12.))
                                    .line_height(px(16.))
                                    .text_color(theme::text())
                                    .whitespace_nowrap()
                                    .child(format(shown)),
                            ),
                        )
                    }),
            ),
        )
        .on_mouse_down(MouseButton::Left, move |ev, window, _| {
            let f = to_fraction(ev.position.x, bounds_for_down.get());
            set_preview(&down_id, Some(f));
            window.refresh();
        })
        .on_drag(SliderDrag(move_id.clone()), |_, _, _, cx| cx.new(|_| EmptyView))
        .on_drag_move::<SliderDrag>(move |ev, window, cx| {
            if ev.drag(cx).0 == move_id {
                let f = to_fraction(ev.event.position.x, ev.bounds);
                if slider_preview(&move_id) != Some(f) {
                    set_preview(&move_id, Some(f));
                    window.refresh();
                }
            }
        })
        .on_mouse_up(MouseButton::Left, move |_, window, cx| end_up(&up_id, &commit_up, window, cx))
        .on_mouse_up_out(MouseButton::Left, move |_, window, cx| end_drag(&out_id, &commit_out, window, cx))
        .on_key_down(move |ev, window, cx| {
            let unit = steps.map_or(0.01, |n| 1. / n as f32);
            let target = match ev.keystroke.key.as_str() {
                "left" | "down" => shown - unit,
                "right" | "up" => shown + unit,
                "pagedown" => shown - unit * 10.,
                "pageup" => shown + unit * 10.,
                "home" => 0.,
                "end" => 1.,
                _ => return,
            };
            cx.stop_propagation();
            let target = snap(target);
            if target != shown {
                mark_changed(&key_id);
                commit_key(target, window, cx);
            }
        })
        .into_any_element()
}

/// One option of a set, drawn as a ToggleButton (accent when selected).
pub fn chip(id: impl Into<ElementId>, text: impl Into<SharedString>, selected: bool) -> Stateful<gpui::Div> {
    focusable(div().id(id.into()))
        .flex_none()
        .px(px(12.))
        .h(px(32.))
        .flex()
        .items_center()
        .rounded(px(theme::RADIUS))
        .border_1()
        .border_color(if selected { gpui::transparent_black() } else { theme::control_stroke().into() })
        .bg(if selected { theme::accent() } else { theme::control() })
        .text_color(if selected { theme::accent_ink() } else { theme::text() })
        .text_size(px(14.))
        .cursor_pointer()
        .hover(move |s| if selected { s.bg(theme::accent_hi()) } else { s.bg(theme::control_hover()) })
        .active(move |s| if selected { s.bg(theme::accent_pressed()) } else { s.bg(theme::control_pressed()) })
        .child(text.into())
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// Segmented control: `items` are (id, label, selected). The selected segment
/// is raised and marked with an accent pill.
pub fn segmented(items: Vec<(SharedString, SharedString, bool)>) -> (gpui::Div, Vec<Stateful<gpui::Div>>) {
    let wrap = div()
        .flex()
        .flex_none()
        .items_center()
        .p(px(2.))
        .gap(px(2.))
        .rounded(px(theme::RADIUS + 2.))
        .bg(theme::control_alt())
        .border_1()
        .border_color(theme::card_stroke());
    let segments = items
        .into_iter()
        .map(|(id, text, selected)| {
            focusable(div().id(ElementId::Name(id)))
                .relative()
                .flex()
                .items_center()
                .h(px(28.))
                .px(px(12.))
                .rounded(px(theme::RADIUS))
                .text_size(px(14.))
                .when(selected, |d| {
                    d.bg(theme::control())
                        .shadow(theme::shadow_card())
                        .child(div().absolute().bottom(px(1.)).left(relative(0.5)).ml(px(-8.)).w(px(16.)).h(px(3.)).rounded_full().bg(theme::accent()))
                })
                .text_color(if selected { theme::text() } else { theme::text_muted() })
                .cursor_pointer()
                .when(!selected, |d| d.hover(|s| s.bg(theme::panel_hi()).text_color(theme::text())))
                .child(text)
                .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
        })
        .collect();
    (wrap, segments)
}

/// Indeterminate ProgressRing: an accent arc that grows, shrinks and spins.
pub fn progress_ring(id: impl Into<SharedString>, size: f32) -> AnyElement {
    let id: SharedString = id.into();
    let color = theme::accent();
    let stroke = (size / 8.).max(2.);
    let ring = div().size(px(size)).flex_none();
    if !theme::motion() {
        return ring.child(icon(Icon::Loader).size(px(size)).text_color(color)).into_any_element();
    }
    ring.with_animation(ElementId::Name(id), Animation::new(Duration::from_millis(2000)).repeat(), move |d, t| {
        d.child(
            canvas(|_, _, _| {}, move |bounds, _, window, _| {
                let r = (bounds.size.width.min(bounds.size.height) - px(stroke)) / 2.;
                let c = bounds.center();
                let sweep = 0.1 + 0.6 * (std::f32::consts::PI * t).sin();
                let start = std::f32::consts::TAU * (t * 1.5);
                let end = start + std::f32::consts::TAU * sweep;
                let at = |a: f32| point(c.x + r * a.cos(), c.y + r * a.sin());
                let mut path = PathBuilder::stroke(px(stroke));
                path.move_to(at(start));
                path.arc_to(point(r, r), px(0.), sweep > 0.5, true, at(end));
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            })
            .size_full(),
        )
    })
    .into_any_element()
}

/// Heading above a group of cards (Body Strong, optional caption).
pub fn section_title(title: impl Into<SharedString>, subtitle: Option<SharedString>) -> gpui::Div {
    div().flex().flex_col().gap(px(2.)).child(label(title)).when_some(subtitle, |d, s| d.child(caption(s)))
}

pub fn kv_row(key: impl Into<SharedString>, value: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .gap(px(12.))
        .items_start()
        .py(px(3.))
        .child(div().w(px(120.)).flex_none().text_size(px(12.)).line_height(px(16.)).text_color(theme::text_muted()).child(key.into()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_family(theme::font_mono())
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(theme::text())
                .child(value.into()),
        )
}

/// A SettingsCard row: title + caption on the left, control on the right.
pub fn setting_row(title_text: impl Into<SharedString>, description: impl Into<SharedString>, control: AnyElement) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .min_h(px(68.))
        .px(px(16.))
        .py(px(12.))
        .child(div().flex_1().min_w_0().flex().flex_col().child(title(title_text)).child(caption(description)))
        .child(div().flex_none().child(control))
}

#[cfg(test)]
mod tests {
    use super::cubic_bezier;

    #[test]
    fn bezier_hits_the_ends_and_decelerates() {
        let ease = cubic_bezier(0., 0., 0., 1.);
        assert!(ease(0.).abs() < 1e-3);
        assert!((ease(1.) - 1.).abs() < 1e-3);
        // A decelerate curve is well past halfway at the midpoint.
        assert!(ease(0.5) > 0.8);
        let linear = cubic_bezier(0.25, 0.25, 0.75, 0.75);
        assert!((linear(0.3) - 0.3).abs() < 1e-2);
    }
}
