//! Borderlands Game of the Year Enhanced (64-bit, 2019 remaster). Keys and
//! sections were checked against a live BL1E config folder.

use super::willow::{self, ANISO, DETAIL_HIGH_FIRST, E, G, I, LOW_MED_HIGH, POST_PROCESS, RESOLUTIONS};
use super::{COMMON_NAV, GameDef, LaunchArg, ModSupport, NavGroup, NavItem, PageKind, Support};
use crate::core::display::DisplayMode;
use crate::setup::{Component, ComponentKind, DxvkTarget, Group};
use crate::theme::{Icon, Rarity};
use crate::tweaks::DefaultValue::{B, C};
use crate::tweaks::{
    Category, Control, EXPERIMENTAL, Impact, MENU, NONE, Opt, Preset, TF, TF_LOWER, TF_UPPER,
    Tweak, choice, custom, key, opt, slider, toggle,
};

const SS: &str = "SystemSettings";

const CATEGORIES: &[Category] = &[
    Category { id: "display", title: "Display", blurb: "Window mode, resolution and field of view. BL1E stores FOV in the ini, so values past the slider stick." },
    Category { id: "framerate", title: "Framerate", blurb: "Frame smoothing acts as the limiter. Turn it on and set the maximum for a cap." },
    Category { id: "quality", title: "World Detail", blurb: "Detail, foliage, lighting and texture streaming. BL1E is 64-bit, so a large texture pool is safe." },
    Category { id: "outlines", title: "Cel Shading & Outlines", blurb: "Swap the post-process chain to remove the ink outlines, same as BL2." },
    Category { id: "postfx", title: "Post-Processing", blurb: "Screen effects." },
    Category { id: "input", title: "Console & Input", blurb: "Developer console and mouse feel." },
    Category { id: "startup", title: "Startup", blurb: "Skip the launcher and intro movies." },
];

const CONSOLE_KEYS: &[Opt] = &[
    opt("", "Disabled"),
    opt("Tilde", "~ Tilde"),
    opt("F1", "F1"),
    opt("F6", "F6"),
    opt("Insert", "Insert"),
];

