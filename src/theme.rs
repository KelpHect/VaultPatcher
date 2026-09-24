//! Borderlands-inspired visual language: cel-shaded ink outlines, hard comic
//! drop shadows, hazard yellow on grimy Pandora browns, and loot rarity
//! colors used as semantic accents.

use gpui::{BoxShadow, Hsla, Rgba, point, px, rgb};

pub const FONT_DISPLAY: &str = "Bangers";
pub const FONT_LABEL: &str = "Bahnschrift";
pub const FONT_BODY: &str = "Segoe UI";
pub const FONT_MONO: &str = "Consolas";
pub const FONT_ICON: &str = "Segoe MDL2 Assets";

pub fn bg() -> Rgba {
    rgb(0x15110d)
}
pub fn bg_deep() -> Rgba {
    rgb(0x0e0b08)
}
pub fn panel() -> Rgba {
    rgb(0x1f1a15)
}
pub fn panel_hi() -> Rgba {
    rgb(0x29231c)
}
pub fn panel_lo() -> Rgba {
    rgb(0x1b1611)
}
pub fn ink() -> Rgba {
    rgb(0x050403)
}
pub fn line() -> Rgba {
    rgb(0x352d23)
}
pub fn text() -> Rgba {
    rgb(0xf3e9d2)
}
pub fn text_muted() -> Rgba {
    rgb(0xcfc2a9)
}
pub fn text_dim() -> Rgba {
    rgb(0x9a8b74)
}
pub fn accent() -> Rgba {
    rgb(0xffc21a)
}
pub fn accent_hi() -> Rgba {
    rgb(0xffd65c)
}
pub fn accent_ink() -> Rgba {
    rgb(0x1a1204)
}
pub fn danger() -> Rgba {
    rgb(0xe8452c)
}
pub fn success() -> Rgba {
    rgb(0x5fd35a)
}
pub fn echo() -> Rgba {
    rgb(0x3fd0ff)
}

/// Loot rarity tiers, reused as badge and status colors across the app.
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
            Rarity::Common => rgb(0xe6e2da),
            Rarity::Uncommon => rgb(0x4fd04a),
            Rarity::Rare => rgb(0x3d8bff),
            Rarity::Epic => rgb(0xa259ff),
            Rarity::Legendary => rgb(0xff8a1c),
            Rarity::Pearlescent => rgb(0x41e3d4),
            Rarity::Seraph => rgb(0xff5aa8),
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

/// The hard, unblurred offset shadow that gives panels their comic-panel look.
pub fn comic_shadow(offset: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: ink().into(),
        offset: point(px(offset), px(offset)),
        blur_radius: px(0.),
        spread_radius: px(0.),
    }]
}

/// Glyphs from Segoe MDL2 Assets (present on every Windows 10/11 install).
/// Kept as a palette for pages to pick from, so not every glyph is in use.
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
    Folder,
    History,
    Save,
    Undo,
    Refresh,
    Play,
    Warning,
    Check,
    Close,
    Info,
    Delete,
    Add,
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
    Minimize,
    Maximize,
    Restore,
    Terminal,
    Puzzle,
}

impl Icon {
    pub fn glyph(self) -> &'static str {
        match self {
            Icon::Home => "\u{E80F}",
            Icon::Settings => "\u{E713}",
            Icon::Display => "\u{E7F4}",
            Icon::Picture => "\u{E8B9}",
            Icon::Sparkle => "\u{E706}",
            Icon::Contrast => "\u{E793}",
            Icon::Texture => "\u{E91B}",
            Icon::Atom => "\u{E945}",
            Icon::Speed => "\u{EC4A}",
            Icon::Camera => "\u{E722}",
            Icon::Mouse => "\u{E962}",
            Icon::Hud => "\u{E7B3}",
            Icon::Game => "\u{E7FC}",
            Icon::Rocket => "\u{E7B5}",
            Icon::Shield => "\u{EA18}",
            Icon::Package => "\u{E7B8}",
            Icon::Download => "\u{E896}",
            Icon::Folder => "\u{E8B7}",
            Icon::History => "\u{E81C}",
            Icon::Save => "\u{E74E}",
            Icon::Undo => "\u{E7A7}",
            Icon::Refresh => "\u{E72C}",
            Icon::Play => "\u{E768}",
            Icon::Warning => "\u{E7BA}",
            Icon::Check => "\u{E73E}",
            Icon::Close => "\u{E8BB}",
            Icon::Info => "\u{E946}",
            Icon::Delete => "\u{E74D}",
            Icon::Add => "\u{E710}",
            Icon::Link => "\u{E71B}",
            Icon::Code => "\u{E943}",
            Icon::Wrench => "\u{E90F}",
            Icon::Lock => "\u{E72E}",
            Icon::Unlock => "\u{E785}",
            Icon::Star => "\u{E735}",
            Icon::Sliders => "\u{E9E9}",
            Icon::Volume => "\u{E767}",
            Icon::Globe => "\u{E774}",
            Icon::ChevronRight => "\u{E76C}",
            Icon::Minimize => "\u{E921}",
            Icon::Maximize => "\u{E922}",
            Icon::Restore => "\u{E923}",
            Icon::Terminal => "\u{E756}",
            Icon::Puzzle => "\u{EA86}",
        }
    }
}
