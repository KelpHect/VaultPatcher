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

/// [`entrance`] for pages that fill the content area and scroll their own
/// panes: the wrapper takes the full height so their scroll views work.
pub fn entrance_fill(key: impl Into<SharedString>, offset: f32, content: impl IntoElement) -> AnyElement {
    let key: SharedString = key.into();
    let wrapper = div().flex_1().min_h_0().size_full().flex().flex_col().relative().child(content);
    if !theme::motion() {
        return wrapper.into_any_element();
    }
    wrapper
        .with_animation(ElementId::Name(key), Animation::new(Duration::from_millis(300)).with_easing(ease_entrance()), move |d, t| {
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
    button_if(true, id, text, icon_glyph, variant)
}

/// [`button`] that is disabled unless `enabled`: WinUI's disabled fills and
/// text, no hover or press, no clicks or Tab stop. Tooltips still show.
pub fn button_if(
    enabled: bool,
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    icon_glyph: Option<Icon>,
    variant: Variant,
) -> Stateful<gpui::Div> {
    if !enabled {
        let (bg, fg, border): (Rgba, Rgba, Rgba) = match variant {
            Variant::Primary => (theme::accent_disabled(), theme::accent_ink_disabled(), gpui::transparent_black().into()),
            Variant::Secondary => (theme::control_disabled(), theme::text_disabled(), theme::control_stroke()),
            Variant::Ghost => (gpui::transparent_black().into(), theme::text_disabled(), gpui::transparent_black().into()),
        };
        return div()
            .id(id.into())
            .inert(true)
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
            .when_some(icon_glyph, |d, i| d.child(icon(i).text_color(fg)))
            .child(text.into());
    }
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

/// [`icon_button`] that is disabled unless `enabled`.
pub fn icon_button_if(enabled: bool, id: impl Into<ElementId>, glyph: Icon, color: Rgba) -> Stateful<gpui::Div> {
    if enabled {
        return icon_button(id, glyph, color);
    }
    div()
        .id(id.into())
        .inert(true)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(32.))
        .rounded(px(theme::RADIUS))
        .child(icon(glyph).text_color(theme::text_disabled()))
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

/// [`focusable`] for full-width rows inside a card: the focus ring is drawn
/// just inside the row (like ListViewItem), so the card's rounded clip
/// doesn't cut it off.
pub fn focusable_row(row: Stateful<gpui::Div>) -> Stateful<gpui::Div> {
    row.tab_index(0).focus_visible(|s| s.outline(px(2.), theme::focus_stroke(), px(-3.), None))
}

/// Attaches a plain-text tooltip.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| gpui_component::tooltip::Tooltip::new(text.clone()).build(window, cx)
}

// ---- selection controls ---------------------------------------------------------

/// ToggleSwitch: "On"/"Off" then the 40×20 track, the whole thing clickable.
/// The knob is 12px, 14 on hover, stretches to 17×14 while pressed, and
/// slides over 367 ms when flipped.
pub fn toggle(id: impl Into<SharedString>, on: bool) -> Stateful<gpui::Div> {
    toggle_if(true, id, on)
}

/// [`toggle`] that is disabled unless `enabled`.
pub fn toggle_if(enabled: bool, id: impl Into<SharedString>, on: bool) -> Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let group = SharedString::from(format!("{id}-toggle"));
    let knob_color = match (enabled, on) {
        (true, true) => theme::accent_ink(),
        (true, false) => theme::text_muted(),
        (false, true) => theme::accent_ink_disabled(),
        (false, false) => theme::text_disabled(),
    };
    // The knob sits 3px in from the track's inner edge; travel is 20px.
    let rest = |on: bool| if on { 23. } else { 3. };
    let knob = div()
        .id(SharedString::from(format!("{id}-knob")))
        .absolute()
        .top(px(3.))
        .size(px(12.))
        .rounded(px(6.))
        .bg(knob_color)
        .when(enabled, |d| {
            d.group_hover(group.clone(), |s| s.size(px(14.)).top(px(2.)).ml(px(-1.)))
                .group_active(group.clone(), |s| s.w(px(17.)).h(px(14.)).top(px(2.)).ml(px(if on { -4. } else { -1. })))
        });
    let knob: AnyElement = if enabled && changed_recently(&id) {
        knob.with_animation(
            ElementId::Name(format!("{id}-knob-{on}").into()),
            Animation::new(Duration::from_millis(367)).with_easing(ease_entrance()),
            move |d, t| d.left(px(rest(!on) + (rest(on) - rest(!on)) * t)),
        )
        .into_any_element()
    } else {
        knob.left(px(rest(on))).into_any_element()
    };
    let (track, stroke) = match (enabled, on) {
        (true, true) => (theme::accent(), theme::accent()),
        (true, false) => (theme::control_alt(), theme::ink()),
        (false, true) => (theme::accent_disabled(), theme::accent_disabled()),
        (false, false) => (gpui::transparent_black().into(), theme::ink_disabled()),
    };
    let mark_id = id.clone();
    let wrap = div().id(ElementId::Name(id)).group(group.clone());
    let wrap = if enabled { focusable(wrap) } else { wrap.inert(true) };
    wrap.flex()
        .flex_none()
        .items_center()
        .gap(px(12.))
        .h(px(32.))
        .rounded(px(theme::RADIUS))
        .when(enabled, |d| d.cursor_pointer())
        .child(
            div()
                .min_w(px(24.))
                .text_size(px(14.))
                .line_height(px(20.))
                .text_color(if enabled { theme::text() } else { theme::text_disabled() })
                .child(if on { "On" } else { "Off" }),
        )
        .child(
            div()
                .relative()
                .flex_none()
                .w(px(40.))
                .h(px(20.))
                .rounded_full()
                .bg(track)
                .border_1()
                .border_color(stroke)
                .when(enabled, |d| {
                    d.group_hover(group.clone(), move |s| if on { s.bg(theme::accent_hi()) } else { s.bg(theme::panel_hi()) })
                })
                .child(knob),
        )
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

/// Per-slider interaction state that must outlive a frame.
#[derive(Clone, Copy, Default)]
struct SliderState {
    /// Where the thumb is while dragging (committed on release).
    preview: Option<f32>,
    /// Pointer offset from the thumb center when the thumb itself was grabbed.
    grab: f32,
    /// Esc pressed during this drag: ignore the rest of it.
    cancelled: bool,
    /// Show the value tooltip until then (after keyboard steps).
    tip_until: Option<Instant>,
}

thread_local! {
    static SLIDERS: RefCell<Vec<(SharedString, SliderState)>> = const { RefCell::new(Vec::new()) };
}

fn slider_state(id: &str) -> SliderState {
    SLIDERS.with(|s| s.borrow().iter().find(|(i, _)| i == id).map(|(_, st)| *st).unwrap_or_default())
}

fn update_slider(id: &SharedString, f: impl FnOnce(&mut SliderState)) {
    SLIDERS.with(|s| {
        let mut s = s.borrow_mut();
        let entry = match s.iter().position(|(i, _)| i == id) {
            Some(i) => i,
            None => {
                s.push((id.clone(), SliderState::default()));
                s.len() - 1
            }
        };
        f(&mut s[entry].1);
    });
}

/// The fraction a slider is being dragged to, if it's being dragged now.
pub fn slider_preview(id: &str) -> Option<f32> {
    slider_state(id).preview
}

/// What a slider shows and how it moves.
pub struct SliderSpec {
    pub id: SharedString,
    /// Current position in `0..=1`.
    pub fraction: f32,
    /// Number of steps between the ends, for snapping and the keyboard;
    /// tick marks show when there are few.
    pub steps: Option<u32>,
    /// Where the game's default sits: a dot under the rail, the magnet while
    /// dragging, and what double-click / Delete reset to.
    pub default: Option<f32>,
    /// A recommended stretch of the rail (fractions), tinted with the accent.
    pub recommended: Option<(f32, f32)>,
    pub accent: Rgba,
}

/// A slider's commit callback, shared by its pointer and key handlers.
type Commit = Rc<dyn Fn(f32, &mut Window, &mut App)>;

/// Rail inset: the thumb's radius, so its center reaches both rail ends.
const THUMB_R: f32 = 9.;

/// WinUI Slider: a 4px rail with an 18px thumb (its accent core is 10px,
/// 14 on hover, 8.5 while pressed), tick marks for coarse steps, a dot at the
/// game's default, and the value in a tooltip while dragging.
///
/// - Dragging previews live; `on_commit` receives the snapped fraction once,
///   on release. Grabbing the thumb doesn't move it; Esc cancels a drag.
///   Near the default the thumb snaps onto it; hold Alt to move freely.
/// - Keys: arrows step (Shift ×10, Ctrl ¼ step), Page Up/Down ×10, Home/End,
///   Delete resets to the default. Double-click resets too.
/// - `on_start` runs when the user starts interacting (to select its row);
///   `format` labels a fraction for the tooltip.
pub fn slider(
    spec: SliderSpec,
    on_start: impl Fn(&mut Window, &mut App) + 'static,
    format: impl Fn(f32) -> SharedString + 'static,
    on_commit: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let SliderSpec { id, fraction, steps, default, recommended, accent } = spec;
    let steps = steps.filter(|s| *s > 0);
    let snap = move |f: f32| -> f32 {
        let f = f.clamp(0., 1.);
        match steps {
            Some(n) => (f * n as f32).round() / n as f32,
            None => f,
        }
    };
    let state = slider_state(&id);
    let dragging = state.preview.is_some();
    let shown = snap(state.preview.unwrap_or(fraction));
    let show_tip = dragging || state.tip_until.is_some_and(|t| Instant::now() < t);
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let on_start = Rc::new(on_start);
    let on_commit: Commit = Rc::new(on_commit);

    // Pointer → fraction along the rail (inset by the thumb radius), with
    // the default acting as a magnet within 4px unless Alt is held.
    let to_fraction = move |x: Pixels, b: Bounds<Pixels>, free: bool| -> f32 {
        let width = b.size.width - px(THUMB_R * 2.);
        if width <= px(0.) {
            return 0.;
        }
        let raw = ((x - b.left() - px(THUMB_R)) / width).clamp(0., 1.);
        if free {
            return raw;
        }
        if let Some(d) = default
            && ((raw - d) * f32::from(width)).abs() <= 4.
        {
            return d;
        }
        snap(raw)
    };

    let thumb_x = {
        let b = bounds.clone();
        move |f: f32| {
            let b = b.get();
            b.left() + px(THUMB_R) + (b.size.width - px(THUMB_R * 2.)) * f
        }
    };

    let bounds_for_canvas = bounds.clone();
    let bounds_down = bounds.clone();
    let (down_id, move_id, up_id, out_id, key_id) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
    let (commit_down, commit_up, commit_out, commit_key) = (on_commit.clone(), on_commit.clone(), on_commit.clone(), on_commit);
    let end_drag = |id: &SharedString, commit: &Commit, window: &mut Window, cx: &mut App| {
        let state = slider_state(id);
        update_slider(id, |s| {
            s.preview = None;
            s.cancelled = false;
        });
        if let Some(f) = state.preview.filter(|_| !state.cancelled) {
            commit(f, window, cx);
        }
        window.refresh();
    };

    // Tick marks under the rail when there are few enough to read.
    let ticks = steps.filter(|n| *n <= 12).map(|n| {
        let mut row = div().absolute().left(px(THUMB_R)).right(px(THUMB_R)).top(px(21.)).h(px(4.));
        for i in 0..=n {
            row = row.child(
                div().absolute().left(relative(i as f32 / n as f32)).ml(px(-0.5)).w(px(1.)).h_full().bg(theme::with_alpha(theme::ink(), 0.6)),
            );
        }
        row
    });
    let core = if dragging { 8.5 } else { 10. };

    div()
        .id(ElementId::Name(id.clone()))
        .group(id.clone())
        .relative()
        .flex_1()
        .min_w(px(160.))
        .h(px(32.))
        .cursor_pointer()
        .tab_index(0)
        .focus_visible(|s| s.outline(px(2.), theme::focus_stroke(), px(1.), Some(theme::focus_stroke_inner().into())))
        .rounded(px(theme::RADIUS))
        .child(canvas(move |b, _, _| bounds_for_canvas.set(b), |_, _, _, _| {}).absolute().size_full())
        .child(
            div()
                .absolute()
                .left(px(THUMB_R))
                .right(px(THUMB_R))
                .top(px(14.))
                .h(px(4.))
                .rounded(px(2.))
                .bg(theme::ink())
                .when_some(recommended, |d, (lo, hi)| {
                    let (lo, hi) = (lo.clamp(0., 1.), hi.clamp(0., 1.));
                    d.child(
                        div()
                            .absolute()
                            .top_0()
                            .h_full()
                            .left(relative(lo))
                            .w(relative((hi - lo).max(0.)))
                            .min_w(px(4.))
                            .rounded(px(2.))
                            .bg(theme::with_alpha(accent, 0.24)),
                    )
                })
                .child(div().h_full().rounded(px(2.)).w(relative(shown)).bg(accent)),
        )
        .children(ticks)
        .when_some(default, |d, f| {
            d.child(div().absolute().left(px(THUMB_R)).right(px(THUMB_R)).top(px(25.)).h(px(4.)).child(
                div().absolute().left(relative(f.clamp(0., 1.))).ml(px(-2.)).size(px(4.)).rounded_full().bg(theme::text_muted()),
            ))
        })
        .child(
            div().absolute().left(px(THUMB_R)).right(px(THUMB_R)).top(px(7.)).h(px(18.)).child(
                div()
                    .absolute()
                    .left(relative(shown))
                    .ml(px(-THUMB_R))
                    .size(px(THUMB_R * 2.))
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
                            .size(px(core))
                            .rounded_full()
                            .bg(accent)
                            .when(!dragging, |d| d.group_hover(id.clone(), |s| s.size(px(14.)))),
                    )
                    .when(show_tip, |d| {
                        d.child(
                            div().absolute().bottom(px(26.)).left(px(-40.)).w(px(98.)).flex().justify_center().child(
                                div()
                                    .px(px(9.))
                                    .pt(px(6.))
                                    .pb(px(8.))
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
        .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
            on_start(window, cx);
            let b = bounds_down.get();
            // Double-click resets to the game's default.
            if ev.click_count >= 2
                && let Some(d) = default
            {
                update_slider(&down_id, |s| *s = SliderState::default());
                commit_down(d, window, cx);
                window.refresh();
                return;
            }
            let at_thumb = thumb_x(shown);
            let grabbed = (ev.position.x - at_thumb).abs() <= px(THUMB_R);
            let f = if grabbed { shown } else { to_fraction(ev.position.x, b, ev.modifiers.alt) };
            let grab = if grabbed { f32::from(ev.position.x - at_thumb) } else { 0. };
            update_slider(&down_id, |s| {
                s.preview = Some(f);
                s.grab = grab;
                s.cancelled = false;
            });
            window.refresh();
        })
        .on_drag(SliderDrag(move_id.clone()), |_, _, _, cx| cx.new(|_| EmptyView))
        .on_drag_move::<SliderDrag>(move |ev, window, cx| {
            if ev.drag(cx).0 != move_id {
                return;
            }
            let state = slider_state(&move_id);
            if state.cancelled {
                return;
            }
            let f = to_fraction(ev.event.position.x - px(state.grab), ev.bounds, ev.event.modifiers.alt);
            if state.preview != Some(f) {
                update_slider(&move_id, |s| s.preview = Some(f));
                window.refresh();
            }
        })
        .on_mouse_up(MouseButton::Left, move |_, window, cx| end_drag(&up_id, &commit_up, window, cx))
        .on_mouse_up_out(MouseButton::Left, move |_, window, cx| end_drag(&out_id, &commit_out, window, cx))
        .on_key_down(move |ev, window, cx| {
            let key = ev.keystroke.key.as_str();
            let mods = ev.keystroke.modifiers;
            if key == "escape" && slider_state(&key_id).preview.is_some() {
                cx.stop_propagation();
                update_slider(&key_id, |s| {
                    s.preview = None;
                    s.cancelled = true;
                });
                window.refresh();
                return;
            }
            let step = steps.map_or(0.01, |n| 1. / n as f32);
            let unit = if mods.shift {
                step * 10.
            } else if mods.control && steps.is_none() {
                step / 4.
            } else {
                step
            };
            let target = match key {
                "left" | "down" => shown - unit,
                "right" | "up" => shown + unit,
                "pagedown" => shown - step * 10.,
                "pageup" => shown + step * 10.,
                "home" => 0.,
                "end" => 1.,
                "delete" | "backspace" => match default {
                    Some(d) => d,
                    None => return,
                },
                _ => return,
            };
            cx.stop_propagation();
            let target = if matches!(key, "delete" | "backspace") { target } else { snap(target) };
            if target == shown {
                return;
            }
            // Show the value for a second after each step.
            let until = Instant::now() + Duration::from_secs(1);
            update_slider(&key_id, |s| s.tip_until = Some(until));
            commit_key(target, window, cx);
            window
                .spawn(cx, async move |cx| {
                    cx.background_executor().timer(Duration::from_millis(1050)).await;
                    cx.update(|window, _| window.refresh()).ok();
                })
                .detach();
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

/// InfoBar: the status disc glyph, a bold title and a wrapping message on the
/// severity background. Add an action button with `.child(..)`.
pub fn info_bar(severity: Severity, title: impl Into<SharedString>, message: impl Into<SharedString>) -> gpui::Div {
    let (bg, color, _) = severity.style();
    let glyph = match severity {
        Severity::Info => Icon::StatusInfo,
        Severity::Success => Icon::StatusSuccess,
        Severity::Warning => Icon::StatusWarning,
        Severity::Error => Icon::StatusError,
    };
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .min_h(px(48.))
        .px(px(16.))
        .py(px(12.))
        .rounded(px(theme::RADIUS))
        .border_1()
        .border_color(theme::card_stroke())
        .bg(bg)
        .child(
            div()
                .relative()
                .size(px(16.))
                .flex_none()
                .mt(px(2.))
                .child(icon(Icon::StatusDisc).absolute().inset_0().text_color(color))
                .child(icon(glyph).absolute().inset_0().text_color(theme::text_inverse())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(px(14.)).line_height(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::text()).child(title.into()))
                .child(div().text_size(px(14.)).line_height(px(20.)).text_color(theme::text()).child(message.into())),
        )
}

/// WinUI ProgressBar: a 3px bar over a 1px track. `None` is indeterminate:
/// a segment sweeping across every 2 s.
pub fn progress_bar(id: impl Into<SharedString>, fraction: Option<f32>, color: Rgba) -> AnyElement {
    let track = div().relative().w_full().h(px(3.)).flex().items_center().child(div().w_full().h(px(1.)).bg(theme::ink()));
    let bar = |d: gpui::Div| d.absolute().top_0().h(px(3.)).rounded(px(1.5)).bg(color);
    match fraction {
        Some(f) => track.child(bar(div()).left_0().w(relative(f.clamp(0., 1.)))).into_any_element(),
        None if !theme::motion() => track.child(bar(div()).left(relative(0.3)).w(relative(0.4))).into_any_element(),
        None => {
            let id: SharedString = id.into();
            track
                .overflow_hidden()
                .child(bar(div()).w(relative(0.4)).with_animation(
                    ElementId::Name(id),
                    Animation::new(Duration::from_secs(2)).repeat().with_easing(ease_entrance()),
                    |d, t| d.left(relative(-0.4 + 1.4 * t)),
                ))
                .into_any_element()
        }
    }
}

/// Diagonal stripes, like the hazard markings on a Borderlands vending
/// machine, drawn in `color`. `shift` (0..1) slides them one period along,
/// so a repeating animation makes them march.
pub fn stripes(color: gpui::Hsla, shift: f32) -> impl IntoElement {
    canvas(|_, _, _| {}, move |bounds, _, window, _| {
        const PERIOD: f32 = 14.;
        let h = f32::from(bounds.size.height);
        let w = f32::from(bounds.size.width);
        let mut x = -h - PERIOD + PERIOD * shift;
        while x < w + PERIOD {
            let mut path = PathBuilder::stroke(px(5.));
            path.move_to(point(bounds.origin.x + px(x), bounds.origin.y + px(h)));
            path.line_to(point(bounds.origin.x + px(x + h), bounds.origin.y));
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
            x += PERIOD;
        }
    })
    .size_full()
}

/// A comet of light on a rounded rectangle's outline, its head at `t`
/// (0..1) of the way round clockwise from the top-left. The outline is a
/// closed loop, so a repeating animation of `t` circles without a seam.
pub fn orbit(color: gpui::Hsla, t: f32, radius: f32) -> impl IntoElement {
    use std::f32::consts::{FRAC_PI_2, PI};
    canvas(|_, _, _| {}, move |bounds, _, window, _| {
        // Centred on the 1px border.
        let (x0, y0) = (f32::from(bounds.origin.x) + 0.5, f32::from(bounds.origin.y) + 0.5);
        let (w, h) = (f32::from(bounds.size.width) - 1., f32::from(bounds.size.height) - 1.);
        let r = radius.min(w / 2.).min(h / 2.);
        let mut pts: Vec<(f32, f32)> = Vec::with_capacity(48);
        let corner = |pts: &mut Vec<(f32, f32)>, cx: f32, cy: f32, from: f32| {
            for i in 0..=8 {
                let a = from + FRAC_PI_2 * i as f32 / 8.;
                pts.push((cx + r * a.cos(), cy + r * a.sin()));
            }
        };
        corner(&mut pts, x0 + w - r, y0 + r, -FRAC_PI_2);
        corner(&mut pts, x0 + w - r, y0 + h - r, 0.);
        corner(&mut pts, x0 + r, y0 + h - r, FRAC_PI_2);
        corner(&mut pts, x0 + r, y0 + r, PI);
        pts.push(pts[0]);
        let mut along = vec![0f32];
        for i in 1..pts.len() {
            let (a, b) = (pts[i - 1], pts[i]);
            along.push(along[i - 1] + ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt());
        }
        let total = *along.last().unwrap_or(&1.);
        let at = |s: f32| {
            let s = s.rem_euclid(total);
            let i = along.partition_point(|&d| d <= s).clamp(1, pts.len() - 1);
            let (a, b) = (pts[i - 1], pts[i]);
            let f = (s - along[i - 1]) / (along[i] - along[i - 1]).max(0.001);
            point(px(a.0 + (b.0 - a.0) * f), px(a.1 + (b.1 - a.1) * f))
        };
        // The tail fades out over a third of the way round.
        const PIECES: usize = 24;
        let (head, tail) = (t * total, total * 0.3);
        // A soft wide glow under a thin bright core.
        for (width, strength) in [(6., 0.3), (2., 1.)] {
            for k in 0..PIECES {
                let s1 = head - tail * k as f32 / PIECES as f32;
                let s0 = head - tail * (k + 1) as f32 / PIECES as f32;
                let mut path = PathBuilder::stroke(px(width));
                path.move_to(at(s0));
                for j in 1..=4 {
                    path.line_to(at(s0 + (s1 - s0) * j as f32 / 4.));
                }
                let mut c = color;
                c.a *= strength * (1. - k as f32 / PIECES as f32).powi(2);
                if let Ok(path) = path.build() {
                    window.paint_path(path, c);
                }
            }
        }
    })
    .size_full()
}

/// One frame of the indeterminate ProgressRing at cycle position `t`
/// (0..1), for views that drive it on their own timer.
pub fn progress_ring_frame(size: f32, t: f32) -> gpui::Div {
    let color = theme::accent();
    let stroke = (size / 8.).max(2.);
    div().size(px(size)).flex_none().child(
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
