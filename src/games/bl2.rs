//! Borderlands 2 — the fully supported flagship title.

use super::willow;
use super::{COMMON_NAV, GameDef, LaunchArg, ModSupport, NavGroup, NavItem, PageKind, Support};
use crate::patches::{ExePatch, ExePatchKind};
use crate::theme::{Icon, Rarity};

/// Shared by Borderlands 2 and The Pre-Sequel: both run on the Willow2 SDK.
pub const WILLOW2_SDK: ModSupport = ModSupport {
    sdk_repo: "bl-sdk/willow2-mod-manager",
    sdk_asset_hint: "willow2-sdk",
    sdk_name: "Willow2 Python SDK",
    sdk_markers: &["Binaries\\Win32\\Plugins\\unrealsdk.dll"],
    sdk_mods_dir: "sdk_mods",
    text_mods_dir: "Binaries",
    core_mods: &[
        "mods_base",
        "console_mod_menu",
        "keybinds",
        "legacy_compat",
        "networking",
        "save_options",
        "ui_utils",
        "willow2_mod_menu",
    ],
    legacy_markers: &["Binaries\\Win32\\python37.dll", "Binaries\\Win32\\Mods\\ModMenu"],
    redist_url: "https://aka.ms/vs/17/release/vc_redist.x86.exe",
    links: &[
        ("SDK mod database", "https://bl-sdk.github.io/willow2-mod-db/"),
        ("Text Mod Loader", "https://bl-sdk.github.io/willow2-mod-db/mods/text-mod-loader/"),
        ("OpenBLCMM", "https://github.com/BLCM/OpenBLCMM/releases/latest"),
        ("ModCabinet (text mods)", "https://github.com/BLCM/ModCabinet/wiki"),
        ("Nexus Mods", "https://www.nexusmods.com/borderlands2"),
    ],
};

/// Signature patches from the BLCMods wiki "Hex-Edits" page and OpenBLCMM's
/// HexDictionary. Each matches exactly once in the current Steam exe.
pub const PATCHES: &[ExePatch] = &[
    ExePatch {
        id: "laa",
        name: "4 GB memory (Large Address Aware)",
        description: "Lets the 32-bit exe use up to 4 GB of RAM, reducing out-of-memory crashes with big mod lists and HD textures.",
        rarity: Rarity::Rare,
        kind: ExePatchKind::LargeAddressAware,
        note: Some("Current Steam builds already ship with this flag set."),
        revertible: false,
    },
    ExePatch {
        id: "hex_set",
        name: "Enable the `set` console command",
        description: "Unlocks `set` in the shipping console, needed to run text mods with `exec` without the Python SDK.",
        rarity: Rarity::Legendary,
        kind: ExePatchKind::Hex {
            original: "83 C4 0C 85 C0 75 1A 6A",
            patched: "83 C4 0C 85 FF 75 1A 6A",
        },
        note: Some("Not needed with the Python SDK, which does this in memory. Breaks Assault on Dragon Keep standalone. Steam 'verify files' undoes it."),
        revertible: true,
    },
    ExePatch {
        id: "hex_say",
        name: "Remove automatic `say`",
        description: "Stops unknown console input from being broadcast as chat, so mod commands don't leak into co-op chat.",
        rarity: Rarity::Epic,
        kind: ExePatchKind::Hex {
            original: "61 00 77 00 20 00 5B 00 47 00 54 00 5D 00 00 00 73 00 61 00 79 00 20 00 00 00 00 00 6D 73 67 20",
            patched: "61 00 77 00 20 00 5B 00 47 00 54 00 5D 00 00 00 00 00 00 00 00 00 20 00 00 00 00 00 6D 73 67 20",
        },
        note: Some("Not needed with the Python SDK."),
        revertible: true,
    },
    ExePatch {
        id: "hex_array",
        name: "Remove the 100-item array limit",
        description: "Lets `getall`/`obj dump` print more than 100 array entries — useful for mod authors.",
        rarity: Rarity::Uncommon,
        kind: ExePatchKind::Hex {
            original: "7E 05 B9 64 00 00 00 3B F9 0F 8D",
            patched: "EB 05 B9 64 00 00 00 3B F9 0F 8D",
        },
        note: Some("Not needed with the Python SDK."),
        revertible: true,
    },
    ExePatch {
        id: "hex_array_msg",
        name: "Silence the array-limit warning",
        description: "Removes the \"array limit reached\" console spam that accompanies the edit above.",
        rarity: Rarity::Common,
        kind: ExePatchKind::Hex {
            original: "8B 40 04 83 F8 64 0F 8C 7B 00 00 00 8B 8D 9C EE FF FF 83 C0 9D 50 68",
            patched: "8B 40 04 83 F8 64 EB 7F 90 90 90 90 8B 8D 9C EE FF FF 83 C0 9D 50 68",
        },
        note: Some("Not needed with the Python SDK."),
        revertible: true,
    },
];

