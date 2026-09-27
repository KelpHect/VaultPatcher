//! Settings-page controls built on `ui`, drawn to the WinUI 3 specs:
//! NumberBox, RangeSlider, the choice select, the
//! FPS-cost meter, the row status dot and reset button, selection cards and
//! the 32px TextBox frame.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, Entity, FontWeight, IntoElement, MouseButton, ParentElement, Pixels,
    Rgba, ScrollDelta, SharedString, Stateful, Styled, Subscription, Window, canvas, div, prelude::*, px, relative,
};
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};

use crate::sound::{self, Sound};
use crate::theme::{self, Icon};
use crate::tweaks::{Control, Impact, Opt};
use crate::ui;

// ---- small pieces -----------------------------------------------------------------

/// A values-table row with the value in body text (paths and keys keep the
/// monospace `ui::kv_row`).
pub fn kv_row_text(key: impl Into<SharedString>, value: impl Into<SharedString>, color: Rgba) -> gpui::Div {
    div()
        .flex()
        .gap(px(12.))
        .items_baseline()
        .py(px(2.))
        .child(div().w(px(112.)).flex_none().text_size(px(12.)).line_height(px(20.)).text_color(theme::text_muted()).child(key.into()))
        .child(div().flex_1().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(color).child(value.into()))
}

/// One line of caption text that ends in an ellipsis when it doesn't fit.
/// gpui measures no-wrap text once, before flex layout settles its width,
/// and keeps that size, so `truncate()` text in a flex item is clipped
/// instead of shortened. Wrapping text is re-measured whenever its width
/// changes, so this wraps and clamps to one line, which ends in "…". Give
/// it a tooltip with the full text.
pub fn caption_line(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<gpui::Div> {
    let text: SharedString = text.into();
    div()
        .id(id.into())
        .min_w_0()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(theme::text_muted())
        .line_clamp(1)
        .text_ellipsis()
        .child(text)
}

/// The row's status:an accent dot while a change waits to be applied, a
/// grey one when the value differs from the game's default. Sits in the
/// row's left padding and names itself in a tooltip.
pub fn status_dot(id: impl Into<ElementId>, waiting: bool, changed: bool) -> Option<AnyElement> {
    let (color, tip) = if waiting {
        (theme::accent(), "Waiting to apply")
    } else if changed {
        (theme::text_muted(), "Changed from the game's default")
    } else {
        return None;
    };
    Some(
        div()
            .id(id.into())
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .child(div().size(px(6.)).rounded_full().bg(color))
            .tooltip(ui::tip(tip))
            .into_any_element(),
    )
}

/// Level (0–3), color and word for an FPS cost.
fn cost(impact: Impact) -> Option<(usize, Rgba, &'static str)> {
    match impact {
        Impact::None => None,
        Impact::Low => Some((1, theme::text_muted(), "Low")),
        Impact::Medium => Some((2, theme::warning(), "Medium")),
        Impact::High => Some((3, theme::danger(), "High")),
    }
}

fn meter_bars(level: usize, color: Rgba) -> gpui::Div {
    div().flex_none().flex().items_center().gap(px(2.)).children((1..=3).map(move |i| {
        div().w(px(3.)).h(px(10.)).rounded(px(1.)).bg(if i <= level { color } else { Rgba::from(theme::with_alpha(theme::ink(), 0.3)) })
    }))
}

/// Three bars showing how much FPS a setting costs, with a tooltip.
pub fn cost_meter(id: impl Into<ElementId>, impact: Impact) -> Option<AnyElement> {
    let (level, color, word) = cost(impact)?;
    Some(
        div()
            .id(id.into())
            .flex_none()
            .h(px(20.))
            .px(px(2.))
            .flex()
            .items_center()
            .child(meter_bars(level, color))
            .tooltip(ui::tip(format!("{word} FPS cost")))
            .into_any_element(),
    )
}

/// The detail pane's neutral "FPS cost: Medium" tag with the meter inside.
pub fn cost_badge(impact: Impact) -> Option<gpui::Div> {
    let (level, color, word) = cost(impact)?;
    Some(
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(20.))
            .px(px(8.))
            .rounded(px(theme::RADIUS))
            .bg(theme::with_alpha(theme::text_muted(), if theme::is_dark() { 0.16 } else { 0.12 }))
            .text_color(theme::text())
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(12.))
            .child(format!("FPS cost: {word}"))
            .child(meter_bars(level, color)),
    )
}

