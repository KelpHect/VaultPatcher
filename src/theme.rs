//! Visual language: native Windows 11 (Fluent / WinUI 3). Colors are the
//! WinUI theme brushes for the user's light or dark app mode, with their
//! Windows accent color; the window sits on Mica where the system supports
//! it. Loot rarity colors remain as small semantic tags.
//!
//! Every widget reads colors through the functions below, and [`apply`]
//! mirrors them into gpui-component's theme so its inputs, menus and tooltips
//! match. [`sync`] re-reads the Windows settings (on start, on a light/dark
//! switch, and whenever the window is activated, which covers accent changes).

use std::sync::RwLock;
use std::time::Duration;

use gpui::{App, BoxShadow, Hsla, Rgba, SharedString, point, px, rgb, rgba};

use crate::win11::SystemTheme;

/// Bundled fallback for PCs without the Segoe UI family (SIL Open Font License).
pub const FONT_FILES: &[&[u8]] = &[
    include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSans-SemiBold.ttf"),
    include_bytes!("../assets/fonts/NotoSans-Bold.ttf"),
];
const FONT_FALLBACK: &str = "Noto Sans";

/// ControlCornerRadius and OverlayCornerRadius.
pub const RADIUS: f32 = 4.;
pub const RADIUS_LG: f32 = 8.;

/// WinUI's ControlNormalAnimationDuration.
pub const NORMAL: Duration = Duration::from_millis(250);

struct State {
    system: SystemTheme,
    mica: bool,
    motion: bool,
    body: &'static str,
    display: &'static str,
    mono: &'static str,
}

static STATE: RwLock<State> = RwLock::new(State {
    system: SystemTheme { dark: true, accent: [0x99ebff, 0x4cc2ff, 0x0091f8, 0x0078d4, 0x005a9e, 0x003e92, 0x001a68] },
    mica: false,
    motion: true,
    body: "Segoe UI",
    display: "Segoe UI",
    mono: "Consolas",
});

fn state<T>(f: impl FnOnce(&State) -> T) -> T {
    f(&STATE.read().unwrap_or_else(|e| e.into_inner()))
}