pub const LAUNCH_ARGS: &[LaunchArg] = &[
    LaunchArg {
        arg: "-NoLauncher",
        label: "Skip the launcher",
        description: "Start straight into the game. Also stops the launcher re-applying its copy of your video settings.",
        default_on: true,
    },
    LaunchArg {
        arg: "-NoStartupMovies",
        label: "No startup movies",
        description: "Skip the logo movies without touching the ini.",
        default_on: true,
    },
    LaunchArg {
        arg: "-log",
        label: "Log window",
        description: "Open a live engine log window alongside the game. Handy for debugging mods.",
        default_on: false,
    },
    LaunchArg {
        arg: "-NoController",
        label: "Ignore controllers",
        description: "Disable gamepad input, which fixes button prompts flipping when a controller is plugged in.",
        default_on: false,
    },
    LaunchArg {
        arg: "-windowed",
        label: "Force windowed",
        description: "Start windowed regardless of the ini, for recovering from a bad resolution.",
        default_on: false,
    },
];

pub const NAV: &[NavGroup] = &[
    NavGroup {
        title: "Game",
        items: &[
            NavItem { kind: PageKind::Overview, title: "Overview", icon: Icon::Home, categories: &[] },
            NavItem { kind: PageKind::Presets, title: "Presets", icon: Icon::Star, categories: &[] },
        ],
    },
    NavGroup {
        title: "Game Settings",
        items: &[
            NavItem { kind: PageKind::Tweaks("display"), title: "Display & FPS", icon: Icon::Display, categories: &["display", "framerate"] },
            NavItem { kind: PageKind::Tweaks("graphics"), title: "Graphics", icon: Icon::Picture, categories: &["quality", "aa"] },
            NavItem { kind: PageKind::Tweaks("textures"), title: "Textures", icon: Icon::Texture, categories: &["textures"] },
            NavItem { kind: PageKind::Tweaks("effects"), title: "Outlines & Effects", icon: Icon::Sparkle, categories: &["outlines", "postfx"] },
            NavItem { kind: PageKind::Tweaks("shadows"), title: "Shadows", icon: Icon::Contrast, categories: &["shadows"] },
            NavItem { kind: PageKind::Tweaks("physx"), title: "PhysX", icon: Icon::Atom, categories: &["physx"] },
            NavItem { kind: PageKind::Tweaks("controls"), title: "Camera & Controls", icon: Icon::Mouse, categories: &["camera", "input"] },
            NavItem { kind: PageKind::Tweaks("gameplay"), title: "HUD & Gameplay", icon: Icon::Hud, categories: &["hud", "gameplay", "audio"] },
            NavItem { kind: PageKind::Tweaks("system"), title: "Startup & Network", icon: Icon::Rocket, categories: &["startup", "network"] },
        ],
    },
    NavGroup {
        title: "Modding",
        items: &[
            NavItem { kind: PageKind::Mods, title: "Mods", icon: Icon::Puzzle, categories: &[] },
            NavItem { kind: PageKind::Patches, title: "Exe Patches", icon: Icon::Wrench, categories: &[] },
        ],
    },
    COMMON_NAV,
];

pub static GAME: GameDef = GameDef {
    id: "bl2",
    name: "Borderlands 2",
    short: "BL2",
    tagline: "Pandora. Handsome Jack. Four new Vault Hunters.",
    support: Support::Full,
    steam_app_ids: &[49520],
    epic_names: &["Borderlands 2"],
    exe: "Binaries\\Win32\\Borderlands2.exe",
    config_subdir: "Borderlands 2\\WillowGame\\Config",
    ini_files: willow::INI_FILES,
    categories: willow::CATEGORIES,
    tweaks: willow::TWEAKS,
    hidden_tweaks: &[],
    presets: willow::PRESETS,
    patches: PATCHES,
    mods: Some(&WILLOW2_SDK),
    launch_args: LAUNCH_ARGS,
    nav: NAV,
    simple_nav: super::SIMPLE_NAV,
    quick: willow::QUICK,
    setup: willow::BL2_SETUP,
    comparisons: willow::COMPARISONS,
};