/// The row's reset-to-default button: a 24px subtle Undo that shows while
/// the row (`group`) is hovered or the button has keyboard focus.
pub fn reset_button(id: impl Into<ElementId>, group: impl Into<SharedString>, tooltip: impl Into<SharedString>) -> Stateful<gpui::Div> {
    let group: SharedString = group.into();
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(24.))
        .rounded(px(theme::RADIUS))
        .cursor_pointer()
        .opacity(0.)
        .group_hover(group, |s| s.opacity(1.))
        .tab_index(0)
        .focus_visible(|s| s.opacity(1.).outline(px(2.), theme::focus_stroke(), px(1.), Some(theme::focus_stroke_inner().into())))
        .hover(|s| s.bg(theme::panel_hi()))
        .active(|s| s.bg(theme::panel_pressed()))
        .child(ui::icon(Icon::Undo).size(px(12.)).text_color(theme::text_muted()))
        .tooltip(ui::tip(tooltip))
        .on_mouse_down(MouseButton::Left, |_, _, _| sound::play(Sound::Click))
}

/// RadioButton glyph (20px): accent with a 12px dot when checked.
pub fn radio(checked: bool) -> gpui::Div {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(if checked { theme::accent() } else { theme::ink() })
        .bg(if checked { theme::accent() } else { theme::control_alt() })
        .when(checked, |d| d.child(div().size(px(12.)).rounded_full().bg(theme::accent_ink())))
}

/// Makes text use tabular (fixed-width) digits so values don't jiggle.
pub fn tabular<E: Styled>(mut e: E) -> E {
    e.text_style().get_or_insert_with(Default::default).font_features =
        Some(gpui::FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)])));
    e
}

// ---- choices ----------------------------------------------------------------------

/// Called with the picked option's value.
pub type OnPick = Rc<dyn Fn(&'static str, &mut Window, &mut App)>;

/// Choices of 3 to 6 options are quality tiers (every game's list reads
/// lowest to highest), so they get a level meter like a game's menu.
fn tier_count(options: &[Opt]) -> Option<usize> {
    (3..=6).contains(&options.len()).then_some(options.len())
}

/// A level meter: `count` short bars, the first `level + 1` lit.
fn level_meter(level: Option<usize>, count: usize, lit: Rgba) -> gpui::Div {
    div().flex().flex_none().items_end().gap(px(2.)).children((0..count).map(move |i| {
        let on = level.is_some_and(|l| i <= l);
        div()
            .w(px(4.))
            .h(px(6. + 6. * i as f32 / (count - 1).max(1) as f32))
            .rounded(px(1.))
            .bg(if on { lit } else { theme::control_stroke_bottom() })
    }))
}

/// The select for every pick-one setting: a WinUI ComboBox face with the
/// current value, a level meter for quality tiers and a chevron; clicking
/// opens the options in the app's acrylic menu, the current one checked and
/// each tier with its own meter. `fill` stretches it to the pane's width.
pub fn select(id: impl Into<ElementId>, current: String, options: &'static [Opt], selected: Option<&'static str>, fill: bool, on_pick: OnPick, cx: &App) -> AnyElement {
    use gpui_component::button::{ButtonCustomVariant, ButtonVariants as _};
    let tiers = tier_count(options);
    let level = selected.and_then(|v| options.iter().position(|o| o.value == v));
    let face = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().flex_1().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(theme::text()).truncate().child(current))
        .children(tiers.map(|count| level_meter(level, count, theme::accent())))
        .child(ui::icon(Icon::ChevronDown).size(px(12.)).text_color(theme::text_muted()));
    Button::new(id)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(theme::control().into())
                .foreground(theme::text().into())
                .border(theme::control_stroke().into())
                .hover(theme::control_hover().into())
                .active(theme::control_pressed().into()),
        )
        .content_fill()
        .h(px(32.))
        .px(px(12.))
        .map(|b| if fill { b.w_full() } else { b.w(px(220.)) })
        .border_b_1()
        .border_color(theme::control_stroke_bottom())
        .child(face)
        .dropdown_menu(move |mut menu, _, _| {
            for (i, o) in options.iter().enumerate() {
                let pick = on_pick.clone();
                let v = o.value;
                let label = o.label;
                menu = menu.item(
                    PopupMenuItem::element(move |_, _| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .min_w(px(180.))
                            .child(div().flex_1().text_size(px(14.)).child(label))
                            .children(tiers.map(|count| level_meter(Some(i), count, theme::text_muted())))
                    })
                    .checked(selected == Some(o.value))
                    .on_click(move |_, window, cx| {
                        sound::play(Sound::Click);
                        pick(v, window, cx)
                    }),
                );
            }
            menu
        })
        .into_any_element()
}