/// Picks the UI fonts from what's installed: Segoe UI Variable (Windows 11),
/// Segoe UI (Windows 10), else the bundled Noto Sans.
pub fn pick_fonts(installed: &[String]) {
    let has = |name: &str| installed.iter().any(|f| f.eq_ignore_ascii_case(name));
    let first = |names: &[&'static str]| names.iter().copied().find(|n| has(n)).unwrap_or(FONT_FALLBACK);
    let mut s = STATE.write().unwrap_or_else(|e| e.into_inner());
    s.body = first(&["Segoe UI Variable Text", "Segoe UI"]);
    s.display = first(&["Segoe UI Variable Display", "Segoe UI Variable Text", "Segoe UI"]);
    s.mono = ["Cascadia Mono", "Consolas"].into_iter().find(|n| has(n)).unwrap_or(FONT_FALLBACK);
}

/// Body text face.
pub fn font_body() -> SharedString {
    state(|s| s.body).into()
}
/// Paths, ini keys and launch arguments.
pub fn font_mono() -> SharedString {
    state(|s| s.mono).into()
}
/// Titles (Title / Subtitle / Display ramp).
pub fn font_display() -> SharedString {
    state(|s| s.display).into()
}

/// Whether the window has the Mica backdrop (set once after it opens).
pub fn set_mica(on: bool) {
    STATE.write().unwrap_or_else(|e| e.into_inner()).mica = on;
}

/// Re-reads light/dark, accent and the animation setting from Windows.
/// Returns true when anything changed (then call [`apply`]).
pub fn sync() -> bool {
    let system = crate::win11::system_theme();
    let motion = crate::win11::animations_enabled();
    let mut s = STATE.write().unwrap_or_else(|e| e.into_inner());
    let changed = s.system != system || s.motion != motion;
    s.system = system;
    s.motion = motion;
    changed
}

pub fn is_dark() -> bool {
    state(|s| s.system.dark)
}

/// False when the user turned off Windows' "Animation effects".
pub fn motion() -> bool {
    state(|s| s.motion)
}

/// Picks the light- or dark-theme value of a brush (`0xRRGGBBAA`).
fn pick(light: u32, dark: u32) -> Rgba {
    rgba(if is_dark() { dark } else { light })
}

fn accent_shade(i: usize) -> Rgba {
    rgb(state(|s| s.system.accent[i]))
}

// ---- backgrounds and layers ----------------------------------------------------

/// Window background: transparent over Mica, else SolidBackgroundFillColorBase.
pub fn bg() -> Rgba {
    if state(|s| s.mica) { rgba(0) } else { bg_deep() }
}
/// SolidBackgroundFillColorBase: opaque base, scrims behind text on art.
pub fn bg_deep() -> Rgba {
    pick(0xf3f3f3ff, 0x202020ff)
}
/// LayerFillColorDefault: the content area that sits on Mica.
pub fn layer() -> Rgba {
    pick(0xffffff80, 0x3a3a3a4c)
}
/// CardBackgroundFillColorDefault: grouped rows and cards.
pub fn panel() -> Rgba {
    pick(0xffffffb3, 0xffffff0d)
}
/// SubtleFillColorSecondary: hovered rows and list items.
pub fn panel_hi() -> Rgba {
    pick(0x00000009, 0xffffff0f)
}
/// SubtleFillColorTertiary: pressed rows and list items.
pub fn panel_pressed() -> Rgba {
    pick(0x00000006, 0xffffff0a)
}
/// Selected list/nav item background (SubtleFillColorSecondary).
pub fn selected() -> Rgba {
    panel_hi()
}
/// CardBackgroundFillColorSecondary: wells, code boxes, image backdrops.
pub fn panel_lo() -> Rgba {
    pick(0xf6f6f680, 0xffffff08)
}
/// ControlFillColorDefault / Secondary / Tertiary: standard buttons, inputs.
pub fn control() -> Rgba {
    pick(0xffffffb3, 0xffffff0f)
}
pub fn control_hover() -> Rgba {
    pick(0xf9f9f980, 0xffffff15)
}
pub fn control_pressed() -> Rgba {
    pick(0xf9f9f94d, 0xffffff08)
}
/// ControlSolidFillColorDefault: slider thumbs.
pub fn control_solid() -> Rgba {
    pick(0xffffffff, 0x454545ff)
}
/// ControlAltFillColorSecondary: the "off" toggle track, slider rails' wells.
pub fn control_alt() -> Rgba {
    pick(0x00000006, 0x00000019)
}
/// Acrylic tint for menus, tooltips and toasts, painted over a blurred
/// backdrop (AcrylicBackgroundFillColorDefault: #FCFCFC / #2C2C2C tints).
pub fn acrylic() -> Rgba {
    pick(0xfcfcfcd0, 0x2c2c2cd0)
}
/// Acrylic blur strength (the Fluent recipe's 30px gaussian).
pub const ACRYLIC_BLUR: f32 = 30.;
/// ContentDialog body (LayerFillColorAlt over the solid base).
pub fn dialog() -> Rgba {
    pick(0xffffffff, 0x2b2b2bff)
}
/// SmokeFillColorDefault: dims the window behind a dialog.
pub fn smoke() -> Rgba {
    rgba(0x0000004d)
}

// ---- strokes ------------------------------------------------------------------

/// ControlStrongStrokeColorDefault: unchecked boxes, toggles, slider rails.
pub fn ink() -> Rgba {
    pick(0x00000072, 0xffffff8b)
}
/// DividerStrokeColorDefault: dividers and hairlines.
pub fn line() -> Rgba {
    pick(0x0000000f, 0xffffff15)
}
/// CardStrokeColorDefault: the outline of cards and the content layer.
pub fn card_stroke() -> Rgba {
    pick(0x0000000f, 0x00000019)
}
/// ControlStrokeColorDefault: outline of buttons and inputs.
pub fn control_stroke() -> Rgba {
    pick(0x0000000f, 0xffffff12)
}
/// ControlStrokeColorSecondary: the bottom edge of buttons (elevation).
pub fn control_stroke_bottom() -> Rgba {
    pick(0x00000029, 0xffffff18)
}
/// SurfaceStrokeColorFlyout: menus, tooltips, dialogs.
pub fn flyout_stroke() -> Rgba {
    pick(0x0000000f, 0x00000033)
}
/// FocusStrokeColorOuter: keyboard focus rectangle.
pub fn focus_stroke() -> Rgba {
    pick(0x000000e4, 0xffffffff)
}
/// FocusStrokeColorInner: the thin ring inside it.
pub fn focus_stroke_inner() -> Rgba {
    pick(0xffffffb3, 0x000000b3)
}

// ---- text ---------------------------------------------------------------------

/// TextFillColorPrimary.
pub fn text() -> Rgba {
    pick(0x000000e4, 0xffffffff)
}
/// TextFillColorSecondary.
pub fn text_muted() -> Rgba {
    pick(0x0000009e, 0xffffffc5)
}
/// TextFillColorTertiary.
pub fn text_dim() -> Rgba {
    pick(0x00000072, 0xffffff87)
}
/// TextFillColorDisabled.
pub fn text_disabled() -> Rgba {
    pick(0x0000005c, 0xffffff5d)
}
/// TextFillColorInverse: glyphs drawn on a status disc.
pub fn text_inverse() -> Rgba {
    pick(0xffffffff, 0x000000e4)
}
/// ControlFillColorDisabled.
pub fn control_disabled() -> Rgba {
    pick(0xf9f9f94d, 0xffffff0b)
}
/// AccentFillColorDisabled.
pub fn accent_disabled() -> Rgba {
    pick(0x00000037, 0xffffff28)
}
/// TextOnAccentFillColorDisabled.
pub fn accent_ink_disabled() -> Rgba {
    pick(0xffffffff, 0xffffff87)
}
/// ControlStrongStrokeColorDisabled.
pub fn ink_disabled() -> Rgba {
    pick(0x00000037, 0xffffff28)
}

// ---- accent -------------------------------------------------------------------

/// AccentFillColorDefault: primary buttons, toggles on, selection pill.
pub fn accent() -> Rgba {
    if is_dark() { accent_shade(1) } else { accent_shade(4) }
}
/// AccentFillColorSecondary (hover): the default at 90% opacity.
pub fn accent_hi() -> Rgba {
    Rgba { a: 0.9, ..accent() }
}
/// AccentFillColorTertiary (pressed): the default at 80% opacity.
pub fn accent_pressed() -> Rgba {
    Rgba { a: 0.8, ..accent() }
}
/// TextOnAccentFillColorPrimary.
pub fn accent_ink() -> Rgba {
    pick(0xffffffff, 0x000000ff)
}
/// AccentTextFillColorPrimary: links and accent-colored text.
pub fn accent_text() -> Rgba {
    if is_dark() { accent_shade(0) } else { accent_shade(5) }
}

// ---- status -------------------------------------------------------------------

/// SystemFillColorCritical.
pub fn danger() -> Rgba {
    pick(0xc42b1cff, 0xff99a4ff)
}
/// SystemFillColorCaution.
pub fn warning() -> Rgba {
    pick(0x9d5d00ff, 0xfce100ff)
}
/// SystemFillColorSuccess.
pub fn success() -> Rgba {
    pick(0x0f7b0fff, 0x6ccb5fff)
}
/// Informational accents (SystemFillColorAttention).
pub fn echo() -> Rgba {
    accent_text()
}
/// InfoBar backgrounds: SystemFillColor{Critical,Caution,Success,Attention}Background.
pub fn danger_bg() -> Rgba {
    pick(0xfde7e9ff, 0x442726ff)
}
pub fn warning_bg() -> Rgba {
    pick(0xfff4ceff, 0x433519ff)
}
pub fn success_bg() -> Rgba {
    pick(0xdff6ddff, 0x393d1bff)
}
pub fn info_bg() -> Rgba {
    pick(0xf6f6f680, 0xffffff08)
}
/// Caption close button, hovered and pressed.
pub fn close_hover() -> Rgba {
    rgb(0xc42b1c)
}
pub fn close_pressed() -> Rgba {
    rgba(0xc42b1ce6)
}

/// Loot rarity tiers, reused as small tags (presets, cost meters).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Pearlescent,
    Seraph,
}

