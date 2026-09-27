//! Mod manager plumbing: installing the community Python SDK, and adding,
//! toggling and removing SDK mods and text mods.
//!
//! Every file the SDK installer extracts is recorded in a manifest so that an
//! uninstall removes exactly those files and never touches the user's mods.

use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::core::manifest::{self, Manifest};
use crate::core::net;
use crate::games::ModSupport;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdkStatus {
    NotInstalled,
    /// Installed by Vaulter; carries the release tag.
    Installed(String),
    /// Marker files exist but we didn't install it (manual install / other tool).
    Detected,
    /// Only the pre-2024 PythonSDK is present; it should be replaced.
    Legacy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModKind {
    /// A packaged `.sdkmod` archive.
    SdkPackage,
    /// An extracted Python mod folder.
    SdkFolder,
    /// A text mod (`.blcm` / `.txt`) of `set` commands.
    TextMod,
}

impl ModKind {
    pub fn label(self) -> &'static str {
        match self {
            ModKind::SdkPackage => "SDK mod",
            ModKind::SdkFolder => "SDK mod (folder)",
            ModKind::TextMod => "Text mod",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModEntry {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub path: PathBuf,
    pub kind: ModKind,
    pub enabled: bool,
    /// Ships with the SDK; disabling or removing it would break the SDK.
    pub core: bool,
}

const SDK: &str = "sdk";

pub fn sdk_status(game_id: &str, support: &ModSupport, root: &Path) -> SdkStatus {
    let present = support.sdk_markers.iter().any(|m| root.join(m).exists());
    if !present {
        if support.legacy_markers.iter().any(|m| root.join(m).exists()) {
            return SdkStatus::Legacy;
        }
        return SdkStatus::NotInstalled;
    }
    match manifest::read(game_id, SDK) {
        Some(m) => SdkStatus::Installed(m.version),
        None => SdkStatus::Detected,
    }
}

fn disabled_dir(dir: &Path) -> PathBuf {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dir.with_file_name(format!("{name}_disabled"))
}

fn is_text_mod(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "blcm" => true,
        "txt" => {
            // Plain .txt files in Binaries are only mods if they contain console commands.
            let mut head = String::new();
            fs::File::open(path)
                .and_then(|f| f.take(64 * 1024).read_to_string(&mut head))
                .is_ok()
                && head.lines().any(|l| {
                    let l = l.trim_start().to_ascii_lowercase();
                    l.starts_with("set ") || l.starts_with("<blcmm") || l.starts_with("exec ")
                })
        }
        _ => false,
    }
}

pub fn scan(support: &ModSupport, root: &Path) -> Vec<ModEntry> {
    let mut out = Vec::new();
    let sdk_dir = root.join(support.sdk_mods_dir);
    for (dir, enabled) in [(sdk_dir.clone(), true), (disabled_dir(&sdk_dir), false)] {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with("__") {
                continue;
            }
            let stem = name.trim_end_matches(".sdkmod");
            let core = support.core_mods.iter().any(|c| c.eq_ignore_ascii_case(stem));
            if path.is_file() && name.to_ascii_lowercase().ends_with(".sdkmod") {
                let meta = sdkmod_metadata(&path);
                out.push(ModEntry {
                    name: meta.0.unwrap_or_else(|| name.trim_end_matches(".sdkmod").to_string()),
                    version: meta.1,
                    description: meta.2,
                    path,
                    kind: ModKind::SdkPackage,
                    enabled,
                    core,
                });
            } else if path.is_dir()
                && (path.join("__init__.py").is_file() || path.join("pyproject.toml").is_file())
            {
                let meta = fs::read_to_string(path.join("pyproject.toml"))
                    .map(|t| pyproject_metadata(&t))
                    .unwrap_or_default();
                out.push(ModEntry {
                    name: meta.0.unwrap_or(name),
                    version: meta.1,
                    description: meta.2,
                    path,
                    kind: ModKind::SdkFolder,
                    enabled,
                    core,
                });
            }
        }
    }
    let text_dir = root.join(support.text_mods_dir);
    for (dir, enabled) in [(text_dir.clone(), true), (text_dir.join(TEXT_DISABLED), false)] {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_text_mod(&path) {
                out.push(ModEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    version: None,
                    description: None,
                    path,
                    kind: ModKind::TextMod,
                    enabled,
                    core: false,
                });
            }
        }
    }
    out.sort_by_key(|m| (!m.core, m.kind as u8, m.name.to_ascii_lowercase()));
    out
}