const TWEAKS: &[Tweak] = &[
    toggle("fullscreen", "display", "Fullscreen", "Exclusive fullscreen. Off runs windowed.",
        &[key(E, SS, "Fullscreen")], TF, true, Impact::None, MENU),
    custom("resolution", "display", "Resolution", "Render resolution.",
        Control::Choice(RESOLUTIONS), willow::read_resolution, willow::write_resolution, C("1920x1080"), Impact::High, MENU),
    slider("fov", "display", "Field of view", "BL1E is Vert- at wide aspect ratios; raise this on ultrawide. The menu slider covers 60–120.",
        &[key(E, SS, "FOVAngle")], (60.0, 130.0, 1.0), 0, "°", 75.0, Impact::Low, MENU),
    toggle("smooth_fps", "framerate", "Frame smoothing / limiter", "Caps the framerate to the maximum below.",
        &[key(E, "Engine.GameEngine", "bSmoothFrameRate")], TF_UPPER, false, Impact::None, NONE),
    slider("smooth_max", "framerate", "Framerate cap", "Upper bound of the smoothed range.",
        &[key(E, "Engine.GameEngine", "MaxSmoothedFrameRate")], (30.0, 400.0, 1.0), 0, "fps", 62.0, Impact::Medium, NONE),
    toggle("one_frame_lag", "framerate", "One frame thread lag", "Off lowers input latency at a small FPS cost.",
        &[key(E, SS, "OneFrameThreadLag")], TF, true, Impact::Low, NONE),
    choice("detail_mode", "quality", "Detail mode", "Engine-level world detail.",
        &[key(E, SS, "DetailMode")], LOW_MED_HIGH, "2", Impact::Low, NONE),
    choice("texture_quality", "quality", "Texture quality", "Menu texture quality (0 is the highest).",
        &[key(E, SS, "TextureQuality")], DETAIL_HIGH_FIRST, "0", Impact::Medium, MENU),
    choice("aniso", "quality", "Anisotropic filtering", "Texture sharpness at glancing angles.",
        &[key(E, SS, "MaxAnisotropy")], ANISO, "4", Impact::Low, MENU),
    slider("foliage", "quality", "Foliage distance", "Grass and foliage draw radius.",
        &[key(E, SS, "FoliageDrawRadiusMultiplier")], (0.0, 1.0, 0.05), 2, "×", 1.0, Impact::Medium, NONE),
    toggle("dynamic_lights", "quality", "Dynamic lights", "Moving lights from effects and weapons.",
        &[key(E, SS, "DynamicLights")], TF, true, Impact::High, NONE),
    toggle("dynamic_shadows", "quality", "Dynamic shadows", "Shadows from characters and moving objects.",
        &[key(E, SS, "DynamicShadows")], TF, true, Impact::High, MENU),
    slider("pool_size", "quality", "Texture pool size", "Streaming pool in MB.",
        &[key(E, "TextureStreaming", "PoolSize")], (400.0, 4096.0, 100.0), 0, "MB", 1200.0, Impact::Medium, NONE),
    choice("post_chain", "outlines", "Outline style", "Classic draws the ink outlines. The alternatives remove them.",
        &[key(E, "Engine.Engine", "DefaultPostProcessName")], POST_PROCESS, "WillowEngineMaterials.WillowScenePostProcess", Impact::Low, NONE),
    toggle("ao", "postfx", "Ambient occlusion", "Contact shadows in corners.",
        &[key(E, SS, "AmbientOcclusion")], TF, true, Impact::High, MENU),
    toggle("bloom", "postfx", "Bloom", "Glow around bright lights.",
        &[key(E, SS, "Bloom")], TF, true, Impact::Low, NONE),
    toggle("dof", "postfx", "Depth of field", "Background blur.",
        &[key(E, SS, "DepthOfField")], TF, true, Impact::Medium, MENU),
    toggle("motion_blur", "postfx", "Motion blur", "Camera motion blur.",
        &[key(E, SS, "MotionBlur")], TF, false, Impact::Low, MENU),
    toggle("lens_flares", "postfx", "Lens flares", "Flares from bright lights.",
        &[key(E, SS, "LensFlares")], TF, true, Impact::Low, NONE),
    toggle("distortion", "postfx", "Heat distortion", "Heat haze around fire and explosions.",
        &[key(E, SS, "Distortion")], TF, true, Impact::Medium, NONE),
    toggle("reflections", "postfx", "Reflections", "Dynamic reflections.",
        &[key(E, SS, "Reflections")], TF, true, Impact::Medium, EXPERIMENTAL),
    choice("console_key", "input", "Console key", "Opens the developer console.",
        &[key(I, "Engine.Console", "ConsoleKey")], CONSOLE_KEYS, "", Impact::None, NONE),
    toggle("mouse_smoothing", "input", "Mouse smoothing", "Engine mouse smoothing. PCGamingWiki recommends turning it off.",
        &[key(I, "Engine.PlayerInput", "bEnableMouseSmoothing")], TF_LOWER, true, Impact::None, NONE),
    toggle("skip_intros", "startup", "Skip intro movies", "Uses the game's own switch rather than a launch option (-nomoviestartup crashes BL1E).",
        &[key(G, "Engine.GameInfo", "DisableIntroMovies")], TF, false, Impact::None, NONE),
    toggle("no_launcher", "startup", "Skip the launcher", "Start straight into the game.",
        &[key(G, "Engine.GameInfo", "DisableLauncher")], TF, false, Impact::None, NONE),
];

const PRESETS: &[Preset] = &[
    Preset {
        id: "fixes",
        name: "Community Essentials",
        rarity: Rarity::Pearlescent,
        description: "Skip the launcher and intros, console on ~, no mouse smoothing, lower input lag.",
        values: &[
            ("skip_intros", B(true)),
            ("no_launcher", B(true)),
            ("console_key", C("Tilde")),
            ("mouse_smoothing", B(false)),
            ("one_frame_lag", B(false)),
        ],
    },
    Preset {
        id: "clean",
        name: "Clean Look",
        rarity: Rarity::Epic,
        description: "No ink outlines, no motion blur or depth of field, 16× filtering.",
        values: &[
            ("post_chain", C("WillowEngineMaterials.RyanScenePostProcess")),
            ("motion_blur", B(false)),
            ("dof", B(false)),
            ("aniso", C("16")),
        ],
    },
    Preset {
        id: "vanilla",
        name: "Factory Settings",
        rarity: Rarity::Seraph,
        description: "Every tweak back to the shipped default.",
        values: &[],
    },
];

fn match_display(c: &mut crate::tweaks::ConfigSet, mode: DisplayMode) {
    c.set(&key(E, SS, "ResX"), &mode.width.to_string());
    c.set(&key(E, SS, "ResY"), &mode.height.to_string());
    c.set(&key(E, "Engine.GameEngine", "bSmoothFrameRate"), "TRUE");
    c.set(&key(E, "Engine.GameEngine", "MaxSmoothedFrameRate"), &mode.refresh_hz.max(60).to_string());
}