impl Rarity {
    pub fn color(self) -> Rgba {
        // Darker shades on light backgrounds so tags stay readable.
        let (light, dark) = match self {
            Rarity::Common => (0x5f5d58, 0xd9d6cf),
            Rarity::Uncommon => (0x2e7d2d, 0x5bc85a),
            Rarity::Rare => (0x1f5fc2, 0x4a8ff5),
            Rarity::Epic => (0x7a3fc9, 0xa66bf5),
            Rarity::Legendary => (0xb35a00, 0xf59131),
            Rarity::Pearlescent => (0x0f7f76, 0x4fd6c9),
            Rarity::Seraph => (0xb8306f, 0xf06aa9),
        };
        rgb(if is_dark() { dark } else { light })
    }

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
            Rarity::Pearlescent => "Pearlescent",
            Rarity::Seraph => "Seraph",
        }
    }
}

pub fn with_alpha(color: Rgba, a: f32) -> Hsla {
    let mut c: Hsla = color.into();
    c.a = a;
    c
}

/// Flyout elevation (menus, tooltips, toasts): a soft ambient + key shadow.
pub fn shadow() -> Vec<BoxShadow> {
    let a = if is_dark() { 0.26 } else { 0.14 };
    vec![
        BoxShadow { color: gpui::hsla(0., 0., 0., a), offset: point(px(0.), px(8.)), blur_radius: px(16.), spread_radius: px(0.) },
        BoxShadow { color: gpui::hsla(0., 0., 0., a * 0.6), offset: point(px(0.), px(0.)), blur_radius: px(2.), spread_radius: px(0.) },
    ]
}