// ---- TextBox frame ------------------------------------------------------------------

/// The WinUI TextBox look around a borderless `Input`: 32px, control fill,
/// 1px stroke and, while focused, a 2px accent line along the bottom (red
/// when `invalid`).
fn text_frame(focused: bool, invalid: bool) -> gpui::Div {
    let underline = if invalid { Some(theme::danger()) } else if focused { Some(theme::accent()) } else { None };
    div()
        .relative()
        .flex()
        .items_center()
        .h(px(32.))
        .rounded(px(theme::RADIUS))
        .overflow_hidden()
        .bg(if focused { theme::control_solid() } else { theme::control() })
        .border_1()
        .border_color(theme::control_stroke())
        .when(!focused, |d| d.hover(|s| s.bg(theme::control_hover())))
        .when_some(underline, |d, c| d.child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(c)))
}

/// A single-line text box (search) at the TextBox spec.
pub fn text_box(input: &Entity<InputState>, prefix: Option<AnyElement>, window: &Window, cx: &App) -> gpui::Div {
    let focused = gpui::Focusable::focus_handle(input.read(cx), cx).is_focused(window);
    text_frame(focused, false).child(
        Styled::h_full(Input::new(input).appearance(false).cleanable(true))
            .flex_1()
            .min_w_0()
            .pl(px(10.))
            .py(px(0.))
            .pr(px(4.))
            .text_size(px(14.))
            .when_some(prefix, |d, p| d.prefix(p)),
    )
}

// ---- NumberBox ----------------------------------------------------------------------

/// Called with a new value.
pub type OnValue = Rc<dyn Fn(f64, &mut Window, &mut App)>;

/// What a NumberBox's event handlers need; refreshed every render.
struct BoxModel {
    value: f64,
    control: Control,
    on_commit: OnValue,
}

struct NumberBoxEntry {
    input: Entity<InputState>,
    model: Rc<RefCell<BoxModel>>,
    invalid: Rc<Cell<bool>>,
    _sub: Subscription,
}


/// One step up or down from what the box shows (Shift: ten steps).
fn step_value(model: &BoxModel, text: &str, up: bool, big: bool) -> f64 {
    let Control::Slider { step, min, max, .. } = model.control else { return model.value };
    let step = if step > 0.0 { step } else { (max - min) / 100.0 };
    let from = model.control.parse_number(text).unwrap_or(model.value);
    let by = step * if big { 10.0 } else { 1.0 };
    model.control.clamp_num(if up { from + by } else { from - by })
}

/// Parses and commits what was typed. Returns false if it isn't a number.
fn commit_text(input: &Entity<InputState>, model: &Rc<RefCell<BoxModel>>, window: &mut Window, cx: &mut App) -> bool {
    let text = input.read(cx).value().to_string();
    let (parsed, control) = {
        let m = model.borrow();
        (m.control.parse_number(&text), m.control)
    };
    let Some(n) = parsed else { return false };
    let n = control.clamp_num(n);
    let shown = control.number_text(n);
    input.update(cx, |s, cx| s.set_value(shown, window, cx));
    let (changed, on_commit) = {
        let m = model.borrow();
        ((m.value - n).abs() > 1e-9, m.on_commit.clone())
    };
    if changed {
        model.borrow_mut().value = n;
        on_commit(n, window, cx);
    }
    true
}

