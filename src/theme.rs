//! Borderlands-inspired visual language: cel-shaded ink outlines, hard comic
//! drop shadows, hazard yellow and ECHO cyan, and loot rarity colors used as
//! semantic accents.
//!
//! Colors come from the active [`Palette`], switchable at runtime; every
//! widget reads them through the functions below.

use std::sync::atomic::{AtomicU8, Ordering};

use gpui::{BoxShadow, Hsla, Rgba, point, px, rgb};
use serde::{Deserialize, Serialize};

/// Big comic titles only.
pub const FONT_DISPLAY: &str = "Bangers";
/// Condensed industrial type for labels, buttons, nav and headings.
pub const FONT_LABEL: &str = "Barlow Condensed";
/// Body copy.
pub const FONT_BODY: &str = "Barlow";
pub const FONT_MONO: &str = "Consolas";
pub const FONT_ICON: &str = "Segoe MDL2 Assets";

/// The bundled typefaces (all SIL Open Font License).
pub const FONT_FILES: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Bangers-Regular.ttf"),
    include_bytes!("../assets/fonts/Barlow-Regular.ttf"),
    include_bytes!("../assets/fonts/Barlow-Medium.ttf"),
    include_bytes!("../assets/fonts/Barlow-SemiBold.ttf"),
    include_bytes!("../assets/fonts/BarlowCondensed-Medium.ttf"),
    include_bytes!("../assets/fonts/BarlowCondensed-SemiBold.ttf"),
    include_bytes!("../assets/fonts/BarlowCondensed-Bold.ttf"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Palette {
    /// Near-black cool neutrals so hazard yellow and ECHO cyan pop — the
    /// Borderlands 3/4 menu look.
    #[default]
    VaultHunter,
    /// Warm, dusty Pandora browns (the original theme).
    Pandora,
    /// Hyperion corporate: deep navy with bright yellow.
    Hyperion,
}

impl Palette {
    pub const ALL: [Palette; 3] = [Palette::VaultHunter, Palette::Pandora, Palette::Hyperion];

    pub fn label(self) -> &'static str {
        match self {
            Palette::VaultHunter => "Vault Hunter",
            Palette::Pandora => "Pandora",
            Palette::Hyperion => "Hyperion",
        }
    }
}

struct Colors {
    bg: u32,
    bg_deep: u32,
    panel: u32,
    panel_hi: u32,
    panel_lo: u32,
    line: u32,
    text: u32,
    text_muted: u32,
    text_dim: u32,
    echo: u32,
}

const VAULT_HUNTER: Colors = Colors {
    bg: 0x111317,
    bg_deep: 0x0a0b0e,
    panel: 0x181b21,
    panel_hi: 0x21252d,
    panel_lo: 0x14161b,
    line: 0x2b3039,
    text: 0xf2f4f6,
    text_muted: 0xc3c9d1,
    text_dim: 0x858e9a,
    echo: 0x34d4ff,
};

const PANDORA: Colors = Colors {
    bg: 0x15110d,
    bg_deep: 0x0e0b08,
    panel: 0x1f1a15,
    panel_hi: 0x29231c,
    panel_lo: 0x1b1611,
    line: 0x352d23,
    text: 0xf3e9d2,
    text_muted: 0xcfc2a9,
    text_dim: 0x9a8b74,
    echo: 0x3fd0ff,
};

const HYPERION: Colors = Colors {
    bg: 0x0e1422,
    bg_deep: 0x080c16,
    panel: 0x152033,
    panel_hi: 0x1d2b43,
    panel_lo: 0x111a2b,
    line: 0x26354f,
    text: 0xf1f5fb,
    text_muted: 0xbfcbe0,
    text_dim: 0x7f8fab,
    echo: 0x4fd8ff,
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

pub fn bg() -> Rgba {
    rgb(colors().bg)
}
pub fn bg_deep() -> Rgba {
    rgb(colors().bg_deep)
}
pub fn panel() -> Rgba {
    rgb(colors().panel)
}
pub fn panel_hi() -> Rgba {
    rgb(colors().panel_hi)
}
pub fn panel_lo() -> Rgba {
    rgb(colors().panel_lo)
}
pub fn ink() -> Rgba {
    rgb(0x050403)
}
pub fn line() -> Rgba {
    rgb(colors().line)
}
pub fn text() -> Rgba {
    rgb(colors().text)
}
pub fn text_muted() -> Rgba {
    rgb(colors().text_muted)
}
pub fn text_dim() -> Rgba {
    rgb(colors().text_dim)
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
    rgb(0xff4d3d)
}
pub fn success() -> Rgba {
    rgb(0x5fdc6a)
}
pub fn echo() -> Rgba {
    rgb(colors().echo)
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
    Music,
    Mute,
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
            Icon::Music => "\u{EC4F}",
            Icon::Mute => "\u{E74F}",
            Icon::Terminal => "\u{E756}",
            Icon::Puzzle => "\u{EA86}",
        }
    }
}
