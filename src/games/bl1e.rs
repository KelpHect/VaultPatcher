//! Borderlands Game of the Year Enhanced (2019 remaster: 64-bit, Direct3D 11).
//!
//! Keys, sections and defaults were checked against a live BL1E install
//! (`WillowGame\Config\DefaultEngine.ini`, `Engine\Config\BaseEngine.ini`) and
//! a generated config folder. The remaster's options menu writes its own keys
//! into `[SystemSettings]` (`WindowMode`, `FramerateMode`, `UseVsync`,
//! `FOVAngle`, `DisableIntroMovies`, …), which differ from BL2's.

use super::willow::{self, ANISO, DETAIL_LOW_TO_HIGH, E, G, I, LOW_MED_HIGH, RESOLUTIONS, TEXTURE_BIAS};
use super::{COMMON_NAV, GameDef, LaunchArg, ModSupport, NavGroup, NavItem, PageKind, Support};
use crate::compare::Comparison;
use crate::core::display::DisplayMode;
use crate::setup::{Component, ComponentKind, DxvkTarget, Group};
use crate::theme::{Icon, Rarity};
use crate::tweaks::DefaultValue::{B, C, N};
use crate::tweaks::{
    Category, ConfigSet, Control, EXPERIMENTAL, Impact, MENU, NONE, Opt, Preset, TF, TF_LOWER, TF_UPPER, Tweak, Value,
    choice, custom, key, opt, slider, toggle,
};

const SS: &str = "SystemSettings";

const CATEGORIES: &[Category] = &[
    Category { id: "display", title: "Display", blurb: "Window mode, resolution, render scale and field of view. BL1E is Vert- on ultrawide; see the Ultrawide Fix in One-Click Setup." },
    Category { id: "framerate", title: "Framerate", blurb: "The menu's framerate lock is the real cap. The default \"Smoothed 22–62\" mode is the usual cause of stutter." },
    Category { id: "quality", title: "World Detail", blurb: "View distance, detail, foliage, decals and lights." },
    Category { id: "textures", title: "Textures", blurb: "Quality, streaming and filtering. BL1E is 64-bit, so a large texture pool is safe." },
    Category { id: "shadows", title: "Shadows", blurb: "Dynamic shadow quality and resolution." },
    Category { id: "outlines", title: "Cel Shading & Outlines", blurb: "Swap the post-process chain to remove the ink outlines." },
    Category { id: "postfx", title: "Post-Processing", blurb: "Anti-aliasing, ambient occlusion (HBAO+), bloom and screen effects." },
    Category { id: "input", title: "Mouse & Console", blurb: "Mouse feel fixes from PCGamingWiki, and the developer console." },
    Category { id: "interface", title: "Interface & Audio", blurb: "Subtitles and window focus behaviour." },
    Category { id: "startup", title: "Startup", blurb: "Skip the launcher and intro movies. Never use -nomoviestartup: it crashes BL1E." },
    Category { id: "network", title: "Co-op & Network", blurb: "Smoother co-op for hosts. Use with care: the game's physics assume the defaults." },
];

const WINDOW_MODES: &[Opt] = &[opt("0", "Fullscreen"), opt("1", "Windowed"), opt("2", "Borderless")];
/// The menu's "Framerate Locking" list (`FramerateMode` index).
const FRAMERATE_MODES: &[Opt] = &[
    opt("0", "Custom range"),
    opt("1", "30"),
    opt("2", "50"),
    opt("3", "60"),
    opt("4", "75"),
    opt("5", "100"),
    opt("6", "120"),
    opt("7", "144"),
    opt("8", "Unlimited"),
];
const CONSOLE_KEYS: &[Opt] = &[opt("", "Disabled"), opt("Tilde", "~ Tilde"), opt("F1", "F1"), opt("F6", "F6"), opt("Insert", "Insert")];
const SHADOW_RES: &[Opt] = &[opt("512", "512"), opt("1024", "1024"), opt("2048", "2048"), opt("4096", "4096")];
const SHADOW_FILTER: &[Opt] = &[opt("2", "Low"), opt("1", "Medium"), opt("0", "High")];
const POPULATION: &[Opt] = &[opt("-40", "Sparse"), opt("-20", "Reduced"), opt("0", "Full")];
const MOUSE_Y: &[Opt] = &[opt("-250", "Game default (slower)"), opt("-300", "Match horizontal")];
const TICK_RATES: &[Opt] = &[opt("30", "30 (default)"), opt("45", "45"), opt("60", "60")];
const POST_PROCESS: &[Opt] = &[
    opt("WillowEngineMaterials.WillowScenePostProcess", "Classic outlines"),
    opt("WillowEngineMaterials.RyanScenePostProcess", "No outlines"),
];

fn read_window_mode(c: &ConfigSet) -> Option<Value> {
    let v = c.get(&key(E, SS, "WindowMode"))?;
    WINDOW_MODES.iter().find(|o| o.value == v.trim()).map(|o| Value::Choice(o.value))
}

/// The remaster reads `WindowMode`; `Fullscreen` is kept consistent with it.
fn write_window_mode(c: &mut ConfigSet, v: &Value) {
    let Value::Choice(mode) = v else { return };
    c.set(&key(E, SS, "WindowMode"), mode);
    c.set(&key(E, SS, "Fullscreen"), if *mode == "0" { "True" } else { "False" });
}

fn read_fxaa(c: &ConfigSet) -> Option<Value> {
    let v = c.get(&key(E, "PostEffects.AntiAliasing", "Type"))?;
    Some(Value::Bool(v.trim() != "0"))
}

fn write_fxaa(c: &mut ConfigSet, v: &Value) {
    let Value::Bool(on) = v else { return };
    c.set(&key(E, "PostEffects.AntiAliasing", "Type"), if *on { "1" } else { "0" });
}