/// Dialog elevation (ContentDialog sits highest).
pub fn shadow_dialog() -> Vec<BoxShadow> {
    let a = if is_dark() { 0.37 } else { 0.19 };
    vec![
        BoxShadow { color: gpui::hsla(0., 0., 0., a), offset: point(px(0.), px(32.)), blur_radius: px(64.), spread_radius: px(0.) },
        BoxShadow { color: gpui::hsla(0., 0., 0., a * 0.5), offset: point(px(0.), px(2.)), blur_radius: px(21.), spread_radius: px(0.) },
    ]
}

/// Card/button elevation: the 1px darker bottom edge is drawn as a border,
/// this adds the barely-there drop.
pub fn shadow_card() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: gpui::hsla(0., 0., 0., if is_dark() { 0.13 } else { 0.04 }),
        offset: point(px(0.), px(1.)),
        blur_radius: px(2.),
        spread_radius: px(0.),
    }]
}

/// Mirrors the brushes into gpui-component's theme (inputs, menus, tooltips,
/// dialogs, scrollbars).
pub fn apply(cx: &mut App) {
    let h = |c: Rgba| -> Hsla { c.into() };
    let mode = if is_dark() { gpui_component::ThemeMode::Dark } else { gpui_component::ThemeMode::Light };
    gpui_component::Theme::change(mode, None, cx);
    // Hover and press fades follow Windows' "Animation effects" too.
    gpui::set_reduce_motion(!motion());
    let theme = gpui_component::Theme::global_mut(cx);
    theme.font_family = font_body();
    theme.font_size = px(14.);
    theme.mono_font_family = font_mono();
    theme.radius = px(RADIUS);
    theme.radius_lg = px(RADIUS_LG);
    theme.shadow = true;
    let c = &mut theme.colors;
    c.background = h(bg());
    c.foreground = h(text());
    c.border = h(control_stroke());
    c.input = h(control_stroke_bottom());
    c.ring = h(focus_stroke());
    c.caret = h(text());
    c.selection = with_alpha(accent(), 0.4);
    c.muted = h(control());
    c.muted_foreground = h(text_muted());
    c.primary = h(accent());
    c.primary_hover = h(accent_hi());
    c.primary_active = h(accent_pressed());
    c.primary_foreground = h(accent_ink());
    c.secondary = h(control());
    c.secondary_hover = h(control_hover());
    c.secondary_active = h(control_pressed());
    c.secondary_foreground = h(text());
    c.accent = h(panel_hi());
    c.accent_foreground = h(text());
    c.popover = h(acrylic());
    c.popover_foreground = h(text());
    c.list = gpui::transparent_black();
    c.list_hover = h(panel_hi());
    c.list_active = h(selected());
    c.list_active_border = h(accent());
    c.list_even = gpui::transparent_black();
    c.list_head = gpui::transparent_black();
    c.table = gpui::transparent_black();
    c.table_head = h(panel_lo());
    c.table_head_foreground = h(text_muted());
    c.table_hover = h(panel_hi());
    c.table_active = h(selected());
    c.table_active_border = h(accent());
    c.table_even = gpui::transparent_black();
    c.table_row_border = h(line());
    c.title_bar = gpui::transparent_black();
    c.title_bar_border = gpui::transparent_black();
    c.sidebar = gpui::transparent_black();
    c.sidebar_foreground = h(text());
    c.sidebar_border = h(line());
    c.sidebar_accent = h(panel_hi());
    c.sidebar_accent_foreground = h(text());
    c.sidebar_primary = h(accent());
    c.sidebar_primary_foreground = h(accent_ink());
    c.switch = h(ink());
    c.switch_thumb = h(text());
    c.slider_bar = h(accent());
    c.slider_thumb = h(accent());
    c.progress_bar = h(accent());
    c.tab = gpui::transparent_black();
    c.tab_active = h(panel());
    c.tab_active_foreground = h(text());
    c.tab_bar = gpui::transparent_black();
    c.tab_foreground = h(text_muted());
    c.link = h(accent_text());
    c.link_hover = with_alpha(accent_text(), 0.8);
    c.link_active = with_alpha(accent_text(), 0.6);
    c.danger = h(danger());
    c.danger_hover = h(danger());
    c.danger_active = h(danger());
    c.danger_foreground = h(accent_ink());
    c.success = h(success());
    c.success_hover = h(success());
    c.success_active = h(success());
    c.success_foreground = h(accent_ink());
    c.warning = h(warning());
    c.warning_hover = h(warning());
    c.warning_active = h(warning());
    c.warning_foreground = h(accent_ink());
    c.info = h(accent());
    c.info_hover = h(accent_hi());
    c.info_active = h(accent_pressed());
    c.info_foreground = h(accent_ink());
    c.scrollbar = gpui::transparent_black();
    c.scrollbar_thumb = h(ink());
    c.scrollbar_thumb_hover = h(text_muted());
    c.overlay = h(smoke());
    c.window_border = h(card_stroke());
    c.drop_target = with_alpha(accent(), 0.12);
    c.drag_border = h(accent());
}