fn revert(input: &Entity<InputState>, model: &Rc<RefCell<BoxModel>>, window: &mut Window, cx: &mut App) {
    let shown = {
        let m = model.borrow();
        m.control.number_text(m.value)
    };
    input.update(cx, |s, cx| s.set_value(shown, window, cx));
}

fn step_and_commit(input: &Entity<InputState>, model: &Rc<RefCell<BoxModel>>, up: bool, big: bool, window: &mut Window, cx: &mut App) {
    let text = input.read(cx).value().to_string();
    let (n, on_commit, control, old) = {
        let m = model.borrow();
        (step_value(&m, &text, up, big), m.on_commit.clone(), m.control, m.value)
    };
    input.update(cx, |s, cx| s.set_value(control.number_text(n), window, cx));
    if (n - old).abs() > 1e-9 {
        model.borrow_mut().value = n;
        on_commit(n, window, cx);
    }
}

/// The box's input and model, kept in the element's own state: GPUI drops it
/// the first frame the box isn't drawn, so a page you leave takes its text
/// inputs with it instead of keeping one per setting ever shown.
fn entry(id: &SharedString, value: f64, control: Control, on_commit: OnValue, window: &mut Window, cx: &mut App) -> (Entity<InputState>, Rc<RefCell<BoxModel>>, Rc<Cell<bool>>) {
    let init_commit = on_commit.clone();
    let state = window.use_keyed_state(ElementId::Name(format!("{id}-state").into()), cx, move |window, cx| {
        let text = control.number_text(value);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let model = Rc::new(RefCell::new(BoxModel { value, control, on_commit: init_commit }));
        let invalid = Rc::new(Cell::new(false));
        let (sub_model, sub_invalid) = (model.clone(), invalid.clone());
        let sub = window.subscribe(&input, cx, move |input, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => {
                let ok = commit_text(&input, &sub_model, window, cx);
                sub_invalid.set(!ok);
                window.refresh();
            }
            InputEvent::Blur => {
                if !commit_text(&input, &sub_model, window, cx) {
                    revert(&input, &sub_model, window, cx);
                }
                sub_invalid.set(false);
                window.refresh();
            }
            InputEvent::Change if sub_invalid.get() => {
                sub_invalid.set(false);
                window.refresh();
            }
            _ => {}
        });
        NumberBoxEntry { input, model, invalid, _sub: sub }
    });
    let e = state.read(cx);
    *e.model.borrow_mut() = BoxModel { value, control, on_commit };
    (e.input.clone(), e.model.clone(), e.invalid.clone())
}