fn read_voice(c: &ConfigSet) -> Option<Value> {
    let v = c.get(&key(E, "VoIP", "bHasVoiceEnabled"))?;
    Some(Value::Bool(!v.trim().eq_ignore_ascii_case("true")))
}

fn write_voice(c: &mut ConfigSet, v: &Value) {
    let Value::Bool(disable) = v else { return };
    c.set(&key(E, "VoIP", "bHasVoiceEnabled"), if *disable { "false" } else { "true" });
}

fn read_tick(c: &ConfigSet) -> Option<Value> {
    let v = c.get(&key(E, "IpDrv.TcpNetDriver", "NetServerMaxTickRate"))?;
    TICK_RATES.iter().find(|o| o.value == v.trim()).map(|o| Value::Choice(o.value)).or(Some(Value::Unknown(v.to_string())))
}

fn write_tick(c: &mut ConfigSet, v: &Value) {
    let Value::Choice(rate) = v else { return };
    c.set(&key(E, "IpDrv.TcpNetDriver", "NetServerMaxTickRate"), rate);
    c.set(&key(E, "IpDrv.TcpNetDriver", "LanServerMaxTickRate"), if *rate == "30" { "35" } else { rate });
}

const TWEAKS: &[Tweak] = &[
    // ---- display
    custom("window_mode", "display", "Window mode", "Fullscreen, windowed, or borderless (instant alt-tab). Borderless is the remaster's default.",
        Control::Choice(WINDOW_MODES), read_window_mode, write_window_mode, C("2"), Impact::None, MENU),
    custom("resolution", "display", "Resolution", "Render resolution. Custom values already in your file show as Custom and are kept until you pick one.",
        Control::Choice(RESOLUTIONS), willow::read_resolution, willow::write_resolution, C("1920x1080"), Impact::High, MENU),
    slider("screen_percentage", "display", "Render scale", "Renders at a lower resolution and upscales. Below 100 trades sharpness for FPS.",
        &[key(E, SS, "ScreenPercentage")], (50.0, 100.0, 5.0), 0, "%", 100.0, Impact::High, NONE),
    slider("fov", "display", "Field of view", "Stored in the ini, so it sticks. BL1E is Vert- on wide screens, so ultrawide users want more.",
        &[key(E, SS, "FOVAngle")], (60.0, 120.0, 1.0), 0, "°", 75.0, Impact::Low, MENU),
    // ---- framerate
    choice("fps_mode", "framerate", "Framerate lock", "The menu's framerate cap. Pick your monitor's refresh rate, or Unlimited with a driver limiter.",
        &[key(E, SS, "FramerateMode")], FRAMERATE_MODES, "0", Impact::None, MENU),
    toggle("vsync", "framerate", "Vertical sync", "Sync to the monitor refresh. Adds input lag; prefer a framerate lock.",
        &[key(E, SS, "UseVsync")], TF, false, Impact::Low, MENU),
    toggle("coop_uncap", "framerate", "Lift the co-op client cap", "Co-op clients are held to about 90 FPS. Engine smoothing with a high maximum lifts it (set the maximum below).",
        &[key(E, "Engine.GameEngine", "bSmoothFrameRate")], TF_UPPER, false, Impact::None, NONE),
    slider("coop_max", "framerate", "Co-op framerate maximum", "Upper bound used when the co-op cap is lifted. Match your refresh rate.",
        &[key(E, "Engine.GameEngine", "MaxSmoothedFrameRate")], (30.0, 360.0, 1.0), 0, "fps", 62.0, Impact::None, NONE),
    toggle("one_frame_lag", "framerate", "One frame thread lag", "Off lowers input latency at a small FPS cost (PCGamingWiki mouse fix).",
        &[key(E, SS, "OneFrameThreadLag")], TF, true, Impact::Low, NONE),
    // ---- world detail
    slider("view_distance", "quality", "View distance", "Draw distance scale. The game's own presets use 0.5 (Low) to 1.0.",
        &[key(E, SS, "MaxDrawDistanceScale")], (0.3, 1.5, 0.05), 2, "×", 1.0, Impact::High, MENU),
    choice("detail_mode", "quality", "Detail mode", "Engine-level world detail (minor meshes and effects).",
        &[key(E, SS, "DetailMode")], LOW_MED_HIGH, "2", Impact::Low, NONE),
    choice("population", "quality", "Population density", "Ambient clutter and props, as the game's Low and Medium presets set it.",
        &[key(E, SS, "PopulationAdjustment")], POPULATION, "0", Impact::Medium, NONE),
    slider("foliage", "quality", "Foliage distance", "Grass and foliage draw radius.",
        &[key(E, SS, "FoliageDrawRadiusMultiplier")], (0.0, 1.5, 0.05), 2, "×", 1.0, Impact::Medium, MENU),
    toggle("speedtree_leaves", "quality", "Tree leaves", "SpeedTree leaf cards.",
        &[key(E, SS, "SpeedTreeLeaves")], TF, true, Impact::Low, NONE),
    toggle("speedtree_fronds", "quality", "Tree fronds", "SpeedTree fronds.",
        &[key(E, SS, "SpeedTreeFronds")], TF, true, Impact::Low, NONE),
    toggle("dynamic_decals", "quality", "Bullet decals", "Bullet holes and blood splats.",
        &[key(E, SS, "DynamicDecals")], TF, true, Impact::Low, MENU),
    slider("decal_distance", "quality", "Decal distance", "How far away decals are drawn.",
        &[key(E, SS, "DecalCullDistanceScale")], (0.25, 2.0, 0.05), 2, "×", 1.0, Impact::Low, NONE),
    toggle("dynamic_lights", "quality", "Dynamic lights", "Moving lights from effects and weapons.",
        &[key(E, SS, "DynamicLights")], TF, true, Impact::High, NONE),
    slider("light_cull", "quality", "Light draw distance", "Dynamic lights further than this are skipped.",
        &[key(E, SS, "CullLightsDistance")], (2000.0, 15000.0, 500.0), 0, "", 7000.0, Impact::Medium, NONE),
    choice("char_lod", "quality", "Character detail", "Higher values use lower-detail character models sooner.",
        &[key(E, SS, "SkeletalMeshLODBias")], &[opt("0", "Full"), opt("1", "Reduced"), opt("2", "Low")], "0", Impact::Low, NONE),
    choice("particle_lod", "quality", "Particle detail", "Higher values use cheaper particle effects.",
        &[key(E, SS, "ParticleLODBias")], &[opt("0", "Full"), opt("1", "Reduced"), opt("2", "Low")], "0", Impact::Low, NONE),
    // ---- textures
    choice("texture_quality", "textures", "Texture quality", "The menu's texture setting.",
        &[key(E, SS, "TextureQuality")], DETAIL_LOW_TO_HIGH, "0", Impact::Medium, MENU),
    custom("texture_bias", "textures", "Texture resolution cap", "Applies an LOD bias to world, character, weapon and vehicle textures. Half or Quarter saves VRAM on old GPUs.",
        Control::Choice(TEXTURE_BIAS), willow::read_texture_bias, willow::write_texture_bias, C("0"), Impact::Medium, NONE),
    slider("pool_size", "textures", "Texture pool size", "Streaming pool in MB. 2048–3072 reduces texture pop-in on GPUs with 4 GB or more.",
        &[key(E, "TextureStreaming", "PoolSize")], (600.0, 4096.0, 100.0), 0, "MB", 1200.0, Impact::Medium, NONE),
    choice("aniso", "textures", "Anisotropic filtering", "Texture sharpness at glancing angles.",
        &[key(E, SS, "MaxAnisotropy")], ANISO, "4", Impact::Low, MENU),
    // ---- shadows
    toggle("dynamic_shadows", "shadows", "Dynamic shadows", "Shadows from characters, vehicles and moving objects.",
        &[key(E, SS, "DynamicShadows")], TF, true, Impact::High, MENU),
    toggle("light_env_shadows", "shadows", "Character self-shadows", "Shadows characters cast from their light environment.",
        &[key(E, SS, "LightEnvironmentShadows")], TF, true, Impact::Medium, NONE),
    choice("shadow_res", "shadows", "Shadow resolution", "Sharpness of dynamic shadows. The game's Ultra preset uses 4096.",
        &[key(E, SS, "MaxShadowResolution")], SHADOW_RES, "1024", Impact::Medium, MENU),
    choice("shadow_filter", "shadows", "Shadow filtering", "Softness of shadow edges.",
        &[key(E, SS, "ShadowFilterQualityBias")], SHADOW_FILTER, "0", Impact::Low, NONE),
    toggle("pssm", "shadows", "Cascaded sun shadows", "Split shadow maps for the sun. Off on the game's Low and Medium presets.",
        &[key(E, SS, "bEnablePSSMShadows")], TF, true, Impact::Medium, NONE),
    // ---- outlines
    choice("post_chain", "outlines", "Outline style", "Classic draws the ink outlines. No Outlines keeps the cel colours without the ink.",
        &[key(E, "Engine.Engine", "DefaultPostProcessName")], POST_PROCESS, "WillowEngineMaterials.WillowScenePostProcess", Impact::Low, NONE),
    toggle("vivid_colors", "outlines", "Vivid colours", "Turns off the remaster's legacy desaturation for a more colourful image.",
        &[key(E, "PostEffects", "UseLegacyDesaturation")], crate::tweaks::TF_INV, false, Impact::None, NONE),
    // ---- post-processing
    custom("fxaa", "postfx", "Anti-aliasing (FXAA)", "The remaster's FXAA. Off is sharper but jaggier.",
        Control::Toggle, read_fxaa, write_fxaa, B(true), Impact::Low, MENU),
    toggle("ao", "postfx", "Ambient occlusion (HBAO+)", "Contact shadows in corners and crevices.",
        &[key(E, SS, "AmbientOcclusion"), key(E, "HBAO", "Enable")], TF, true, Impact::High, MENU),
    slider("hbao_radius", "postfx", "Ambient occlusion radius", "How far HBAO+ reaches.",
        &[key(E, "HBAO", "Radius")], (1.0, 12.0, 0.5), 1, "", 6.0, Impact::Low, NONE),
    slider("hbao_intensity", "postfx", "Ambient occlusion strength", "How dark HBAO+ makes contact shadows.",
        &[key(E, "HBAO", "Intensity")], (1.0, 12.0, 0.5), 1, "", 8.0, Impact::None, NONE),
    toggle("bloom", "postfx", "Bloom", "Glow around bright lights.",
        &[key(E, SS, "Bloom")], TF, true, Impact::Low, MENU),
    toggle("hq_bloom", "postfx", "High-quality bloom", "Smoother bloom (on in the game's Ultra preset).",
        &[key(E, SS, "UseHighQualityBloom")], TF, true, Impact::Low, NONE),
    toggle("dof", "postfx", "Depth of field", "Background blur, most noticeable when aiming.",
        &[key(E, SS, "DepthOfField")], TF, true, Impact::Medium, MENU),
    toggle("motion_blur", "postfx", "Motion blur", "Camera and object motion blur.",
        &[key(E, SS, "MotionBlur")], TF, false, Impact::Low, NONE),
    toggle("lens_flares", "postfx", "Lens flares", "Flares from bright lights.",
        &[key(E, SS, "LensFlares")], TF, true, Impact::Low, MENU),
    toggle("flare_outs", "postfx", "Light flare-outs", "Glare when looking at bright light sources.",
        &[key(E, SS, "FlareOuts")], TF, true, Impact::Low, NONE),
    toggle("distortion", "postfx", "Heat distortion", "Heat haze around fire and explosions.",
        &[key(E, SS, "Distortion")], TF, true, Impact::Medium, NONE),
    toggle("reflections", "postfx", "Reflections", "Dynamic reflections.",
        &[key(E, SS, "Reflections")], TF, true, Impact::Medium, EXPERIMENTAL),
    // ---- input
    choice("console_key", "input", "Console key", "Opens the developer console.",
        &[key(I, "Engine.Console", "ConsoleKey")], CONSOLE_KEYS, "", Impact::None, NONE),
    toggle("mouse_smoothing", "input", "Mouse smoothing", "Engine mouse smoothing. PCGamingWiki recommends turning it off for raw aim.",
        &[key(I, "Engine.PlayerInput", "bEnableMouseSmoothing")], TF_LOWER, true, Impact::None, NONE),
    choice("mouse_y", "input", "Vertical mouse speed", "BL1E turns slower vertically than horizontally. Match makes both axes equal (PCGamingWiki).",
        &[key(I, "Engine.PlayerInput", "LookUpScale")], MOUSE_Y, "-250", Impact::None, NONE),
    slider("mouse_sens", "input", "Mouse sensitivity", "Hip-fire sensitivity (the menu slider).",
        &[key(I, "Engine.PlayerInput", "MouseSensitivity"), key(I, "Engine.PlayerInput", "MouseSensitivityVertical")], (10.0, 100.0, 1.0), 0, "", 60.0, Impact::None, MENU),
    slider("scoped_sens", "input", "Scoped sensitivity", "Sensitivity while aiming down sights.",
        &[key(I, "Engine.PlayerInput", "ScopedMouseSensitivity"), key(I, "Engine.PlayerInput", "ScopedMouseSensitivityVertical")], (10.0, 100.0, 1.0), 0, "", 60.0, Impact::None, MENU),
    // ---- interface & audio
    toggle("subtitles", "interface", "Subtitles", "Dialogue subtitles.",
        &[key(E, SS, "Subtitles")], TF, false, Impact::None, MENU),
    toggle("mute_unfocused", "interface", "Mute when alt-tabbed", "Silence the game while it isn't the active window.",
        &[key(E, SS, "MuteAudioOnFocusLost")], TF, true, Impact::None, MENU),
    toggle("disable_win_key", "interface", "Disable the Windows key", "Stops the Start menu popping up mid-fight.",
        &[key(E, SS, "DisableWindowsKey")], TF, false, Impact::None, MENU),
    // ---- startup
    toggle("skip_intros", "startup", "Skip intro movies", "The game's own switch. Safe, unlike -nomoviestartup, which crashes BL1E.",
        &[key(E, SS, "DisableIntroMovies")], TF, false, Impact::None, MENU),
    toggle("no_launcher", "startup", "Skip the launcher", "Start straight into the game.",
        &[key(G, "Engine.GameInfo", "DisableLauncher")], TF, false, Impact::None, MENU),
    // ---- network
    custom("no_voice", "network", "Disable voice chat", "Turns off VoIP, which reduces co-op lag (PCGamingWiki).",
        Control::Toggle, read_voice, write_voice, B(false), Impact::None, NONE),
    custom("tick_rate", "network", "Host tick rate", "How often a co-op host updates clients. 60 is smoother but can break physics in places.",
        Control::Choice(TICK_RATES), read_tick, write_tick, C("30"), Impact::Low, EXPERIMENTAL),
];

