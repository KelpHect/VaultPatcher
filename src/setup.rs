//! One-click setup components (Simple mode). A game lists the components its
//! "Patch" button installs; each knows how to detect, install and remove
//! itself. Components that download run on a background thread.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::core::display::DisplayMode;
use crate::core::manifest::{self, Manifest};
use crate::core::{backup, binpatch::PatchState, net};
use crate::mods::SdkStatus;
use crate::tweaks::{ConfigSet, DefaultValue};
use crate::workspace::GameState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Essentials,
    Performance,
    Fixes,
    Mods,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Essentials => "Essentials",
            Group::Performance => "Performance",
            Group::Fixes => "Bug Fixes",
            Group::Mods => "Mods & Quality of Life",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DxvkTarget {
    /// 32-bit Direct3D 9 games (BL2, TPS): `x32/d3d9.dll`.
    D3d9Win32,
    /// 64-bit Direct3D 11 games (BL1E): `x64/d3d11.dll` + `x64/dxgi.dll`.
    D3d11Win64,
}

impl DxvkTarget {
    fn dlls(self) -> &'static [&'static str] {
        match self {
            DxvkTarget::D3d9Win32 => &["d3d9.dll"],
            DxvkTarget::D3d11Win64 => &["d3d11.dll", "dxgi.dll"],
        }
    }

    fn archive_dir(self) -> &'static str {
        match self {
            DxvkTarget::D3d9Win32 => "x32",
            DxvkTarget::D3d11Win64 => "x64",
        }
    }
}

/// Writes display-dependent values (native resolution, refresh-rate cap).
pub type DisplayHook = fn(&mut ConfigSet, DisplayMode);

#[derive(Clone, Copy)]
pub enum ComponentKind {
    /// A curated set of tweak values, plus an optional display hook.
    Settings {
        values: &'static [(&'static str, DefaultValue)],
        display: Option<DisplayHook>,
    },
    /// One of the game's `ExePatch`es, by id.
    ExePatch(&'static str),
    /// A launch switch added to the Play button.
    LaunchArg(&'static str),
    /// DXVK from its latest GitHub release, plus a tuned `dxvk.conf`.
    Dxvk {
        target: DxvkTarget,
        /// Folder of the game exe, relative to the install root.
        exe_dir: &'static str,
        conf: &'static str,
    },
    /// The game's Python SDK (see `GameDef::mods`).
    Sdk,
    /// A file downloaded into the install folder (an SDK mod, a text mod, a
    /// proxy DLL fix...). `dest` is relative to the install root.
    File {
        url: &'static str,
        dest: &'static str,
        /// SDK module to switch on after install (new SDK mods start disabled).
        enable: Option<&'static str>,
    },
    /// A zipped SDK mod whose single top-level `folder` goes into `sdk_mods`.
    SdkZip { url: &'static str, folder: &'static str },
    /// Text mod sources merged (with every other `TextPatch` component of
    /// the game) into one offline file that Text Mod Loader auto-runs.
    TextPatch {
        /// BLCMM game tag ("BL2" / "TPS").
        game: &'static str,
        /// Gearbox's official hotfixes, which an offline patch must carry.
        gearbox_url: &'static str,
        sources: &'static [TextSource],
    },
}

/// One upstream text mod, downloaded on the user's machine and filtered to
/// the categories we want. Nothing is redistributed by Vault Patcher.
#[derive(Clone, Copy)]
pub struct TextSource {
    pub title: &'static str,
    pub credit: &'static str,
    pub url: &'static str,
    /// Category paths (`A/B/C`) to keep; empty keeps everything.
    pub include: &'static [&'static str],
    /// Category paths to drop even if included.
    pub exclude: &'static [&'static str],
}

/// Where the merged community patch lives, relative to the install root.
/// Text Mod Loader only scans files directly inside `Binaries`.
pub const TEXT_PATCH_FILE: &str = "Binaries/VaultPatcher.blcm";