/// WinUI NumberBox: type a value and press Enter (or leave the box); Up/Down
/// and the mouse wheel step (Shift ×10), Esc reverts. Compact spin chevrons
/// show on hover. `value` is what the box shows when it isn't being edited.
pub fn number_box(id: impl Into<SharedString>, value: f64, control: Control, on_commit: OnValue, window: &mut Window, cx: &mut App) -> AnyElement {
    let id: SharedString = id.into();
    let (input, model, invalid) = entry(&id, value, control, on_commit, window, cx);
    let focused = gpui::Focusable::focus_handle(input.read(cx), cx).is_focused(window);
    // Follow outside changes (slider drags, resets) unless someone is typing.
    let want = control.number_text(value);
    if !focused && input.read(cx).value().as_ref() != want {
        input.update(cx, |s, cx| s.set_value(want, window, cx));
        invalid.set(false);
    }
    let Control::Slider { min, max, unit, .. } = control else { return div().into_any_element() };
    let shows_label = control.slider_label(value).is_some() && !focused;
    let group = SharedString::from(format!("{id}-nb"));
    let range_tip = format!("Enter {}–{}", control.number_text(min), control.number_text(max));

    let key_input = input.clone();
    let key_model = model.clone();
    let key_invalid = invalid.clone();
    let wheel_input = input.clone();
    let wheel_model = model.clone();
    let spin = |up: bool| {
        let input = input.clone();
        let model = model.clone();
        div()
            .id(if up { "up" } else { "down" })
            .w(px(16.))
            .h(px(14.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(2.))
            .cursor_pointer()
            .hover(|s| s.bg(theme::panel_hi()))
            .active(|s| s.bg(theme::panel_pressed()))
            .child(ui::icon(if up { Icon::ChevronUp } else { Icon::ChevronDown }).size(px(8.)).text_color(theme::text_muted()))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |ev, window, cx| {
                cx.stop_propagation();
                step_and_commit(&input, &model, up, ev.modifiers().shift, window, cx);
            })
    };

    let frame = text_frame(focused, invalid.get())
        .w_full()
        .min_w(px(72.))
        .child(
            Styled::h_full(Input::new(&input).appearance(false))
                .flex_1()
                .min_w_0()
                .pl(px(10.))
                .py(px(0.))
                .pr(px(2.))
                .text_size(px(14.)),
        )
        .when(!unit.is_empty() && !shows_label, |d| {
            d.child(div().flex_none().pr(px(4.)).text_size(px(14.)).text_color(theme::text_muted()).child(unit))
        })
        .child(
            div()
                .id("spin")
                .flex_none()
                .flex()
                .flex_col()
                .mr(px(4.))
                .when(!focused, |d| d.opacity(0.).group_hover(group.clone(), |s| s.opacity(1.)))
                .child(spin(true))
                .child(spin(false)),
        );
    div()
        .id(ElementId::Name(id))
        .group(group)
        .w_full()
        .child(frame)
        .when(invalid.get(), |d| d.tooltip(ui::tip(range_tip)))
        .capture_key_down(move |ev, window, cx| {
            let key = ev.keystroke.key.as_str();
            match key {
                "up" | "down" => {
                    cx.stop_propagation();
                    key_invalid.set(false);
                    step_and_commit(&key_input, &key_model, key == "up", ev.keystroke.modifiers.shift, window, cx);
                }
                "escape" => {
                    cx.stop_propagation();
                    key_invalid.set(false);
                    revert(&key_input, &key_model, window, cx);
                    window.refresh();
                }
                _ => {}
            }
        })
        .on_scroll_wheel(move |ev, window, cx| {
            // Only a focused box takes the wheel, and only whole notches, so
            // scrolling the page never changes a value by accident.
            let focused = gpui::Focusable::focus_handle(wheel_input.read(cx), cx).is_focused(window);
            let ScrollDelta::Lines(lines) = ev.delta else { return };
            if !focused || lines.y == 0. {
                return;
            }
            cx.stop_propagation();
            step_and_commit(&wheel_input, &wheel_model, lines.y > 0., ev.modifiers.shift, window, cx);
        })
        .into_any_element()
}

/// The row's read-only NumberBox look: the value right-aligned in tabular
/// digits, the unit as a Secondary suffix ("110°", "62 fps").
pub fn number_readout(number: String, unit: &'static str) -> gpui::Div {
    let hug = matches!(unit, "°" | "%" | "×");
    tabular(
        div()
            .flex_none()
            .min_w(px(72.))
            .h(px(32.))
            .px(px(10.))
            .flex()
            .items_center()
            .justify_end()
            .gap(px(if hug { 0. } else { 4. }))
            .rounded(px(theme::RADIUS))
            .bg(theme::control())
            .border_1()
            .border_color(theme::control_stroke())
            .text_size(px(14.))
            .whitespace_nowrap()
            .text_color(theme::text())
            .child(number)
            .when(!unit.is_empty(), |d| d.child(div().text_color(theme::text_muted()).child(unit))),
    )
}

// ---- labelled steps ------------------------------------------------------------------