/// Segoe Fluent Icons glyphs, drawn from the installed system font (see
/// [`crate::win11::glyph_svg`]). Kept as a palette for pages to pick from.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Home,
    Settings,
    Display,
    Picture,
    Sparkle,
    Contrast,
    Texture,
    Atom,
    Speed,
    Camera,
    Mouse,
    Hud,
    Game,
    Rocket,
    Shield,
    Package,
    Download,
    Upload,
    Folder,
    History,
    Save,
    Undo,
    Refresh,
    Play,
    Warning,
    Check,
    CheckCircle,
    Close,
    Info,
    Delete,
    Add,
    Minus,
    Link,
    Code,
    Wrench,
    Lock,
    Unlock,
    Star,
    Sliders,
    Volume,
    Globe,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    Minimize,
    Maximize,
    Restore,
    WindowClose,
    Music,
    Mute,
    Terminal,
    Puzzle,
    Search,
    Copy,
    Compare,
    Filter,
    Bookmark,
    FileDown,
    FileUp,
    Bug,
    Health,
    Profile,
    Keyboard,
    More,
    Help,
    Dashed,
    Alert,
    Wand,
    Edit,
    Loader,
    Expand,
    Back,
    Hamburger,
    /// InfoBar status disc and the glyphs drawn on it.
    StatusDisc,
    StatusInfo,
    StatusSuccess,
    StatusWarning,
    StatusError,
}