#[derive(Clone, Copy)]
pub struct Component {
    pub id: &'static str,
    pub name: &'static str,
    /// One short line shown in lists.
    pub summary: &'static str,
    /// The full explanation, shown when the row is expanded.
    pub description: &'static str,
    pub group: Group,
    /// Pre-selected in the setup list.
    pub recommended: bool,
    pub kind: ComponentKind,
    /// Components that must be installed first (e.g. SDK mods need the SDK).
    pub requires: &'static [&'static str],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Active(Option<String>),
    Partial,
    Missing,
    /// Can't be installed right now; the string says why.
    Blocked(String),
}

impl Status {
    pub fn is_active(&self) -> bool {
        matches!(self, Status::Active(_))
    }
}

pub fn status(c: &Component, game: &GameState, launch_args: &[String]) -> Status {
    let root = game.install.as_ref().map(|i| i.root.as_path());
    let needs_install = || Status::Blocked("Game install not found".into());
    match c.kind {
        ComponentKind::Settings { values, .. } => {
            if !game.config_found() {
                return Status::Blocked("Launch the game once to create its config".into());
            }
            // Tweaks hidden for this game don't count.
            let applicable: Vec<_> = values
                .iter()
                .filter_map(|(id, v)| game.def.tweak(id).map(|t| (t, v)))
                .collect();
            let matching = applicable
                .iter()
                .filter(|(t, v)| game.current(t).unwrap_or_else(|| t.default.to_value()) == v.to_value())
                .count();
            if matching == applicable.len() {
                Status::Active(None)
            } else if matching * 2 >= applicable.len() {
                Status::Partial
            } else {
                Status::Missing
            }
        }
        ComponentKind::ExePatch(id) => match game.exe.as_ref().and_then(|e| e.patch_states.get(id)) {
            Some(PatchState::Patched) => Status::Active(None),
            Some(PatchState::Unpatched) => Status::Missing,
            Some(PatchState::Unsupported) => Status::Blocked("Unrecognized exe version".into()),
            None => needs_install(),
        },
        ComponentKind::LaunchArg(arg) => {
            if launch_args.iter().any(|a| a.eq_ignore_ascii_case(arg)) {
                Status::Active(None)
            } else {
                Status::Missing
            }
        }
        ComponentKind::Dxvk { target, exe_dir, .. } => {
            let Some(root) = root else { return needs_install() };
            let dir = root.join(exe_dir);
            let present = target.dlls().iter().all(|d| dir.join(d).is_file());
            match manifest::read(game.def.id, c.id) {
                Some(m) if present => Status::Active(Some(m.version)),
                _ if present => Status::Blocked(format!(
                    "Another {} is already installed (ReShade or a manual DXVK?)",
                    target.dlls()[0]
                )),
                _ => Status::Missing,
            }
        }
        ComponentKind::Sdk => match (&game.sdk, root) {
            (_, None) => needs_install(),
            (SdkStatus::Installed(v), _) => Status::Active(Some(v.clone())),
            (SdkStatus::Detected, _) => Status::Active(None),
            (SdkStatus::Legacy, _) => Status::Partial,
            (SdkStatus::NotInstalled, _) => Status::Missing,
        },
        ComponentKind::SdkZip { folder, .. } => {
            let Some(root) = root else { return needs_install() };
            if root.join("sdk_mods").join(folder).is_dir() {
                Status::Active(None)
            } else {
                Status::Missing
            }
        }
        ComponentKind::TextPatch { .. } => {
            let Some(root) = root else { return needs_install() };
            let included = text_patch_parts(game.def.id);
            if included.iter().any(|p| p == c.id) && root.join(TEXT_PATCH_FILE).is_file() {
                Status::Active(None)
            } else {
                Status::Missing
            }
        }
        ComponentKind::File { dest, .. } => {
            let Some(root) = root else { return needs_install() };
            let path = root.join(dest);
            if path.is_file() || disabled_twin(&path).is_some_and(|p| p.is_file()) {
                Status::Active(None)
            } else {
                Status::Missing
            }
        }
    }
}