/// The game's own four quality levels (WillowCompat.ini buckets), plus a
/// few extras on top.
const PRESETS: &[Preset] = &[
    Preset {
        id: "fixes",
        name: "Community Essentials",
        rarity: Rarity::Pearlescent,
        description: "The PCGamingWiki fixes: skip the launcher and intros, no mouse smoothing, even mouse axes, lower input lag, console on ~.",
        values: &[
            ("skip_intros", B(true)),
            ("no_launcher", B(true)),
            ("mouse_smoothing", B(false)),
            ("mouse_y", C("-300")),
            ("one_frame_lag", B(false)),
            ("console_key", C("Tilde")),
        ],
    },
    Preset {
        id: "clean",
        name: "Clean Look",
        rarity: Rarity::Epic,
        description: "No ink outlines, vivid colours, no motion blur or depth of field, 16× filtering.",
        values: &[
            ("post_chain", C("WillowEngineMaterials.RyanScenePostProcess")),
            ("vivid_colors", B(true)),
            ("motion_blur", B(false)),
            ("dof", B(false)),
            ("aniso", C("16")),
        ],
    },
    Preset {
        id: "potato",
        name: "Potato Mode",
        rarity: Rarity::Common,
        description: "The game's Low preset and then some: no AO, bloom, flares or dynamic shadows, half-resolution textures, sparse clutter.",
        values: &[
            ("view_distance", N(0.5)),
            ("detail_mode", C("0")),
            ("population", C("-40")),
            ("foliage", N(0.5)),
            ("texture_quality", C("2")),
            ("texture_bias", C("1")),
            ("aniso", C("1")),
            ("dynamic_shadows", B(false)),
            ("shadow_res", C("512")),
            ("pssm", B(false)),
            ("ao", B(false)),
            ("bloom", B(false)),
            ("hq_bloom", B(false)),
            ("dof", B(false)),
            ("lens_flares", B(false)),
            ("flare_outs", B(false)),
            ("dynamic_decals", B(false)),
            ("dynamic_lights", B(false)),
            ("screen_percentage", N(85.0)),
        ],
    },
    Preset {
        id: "deck",
        name: "Steam Deck",
        rarity: Rarity::Rare,
        description: "1280×800 at a locked 60: medium shadows, no AO, standard texture pool. Also good for handhelds and older laptops.",
        values: &[
            ("resolution", C("1280x800")),
            ("window_mode", C("0")),
            ("fps_mode", C("3")),
            ("vsync", B(false)),
            ("ao", B(false)),
            ("shadow_res", C("1024")),
            ("pool_size", N(1200.0)),
            ("foliage", N(0.75)),
            ("view_distance", N(0.85)),
        ],
    },
    Preset {
        id: "competitive",
        name: "Competitive FPS",
        rarity: Rarity::Uncommon,
        description: "Maximum framerate while keeping the art style: unlocked FPS, no AO, DOF, blur or distortion, low input lag.",
        values: &[
            ("fps_mode", C("8")),
            ("vsync", B(false)),
            ("one_frame_lag", B(false)),
            ("ao", B(false)),
            ("dof", B(false)),
            ("motion_blur", B(false)),
            ("distortion", B(false)),
            ("lens_flares", B(false)),
            ("shadow_res", C("1024")),
            ("mouse_smoothing", B(false)),
        ],
    },
    Preset {
        id: "balanced",
        name: "Balanced",
        rarity: Rarity::Rare,
        description: "The game's High preset: 2048 shadows and full detail, with AO and bloom, without the costliest extras.",
        values: &[
            ("view_distance", N(1.0)),
            ("population", C("0")),
            ("foliage", N(1.0)),
            ("texture_quality", C("0")),
            ("pool_size", N(2048.0)),
            ("aniso", C("8")),
            ("shadow_res", C("2048")),
            ("pssm", B(true)),
            ("ao", B(true)),
            ("hq_bloom", B(false)),
            ("dynamic_decals", B(true)),
        ],
    },
    Preset {
        id: "ultra",
        name: "Pandora Ultra",
        rarity: Rarity::Legendary,
        description: "The game's Ultra preset and beyond: 4096 shadows, extended view and foliage distance, a larger texture pool.",
        values: &[
            ("view_distance", N(1.25)),
            ("detail_mode", C("2")),
            ("population", C("0")),
            ("foliage", N(1.25)),
            ("texture_quality", C("0")),
            ("texture_bias", C("0")),
            ("pool_size", N(3072.0)),
            ("aniso", C("16")),
            ("shadow_res", C("4096")),
            ("pssm", B(true)),
            ("ao", B(true)),
            ("bloom", B(true)),
            ("hq_bloom", B(true)),
            ("lens_flares", B(true)),
            ("flare_outs", B(true)),
            ("dynamic_decals", B(true)),
            ("light_cull", N(10000.0)),
        ],
    },
    Preset {
        id: "vanilla",
        name: "Factory Settings",
        rarity: Rarity::Seraph,
        description: "Every setting back to the game's shipped default. Resolution and window mode are left alone.",
        values: &[],
    },
];

