//! Visual language: a dense, dark desktop UI (Fluent-style metrics) with
//! Borderlands hazard yellow as the single accent. The game's own artwork
//! (banner, logo, icon) carries the Borderlands identity; loot rarity colors
//! remain as small semantic tags.
//!
//! Colors come from the active [`Palette`], switchable at runtime; every
//! widget reads them through the functions below, and [`apply`] mirrors them
//! into gpui-component's theme so its inputs, menus and tooltips match.

use std::sync::atomic::{AtomicU8, Ordering};

use gpui::{App, BoxShadow, Hsla, Rgba, point, px, rgb};
use serde::{Deserialize, Serialize};

/// Page titles, game names and the wordmark.
pub const FONT_TITLE: &str = "Barlow Condensed";
/// Everything else: the Windows UI font.
pub const FONT_BODY: &str = "Segoe UI";
pub const FONT_MONO: &str = "Consolas";

/// The bundled typefaces (SIL Open Font License).
pub const FONT_FILES: &[&[u8]] = &[
    include_bytes!("../assets/fonts/BarlowCondensed-Medium.ttf"),
    include_bytes!("../assets/fonts/BarlowCondensed-SemiBold.ttf"),
    include_bytes!("../assets/fonts/BarlowCondensed-Bold.ttf"),
];

/// Corner radii: controls and surfaces.
pub const RADIUS: f32 = 4.;
pub const RADIUS_LG: f32 = 8.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Palette {
    /// Cool graphite neutrals.
    #[default]
    VaultHunter,
    /// Warm, dusty Pandora browns.
    Pandora,
    /// Hyperion corporate navy.
    Hyperion,
}

impl Palette {
    pub const ALL: [Palette; 3] = [Palette::VaultHunter, Palette::Pandora, Palette::Hyperion];

    pub fn label(self) -> &'static str {
        match self {
            Palette::VaultHunter => "Graphite",
            Palette::Pandora => "Pandora",
            Palette::Hyperion => "Hyperion",
        }
    }
}

struct Colors {
    bg: u32,
    bg_alt: u32,
    surface: u32,
    surface_hover: u32,
    surface_selected: u32,
    sunken: u32,
    border: u32,
    border_strong: u32,
    text: u32,
    text2: u32,
    text3: u32,
}

const VAULT_HUNTER: Colors = Colors {
    bg: 0x1a1b1e,
    bg_alt: 0x141517,
    surface: 0x232428,
    surface_hover: 0x2b2d31,
    surface_selected: 0x33302a,
    sunken: 0x161719,
    border: 0x34363b,
    border_strong: 0x45484e,
    text: 0xededef,
    text2: 0xa9abb2,
    text3: 0x7c7e86,
};

const PANDORA: Colors = Colors {
    bg: 0x1c1814,
    bg_alt: 0x15120f,
    surface: 0x26211b,
    surface_hover: 0x2f2921,
    surface_selected: 0x3a3122,
    sunken: 0x17140f,
    border: 0x3a3228,
    border_strong: 0x4c4235,
    text: 0xf1e9da,
    text2: 0xb8ac98,
    text3: 0x8a7f6d,
};

const HYPERION: Colors = Colors {
    bg: 0x141a26,
    bg_alt: 0x0f141e,
    surface: 0x1b2331,
    surface_hover: 0x222c3d,
    surface_selected: 0x2c3140,
    sunken: 0x10151f,
    border: 0x2a3547,
    border_strong: 0x3a4760,
    text: 0xeef2f8,
    text2: 0xa6b1c4,
    text3: 0x76849e,
};

static ACTIVE: AtomicU8 = AtomicU8::new(0);

pub fn set_palette(palette: Palette) {
    ACTIVE.store(palette as u8, Ordering::Relaxed);
}

pub fn palette() -> Palette {
    match ACTIVE.load(Ordering::Relaxed) {
        1 => Palette::Pandora,
        2 => Palette::Hyperion,
        _ => Palette::VaultHunter,
    }
}

fn colors() -> &'static Colors {
    match palette() {
        Palette::VaultHunter => &VAULT_HUNTER,
        Palette::Pandora => &PANDORA,
        Palette::Hyperion => &HYPERION,
    }
}