/// Where the mod manager parks a disabled copy of an SDK mod file.
fn disabled_twin(path: &Path) -> Option<PathBuf> {
    let dir = path.parent()?;
    let name = dir.file_name()?.to_string_lossy();
    Some(dir.with_file_name(format!("{name}_disabled")).join(path.file_name()?))
}

/// Downloads the latest DXVK, installs its DLLs next to the exe and writes
/// `dxvk.conf`. Returns the installed version.
pub fn install_dxvk(game_id: &str, component: &str, root: &Path, target: DxvkTarget, exe_dir: &str, conf: &str) -> Result<String> {
    let asset = net::latest_asset("doitsujin/dxvk", |n| {
        n.starts_with("dxvk-") && !n.contains("native") && n.ends_with(".tar.gz")
    })?;
    let tmp = std::env::temp_dir().join(format!("vaultpatcher-{}", asset.name));
    net::download(&asset.url, &tmp)?;

    let dest_dir = root.join(exe_dir);
    let mut targets: Vec<PathBuf> = target.dlls().iter().map(|d| dest_dir.join(d)).collect();
    targets.push(dest_dir.join("dxvk.conf"));
    let existing: Vec<PathBuf> = targets.iter().filter(|p| p.exists()).cloned().collect();
    if !existing.is_empty() {
        backup::create(game_id, "Before installing DXVK", &existing)?;
    }

    let mut wanted: Vec<(String, PathBuf)> = target
        .dlls()
        .iter()
        .map(|d| (format!("/{}/{d}", target.archive_dir()), dest_dir.join(d)))
        .collect();
    let archive = flate2::read::GzDecoder::new(fs::File::open(&tmp)?);
    let mut tar = tar::Archive::new(archive);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().replace('\\', "/");
        if let Some(i) = wanted.iter().position(|(suffix, _)| name.ends_with(suffix.as_str())) {
            let (_, out) = wanted.remove(i);
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            backup::clear_readonly(&out);
            fs::write(&out, bytes).with_context(|| format!("writing {} (close the game first)", out.display()))?;
        }
    }
    fs::remove_file(&tmp).ok();
    if !wanted.is_empty() {
        bail!("DXVK archive layout changed: missing {}", wanted[0].0);
    }
    fs::write(dest_dir.join("dxvk.conf"), conf)?;
    manifest::write(
        game_id,
        component,
        &Manifest {
            version: asset.tag.clone(),
            files: targets,
        },
    )?;
    Ok(asset.tag)
}

/// Downloads a single file into the install folder, backing up anything it
/// replaces.
pub fn install_file(
    game_id: &str,
    component: &str,
    root: &Path,
    url: &str,
    dest: &str,
    enable: Option<&str>,
) -> Result<String> {
    let out = root.join(dest);
    if out.exists() {
        backup::create(game_id, &format!("Before installing {component}"), std::slice::from_ref(&out))?;
    }
    net::download(url, &out)?;
    if let Some(module) = enable {
        enable_sdk_module(root, module)?;
    }
    manifest::write(
        game_id,
        component,
        &Manifest {
            version: "latest".into(),
            files: vec![out],
        },
    )?;
    Ok("latest".into())
}

/// Marks an SDK mod enabled via `sdk_mods/settings/<module>.json`, the file
/// mods_base reads at startup. An existing settings file (the user's own
/// choices) is left alone.
fn enable_sdk_module(root: &Path, module: &str) -> Result<()> {
    let settings = root.join("sdk_mods").join("settings").join(format!("{module}.json"));
    if !settings.exists() {
        fs::create_dir_all(settings.parent().expect("settings path has a parent"))?;
        fs::write(settings, "{\n    \"enabled\": true\n}\n")?;
    }
    Ok(())
}