/// Native resolution and a framerate lock matching the monitor: a menu mode
/// when one fits, otherwise Unlimited with engine smoothing as the cap.
fn match_display(c: &mut ConfigSet, mode: DisplayMode) {
    c.set(&key(E, SS, "ResX"), &mode.width.to_string());
    c.set(&key(E, SS, "ResY"), &mode.height.to_string());
    let hz = mode.refresh_hz.max(60);
    let fps_mode = match hz {
        60 => "3",
        75 => "4",
        100 => "5",
        120 => "6",
        144 => "7",
        _ => "8",
    };
    c.set(&key(E, SS, "FramerateMode"), fps_mode);
    // Also lifts the co-op client cap and caps "Unlimited" at the refresh rate.
    c.set(&key(E, "Engine.GameEngine", "bSmoothFrameRate"), "TRUE");
    c.set(&key(E, "Engine.GameEngine", "MaxSmoothedFrameRate"), &hz.to_string());
}

const MODERN_DEFAULTS: &[(&str, crate::tweaks::DefaultValue)] = &[
    ("window_mode", C("2")),
    ("vsync", B(false)),
    ("skip_intros", B(true)),
    ("no_launcher", B(true)),
    ("console_key", C("Tilde")),
    ("mouse_smoothing", B(false)),
    ("mouse_y", C("-300")),
    ("one_frame_lag", B(false)),
    ("motion_blur", B(false)),
    ("aniso", C("16")),
    ("pool_size", N(2048.0)),
    ("fov", N(90.0)),
    ("subtitles", B(true)),
];