impl Icon {
    /// Segoe Fluent Icons code point.
    pub fn glyph(self) -> char {
        let cp: u32 = match self {
            Icon::Home => 0xE80F,
            Icon::Settings => 0xE713,
            Icon::Display => 0xE7F4,
            Icon::Picture => 0xE8B9,
            Icon::Sparkle => 0xE794,
            Icon::Contrast => 0xE706,
            Icon::Texture => 0xF0E2,
            Icon::Atom => 0xE945,
            Icon::Speed => 0xF42F,
            Icon::Camera => 0xE722,
            Icon::Mouse => 0xE7C9,
            Icon::Hud => 0xE81E,
            Icon::Game => 0xE7FC,
            Icon::Rocket => 0xE8AD,
            Icon::Shield => 0xF760,
            Icon::Package => 0xE7B8,
            Icon::Download => 0xE896,
            Icon::Upload => 0xE898,
            Icon::Folder => 0xE838,
            Icon::History => 0xE81C,
            Icon::Save => 0xE74E,
            Icon::Undo => 0xE7A7,
            Icon::Refresh => 0xE72C,
            Icon::Play => 0xE768,
            Icon::Warning => 0xE7BA,
            Icon::Check => 0xE73E,
            Icon::CheckCircle => 0xE930,
            Icon::Close => 0xE711,
            Icon::Info => 0xE946,
            Icon::Delete => 0xE74D,
            Icon::Add => 0xE710,
            Icon::Minus => 0xE738,
            Icon::Link => 0xE8A7,
            Icon::Code => 0xE756,
            Icon::Wrench => 0xE90F,
            Icon::Lock => 0xE72E,
            Icon::Unlock => 0xE785,
            Icon::Star => 0xE734,
            Icon::Sliders => 0xE9E9,
            Icon::Volume => 0xE767,
            Icon::Globe => 0xE774,
            Icon::ChevronRight => 0xE76C,
            Icon::ChevronDown => 0xE70D,
            Icon::ChevronUp => 0xE70E,
            Icon::ChevronLeft => 0xE76B,
            Icon::Minimize => 0xE921,
            Icon::Maximize => 0xE922,
            Icon::Restore => 0xE923,
            Icon::WindowClose => 0xE8BB,
            Icon::Music => 0xEC4F,
            Icon::Mute => 0xE74F,
            Icon::Terminal => 0xE756,
            Icon::Puzzle => 0xEA86,
            Icon::Search => 0xE721,
            Icon::Copy => 0xE8C8,
            Icon::Compare => 0xE8AB,
            Icon::Filter => 0xE71C,
            Icon::Bookmark => 0xE8EC,
            Icon::FileDown => 0xE78C,
            Icon::FileUp => 0xE8E5,
            Icon::Bug => 0xEBE8,
            Icon::Health => 0xE95E,
            Icon::Profile => 0xEF58,
            Icon::Keyboard => 0xE765,
            Icon::More => 0xE712,
            Icon::Help => 0xE9CE,
            Icon::Dashed => 0xF16A,
            Icon::Alert => 0xE783,
            Icon::Wand => 0xF1D5,
            Icon::Edit => 0xE70F,
            Icon::Loader => 0xF16A,
            Icon::Expand => 0xE740,
            Icon::Back => 0xE72B,
            Icon::Hamburger => 0xE700,
            Icon::StatusDisc => 0xF136,
            Icon::StatusInfo => 0xF13F,
            Icon::StatusSuccess => 0xF13E,
            Icon::StatusWarning => 0xF13C,
            Icon::StatusError => 0xF13D,
        };
        char::from_u32(cp).unwrap_or(' ')
    }