/// Captions under each tick of an ordinal slider (inset by the thumb radius
/// like the rail): the ends align to the rail ends, the selected one is bold.
pub fn step_labels(labels: Vec<String>, selected: Option<usize>) -> gpui::Div {
    let n = labels.len().saturating_sub(1).max(1) as f32;
    let last = labels.len().saturating_sub(1);
    let mut row = div().relative().h(px(16.)).mx(px(9.));
    for (i, text) in labels.into_iter().enumerate() {
        let chosen = selected == Some(i);
        let caption = div()
            .text_size(px(12.))
            .line_height(px(16.))
            .whitespace_nowrap()
            .text_color(if chosen { theme::text() } else { theme::text_muted() })
            .when(chosen, |d| d.font_weight(FontWeight::SEMIBOLD))
            .child(text);
        let slot = div().absolute().top_0().flex();
        row = row.child(if i == 0 {
            slot.left(px(-9.)).child(caption)
        } else if i == last {
            slot.right(px(-9.)).child(caption)
        } else {
            slot.left(relative(i as f32 / n)).ml(px(-32.)).w(px(64.)).justify_center().child(caption)
        });
    }
    row
}

// ---- RangeSlider ----------------------------------------------------------------------

/// Moves one thumb of a range to `to`, inside that thumb's own bounds and
/// without crossing the other thumb. Returns the new (low, high).
pub fn move_thumb(lo: f32, hi: f32, moving_lo: bool, to: f32, lo_bounds: (f32, f32), hi_bounds: (f32, f32)) -> (f32, f32) {
    if moving_lo {
        (to.clamp(lo_bounds.0, lo_bounds.1).min(hi), hi)
    } else {
        (lo, to.clamp(hi_bounds.0, hi_bounds.1).max(lo))
    }
}

/// Picks the thumb nearer to `at`; on a tie (stacked thumbs) the side the
/// pointer is on decides.
fn nearer_is_lo(lo: f32, hi: f32, at: f32) -> bool {
    let (dl, dh) = ((at - lo).abs(), (at - hi).abs());
    if (dl - dh).abs() < 1e-4 { at < lo } else { dl < dh }
}

#[derive(Clone, Copy, Default)]
struct RangeState {
    /// Positions while dragging (committed on release).
    preview: Option<(f32, f32)>,
    /// The thumb being dragged, or last moved (for the keyboard).
    lo_active: bool,
    grab: f32,
}

thread_local! {
    static RANGES: RefCell<HashMap<SharedString, RangeState>> = RefCell::new(HashMap::new());
}

fn range_state(id: &SharedString) -> RangeState {
    RANGES.with(|r| r.borrow().get(id).copied().unwrap_or_default())
}

fn update_range(id: &SharedString, f: impl FnOnce(&mut RangeState)) {
    RANGES.with(|r| f(r.borrow_mut().entry(id.clone()).or_default()));
}

/// The (low, high) fractions a range slider is being dragged to, if any.
pub fn range_preview(id: &SharedString) -> Option<(f32, f32)> {
    range_state(id).preview
}

pub struct RangeSpec {
    pub id: SharedString,
    pub lo: f32,
    pub hi: f32,
    /// Steps across the whole rail, for snapping and the keyboard.
    pub steps: Option<u32>,
    /// Where each thumb may go (fractions of the rail).
    pub lo_bounds: (f32, f32),
    pub hi_bounds: (f32, f32),
}

/// Called with the new (low, high) fractions and whether the low end moved.
pub type OnRange = Rc<dyn Fn(f32, f32, bool, &mut Window, &mut App)>;
/// Called when the user starts interacting (to select the row).
pub type OnStart = Rc<dyn Fn(&mut Window, &mut App)>;
/// Labels a rail fraction for the drag tooltip.
pub type Format = Rc<dyn Fn(f32) -> SharedString>;

const THUMB_R: f32 = 9.;