type Meta = (Option<String>, Option<String>, Option<String>);

fn sdkmod_metadata(path: &Path) -> Meta {
    let Ok(file) = fs::File::open(path) else { return Default::default() };
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return Default::default() };
    for i in 0..archive.len() {
        let Ok(mut f) = archive.by_index(i) else { continue };
        if f.name().ends_with("pyproject.toml") && f.name().matches('/').count() <= 1 {
            let mut text = String::new();
            if f.read_to_string(&mut text).is_ok() {
                return pyproject_metadata(&text);
            }
        }
    }
    Default::default()
}

/// Pulls name/version/description out of a pyproject.toml without a full
/// TOML parser: the keys we want are simple strings in `[project]`.
fn pyproject_metadata(text: &str) -> Meta {
    let mut in_project = false;
    let (mut name, mut version, mut description) = (None, None, None);
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.starts_with('[') {
            in_project = line == "[project]";
            continue;
        }
        if !in_project {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        // Multi-line strings ("""…""" or '''…''') with `\` line continuations.
        let v = match [r#"""""#, "'''"].into_iter().find(|q| v.starts_with(q)) {
            Some(q) => {
                let mut body = v[3..].to_string();
                while !body.contains(q) {
                    let Some(next) = lines.next() else { break };
                    body.push('\n');
                    body.push_str(next);
                }
                body.split(q)
                    .next()
                    .unwrap_or_default()
                    .lines()
                    .map(|l| l.trim().trim_end_matches('\\').trim())
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            None => v.trim_matches('"').trim_matches('\'').to_string(),
        };
        match k.trim() {
            "name" => name = Some(v),
            "version" => version = Some(v),
            "description" => description = Some(v).filter(|d| !d.is_empty()),
            _ => {}
        }
    }
    (name, version, description)
}

/// Rejects zip entries that would escape the destination folder.
fn safe_relative(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Works out where an SDK release zip should be extracted from its layout:
/// zips rooted at the game folder contain `Binaries/`, zips rooted at
/// `Binaries/` contain `Win32/` (or `Win64/`) or `sdk_mods/` directly.
fn sdk_extract_root(archive: &mut zip::ZipArchive<fs::File>, root: &Path) -> PathBuf {
    let tops: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .filter_map(|n| n.split('/').next().map(|s| s.to_ascii_lowercase()))
        .collect();
    if tops.iter().any(|t| t == "binaries") {
        root.to_path_buf()
    } else if tops.iter().any(|t| t == "win32" || t == "win64") {
        root.join("Binaries")
    } else {
        root.to_path_buf()
    }
}

pub fn install_sdk_zip(game_id: &str, support: &ModSupport, root: &Path, zip_path: &Path) -> Result<()> {
    install_sdk_zip_versioned(game_id, support, root, zip_path, "manual")
}

fn install_sdk_zip_versioned(
    game_id: &str,
    support: &ModSupport,
    root: &Path,
    zip_path: &Path,
    version: &str,
) -> Result<()> {
    let file = fs::File::open(zip_path).with_context(|| format!("opening {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("not a valid zip file")?;
    let dest = sdk_extract_root(&mut archive, root);
    let sdk_mods = root.join(support.sdk_mods_dir);

    // Snapshot any files we're about to overwrite (except user mod content).
    let mut overwritten = Vec::new();
    let mut targets = Vec::new();
    for i in 0..archive.len() {
        let f = archive.by_index(i)?;
        let Some(rel) = safe_relative(f.name()) else {
            bail!("zip contains an unsafe path: {}", f.name());
        };
        if f.is_dir() {
            continue;
        }
        let target = dest.join(rel);
        if target.exists() && !target.starts_with(&sdk_mods) {
            overwritten.push(target.clone());
        }
        targets.push((i, target));
    }
    let previous = manifest::read(game_id, SDK).map(|m| m.files).unwrap_or_default();
    let replaced =
        manifest::plan_replace(game_id, SDK, &format!("Before installing {}", support.sdk_name), &overwritten)?;

    // The zip never contains user mods, so everything it writes belongs to
    // the SDK, except the settings folder, which holds the user's mod options.
    let settings = sdk_mods.join("settings");
    let same = |a: &Path, b: &Path| a.as_os_str().eq_ignore_ascii_case(b.as_os_str());
    let files: Vec<PathBuf> = targets.iter().map(|(_, t)| t.clone()).filter(|p| !p.starts_with(&settings)).collect();
    // What the previous version installed that this one doesn't ship.
    let stale: Vec<PathBuf> = previous.into_iter().filter(|old| !files.iter().any(|f| same(f, old))).collect();
    // Recorded before extraction, with the previous version's files too, so
    // a half-finished install still shows up as ours and can be removed.
    manifest::write(
        game_id,
        SDK,
        &Manifest {
            version: format!("{version} (incomplete)"),
            files: files.iter().chain(&stale).cloned().collect(),
            replaced: replaced.clone(),
        },
    )?;
    for (i, target) in &targets {
        let mut f = archive.by_index(*i)?;
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;
        crate::core::atomic::write(target, &bytes).with_context(|| format!("close the game first ({})", target.display()))?;
    }
    // A stale core module left in sdk_mods would load next to its
    // replacement; anything that can't be removed stays on the record.
    let kept: Vec<PathBuf> = stale
        .into_iter()
        .filter(|old| {
            crate::core::backup::clear_readonly(old);
            old.is_file() && fs::remove_file(old).is_err()
        })
        .collect();
    manifest::write(
        game_id,
        SDK,
        &Manifest { version: version.to_string(), files: files.into_iter().chain(kept).collect(), replaced },
    )?;
    fs::create_dir_all(&sdk_mods).ok();
    Ok(())
}

/// Downloads the latest release asset from GitHub and installs it. Returns the tag.
pub fn install_sdk_latest(game_id: &str, support: &ModSupport, root: &Path) -> Result<String> {
    let asset = net::latest_asset(support.sdk_repo, |n| {
        n.contains(support.sdk_asset_hint) && n.ends_with(".zip")
    })?;
    let tmp = crate::core::atomic::temp_file(&format!("{game_id}-sdk.zip"));
    net::download(&asset.url, &tmp)?;
    let result = install_sdk_zip_versioned(game_id, support, root, &tmp, &asset.tag);
    fs::remove_file(&tmp).ok();
    result.map(|()| asset.tag)
}

pub fn uninstall_sdk(game_id: &str, support: &ModSupport, root: &Path) -> Result<()> {
    if manifest::read(game_id, SDK).is_none() {
        bail!(
            "{} wasn't installed by Vaulter, so its file list is unknown. Remove it by hand (see its readme).",
            support.sdk_name
        );
    }
    manifest::uninstall_with(game_id, SDK, root, |f| disabled_twin(root, f))
}

/// Folder the Mods page moves disabled text mods into.
const TEXT_DISABLED: &str = "VaultPatcher_disabled";

/// Where the Mods page parks `path` (a file or folder under `root`) when the
/// user disables it: the top-level entry under `sdk_mods/` moves to
/// `sdk_mods_disabled/` with everything inside it, and a text mod in
/// `Binaries/` moves into `Binaries/VaultPatcher_disabled/`. `None` for
/// anything the Mods page doesn't move.
pub fn disabled_twin(root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(root).ok()?.to_string_lossy().into_owned();
    let parts: Vec<&str> = rel.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        [dir, rest @ ..] if dir.eq_ignore_ascii_case("sdk_mods") && !rest.is_empty() => {
            Some(rest.iter().fold(disabled_dir(&root.join(dir)), |p, part| p.join(part)))
        }
        [dir, file] if dir.eq_ignore_ascii_case("Binaries") && {
            let lower = file.to_ascii_lowercase();
            lower.ends_with(".blcm") || lower.ends_with(".txt")
        } => Some(root.join(dir).join(TEXT_DISABLED).join(file)),
        _ => None,
    }
}

pub fn install_mod(support: &ModSupport, root: &Path, file: &Path) -> Result<()> {
    let name = file
        .file_name()
        .context("invalid file")?
        .to_string_lossy()
        .into_owned();
    let lower = name.to_ascii_lowercase();
    let sdk_dir = root.join(support.sdk_mods_dir);
    let text_dir = root.join(support.text_mods_dir);
    if lower.ends_with(".sdkmod") {
        fs::create_dir_all(&sdk_dir)?;
        fs::copy(file, sdk_dir.join(&name))?;
    } else if lower.ends_with(".blcm") || lower.ends_with(".txt") {
        fs::copy(file, text_dir.join(&name))?;
    } else if lower.ends_with(".zip") {
        install_mod_zip(&sdk_dir, &text_dir, file)?;
    } else {
        bail!("{name}: unsupported file type (expected .sdkmod, .zip, .blcm or .txt)");
    }
    Ok(())
}

/// Installs a zipped mod: python mod folders go to sdk_mods, `.sdkmod` and
/// text mod files are extracted to their folders.
fn install_mod_zip(sdk_dir: &Path, text_dir: &Path, file: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(fs::File::open(file)?).context("not a valid zip file")?;
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    let python_root = names
        .iter()
        .filter(|n| n.ends_with("__init__.py") || n.ends_with("pyproject.toml"))
        .map(|n| n.matches('/').count())
        .min();
    let mut installed = 0;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        let Some(rel) = safe_relative(f.name()) else {
            bail!("zip contains an unsafe path: {}", f.name());
        };
        let lower = f.name().to_ascii_lowercase();
        let file_name = rel.file_name().map(|n| n.to_owned()).unwrap_or_default();
        let target = if lower.ends_with(".sdkmod") {
            sdk_dir.join(file_name)
        } else if let Some(depth) = python_root {
            // Keep the mod folder: strip everything above it.
            let skip = depth.saturating_sub(1);
            let stripped: PathBuf = rel.components().skip(skip).collect();
            if stripped.components().count() < 2 {
                continue;
            }
            sdk_dir.join(stripped)
        } else if lower.ends_with(".blcm") || lower.ends_with(".txt") {
            text_dir.join(file_name)
        } else {
            continue;
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut f, &mut fs::File::create(&target)?)?;
        installed += 1;
    }
    if installed == 0 {
        bail!("no recognizable mod files in {}", file.display());
    }
    Ok(())
}

pub fn set_enabled(support: &ModSupport, root: &Path, entry: &ModEntry, enabled: bool) -> Result<()> {
    if entry.enabled == enabled {
        return Ok(());
    }
    let (on_dir, off_dir) = match entry.kind {
        ModKind::TextMod => {
            let dir = root.join(support.text_mods_dir);
            (dir.clone(), dir.join(TEXT_DISABLED))
        }
        _ => {
            let dir = root.join(support.sdk_mods_dir);
            (dir.clone(), disabled_dir(&dir))
        }
    };
    let target_dir = if enabled { on_dir } else { off_dir };
    fs::create_dir_all(&target_dir)?;
    let target = target_dir.join(entry.path.file_name().context("invalid mod path")?);
    if target.exists() {
        bail!("{} already exists", target.display());
    }
    fs::rename(&entry.path, &target).with_context(|| format!("moving {}", entry.path.display()))?;
    Ok(())
}

pub fn remove(entry: &ModEntry) -> Result<()> {
    if entry.path.is_dir() {
        fs::remove_dir_all(&entry.path)?;
    } else {
        fs::remove_file(&entry.path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pyproject_fields() {
        let meta = pyproject_metadata(
            "[build-system]\nname = \"x\"\n[project]\nname = \"Cool Mod\"\nversion = \"1.2\"\ndescription = 'Does things'\n",
        );
        assert_eq!(meta, (Some("Cool Mod".into()), Some("1.2".into()), Some("Does things".into())));
        let multi = pyproject_metadata("[project]\nname = \"Quick\"\ndescription = \"\"\"\\\n    Loads straight\n    into a save.\"\"\"\nversion = \"1\"\n");
        assert_eq!(multi.2.as_deref(), Some("Loads straight into a save."));
        assert_eq!(multi.1.as_deref(), Some("1"));
    }

    #[test]
    #[ignore]
    fn live_sdk_installs_into_sandbox_and_uninstalls() {
        let support = &crate::games::bl2::WILLOW2_SDK;
        let root = std::env::temp_dir().join("vaulter-sdk-sandbox");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Binaries/Win32")).unwrap();
        let tag = install_sdk_latest("sandbox", support, &root).unwrap();
        println!("installed SDK {tag}");
        assert!(root.join("Binaries/Win32/ddraw.dll").is_file());
        assert_eq!(sdk_status("sandbox", support, &root), SdkStatus::Installed(tag.clone()));
        let mods = scan(support, &root);
        assert!(mods.iter().any(|m| m.core), "core SDK mods should be listed as core");
        uninstall_sdk("sandbox", support, &root).unwrap();
        assert!(!root.join("Binaries/Win32/ddraw.dll").exists());
        assert!(!root.join("Binaries/Win32/Plugins/unrealsdk.dll").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn disabled_twins_follow_the_mods_page() {
        let root = Path::new(r"D:\Games\BL");
        let twin = |p: &str| disabled_twin(root, &root.join(p));
        assert_eq!(twin(r"sdk_mods\BloodwingReturnFix\__init__.py"), Some(root.join(r"sdk_mods_disabled\BloodwingReturnFix\__init__.py")));
        assert_eq!(twin(r"sdk_mods\BloodwingReturnFix"), Some(root.join(r"sdk_mods_disabled\BloodwingReturnFix")));
        assert_eq!(twin("sdk_mods/firing_fix.sdkmod"), Some(root.join(r"sdk_mods_disabled\firing_fix.sdkmod")));
        assert_eq!(twin(r"Binaries\VaultPatcher.blcm"), Some(root.join(r"Binaries\VaultPatcher_disabled\VaultPatcher.blcm")));
        assert_eq!(twin("sdk_mods"), None);
        assert_eq!(twin(r"Binaries\Win32\ddraw.dll"), None);
        assert_eq!(disabled_twin(root, Path::new(r"C:\elsewhere\sdk_mods\x.sdkmod")), None);
    }

    fn sdk_zip(path: &Path, files: &[&str]) {
        use std::io::Write as _;
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for name in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(name.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn sdk_installs_are_recorded_up_front_and_upgrades_drop_stale_files() {
        let support = &crate::games::bl2::WILLOW2_SDK;
        let game = "test-sdk";
        let base = std::env::temp_dir().join("vaulter-sdk-offline");
        let _ = fs::remove_dir_all(&base);
        let root = base.join("Game");
        fs::create_dir_all(root.join("Binaries/Win32")).unwrap();
        fs::write(root.join("Binaries/Win32/ddraw.dll"), "user's ddraw").unwrap();
        let zip = base.join("sdk.zip");
        let v1 = ["Binaries/Win32/ddraw.dll", "Binaries/Win32/Plugins/unrealsdk.dll", "sdk_mods/old_core.sdkmod", "sdk_mods/settings/x.json"];
        sdk_zip(&zip, &v1);
        install_sdk_zip_versioned(game, support, &root, &zip, "v1").unwrap();
        assert_eq!(sdk_status(game, support, &root), SdkStatus::Installed("v1".into()));

        // v2 no longer ships old_core; its extraction fails half-way.
        let v2 = ["Binaries/Win32/Plugins/unrealsdk.dll", "sdk_mods/new_core.sdkmod", "sdk_mods/blocked.sdkmod"];
        sdk_zip(&zip, &v2);
        fs::create_dir_all(root.join("sdk_mods/blocked.sdkmod/in-the-way")).unwrap();
        assert!(install_sdk_zip_versioned(game, support, &root, &zip, "v2").is_err());
        assert_eq!(sdk_status(game, support, &root), SdkStatus::Installed("v2 (incomplete)".into()), "still ours, so it can be retried or removed");
        fs::remove_dir_all(root.join("sdk_mods/blocked.sdkmod")).unwrap();
        install_sdk_zip_versioned(game, support, &root, &zip, "v2").unwrap();
        assert_eq!(sdk_status(game, support, &root), SdkStatus::Installed("v2".into()));
        assert!(!root.join("sdk_mods/old_core.sdkmod").exists(), "a module the new version dropped is removed");
        assert!(root.join("sdk_mods/settings/x.json").is_file(), "mod settings are the user's");

        uninstall_sdk(game, support, &root).unwrap();
        assert!(!root.join("Binaries/Win32/Plugins/unrealsdk.dll").exists() && !root.join("sdk_mods/new_core.sdkmod").exists());
        assert_eq!(fs::read_to_string(root.join("Binaries/Win32/ddraw.dll")).unwrap(), "user's ddraw", "the user's own file comes back");
        assert_eq!(sdk_status(game, support, &root), SdkStatus::NotInstalled);
        for b in crate::core::backup::list(game) {
            let _ = crate::core::backup::delete(&b);
        }
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_escaping_zip_paths() {
        assert!(safe_relative("../evil.dll").is_none());
        assert!(safe_relative("/abs").is_none());
        assert!(safe_relative("Binaries/Win32/x.dll").is_some());
    }
}