const MODERN_DEFAULTS: &[(&str, crate::tweaks::DefaultValue)] = &[
    ("skip_intros", B(true)),
    ("no_launcher", B(true)),
    ("console_key", C("Tilde")),
    ("mouse_smoothing", B(false)),
    ("one_frame_lag", B(false)),
    ("motion_blur", B(false)),
    ("aniso", C("16")),
    ("fov", crate::tweaks::DefaultValue::N(90.0)),
];

const DXVK_CONF: &str = "# Vault Patcher DXVK profile for Borderlands GOTY Enhanced\n\
dxgi.maxFrameLatency = 1\n\
d3d11.samplerAnisotropy = 16\n";

const SETUP: &[Component] = &[
    Component {
        id: "modern",
        name: "Modern Defaults",
        summary: "Native resolution, smooth framerate, no blur, straight to the menu",
        description: "Native resolution, framerate capped at your monitor's refresh rate, a wider 90° field of view, raw mouse feel, lower input lag, and no launcher or intro movies.",
        group: Group::Essentials,
        recommended: true,
        kind: ComponentKind::Settings { values: MODERN_DEFAULTS, display: Some(match_display) },
        requires: &[],
    },
    Component {
        id: "dxvk",
        name: "DXVK Vulkan Renderer",
        summary: "Vulkan renderer for steadier frame times",
        description: "Runs the DirectX 11 renderer on Vulkan. Optional: BL1E has no known DXVK issues to fix. Needs an up-to-date driver with Vulkan 1.4.",
        group: Group::Performance,
        recommended: false,
        kind: ComponentKind::Dxvk { target: DxvkTarget::D3d11Win64, exe_dir: "Binaries\\Win64", conf: DXVK_CONF },
        requires: &[],
    },
    Component {
        id: "exit_fix",
        name: "Exit Hang Fix",
        summary: "The game actually closes when you quit",
        description: "Fixes the game freezing instead of closing when you quit. A tiny proxy DLL by endjynn.",
        group: Group::Fixes,
        recommended: true,
        kind: ComponentKind::File {
            url: "https://github.com/endjynn/borderlands-proxy/releases/latest/download/version.dll",
            dest: "Binaries\\Win64\\version.dll",
            enable: None,
        },
        requires: &[],
    },
    Component {
        id: "sdk",
        name: "Python SDK (mod loader)",
        summary: "The community mod loader",
        description: "The community mod loader for BL1: adds a MODS menu and runs SDK mods.",
        group: Group::Mods,
        recommended: true,
        kind: ComponentKind::Sdk,
        requires: &[],
    },
    bl1_mod("skill_ui_fix", "Skill Tree Fix", "Fixes skill tree display glitches", "Fixes skill tree UI glitches in the Enhanced edition. By Ry0511.", Group::Fixes, true,
        "https://raw.githubusercontent.com/Ry0511/my_bl1_sdk_mods/refs/heads/master/packaged/skill_tree_tweaks.sdkmod", "sdk_mods/skill_tree_tweaks.sdkmod", "skill_tree_tweaks"),
    bl1_mod("movie_skipper", "Startup Movie Skipper", "Straight to the main menu", "Straight to the main menu. By Ry0511.", Group::Mods, true,
        "https://raw.githubusercontent.com/Ry0511/my_bl1_sdk_mods/refs/heads/master/packaged/startup_movie_skipper.sdkmod", "sdk_mods/startup_movie_skipper.sdkmod", "startup_movie_skipper"),
    bl1_mod("auto_pickup", "Auto Pickup", "Money, health and chest loot picked up automatically", "Picks up money, ammo and health automatically. By galqawala.", Group::Mods, true,
        "https://github.com/galqawala/AutopickupBL1E/raw/refs/heads/master/AutopickupBL1E.sdkmod", "sdk_mods/AutopickupBL1E.sdkmod", "AutopickupBL1E"),
    bl1_mod("hide_full", "Hide Full Pickups", "No prompts for ammo you can't carry", "Stops showing pickup prompts for ammo and health you can't carry. By EerieGoesD.", Group::Mods, true,
        "https://github.com/EerieGoesD/borderlands-1-goty-mods/raw/refs/heads/main/HideFullPickups/HideFullPickups.sdkmod", "sdk_mods/HideFullPickups.sdkmod", "HideFullPickups"),
    bl1_mod("auto_save", "Configurable Auto-Save", "Frequent, configurable auto-saves", "More frequent, configurable auto-saves like modern games. By Ry0511.", Group::Mods, false,
        "https://raw.githubusercontent.com/Ry0511/my_bl1_sdk_mods/refs/heads/master/packaged/rys_auto_save.sdkmod", "sdk_mods/rys_auto_save.sdkmod", "rys_auto_save"),
];