const DXVK_CONF: &str = "# Vault Patcher DXVK profile for Borderlands GOTY Enhanced\n\
dxgi.maxFrameLatency = 1\n\
dxgi.syncInterval = -1\n\
d3d11.samplerAnisotropy = 16\n";

const SETUP: &[Component] = &[
    Component {
        id: "modern",
        name: "Modern Defaults",
        summary: "Native resolution, framerate locked to your monitor, raw mouse, straight to the menu",
        description: "Borderless at native resolution with the framerate locked to your monitor's refresh rate (which also lifts the co-op client cap), a 90° field of view, no mouse smoothing and even mouse axes, lower input lag, 16× filtering, a larger texture pool, subtitles on, console on ~, and no launcher or intro movies.",
        group: Group::Essentials,
        recommended: true,
        kind: ComponentKind::Settings { values: MODERN_DEFAULTS, display: Some(match_display) },
        requires: &[],
    },
    Component {
        id: "exit_fix",
        name: "Exit Hang Fix",
        summary: "The game actually closes when you quit",
        description: "Fixes the game freezing instead of closing when you quit. A tiny proxy DLL (version.dll) by endjynn, recommended by PCGamingWiki.",
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
        id: "no_upsell",
        name: "Remove the Borderlands 3 Ad",
        summary: "No BL3 advert on the main menu",
        description: "Moves the main menu's Borderlands 3 upsell package aside (it's renamed, not deleted, and comes back if you untick this or verify files in Steam).",
        group: Group::Essentials,
        recommended: true,
        kind: ComponentKind::Hide { path: "WillowGame\\CookedPC\\Packages\\Interface\\ui_frontend_upsell_PC.upk" },
        requires: &[],
    },
    Component {
        id: "dxvk",
        name: "DXVK Vulkan Renderer",
        summary: "Smoother frame pacing on Vulkan",
        description: "Runs the Direct3D 11 renderer on Vulkan, which fixes the remaster's uneven frame pacing for many players. Don't combine with ReShade or Luma (they also use dxgi.dll). If the game stays at 1080p on a multi-monitor PC, remove it. Needs an up-to-date driver with Vulkan 1.4.",
        group: Group::Performance,
        recommended: false,
        kind: ComponentKind::Dxvk { target: DxvkTarget::D3d11Win64, exe_dir: "Binaries\\Win64", conf: DXVK_CONF },
        requires: &[],
    },
    Component {
        id: "ultrawide",
        name: "Ultrawide Fix",
        summary: "Correct field of view and HUD on 21:9 and 32:9",
        description: "PolarWizard's fix for ultrawide monitors (an ASI plugin loaded through winmm.dll). BL1E is Vert-, so without it wide screens see less, not more. Set the in-game FOV to 120 and don't change resolution in-game while it's installed. Only useful on screens wider than 16:9.",
        group: Group::Performance,
        recommended: false,
        kind: ComponentKind::Archive {
            url: "https://github.com/PolarWizard/BorderlandsGOTYEnhancedFix/releases/download/v2.1.0/BorderlandsGOTYEnhancedFix_v2.1.0.zip",
            dest: "Binaries\\Win64",
            skip: &["README.txt"],
            enable: None,
        },
        requires: &[],
    },
    Component {
        id: "sdk",
        name: "Python SDK (mod loader)",
        summary: "The community mod loader",
        description: "The Willow1 mod loader for the Enhanced edition: adds a MODS menu and runs SDK mods. Installs a dinput8.dll next to the game and needs the 64-bit Visual C++ runtime.",
        group: Group::Mods,
        recommended: true,
        kind: ComponentKind::Sdk,
        requires: &[],
    },
    bl1_mod("skill_ui_fix", "Skill Tree Fix", "Fixes skill tree display glitches", "Fixes skill tree UI glitches in the Enhanced edition. By Ry0511.", Group::Fixes, true,
        "https://raw.githubusercontent.com/Ry0511/my_bl1_sdk_mods/refs/heads/master/packaged/skill_tree_tweaks.sdkmod", "sdk_mods/skill_tree_tweaks.sdkmod", "skill_tree_tweaks"),
    Component {
        id: "bloodwing_fix",
        name: "Bloodwing Return Fix",
        summary: "Mordecai's Bloodwing always comes back",
        description: "Fixes Bloodwing sometimes never returning after an attack, leaving the action skill on cooldown. By RedxYeti.",
        group: Group::Fixes,
        recommended: true,
        kind: ComponentKind::Archive {
            url: "https://github.com/RedxYeti/Yeti-BL1-SDK-Mods/raw/refs/heads/main/BloodwingReturnFix/BloodwingReturnFix.zip",
            dest: "sdk_mods\\BloodwingReturnFix",
            skip: &[],
            enable: Some("BloodwingReturnFix"),
        },
        requires: &["sdk"],
    },
    bl1_mod("auto_pickup", "Auto Pickup", "Money, ammo and health picked up automatically", "Picks up money, ammo and health automatically, like the later games. By galqawala.", Group::Mods, true,
        "https://github.com/galqawala/AutopickupBL1E/raw/refs/heads/master/AutopickupBL1E.sdkmod", "sdk_mods/AutopickupBL1E.sdkmod", "AutopickupBL1E"),
    bl1_mod("hide_full", "Hide Full Pickups", "No prompts for ammo you can't carry", "Stops showing pickup prompts for ammo and health you can't carry. By EerieGoesD.", Group::Mods, true,
        "https://github.com/EerieGoesD/borderlands-1-goty-mods/raw/refs/heads/main/HideFullPickups/HideFullPickups.sdkmod", "sdk_mods/HideFullPickups.sdkmod", "HideFullPickups"),
    bl1_mod("quick_vendors", "Quick Use Vendors", "Refill health and ammo from a vendor with one key", "Buy ammo and health straight from a vendor without opening its menu, like BL3. By RedxYeti.", Group::Mods, true,
        "https://github.com/RedxYeti/Yeti-BL1-SDK-Mods/raw/refs/heads/main/QuickUseVendors/QuickUseVendors.sdkmod", "sdk_mods/QuickUseVendors.sdkmod", "QuickUseVendors"),
    bl1_mod("boss_bars", "Boss Health Bars", "Big health bars for bosses", "Shows a boss health bar at the top of the screen, like the later games. By Ry0511.", Group::Mods, false,
        "https://raw.githubusercontent.com/Ry0511/my_bl1_sdk_mods/refs/heads/master/packaged/boss_bars.sdkmod", "sdk_mods/boss_bars.sdkmod", "boss_bars"),
    bl1_mod("auto_save", "Configurable Auto-Save", "Frequent, configurable auto-saves", "More frequent, configurable auto-saves like modern games. By Ry0511.", Group::Mods, false,
        "https://raw.githubusercontent.com/Ry0511/my_bl1_sdk_mods/refs/heads/master/packaged/rys_auto_save.sdkmod", "sdk_mods/rys_auto_save.sdkmod", "rys_auto_save"),
    bl1_mod("extended_options", "Extended Options", "More settings in the game's own menu", "Adds options the remaster hides to its in-game menu: FPS slider, window mode, outlines, crosshair, and scrolling in pause menus. By juso40.", Group::Mods, false,
        "https://github.com/juso40/bl1sdk-mods/raw/refs/heads/main/extended_options/extended_options.sdkmod", "sdk_mods/extended_options.sdkmod", "extended_options"),
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
    Component { id, name, summary, description, group, recommended, kind: ComponentKind::File { url, dest, enable: Some(module) }, requires: &["sdk"] }
}