    /// Stand-in for glyphs Segoe MDL2 Assets (Windows 10) lacks.
    fn fallback(self) -> Option<char> {
        let cp: u32 = match self {
            Icon::Sparkle | Icon::Wand => 0xE90F,
            Icon::Speed => 0xEC4A,
            Icon::Shield => 0xEA18,
            _ => return None,
        };
        char::from_u32(cp)
    }

    /// Asset path the SVG pipeline loads (served by [`Assets`]).
    pub fn path(self) -> SharedString {
        match self.fallback() {
            Some(f) => format!("fluent/{:04X}-{:04X}.svg", self.glyph() as u32, f as u32).into(),
            None => format!("fluent/{:04X}.svg", self.glyph() as u32).into(),
        }
    }
}

/// gpui-component's built-in icons redirected to their Segoe Fluent
/// equivalents, so menus, inputs and dialogs match the rest of the app.
fn component_icon(name: &str) -> Option<Icon> {
    Some(match name {
        "check.svg" => Icon::Check,
        "chevron-down.svg" => Icon::ChevronDown,
        "chevron-up.svg" => Icon::ChevronUp,
        "chevron-left.svg" => Icon::ChevronLeft,
        "chevron-right.svg" => Icon::ChevronRight,
        "close.svg" | "circle-x.svg" => Icon::Close,
        "search.svg" => Icon::Search,
        "info.svg" => Icon::Info,
        "circle-check.svg" => Icon::CheckCircle,
        "triangle-alert.svg" => Icon::Warning,
        "minus.svg" | "dash.svg" => Icon::Minus,
        "plus.svg" => Icon::Add,
        "copy.svg" => Icon::Copy,
        "ellipsis.svg" | "ellipsis-vertical.svg" => Icon::More,
        "external-link.svg" => Icon::Link,
        "folder.svg" | "folder-open.svg" | "folder-closed.svg" => Icon::Folder,
        "settings.svg" | "settings-2.svg" => Icon::Settings,
        "globe.svg" => Icon::Globe,
        "star.svg" => Icon::Star,
        "window-minimize.svg" => Icon::Minimize,
        "window-maximize.svg" => Icon::Maximize,
        "window-restore.svg" => Icon::Restore,
        "window-close.svg" => Icon::WindowClose,
        "maximize.svg" => Icon::Expand,
        "undo.svg" | "undo-2.svg" => Icon::Undo,
        "menu.svg" => Icon::Hamburger,
        _ => return None,
    })
}

/// Serves the Segoe Fluent glyphs (`fluent/…`), the brand art (`brand/…`) and
/// gpui-component's assets (`icons/…`, redirected to Fluent where we can).
pub struct Assets;

#[derive(rust_embed::RustEmbed)]
#[folder = "assets/brand"]
#[include = "*.svg"]
struct Brand;

/// The app logo (full colour), for `img()`.
pub const LOGO: &str = "brand/logo.svg";
/// Simplified logo for small sizes (title bar).
pub const LOGO_SMALL: &str = "brand/logo-small.svg";

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if let Some(hex) = path.strip_prefix("fluent/").and_then(|p| p.strip_suffix(".svg")) {
            // "E794" or "E794-E90F": the glyph, then a stand-in for older fonts.
            let svg = hex
                .split('-')
                .filter_map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32))
                .find_map(crate::win11::glyph_svg);
            return Ok(svg.map(std::borrow::Cow::Owned));
        }
        if let Some(name) = path.strip_prefix("brand/") {
            return Ok(Brand::get(name).map(|f| f.data));
        }
        if let Some(icon) = path.strip_prefix("icons/").and_then(component_icon)
            && let Some(svg) = crate::win11::glyph_svg(icon.glyph())
        {
            return Ok(Some(std::borrow::Cow::Owned(svg)));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}