/// WinUI-style RangeSlider: two thumbs on one rail, the span between them in
/// the accent. Drag either thumb (the nearer one is picked); they can't
/// cross. Arrows move the last-used thumb (Shift ×10). Commits on release.
pub fn range_slider(spec: RangeSpec, on_start: OnStart, format: Format, on_commit: OnRange) -> AnyElement {
    let RangeSpec { id, lo, hi, steps, lo_bounds, hi_bounds } = spec;
    let steps = steps.filter(|s| *s > 0);
    let snap = move |f: f32| match steps {
        Some(n) => (f.clamp(0., 1.) * n as f32).round() / n as f32,
        None => f.clamp(0., 1.),
    };
    let state = range_state(&id);
    let (shown_lo, shown_hi) = state.preview.unwrap_or((lo, hi));
    let dragging = state.preview.is_some();
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let to_fraction = move |x: Pixels, b: Bounds<Pixels>| -> f32 {
        let width = b.size.width - px(THUMB_R * 2.);
        if width <= px(0.) {
            return 0.;
        }
        snap((x - b.left() - px(THUMB_R)) / width)
    };
    let thumb_x = move |b: Bounds<Pixels>, f: f32| b.left() + px(THUMB_R) + (b.size.width - px(THUMB_R * 2.)) * f;

    let thumb = |f: f32, is_lo: bool| {
        let active = dragging && state.lo_active == is_lo;
        div()
            .absolute()
            .left(relative(f))
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
            .child(div().size(px(if active { 8.5 } else { 10. })).rounded_full().bg(theme::accent()))
            .when(active, |d| {
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
                            .child(format(f)),
                    ),
                )
            })
    };

    let bounds_canvas = bounds.clone();
    let bounds_down = bounds.clone();
    let (down_id, move_id, up_id, out_id, key_id) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
    let (commit_up, commit_out, commit_key) = (on_commit.clone(), on_commit.clone(), on_commit);
    let end_drag = move |id: &SharedString, commit: &OnRange, window: &mut Window, cx: &mut App| {
        let state = range_state(id);
        update_range(id, |s| s.preview = None);
        if let Some((l, h)) = state.preview
            && ((l - lo).abs() > 1e-6 || (h - hi).abs() > 1e-6)
        {
            commit(l, h, state.lo_active, window, cx);
        }
        window.refresh();
    };

    div()
        .id(ElementId::Name(id.clone()))
        .relative()
        .w_full()
        .min_w(px(160.))
        .h(px(32.))
        .cursor_pointer()
        .tab_index(0)
        .focus_visible(|s| s.outline(px(2.), theme::focus_stroke(), px(1.), Some(theme::focus_stroke_inner().into())))
        .rounded(px(theme::RADIUS))
        .child(canvas(move |b, _, _| bounds_canvas.set(b), |_, _, _, _| {}).absolute().size_full())
        .child(
            div()
                .absolute()
                .left(px(THUMB_R))
                .right(px(THUMB_R))
                .top(px(14.))
                .h(px(4.))
                .rounded(px(2.))
                .bg(theme::ink())
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .h_full()
                        .left(relative(shown_lo))
                        .w(relative((shown_hi - shown_lo).max(0.)))
                        .rounded(px(2.))
                        .bg(theme::accent()),
                ),
        )
        .child(
            div()
                .absolute()
                .left(px(THUMB_R))
                .right(px(THUMB_R))
                .top(px(7.))
                .h(px(18.))
                .child(thumb(shown_lo, true))
                .child(thumb(shown_hi, false)),
        )
        .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
            on_start(window, cx);
            let b = bounds_down.get();
            let at = to_fraction(ev.position.x, b);
            let is_lo = nearer_is_lo(lo, hi, at);
            let thumb_at = thumb_x(b, if is_lo { lo } else { hi });
            let grabbed = (ev.position.x - thumb_at).abs() <= px(THUMB_R);
            let grab = if grabbed { f32::from(ev.position.x - thumb_at) } else { 0. };
            let (l, h) = if grabbed { (lo, hi) } else { move_thumb(lo, hi, is_lo, at, lo_bounds, hi_bounds) };
            update_range(&down_id, |s| {
                s.preview = Some((l, h));
                s.lo_active = is_lo;
                s.grab = grab;
            });
            window.refresh();
        })
        .on_drag(ui::SliderDrag(move_id.clone()), |_, _, _, cx| cx.new(|_| ui::EmptyView))
        .on_drag_move::<ui::SliderDrag>(move |ev, window, cx| {
            if ev.drag(cx).0 != move_id {
                return;
            }
            let state = range_state(&move_id);
            let Some((l, h)) = state.preview else { return };
            let to = to_fraction(ev.event.position.x - px(state.grab), ev.bounds);
            let next = move_thumb(l, h, state.lo_active, to, lo_bounds, hi_bounds);
            if next != (l, h) {
                update_range(&move_id, |s| s.preview = Some(next));
                window.refresh();
            }
        })
        .on_mouse_up(MouseButton::Left, move |_, window, cx| end_drag(&up_id, &commit_up, window, cx))
        .on_mouse_up_out(MouseButton::Left, move |_, window, cx| end_drag(&out_id, &commit_out, window, cx))
        .on_key_down(move |ev, window, cx| {
            let key = ev.keystroke.key.as_str();
            if key == "escape" && range_state(&key_id).preview.is_some() {
                cx.stop_propagation();
                update_range(&key_id, |s| s.preview = None);
                window.refresh();
                return;
            }
            let step = steps.map_or(0.01, |n| 1. / n as f32) * if ev.keystroke.modifiers.shift { 10. } else { 1. };
            let delta = match key {
                "left" | "down" => -step,
                "right" | "up" => step,
                _ => return,
            };
            cx.stop_propagation();
            let is_lo = range_state(&key_id).lo_active;
            let from = if is_lo { lo } else { hi };
            let (l, h) = move_thumb(lo, hi, is_lo, snap(from + delta), lo_bounds, hi_bounds);
            if (l, h) != (lo, hi) {
                commit_key(l, h, is_lo, window, cx);
            }
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tweaks::opt;

    #[test]
    fn tiers_get_a_meter() {
        assert_eq!(tier_count(&[opt("0", "Low"), opt("1", "Medium"), opt("2", "High")]), Some(3));
        // Two options (on/off-like) or long lists (resolutions) don't.
        assert_eq!(tier_count(&[opt("a", "Classic outlines"), opt("b", "No outlines")]), None);
        assert_eq!(tier_count(&[opt("1", "1"), opt("2", "2"), opt("3", "3"), opt("4", "4"), opt("5", "5"), opt("6", "6"), opt("7", "7")]), None);
    }

    #[test]
    fn range_thumbs_cannot_cross_or_leave_their_bounds() {
        let all = (0., 1.);
        // Low end dragged past the high end stops on it.
        assert_eq!(move_thumb(0.2, 0.5, true, 0.8, all, all), (0.5, 0.5));
        // High end dragged below the low end stops on it.
        assert_eq!(move_thumb(0.2, 0.5, false, 0.1, all, all), (0.2, 0.2));
        // Each end keeps to its own range.
        assert_eq!(move_thumb(0.2, 0.5, true, 0.95, (0., 0.9), all), (0.5, 0.5));
        assert_eq!(move_thumb(0.2, 0.5, false, 0.01, all, (0.05, 1.)), (0.2, 0.2));
        assert_eq!(move_thumb(0.2, 0.9, false, 0.01, (0., 1.), (0.05, 1.)), (0.2, 0.2));
        assert_eq!(move_thumb(0.02, 0.9, false, 0.01, (0., 1.), (0.05, 1.)), (0.02, 0.05));
        assert_eq!(move_thumb(0.2, 0.5, true, 0.3, all, all), (0.3, 0.5));
    }

    #[test]
    fn the_nearer_thumb_is_picked() {
        assert!(nearer_is_lo(0.2, 0.6, 0.3));
        assert!(!nearer_is_lo(0.2, 0.6, 0.5));
        // Stacked thumbs: the pointer's side decides.
        assert!(nearer_is_lo(0.5, 0.5, 0.4));
        assert!(!nearer_is_lo(0.5, 0.5, 0.6));
    }
}