const QUICK: &[super::QuickSection] = &[
    super::QuickSection {
        title: "Look & Feel",
        blurb: "The classic comic-book ink, or a cleaner modern look.",
        tweaks: &["post_chain", "vivid_colors", "motion_blur", "dof"],
        quality_presets: &[],
    },
    super::QuickSection {
        title: "Graphics Quality",
        blurb: "Pick a starting point, then fine-tune the essentials.",
        tweaks: &["view_distance", "shadow_res", "ao", "fxaa"],
        quality_presets: &["potato", "balanced", "ultra"],
    },
    super::QuickSection {
        title: "Display",
        blurb: "How the game fits your screen.",
        tweaks: &["window_mode", "resolution", "fps_mode", "vsync", "fov"],
        quality_presets: &[],
    },
    super::QuickSection {
        title: "Comfort",
        blurb: "Small things that make the old game feel new.",
        tweaks: &["skip_intros", "no_launcher", "mouse_smoothing", "mouse_y", "subtitles", "console_key"],
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
    core_mods: &["mods_base", "console_mod_menu", "keybinds", "networking", "save_options", "ui_utils", "willow1_mod_menu"],
    legacy_markers: &[],
    redist_url: "https://aka.ms/vs/17/release/vc_redist.x64.exe",
    links: &[
        ("SDK mod database", "https://bl-sdk.github.io/willow1-mod-db/"),
        ("Nexus Mods", "https://www.nexusmods.com/borderlandsgotyenhanced"),
        ("PCGamingWiki", "https://www.pcgamingwiki.com/wiki/Borderlands:_Game_of_the_Year_Enhanced"),
    ],
};

const LAUNCH_ARGS: &[LaunchArg] = &[
    LaunchArg {
        arg: "-nosplash",
        label: "No splash screen",
        description: "Skip the splash image while the game loads.",
        default_on: true,
    },
    LaunchArg {
        arg: "-nohomedir",
        label: "Saves next to the game",
        description: "Fixes saving when your Documents folder has been moved (e.g. to OneDrive). Saves then live in the game folder.",
        default_on: false,
    },
    LaunchArg {
        arg: "-log",
        label: "Log window",
        description: "Open a live engine log window. Handy for debugging mods.",
        default_on: false,
    },
];

const NAV: &[NavGroup] = &[
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
            NavItem { kind: PageKind::Tweaks("graphics"), title: "Graphics", icon: Icon::Picture, categories: &["quality"] },
            NavItem { kind: PageKind::Tweaks("textures"), title: "Textures & Shadows", icon: Icon::Texture, categories: &["textures", "shadows"] },
            NavItem { kind: PageKind::Tweaks("effects"), title: "Outlines & Effects", icon: Icon::Sparkle, categories: &["outlines", "postfx"] },
            NavItem { kind: PageKind::Tweaks("controls"), title: "Controls & Interface", icon: Icon::Mouse, categories: &["input", "interface"] },
            NavItem { kind: PageKind::Tweaks("system"), title: "Startup & Network", icon: Icon::Rocket, categories: &["startup", "network"] },
        ],
    },
    NavGroup {
        title: "Modding",
        items: &[NavItem { kind: PageKind::Mods, title: "Mods", icon: Icon::Puzzle, categories: &[] }],
    },
    COMMON_NAV,
];

