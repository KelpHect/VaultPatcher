//! Maintainer command line. Not needed by players; used to produce the
//! comparison image set that the app downloads.
//!
//! `VaultPatcher.exe --capture <game> [--save Save0001.sav] [--settle 25] [--only a,b] [--limit N]`
//! `VaultPatcher.exe --package-comparisons <game> [--out comparisons-<game>.zip]`
//! `VaultPatcher.exe --install-sdk <game>`

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

use crate::compare::{self, CaptureProgress, CaptureRequest};
use crate::core::detect;
use crate::games;
use crate::mods::{self, SdkStatus};
use crate::setup::{self, ComponentKind};
use crate::workspace::AppSettings;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// Returns `Some(exit code)` if the arguments asked for a CLI command.
pub fn run(args: &[String]) -> Option<i32> {
    if let Some(game) = arg(args, "--package-comparisons") {
        let out = arg(args, "--out").unwrap_or_else(|| format!("comparisons-{game}.zip"));
        return Some(match compare::package(&game, std::path::Path::new(&out)) {
            Ok(n) => {
                println!("packed {n} image(s) into {out}");
                0
            }
            Err(e) => {
                eprintln!("error: {e:#}");
                1
            }
        });
    }
    if let Some(game) = arg(args, "--install-sdk") {
        return Some(match install_sdk(&game) {
            Ok(v) => {
                println!("installed {v}");
                0
            }
            Err(e) => {
                eprintln!("error: {e:#}");
                1
            }
        });
    }
    let game = arg(args, "--capture")?;
    Some(match capture(&game, args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    })
}

/// Installs (or updates) a game's mod SDK exactly as One-Click Setup does.
fn install_sdk(game_id: &str) -> Result<String> {
    let def = games::all().iter().find(|g| g.id == game_id).context("unknown game id")?;
    let settings = AppSettings::load();
    let root = settings
        .manual_installs
        .get(def.id)
        .cloned()
        .or_else(|| detect::detect(&def.detect_spec()).map(|i| i.root))
        .context("game install not found")?;
    if compare::game_running(&root.join(def.exe)) {
        bail!("close the game first");
    }
    let support = def.mods.context("this game has no SDK support")?;
    mods::install_sdk_latest(def.id, support, &root)
}

fn capture(game_id: &str, args: &[String]) -> Result<()> {
    let def = games::all().iter().find(|g| g.id == game_id).context("unknown game id")?;
    let settings = AppSettings::load();
    let root = settings
        .manual_installs
        .get(def.id)
        .cloned()
        .or_else(|| detect::detect(&def.detect_spec()).map(|i| i.root))
        .context("game install not found")?;
    let config_dir = settings
        .manual_config_dirs
        .get(def.id)
        .cloned()
        .or_else(|| def.default_config_dir())
        .context("config folder not found")?;
    println!("game:   {}\nconfig: {}", root.display(), config_dir.display());

    // Prerequisites, installed the same way One-Click Setup does.
    let support = def.mods.context("this game has no SDK support")?;
    if matches!(mods::sdk_status(def.id, support, &root), SdkStatus::NotInstalled | SdkStatus::Legacy) {
        println!("installing {}…", support.sdk_name);
        println!("  {}", mods::install_sdk_latest(def.id, support, &root)?);
    }
    let profile = def.capture.context("comparison capture isn't available for this game")?;
    if let Some(prereq) = profile.prerequisite.and_then(|id| def.component(id))
        && let ComponentKind::File { url, dest, enable } = prereq.kind
        && !root.join(dest).is_file()
    {
        println!("installing {}…", prereq.name);
        setup::install_file(def.id, prereq.id, &root, url, dest, enable)?;
    }

    let save = match arg(args, "--save") {
        Some(s) => s,
        None => compare::find_saves(&config_dir, profile).into_iter().next().context("no save files found")?,
    };
    let settle = arg(args, "--settle").and_then(|s| s.parse().ok()).unwrap_or(25);
    let only: Option<Vec<String>> = arg(args, "--only").map(|s| s.split(',').map(str::to_string).collect());
    let limit: usize = arg(args, "--limit").and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);

    let shots: Vec<_> = def
        .comparisons
        .iter()
        .filter(|c| c.capture && only.as_ref().is_none_or(|o| o.iter().any(|t| t == c.tweak)))
        .filter_map(|c| def.tweak(c.tweak))
        .flat_map(|t| compare::capture_values(t).into_iter().map(move |(v, l)| (t, v, l)))
        .take(limit)
        .collect();
    if shots.is_empty() {
        bail!("nothing to capture");
    }
    println!("save:   {save}\nsettle: {settle}s\nshots:  {}", shots.len());

    let progress = Arc::new(Mutex::new(CaptureProgress { total: shots.len(), ..Default::default() }));
    let request = CaptureRequest {
        game_id: def.id,
        exe: root.join(def.exe),
        root,
        config_dir,
        ini_files: def.ini_files,
        save,
        settle_seconds: settle,
        shots,
        profile,
    };
    let worker = {
        let progress = progress.clone();
        std::thread::spawn(move || compare::run(request, progress))
    };
    let mut printed = 0;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let p = progress.lock().expect("progress lock").clone();
        for line in &p.log[printed..] {
            println!("[{}/{}] {line}", p.log.len(), p.total);
        }
        printed = p.log.len();
        if worker.is_finished() {
            break;
        }
    }
    let captured = worker.join().expect("capture thread panicked")?;
    println!("done: {captured} image(s) in {}", compare::comparisons_dir().display());
    Ok(())
}