/// Downloads a zipped SDK mod and extracts its folder into `sdk_mods`.
pub fn install_sdk_zip(game_id: &str, component: &str, root: &Path, url: &str, folder: &str) -> Result<String> {
    let tmp = std::env::temp_dir().join(format!("vaultpatcher-{component}.zip"));
    net::download(url, &tmp)?;
    let mut archive = zip::ZipArchive::new(fs::File::open(&tmp)?).context("not a valid zip")?;
    let sdk_mods = root.join("sdk_mods");
    let mut written = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().replace('\\', "/");
        if entry.is_dir() || !name.starts_with(&format!("{folder}/")) || name.split('/').any(|p| p == "..") {
            continue;
        }
        let out = sdk_mods.join(&name);
        fs::create_dir_all(out.parent().expect("zip entry has a parent"))?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        fs::write(&out, bytes).with_context(|| format!("writing {} (close the game first)", out.display()))?;
        written.push(out);
    }
    fs::remove_file(&tmp).ok();
    if written.is_empty() {
        bail!("{url} doesn't contain a {folder}/ folder");
    }
    manifest::write(game_id, component, &Manifest { version: "latest".into(), files: written })?;
    Ok("latest".into())
}

// ---- merged text patch -----------------------------------------------------------

fn parts_path(game_id: &str) -> PathBuf {
    backup::data_dir().join("installs").join(format!("{game_id}-textpatch.json"))
}

/// Ids of the `TextPatch` components currently merged into the game's file.
pub fn text_patch_parts(game_id: &str) -> Vec<String> {
    fs::read_to_string(parts_path(game_id))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn download_text(url: &str) -> Result<String> {
    let tmp = std::env::temp_dir().join(format!("vaultpatcher-textmod-{:x}.txt", {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut h);
        h.finish()
    }));
    net::download(url, &tmp)?;
    let bytes = fs::read(&tmp)?;
    fs::remove_file(&tmp).ok();
    Ok(crate::textmod::decode(&bytes))
}

/// Rebuilds `Binaries/VaultPatcher.blcm` from the given parts (component id
/// + sources) and points Text Mod Loader at it. With no parts, removes it.
pub fn rebuild_text_patch(
    game_id: &str,
    root: &Path,
    game_tag: &str,
    gearbox_url: &str,
    parts: &[(&str, &[TextSource])],
) -> Result<String> {
    use crate::textmod::{self, MergeInfo, Source};
    let out = root.join(TEXT_PATCH_FILE);
    if parts.is_empty() {
        if out.is_file() {
            fs::remove_file(&out)?;
        }
        set_tml_auto_enable(root, &out, false)?;
        fs::remove_file(parts_path(game_id)).ok();
        return Ok("Removed".into());
    }
    let gearbox = textmod::parse(&download_text(gearbox_url)?);
    let mut sources = Vec::new();
    for (_, list) in parts {
        for s in list.iter() {
            let tree = textmod::parse(&download_text(s.url)?);
            let nodes = textmod::filter(&tree, s.include, s.exclude);
            if nodes.is_empty() {
                bail!("{} changed upstream: none of the expected categories were found", s.title);
            }
            sources.push(Source { title: s.title.into(), credit: s.credit.into(), nodes });
        }
    }
    let text = textmod::merge(
        &MergeInfo {
            game: game_tag,
            title: "Vault Patcher Community Patch",
            author: "Vault Patcher (built from community mods)",
            version: env!("CARGO_PKG_VERSION"),
            description: "Balance-neutral bug fixes and quality of life, merged into one file so Text Mod Loader can run them together.",
        },
        &gearbox,
        &sources,
    );
    if out.exists() {
        backup::create(game_id, "Before rebuilding the community patch", std::slice::from_ref(&out))?;
    }
    fs::write(&out, textmod::encode(&text)).with_context(|| format!("writing {}", out.display()))?;
    set_tml_auto_enable(root, &out, true)?;
    let ids: Vec<&str> = parts.iter().map(|(id, _)| *id).collect();
    fs::create_dir_all(parts_path(game_id).parent().expect("has parent"))?;
    fs::write(parts_path(game_id), serde_json::to_vec(&ids)?)?;
    Ok(format!("{} mods merged", sources.len()))
}