/// Settings with captured comparison images (no Nvidia guide exists for BL1E).
const COMPARISONS: &[Comparison] = &[
    Comparison { tweak: "post_chain", link: None, capture: true },
    Comparison { tweak: "vivid_colors", link: None, capture: true },
    Comparison { tweak: "ao", link: None, capture: true },
    Comparison { tweak: "fxaa", link: None, capture: true },
    Comparison { tweak: "shadow_res", link: None, capture: true },
    Comparison { tweak: "dynamic_shadows", link: None, capture: true },
    Comparison { tweak: "texture_quality", link: None, capture: true },
    Comparison { tweak: "bloom", link: None, capture: true },
    Comparison { tweak: "dof", link: None, capture: true },
    Comparison { tweak: "population", link: None, capture: true },
    Comparison { tweak: "detail_mode", link: None, capture: true },
];

/// The remaster has no load-into-save switch, so the helper drives the title
/// screen (Continue, single player, start) into the last-played character.
/// Never `-nomoviestartup`: it crashes BL1E.
const CAPTURE: crate::compare::CaptureProfile = crate::compare::CaptureProfile {
    load: crate::compare::CaptureLoad::MainMenu,
    launch_args: &["-nosplash"],
    settings: &[
        (E, SS, "WindowMode", "2"),
        (E, SS, "Fullscreen", "False"),
        (E, SS, "Subtitles", "False"),
        (E, "Engine.Engine", "bSubtitlesForcedOff", "TRUE"),
        (E, SS, "DisableIntroMovies", "True"),
        (G, "Engine.GameInfo", "DisableLauncher", "True"),
    ],
    // Fyrestone's spawn faces a wall; a quarter turn left looks down the street.
    turn: 49152,
    hud: crate::compare::HudHide::CloseMovie,
    prerequisite: None,
    saves: r"..\..\Binaries\SaveData",
    save_subfolders: false,
};

