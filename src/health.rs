//! The Overview health checklist and the "copy diagnostics" report: both are
//! read-only views over the workspace.

use std::fmt::Write as _;

use crate::core::backup;
use crate::core::binpatch::PatchState;
use crate::games::PageKind;
use crate::mods::SdkStatus;
use crate::setup::Status;
use crate::workspace::Workspace;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Ok,
}

pub struct Check {
    pub level: Level,
    pub title: String,
    pub detail: String,
    /// Where to go to fix it.
    pub fix: Option<(&'static str, PageKind)>,
}

fn check(level: Level, title: impl Into<String>, detail: impl Into<String>, fix: Option<(&'static str, PageKind)>) -> Check {
    Check { level, title: title.into(), detail: detail.into(), fix }
}

pub fn checks(ws: &Workspace) -> Vec<Check> {
    let game = ws.game();
    let def = game.def;
    let mut out = Vec::new();

    match &game.install {
        Some(i) => out.push(check(Level::Ok, "Game found", format!("{} · {}", i.store.label(), i.root.display()), None)),
        None => out.push(check(
            Level::Error,
            "Game not found",
            "Patches, DXVK and mods need the install folder.",
            Some(("Set folder", PageKind::Settings)),
        )),
    }

    let unreadable: Vec<String> = game
        .config
        .unreadable()
        .filter_map(|f| f.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    if !game.config_found() {
        out.push(check(
            Level::Error,
            "Settings files not found",
            "Launch the game once so it creates them, or pick the folder.",
            Some(("Set folder", PageKind::Settings)),
        ));
    } else if !unreadable.is_empty() {
        out.push(check(
            Level::Error,
            "Some settings files can't be read",
            format!("{} — close the game or check antivirus/OneDrive.", unreadable.join(", ")),
            None,
        ));
    } else {
        out.push(check(Level::Ok, "Settings files readable", "All config files load cleanly.", None));
    }

    let read_only: Vec<String> = game
        .config
        .files()
        .filter(|(_, f)| f.exists && backup::is_readonly(&f.path))
        .filter_map(|(_, f)| f.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    if !read_only.is_empty() && !ws.settings.lock_configs_after_apply {
        out.push(check(
            Level::Warn,
            "Settings files are read-only",
            format!("{} — the in-game menu can't save changes.", read_only.join(", ")),
            Some(("Settings", PageKind::Settings)),
        ));
    }

    if game.is_running() {
        out.push(check(Level::Info, "Game is running", "Close it before changing settings or installing anything.", None));
    }

    if game.config_found() && !game.backups.iter().any(|b| b.label == backup::ORIGINAL_LABEL) {
        out.push(check(
            Level::Info,
            "No original-settings backup yet",
            "One is made automatically before the first change.",
            None,
        ));
    }

    if let Some(state) = game.exe.as_ref().and_then(|e| e.patch_states.get("laa")) {
        match state {
            PatchState::Patched => out.push(check(Level::Ok, "4 GB memory patch applied", "Fewer out-of-memory crashes with mods and high settings.", None)),
            PatchState::Unpatched => out.push(check(
                Level::Warn,
                "4 GB memory patch not applied",
                "The game can only use 2 GB of RAM, which causes crashes with mods and high textures.",
                Some(("Fix", PageKind::Setup)),
            )),
            PatchState::Unsupported => {}
        }
    }

    if def.mods.is_some() && game.install.is_some() {
        match &game.sdk {
            SdkStatus::Installed(v) => match ws.updates.sdk.as_deref() {
                Some(latest) if latest != v => out.push(check(
                    Level::Warn,
                    format!("Mod SDK update: {v} → {latest}"),
                    "Update from the Mods page.",
                    Some(("Mods", PageKind::Mods)),
                )),
                _ => out.push(check(Level::Ok, format!("Mod SDK {v}"), "Up to date.", None)),
            },
            SdkStatus::Detected => out.push(check(Level::Ok, "Mod SDK installed", "Installed outside Vault Patcher.", None)),
            SdkStatus::Legacy => out.push(check(
                Level::Warn,
                "Old PythonSDK installed",
                "The pre-2024 SDK doesn't run current mods; replace it with the Willow2 SDK.",
                Some(("Mods", PageKind::Mods)),
            )),
            SdkStatus::NotInstalled => out.push(check(Level::Info, "No mod SDK", "Needed for bug-fix and quality-of-life mods.", Some(("Mods", PageKind::Mods)))),
        }
    }

    if let Some(dxvk) = def.component("dxvk") {
        match ws.component_status(dxvk) {
            Status::Active(_) => out.push(check(Level::Ok, "DXVK installed", "Vulkan renderer for steadier frame times.", None)),
            Status::Blocked(reason) => out.push(check(Level::Info, "DXVK unavailable", reason, None)),
            _ => {}
        }
    }

    if !game.pending.is_empty() {
        out.push(check(
            Level::Info,
            format!("{} change(s) waiting", game.pending.len()),
            "Nothing is written until you press Apply.",
            None,
        ));
    }

    // Something rewrote what Vault Patcher last saved — the game's launcher
    // keeping its own video settings, an in-game menu, "verify files"...
    let drifted = crate::applied::resolve(def, &game.applied)
        .iter()
        .filter(|(t, want)| game.current(t).as_ref() != Some(want))
        .count();
    let reverted = applied_patches_reverted(game);
    if drifted + reverted > 0 {
        let mut parts = Vec::new();
        if drifted > 0 {
            parts.push(format!("{drifted} setting(s)"));
        }
        if reverted > 0 {
            parts.push(format!("{reverted} exe patch(es)"));
        }
        out.push(check(
            Level::Warn,
            format!("{} changed outside Vault Patcher", parts.join(" · ")),
            "The game or its launcher rewrote them — Re-apply puts them all back.",
            Some(("Re-apply", PageKind::Setup)),
        ));
    }

    out.sort_by_key(|c| c.level);
    out
}

/// Exe patches recorded as applied that are currently back to unpatched.
fn applied_patches_reverted(game: &crate::workspace::GameState) -> usize {
    let Some(exe) = game.exe.as_ref() else {
        return 0;
    };
    game.applied
        .patches
        .iter()
        .filter(|id| exe.patch_states.get(id.as_str()) == Some(&PatchState::Unpatched))
        .count()
}

/// A plain-text report for bug reports: paths, versions and states, no
/// personal files.
pub fn diagnostics(ws: &Workspace) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "Vault Patcher {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(s, "OS: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    match crate::core::gpu::best() {
        Some(g) => {
            let _ = writeln!(s, "GPU: {} (Vulkan {}.{}, vendor {:#06x})", g.name, g.api_major, g.api_minor, g.vendor_id);
        }
        None => {
            let _ = writeln!(s, "GPU: no Vulkan driver");
        }
    }
    let _ = writeln!(s, "Mode: {:?} · sounds: {}", ws.mode(), if crate::sound::available() { "found" } else { "not found" });
    for (gi, game) in ws.games.iter().enumerate() {
        let def = game.def;
        let _ = writeln!(s, "\n[{}]", def.name);
        match &game.install {
            Some(i) => {
                let _ = writeln!(s, "Install: {} ({})", i.root.display(), i.store.label());
            }
            None => {
                let _ = writeln!(s, "Install: not found");
            }
        }
        let _ = writeln!(s, "Config: {}", game.config_dir.as_ref().map_or("none".into(), |d| d.display().to_string()));
        for (id, f) in game.config.files() {
            let state = match (f.exists, f.readable, backup::is_readonly(&f.path)) {
                (false, ..) => "missing",
                (true, false, _) => "UNREADABLE",
                (true, true, true) => "read-only",
                _ => "ok",
            };
            let _ = writeln!(s, "  {id}: {state}");
        }
        if let Some(exe) = &game.exe {
            let mut patches: Vec<String> = exe.patch_states.iter().map(|(id, st)| format!("{id}={st:?}")).collect();
            patches.sort();
            let _ = writeln!(s, "Exe: {} bytes · {}", exe.size, patches.join(" "));
        }
        let _ = writeln!(s, "SDK: {:?}", game.sdk);
        let enabled = game.mods.iter().filter(|m| m.enabled).count();
        let _ = writeln!(s, "Mods: {} ({} enabled)", game.mods.len(), enabled);
        for m in &game.mods {
            let _ = writeln!(s, "  {} {}{}", if m.enabled { "[x]" } else { "[ ]" }, m.name, m.version.as_deref().map(|v| format!(" {v}")).unwrap_or_default());
        }
        let upgrades: Vec<String> = def
            .setup
            .iter()
            .map(|c| {
                let st = match ws.component_status_for(gi, c) {
                    Status::Active(_) => "on",
                    Status::Partial => "partial",
                    Status::Missing => "off",
                    Status::Blocked(_) => "blocked",
                };
                format!("{}={st}", c.id)
            })
            .collect();
        let _ = writeln!(s, "Upgrades: {}", upgrades.join(" "));
        let _ = writeln!(s, "Launch: {}", ws.launch_args_for(gi).join(" "));
        let _ = writeln!(s, "Backups: {} · pending changes: {}", game.backups.len(), game.pending.len());
    }
    // Keep the Windows user name out of shared reports.
    match dirs::home_dir().map(|h| h.display().to_string()).filter(|h| h.len() > 3) {
        Some(home) => s.replace(&home, "%USERPROFILE%"),
        None => s,
    }
}