#[allow(clippy::too_many_arguments)]
const fn bl1_mod(
    id: &'static str,
    name: &'static str,
    summary: &'static str,
    description: &'static str,
    group: Group,
    recommended: bool,
    url: &'static str,
    dest: &'static str,
    module: &'static str,
) -> Component {
    Component {
        id,
        name,
        summary,
        description,
        group,
        recommended,
        kind: ComponentKind::File { url, dest, enable: Some(module) },
        requires: &["sdk"],
    }
}

const QUICK: &[super::QuickSection] = &[
    super::QuickSection {
        title: "Look & Feel",
        blurb: "The classic comic-book ink, or a cleaner modern look.",
        tweaks: &["post_chain", "motion_blur", "dof"],
        quality_presets: &[],
    },
    super::QuickSection {
        title: "Display",
        blurb: "How the game fits your screen.",
        tweaks: &["fullscreen", "resolution", "fov", "smooth_fps", "smooth_max"],
        quality_presets: &[],
    },
    super::QuickSection {
        title: "Comfort",
        blurb: "Small things that make the old game feel new.",
        tweaks: &["skip_intros", "no_launcher", "console_key", "mouse_smoothing"],
        quality_presets: &[],
    },
];

const WILLOW1_SDK: ModSupport = ModSupport {
    sdk_repo: "bl-sdk/willow1-mod-manager",
    sdk_asset_hint: "bl1-enhanced-sdk",
    sdk_name: "Willow1 Python SDK",
    sdk_markers: &["Binaries\\Win64\\Plugins\\unrealsdk.dll"],
    sdk_mods_dir: "sdk_mods",
    text_mods_dir: "Binaries",
    core_mods: &[
        "mods_base",
        "console_mod_menu",
        "keybinds",
        "networking",
        "save_options",
        "ui_utils",
        "willow1_mod_menu",
    ],
    legacy_markers: &[],
    redist_url: "https://aka.ms/vs/17/release/vc_redist.x64.exe",
    links: &[
        ("SDK mod database", "https://bl-sdk.github.io/willow1-mod-db/"),
        ("Nexus Mods", "https://www.nexusmods.com/borderlandsgotyenhanced"),
    ],
};

const LAUNCH_ARGS: &[LaunchArg] = &[LaunchArg {
    arg: "-log",
    label: "Log window",
    description: "Open a live engine log window.",
    default_on: false,
}];

const NAV: &[NavGroup] = &[
    NavGroup {
        title: "Command Center",
        tabs: None,
        items: &[
            NavItem { kind: PageKind::Overview, title: "Overview", icon: Icon::Home, categories: &[] },
            NavItem { kind: PageKind::Presets, title: "Presets", icon: Icon::Star, categories: &[] },
        ],
    },
    NavGroup {
        title: "Game Settings",
        tabs: Some(Icon::Sliders),
        items: &[
            NavItem { kind: PageKind::Tweaks("display"), title: "Display & FPS", icon: Icon::Display, categories: &["display", "framerate"] },
            NavItem { kind: PageKind::Tweaks("graphics"), title: "Graphics", icon: Icon::Picture, categories: &["quality"] },
            NavItem { kind: PageKind::Tweaks("effects"), title: "Outlines & Effects", icon: Icon::Sparkle, categories: &["outlines", "postfx"] },
            NavItem { kind: PageKind::Tweaks("system"), title: "Controls & Startup", icon: Icon::Rocket, categories: &["input", "startup"] },
        ],
    },
    NavGroup {
        title: "Modding",
        tabs: None,
        items: &[NavItem { kind: PageKind::Mods, title: "Mods", icon: Icon::Puzzle, categories: &[] }],
    },
    COMMON_NAV,
];

pub static GAME: GameDef = GameDef {
    id: "bl1e",
    name: "Borderlands GOTY Enhanced",
    short: "BL1E",
    tagline: "Where it all began — remastered.",
    support: Support::Preview,
    steam_app_ids: &[729040],
    epic_names: &["Borderlands Game of the Year"],
    exe: "Binaries\\Win64\\BorderlandsGOTY.exe",
    config_subdir: "Borderlands Game of the Year\\WillowGame\\Config",
    ini_files: &[(E, "WillowEngine.ini"), (G, "WillowGame.ini"), (I, "WillowInput.ini")],
    categories: CATEGORIES,
    tweaks: TWEAKS,
    hidden_tweaks: &[],
    presets: PRESETS,
    patches: &[],
    mods: Some(&WILLOW1_SDK),
    launch_args: LAUNCH_ARGS,
    nav: NAV,
    simple_nav: super::SIMPLE_NAV,
    quick: QUICK,
    setup: SETUP,
    comparisons: &[],
};