pub static GAME: GameDef = GameDef {
    id: "bl1e",
    name: "Borderlands GOTY Enhanced",
    short: "BL1E",
    tagline: "Where it all began, remastered.",
    support: Support::Full,
    steam_app_ids: &[729040],
    epic_names: &["Borderlands Game of the Year"],
    exe: "Binaries\\Win64\\BorderlandsGOTY.exe",
    launcher: None,
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
    comparisons: COMPARISONS,
    capture: Some(&CAPTURE),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ini::IniDoc;

    #[test]
    fn catalog_references_are_valid() {
        let mut ids = std::collections::HashSet::new();
        for t in TWEAKS {
            assert!(ids.insert(t.id), "duplicate tweak id {}", t.id);
            assert!(CATEGORIES.iter().any(|c| c.id == t.category), "{} has unknown category", t.id);
        }
        for p in PRESETS {
            for (id, _) in p.values {
                assert!(ids.contains(id), "preset {} references {id}", p.id);
            }
        }
        for s in QUICK {
            s.tweaks.iter().for_each(|id| assert!(ids.contains(id), "quick section {} references {id}", s.title));
            s.quality_presets.iter().for_each(|id| assert!(PRESETS.iter().any(|p| p.id == *id), "quick preset {id}"));
        }
        for (id, _) in MODERN_DEFAULTS {
            assert!(ids.contains(id), "modern defaults reference {id}");
        }
        for c in COMPARISONS {
            let t = TWEAKS.iter().find(|t| t.id == c.tweak).expect("comparison tweak exists");
            assert!(!crate::compare::capture_values(t).is_empty(), "{} can't be captured (slider?)", t.id);
        }
        // Every category is reachable from the navigation.
        for c in CATEGORIES {
            assert!(NAV.iter().flat_map(|g| g.items).any(|i| i.categories.contains(&c.id)), "category {} not in nav", c.id);
        }
    }

    #[test]
    fn window_mode_keeps_fullscreen_consistent() {
        let mut c = ConfigSet::from_docs(&[(E, IniDoc::parse("[SystemSettings]\r\nFullscreen=False\r\nWindowMode=2\r\n"))]);
        let t = TWEAKS.iter().find(|t| t.id == "window_mode").unwrap();
        t.write(&mut c, &Value::Choice("0"));
        assert_eq!(c.get(&key(E, SS, "Fullscreen")), Some("True"));
        assert_eq!(t.read(&c), Some(Value::Choice("0")));
        t.write(&mut c, &Value::Choice("2"));
        assert_eq!(c.get(&key(E, SS, "Fullscreen")), Some("False"));
    }

    fn config_dir() -> Option<std::path::PathBuf> {
        let dir = dirs::document_dir()?.join(r"My Games\Borderlands Game of the Year\WillowGame\Config");
        dir.is_dir().then_some(dir)
    }

    /// Reads the real config (read-only): every file round-trips byte for
    /// byte and every setting finds its key.
    #[test]
    #[ignore = "reads a local BL1E config"]
    fn live_every_tweak_reads() {
        let Some(dir) = config_dir() else { return };
        for (_, name) in GAME.ini_files {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            assert_eq!(IniDoc::from_bytes(&bytes).to_bytes(), bytes, "{name} did not round-trip");
        }
        let config = ConfigSet::load(&dir, GAME.ini_files);
        for t in TWEAKS {
            let v = t.read(&config);
            println!("{:<20} {:?}", t.id, v);
            assert!(v.is_some(), "{} found no key in the live config", t.id);
            assert!(!matches!(v, Some(Value::Unknown(_))) || t.id == "resolution", "{} read an unknown value", t.id);
        }
    }

    /// Copies the real config to a temp folder, changes every setting, saves,
    /// reloads and checks each reads back. Never touches the originals.
    #[test]
    #[ignore = "reads a local BL1E config"]
    fn live_every_tweak_writes_and_reads_back() {
        let Some(dir) = config_dir() else { return };
        let sandbox = std::env::temp_dir().join("vaultpatcher-bl1e-sandbox");
        let _ = std::fs::remove_dir_all(&sandbox);
        std::fs::create_dir_all(&sandbox).unwrap();
        for (_, name) in GAME.ini_files {
            std::fs::copy(dir.join(name), sandbox.join(name)).unwrap();
        }
        let mut config = ConfigSet::load(&sandbox, GAME.ini_files);
        let mut expected = Vec::new();
        for t in TWEAKS {
            let current = t.read(&config);
            let next = match t.control {
                Control::Toggle => Value::Bool(!matches!(current, Some(Value::Bool(true)))),
                Control::Slider { min, max, .. } => {
                    if current.as_ref().and_then(Value::as_num) == Some(max) { Value::Num(min) } else { Value::Num(max) }
                }
                Control::Choice(options) => {
                    let o = options.iter().find(|o| current != Some(Value::Choice(o.value)) && !o.value.is_empty()).unwrap();
                    Value::Choice(o.value)
                }
            };
            t.write(&mut config, &next);
            expected.push((t, next));
        }
        config.save_dirty().unwrap();
        let reloaded = ConfigSet::load(&sandbox, GAME.ini_files);
        for (t, want) in &expected {
            assert_eq!(t.read(&reloaded).as_ref(), Some(want), "{} did not read back", t.id);
        }
        for (_, name) in GAME.ini_files {
            let before = std::fs::read_to_string(dir.join(name)).unwrap();
            let after = std::fs::read_to_string(sandbox.join(name)).unwrap();
            assert_eq!(before.lines().count(), after.lines().count(), "{name}: settings should edit lines in place, not add them");
        }
        let _ = std::fs::remove_dir_all(&sandbox);
    }
}