/// Window background.
pub fn bg() -> Rgba {
    rgb(colors().bg)
}
/// Title bar, navigation rail and status bar.
pub fn bg_deep() -> Rgba {
    rgb(colors().bg_alt)
}
/// Grouped rows and cards.
pub fn panel() -> Rgba {
    rgb(colors().surface)
}
/// Hovered rows, secondary buttons.
pub fn panel_hi() -> Rgba {
    rgb(colors().surface_hover)
}
/// Selected rows (a warm tint of the accent).
pub fn selected() -> Rgba {
    rgb(colors().surface_selected)
}
/// Wells: tracks, code boxes, image backdrops.
pub fn panel_lo() -> Rgba {
    rgb(colors().sunken)
}
/// Strong outline (inputs, unchecked controls).
pub fn ink() -> Rgba {
    rgb(colors().border_strong)
}
pub fn line() -> Rgba {
    rgb(colors().border)
}
pub fn text() -> Rgba {
    rgb(colors().text)
}
pub fn text_muted() -> Rgba {
    rgb(colors().text2)
}
pub fn text_dim() -> Rgba {
    rgb(colors().text3)
}
pub fn accent() -> Rgba {
    rgb(0xffc20e)
}
pub fn accent_hi() -> Rgba {
    rgb(0xffd04a)
}
pub fn accent_pressed() -> Rgba {
    rgb(0xe0a800)
}
pub fn accent_ink() -> Rgba {
    rgb(0x111111)
}
pub fn danger() -> Rgba {
    rgb(0xf2555a)
}
pub fn warning() -> Rgba {
    rgb(0xf5a524)
}
pub fn success() -> Rgba {
    rgb(0x4cc38a)
}
/// Links and informational accents.
pub fn echo() -> Rgba {
    rgb(0x4ea8f2)
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
        match self {
            Rarity::Common => rgb(0xd9d6cf),
            Rarity::Uncommon => rgb(0x5bc85a),
            Rarity::Rare => rgb(0x4a8ff5),
            Rarity::Epic => rgb(0xa66bf5),
            Rarity::Legendary => rgb(0xf59131),
            Rarity::Pearlescent => rgb(0x4fd6c9),
            Rarity::Seraph => rgb(0xf06aa9),
        }
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

/// Soft elevation for popups, toasts and the viewer.
pub fn shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: gpui::hsla(0., 0., 0., 0.45),
        offset: point(px(0.), px(8.)),
        blur_radius: px(24.),
        spread_radius: px(0.),
    }]
}

/// Mirrors the palette into gpui-component's theme (inputs, menus, tooltips,
/// dialogs, scrollbars).
pub fn apply(cx: &mut App) {
    let h = |c: Rgba| -> Hsla { c.into() };
    let theme = gpui_component::Theme::global_mut(cx);
    theme.font_family = FONT_BODY.into();
    theme.font_size = px(16.);
    theme.mono_font_family = FONT_MONO.into();
    theme.radius = px(RADIUS);
    theme.radius_lg = px(RADIUS_LG);
    theme.shadow = true;
    let c = &mut theme.colors;
    c.background = h(bg());
    c.foreground = h(text());
    c.border = h(line());
    c.input = h(ink());
    c.ring = h(accent());
    c.caret = h(accent());
    c.selection = with_alpha(accent(), 0.3);
    c.muted = h(panel());
    c.muted_foreground = h(text_muted());
    c.primary = h(accent());
    c.primary_hover = h(accent_hi());
    c.primary_active = h(accent_pressed());
    c.primary_foreground = h(accent_ink());
    c.secondary = h(panel_hi());
    c.secondary_hover = h(line());
    c.secondary_active = h(ink());
    c.secondary_foreground = h(text());
    c.accent = h(panel_hi());
    c.accent_foreground = h(text());
    c.popover = h(panel());
    c.popover_foreground = h(text());
    c.list = h(bg());
    c.list_hover = h(panel_hi());
    c.list_active = h(selected());
    c.list_active_border = h(accent());
    c.list_even = h(bg());
    c.list_head = h(bg_deep());
    c.table = h(bg());
    c.table_head = h(bg_deep());
    c.table_head_foreground = h(text_muted());
    c.table_hover = h(panel_hi());
    c.table_active = h(selected());
    c.table_active_border = h(accent());
    c.table_even = h(bg());
    c.table_row_border = h(line());
    c.title_bar = h(bg_deep());
    c.title_bar_border = h(line());
    c.sidebar = h(bg_deep());
    c.sidebar_foreground = h(text());
    c.sidebar_border = h(line());
    c.sidebar_accent = h(panel_hi());
    c.sidebar_accent_foreground = h(text());
    c.sidebar_primary = h(accent());
    c.sidebar_primary_foreground = h(accent_ink());
    c.switch = h(ink());
    c.switch_thumb = h(text());
    c.slider_bar = h(accent());
    c.slider_thumb = h(text());
    c.progress_bar = h(accent());
    c.tab = h(bg());
    c.tab_active = h(panel());
    c.tab_active_foreground = h(text());
    c.tab_bar = h(bg_deep());
    c.tab_foreground = h(text_muted());
    c.link = h(echo());
    c.link_hover = h(echo());
    c.link_active = h(echo());
    c.danger = h(danger());
    c.danger_hover = h(danger());
    c.danger_active = h(danger());
    c.danger_foreground = h(text());
    c.success = h(success());
    c.success_hover = h(success());
    c.success_active = h(success());
    c.success_foreground = h(accent_ink());
    c.warning = h(warning());
    c.warning_hover = h(warning());
    c.warning_active = h(warning());
    c.warning_foreground = h(accent_ink());
    c.info = h(echo());
    c.info_hover = h(echo());
    c.info_active = h(echo());
    c.info_foreground = h(accent_ink());
    c.scrollbar = gpui::transparent_black();
    c.scrollbar_thumb = with_alpha(text_dim(), 0.45);
    c.scrollbar_thumb_hover = with_alpha(text_muted(), 0.6);
    c.overlay = gpui::hsla(0., 0., 0., 0.6);
    c.window_border = h(line());
    c.drop_target = with_alpha(accent(), 0.12);
    c.drag_border = h(accent());
}