/// Adds (or removes) `file` in Text Mod Loader's auto-enable list, keeping
/// every other setting the user has.
fn set_tml_auto_enable(root: &Path, file: &Path, enabled: bool) -> Result<()> {
    let settings = root.join("sdk_mods").join("settings").join("text_mod_loader.json");
    let mut json: serde_json::Value = fs::read_to_string(&settings)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !json.is_object() {
        json = serde_json::json!({});
    }
    let options = json
        .as_object_mut()
        .expect("object")
        .entry("options")
        .or_insert_with(|| serde_json::json!({"mod_info": {}, "version": 2}));
    let list = options
        .as_object_mut()
        .context("text_mod_loader.json: options isn't an object")?
        .entry("auto_enable")
        .or_insert_with(|| serde_json::json!([]));
    let path = file.to_string_lossy().to_string();
    let arr = list.as_array_mut().context("auto_enable isn't a list")?;
    arr.retain(|v| v.as_str().is_none_or(|s| !s.eq_ignore_ascii_case(&path)));
    if enabled {
        arr.push(serde_json::Value::String(path));
    }
    fs::create_dir_all(settings.parent().expect("has parent"))?;
    fs::write(&settings, serde_json::to_vec_pretty(&json)?)?;
    Ok(())
}

pub fn uninstall_files(game_id: &str, component: &str, root: &Path) -> Result<()> {
    manifest::uninstall(game_id, component, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_ids_are_unique_and_dependencies_exist() {
        for game in crate::games::all() {
            let mut seen = std::collections::HashSet::new();
            for (i, c) in game.setup.iter().enumerate() {
                assert!(seen.insert(c.id), "{}: duplicate component {}", game.id, c.id);
                for dep in c.requires {
                    let at = game.setup.iter().position(|d| d.id == *dep);
                    assert!(at.is_some_and(|at| at < i), "{}: {} needs {dep} listed before it", game.id, c.id);
                }
                if let ComponentKind::Settings { values, .. } = c.kind {
                    for (id, _) in values {
                        assert!(game.tweaks.iter().any(|t| t.id == *id), "{}: {} references {id}", game.id, c.id);
                    }
                }
                if let ComponentKind::ExePatch(p) = c.kind {
                    assert!(game.patches.iter().any(|x| x.id == p), "{}: unknown patch {p}", game.id);
                }
            }
        }
    }
}

/// Network tests that install into a temp folder; run with `--ignored`.
#[cfg(test)]
mod live_tests {
    use super::*;

    #[test]
    #[ignore]
    fn live_every_download_url_resolves() {
        for game in crate::games::all() {
            for c in game.setup {
                if let ComponentKind::File { url, .. } = c.kind {
                    let status = ureq::head(url).set("User-Agent", "VaultPatcher-test").call().map(|r| r.status());
                    println!("{:<5} {:<16} {:?}", game.id, c.id, status);
                    assert_eq!(status.ok(), Some(200), "{} {}", game.id, c.id);
                }
            }
        }
    }

    /// Builds the real community patch (downloads) into a sandbox and checks
    /// it is well formed and excludes the balance changes.
    #[test]
    #[ignore]
    fn live_community_patch_builds() {
        let game = &crate::games::bl2::GAME;
        let root = std::env::temp_dir().join("vaultpatcher-textpatch-sandbox");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Binaries")).unwrap();
        let parts: Vec<(&str, &[TextSource])> = game
            .setup
            .iter()
            .filter_map(|c| match c.kind {
                ComponentKind::TextPatch { sources, .. } => Some((c.id, sources)),
                _ => None,
            })
            .collect();
        let ComponentKind::TextPatch { game: tag, gearbox_url, .. } = game.component("community_patch").unwrap().kind else {
            panic!()
        };
        let msg = rebuild_text_patch("sandbox", &root, tag, gearbox_url, &parts).unwrap();
        println!("{msg}");
        let text = crate::textmod::decode(&fs::read(root.join(TEXT_PATCH_FILE)).unwrap());
        assert!(!text.contains("Permaslag"));
        assert!(text.contains("Fixed Moonshiner Audio"));
        let keys = text.lines().find(|l| l.contains("SparkServiceConfiguration_0 Keys")).unwrap();
        let values = text.lines().find(|l| l.contains("SparkServiceConfiguration_0 Values")).unwrap();
        let n_keys = keys.matches("-BLCMM").count();
        println!("{} commands, {n_keys} hotfixes, {} bytes", text.split("#Commands:").nth(1).unwrap().lines().filter(|l| l.starts_with("set")).count(), text.len());
        assert!(n_keys > 100);
        // Values are a quoted list with escaped quotes inside; count top-level entries.
        let mut entries = 0;
        let (mut in_str, mut escaped) = (false, false);
        for ch in values.split_once('(').unwrap().1.chars() {
            match (in_str, escaped, ch) {
                (true, true, _) => escaped = false,
                (true, false, '\\') => escaped = true,
                (true, false, '"') => in_str = false,
                (false, _, '"') => { in_str = true; entries += 1; }
                _ => {}
            }
        }
        assert_eq!(entries, n_keys, "Keys and Values must line up");
        let tml = fs::read_to_string(root.join("sdk_mods/settings/text_mod_loader.json")).unwrap();
        assert!(tml.contains("VaultPatcher.blcm"));
        assert_eq!(text_patch_parts("sandbox").len(), parts.len());
        rebuild_text_patch("sandbox", &root, tag, gearbox_url, &[]).unwrap();
        assert!(!root.join(TEXT_PATCH_FILE).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore]
    fn live_sdk_mod_installs_and_is_enabled() {
        let root = std::env::temp_dir().join("vaultpatcher-mod-sandbox");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        install_file(
            "sandbox",
            "firing_fix",
            &root,
            "https://github.com/ZetaDaemon/willow2-sdk-mods/releases/download/nightly/firing_fix.sdkmod",
            "sdk_mods/firing_fix.sdkmod",
            Some("firing_fix"),
        )
        .unwrap();
        let archive = zip::ZipArchive::new(fs::File::open(root.join("sdk_mods/firing_fix.sdkmod")).unwrap()).unwrap();
        assert!(archive.file_names().any(|n| n.starts_with("firing_fix/")), "sdkmod root folder must match its stem");
        let settings = fs::read_to_string(root.join("sdk_mods/settings/firing_fix.json")).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&settings).unwrap()["enabled"] == true);
        uninstall_files("sandbox", "firing_fix", &root).unwrap();
        assert!(!root.join("sdk_mods/firing_fix.sdkmod").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore]
    fn live_dxvk_installs_into_sandbox_and_uninstalls() {
        let root = std::env::temp_dir().join("vaultpatcher-dxvk-sandbox");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Binaries/Win32")).unwrap();
        let version = install_dxvk("sandbox", "dxvk", &root, DxvkTarget::D3d9Win32, "Binaries/Win32", "x = 1\n").unwrap();
        println!("installed DXVK {version}");
        let dll = root.join("Binaries/Win32/d3d9.dll");
        let bytes = fs::read(&dll).unwrap();
        assert!(bytes.len() > 100_000 && &bytes[..2] == b"MZ");
        // 32-bit PE: machine type i386 (0x14C)
        let pe = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize;
        assert_eq!(u16::from_le_bytes([bytes[pe + 4], bytes[pe + 5]]), 0x14C);
        assert!(root.join("Binaries/Win32/dxvk.conf").is_file());
        uninstall_files("sandbox", "dxvk", &root).unwrap();
        assert!(!dll.exists());
        let _ = fs::remove_dir_all(&root);
    }
}