/// Lucide icons (ISC) under `assets/icons`, plus a few from
/// gpui-component's own set. Kept as a palette for pages to pick from.
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
    ChevronLeft,
    Minimize,
    Maximize,
    Restore,
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
}

impl Icon {
    pub fn path(self) -> &'static str {
        match self {
            Icon::Home => "vp/house.svg",
            Icon::Settings => "vp/settings.svg",
            Icon::Display => "vp/monitor.svg",
            Icon::Picture => "vp/image.svg",
            Icon::Sparkle => "vp/sparkles.svg",
            Icon::Contrast => "vp/sun.svg",
            Icon::Texture => "vp/layout-grid.svg",
            Icon::Atom => "vp/zap.svg",
            Icon::Speed => "vp/gauge.svg",
            Icon::Camera => "vp/camera.svg",
            Icon::Mouse => "vp/mouse-pointer-click.svg",
            Icon::Hud => "vp/layers.svg",
            Icon::Game => "vp/gamepad-2.svg",
            Icon::Rocket => "vp/rocket.svg",
            Icon::Shield => "vp/shield-check.svg",
            Icon::Package => "vp/package.svg",
            Icon::Download => "vp/download.svg",
            Icon::Upload => "vp/upload.svg",
            Icon::Folder => "vp/folder-open.svg",
            Icon::History => "vp/history.svg",
            Icon::Save => "vp/save.svg",
            Icon::Undo => "vp/undo-2.svg",
            Icon::Refresh => "vp/refresh-cw.svg",
            Icon::Play => "vp/play.svg",
            Icon::Warning => "vp/triangle-alert.svg",
            Icon::Check => "vp/check.svg",
            Icon::CheckCircle => "vp/circle-check-big.svg",
            Icon::Close => "vp/x.svg",
            Icon::Info => "vp/info.svg",
            Icon::Delete => "vp/trash-2.svg",
            Icon::Add => "vp/plus.svg",
            Icon::Minus => "vp/minus.svg",
            Icon::Link => "vp/external-link.svg",
            Icon::Code => "vp/terminal.svg",
            Icon::Wrench => "vp/wrench.svg",
            Icon::Lock => "vp/lock.svg",
            Icon::Unlock => "vp/lock-open.svg",
            Icon::Star => "vp/star.svg",
            Icon::Sliders => "vp/sliders-horizontal.svg",
            Icon::Volume => "vp/volume-2.svg",
            Icon::Globe => "icons/globe.svg",
            Icon::ChevronRight => "vp/chevron-right.svg",
            Icon::ChevronDown => "vp/chevron-down.svg",
            Icon::ChevronLeft => "vp/chevron-left.svg",
            Icon::Minimize => "icons/window-minimize.svg",
            Icon::Maximize => "icons/window-maximize.svg",
            Icon::Restore => "icons/window-restore.svg",
            Icon::Music => "vp/music.svg",
            Icon::Mute => "vp/volume-x.svg",
            Icon::Terminal => "vp/terminal.svg",
            Icon::Puzzle => "vp/puzzle.svg",
            Icon::Search => "vp/search.svg",
            Icon::Copy => "vp/clipboard-copy.svg",
            Icon::Compare => "vp/arrow-left-right.svg",
            Icon::Filter => "vp/list-filter.svg",
            Icon::Bookmark => "vp/bookmark.svg",
            Icon::FileDown => "vp/file-down.svg",
            Icon::FileUp => "vp/file-up.svg",
            Icon::Bug => "vp/bug.svg",
            Icon::Health => "vp/stethoscope.svg",
            Icon::Profile => "vp/user-round-cog.svg",
            Icon::Keyboard => "vp/keyboard.svg",
            Icon::More => "vp/ellipsis-vertical.svg",
            Icon::Help => "vp/circle-help.svg",
            Icon::Dashed => "vp/circle-dashed.svg",
            Icon::Alert => "vp/circle-alert.svg",
            Icon::Wand => "vp/wand-sparkles.svg",
            Icon::Edit => "vp/square-pen.svg",
            Icon::Loader => "vp/loader-circle.svg",
            Icon::Expand => "icons/maximize.svg",
        }
    }
}

/// Serves our icons (`vp/…`) and gpui-component's (`icons/…`).
pub struct Assets;

#[derive(rust_embed::RustEmbed)]
#[folder = "assets/icons"]
#[include = "*.svg"]
struct OwnIcons;

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
        if let Some(name) = path.strip_prefix("vp/") {
            return Ok(OwnIcons::get(name).map(|f| f.data));
        }
        if let Some(name) = path.strip_prefix("brand/") {
            return Ok(Brand::get(name).map(|f| f.data));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}
