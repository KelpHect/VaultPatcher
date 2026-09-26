//! Records exactly which files an installed component put on disk, so it can
//! be removed later without touching anything else.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use super::backup;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub files: Vec<PathBuf>,
    /// Backup of files that existed before the install; put back on uninstall.
    #[serde(default)]
    pub replaced: Option<PathBuf>,
}

/// `path` is inside `root`, ignoring case (Windows paths).
fn inside(path: &Path, root: &Path) -> bool {
    let p = path.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    let r = root.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    let r = r.trim_end_matches('\\');
    p.starts_with(r) && p[r.len()..].starts_with('\\')
}

/// Before (re)installing a component: which of `targets` are the user's own
/// files that need backing up, and the backup to keep. Files a previous
/// install of ours wrote are not the user's, and an earlier backup of the
/// user's originals is carried over rather than replaced.
pub fn plan_replace(
    game_id: &str,
    component: &str,
    label: &str,
    targets: &[PathBuf],
) -> Result<Option<PathBuf>> {
    let previous = read(game_id, component);
    let ours: Vec<&PathBuf> = previous.iter().flat_map(|m| m.files.iter()).collect();
    let users: Vec<PathBuf> = targets
        .iter()
        .filter(|t| t.exists() && !ours.iter().any(|o| o.as_os_str().eq_ignore_ascii_case(t.as_os_str())))
        .cloned()
        .collect();
    let carried = previous.and_then(|m| m.replaced);
    if users.is_empty() {
        return Ok(carried);
    }
    // User files first seen on this run join the earlier backup, so
    // uninstall puts back every one of them.
    if let Some(mut earlier) = carried.as_deref().and_then(backup::load) {
        backup::add_files(&mut earlier, &users)?;
        return Ok(Some(earlier.dir));
    }
    Ok(Some(backup::create(game_id, label, &users)?.dir))
}

pub fn remove(game_id: &str, component: &str) {
    fs::remove_file(path(game_id, component)).ok();
}

fn path(game_id: &str, component: &str) -> PathBuf {
    backup::data_dir()
        .join("installs")
        .join(format!("{game_id}-{component}.json"))
}

pub fn read(game_id: &str, component: &str) -> Option<Manifest> {
    serde_json::from_str(&fs::read_to_string(path(game_id, component)).ok()?).ok()
}

pub fn write(game_id: &str, component: &str, manifest: &Manifest) -> Result<()> {
    let path = path(game_id, component);
    super::atomic::write(&path, &serde_json::to_vec_pretty(manifest)?)
}

/// Deletes the recorded files (only those under `root`), then any folders the
/// install created that are now empty, then the manifest itself.
#[cfg_attr(not(test), allow(dead_code))]
pub fn uninstall(game_id: &str, component: &str, root: &Path) -> Result<()> {
    uninstall_with(game_id, component, root, |_| None)
}

/// `uninstall`, also deleting each recorded file's `moved` copy (where the
/// Mods page parks a file the user disabled), so nothing is left behind.
pub fn uninstall_with(game_id: &str, component: &str, root: &Path, moved: impl Fn(&Path) -> Option<PathBuf>) -> Result<()> {
    let Some(manifest) = read(game_id, component) else {
        anyhow::bail!("no install record for {component}");
    };
    // If the game moved, deleting nothing and forgetting the record would
    // strand the files; refuse instead.
    if let Some(outside) = manifest.files.iter().find(|f| !inside(f, root)) {
        anyhow::bail!(
            "{component} was installed at {} — the game folder changed, so it can't be removed safely",
            outside.display()
        );
    }
    let twins: Vec<PathBuf> = manifest.files.iter().filter_map(|f| moved(f)).filter(|t| inside(t, root)).collect();
    for file in manifest.files.iter().chain(&twins) {
        if file.is_file() {
            backup::clear_readonly(file);
            fs::remove_file(file)
                .with_context(|| format!("removing {} (close the game first)", file.display()))?;
        }
    }
    let mut dirs: Vec<PathBuf> = manifest
        .files
        .iter()
        .chain(&twins)
        .flat_map(|f| f.ancestors().skip(1).map(Path::to_path_buf).collect::<Vec<_>>())
        .filter(|d| inside(d, root))
        .collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dirs.dedup();
    for dir in dirs {
        fs::remove_dir(&dir).ok();
    }
    if let Some(dir) = &manifest.replaced
        && let Some(backup) = backup::load(dir)
    {
        backup::restore(&backup).context("restoring the files this install replaced")?;
    }
    fs::remove_file(path(game_id, component)).ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_is_case_insensitive_and_component_aware() {
        let root = Path::new(r"D:\Games\Borderlands 2");
        assert!(inside(Path::new(r"d:\games\BORDERLANDS 2\Binaries\Win32\d3d9.dll"), root));
        assert!(!inside(Path::new(r"D:\Games\Borderlands 2 GOTY\x.dll"), root));
        assert!(!inside(Path::new(r"D:\Games\Borderlands 2"), root));
    }
}

#[cfg(test)]
mod install_tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaultpatcher-manifest-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn uninstall_refuses_and_keeps_the_record_when_the_game_moved() {
        let game = "test-moved";
        let old_root = sandbox("old");
        let new_root = sandbox("new");
        let file = old_root.join("d3d9.dll");
        fs::write(&file, b"x").unwrap();
        write(game, "dxvk", &Manifest { version: "1".into(), files: vec![file.clone()], replaced: None }).unwrap();
        assert!(uninstall(game, "dxvk", &new_root).is_err());
        assert!(read(game, "dxvk").is_some(), "record kept so it can still be removed");
        assert!(file.exists());
        uninstall(game, "dxvk", &old_root).unwrap();
        assert!(!file.exists() && read(game, "dxvk").is_none());
        let _ = fs::remove_dir_all(&old_root);
        let _ = fs::remove_dir_all(&new_root);
    }

    #[test]
    fn reinstall_keeps_the_users_original_and_uninstall_restores_it() {
        let game = "test-replace";
        let root = sandbox("replace");
        let conf = root.join("dxvk.conf");
        fs::write(&conf, "user's own").unwrap();
        // First install replaces the user's file.
        let replaced = plan_replace(game, "dxvk", "first", std::slice::from_ref(&conf)).unwrap();
        assert!(replaced.is_some());
        write(game, "dxvk", &Manifest { version: "1".into(), files: vec![conf.clone()], replaced: replaced.clone() }).unwrap();
        fs::write(&conf, "ours v1").unwrap();
        // Updating over our own file must not treat it as the user's.
        let again = plan_replace(game, "dxvk", "update", std::slice::from_ref(&conf)).unwrap();
        assert_eq!(again, replaced, "the original backup is carried over");
        write(game, "dxvk", &Manifest { version: "2".into(), files: vec![conf.clone()], replaced: again }).unwrap();
        fs::write(&conf, "ours v2").unwrap();
        uninstall(game, "dxvk", &root).unwrap();
        assert_eq!(fs::read_to_string(&conf).unwrap(), "user's own");
        for b in backup::list(game) {
            let _ = backup::delete(&b);
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn user_files_first_seen_on_an_update_are_restored_too() {
        let game = "test-replace-new";
        let root = sandbox("replace-new");
        let (dll, conf) = (root.join("dxgi.dll"), root.join("dxvk.conf"));
        fs::write(&dll, "user's dll").unwrap();
        let first = plan_replace(game, "dxvk", "first", &[dll.clone(), conf.clone()]).unwrap();
        write(game, "dxvk", &Manifest { version: "1".into(), files: vec![dll.clone()], replaced: first.clone() }).unwrap();
        fs::write(&dll, "ours").unwrap();
        // The user adds their own conf later; the update replaces it.
        fs::write(&conf, "user's conf").unwrap();
        let again = plan_replace(game, "dxvk", "update", &[dll.clone(), conf.clone()]).unwrap();
        assert_eq!(again, first, "still one backup");
        write(game, "dxvk", &Manifest { version: "2".into(), files: vec![dll.clone(), conf.clone()], replaced: again }).unwrap();
        fs::write(&conf, "ours").unwrap();
        uninstall(game, "dxvk", &root).unwrap();
        assert_eq!(fs::read_to_string(&dll).unwrap(), "user's dll");
        assert_eq!(fs::read_to_string(&conf).unwrap(), "user's conf");
        for b in backup::list(game) {
            let _ = backup::delete(&b);
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn uninstall_also_removes_disabled_copies() {
        let game = "test-moved-copy";
        let root = sandbox("moved-copy");
        let file = root.join("sdk_mods").join("fix.sdkmod");
        let parked = root.join("sdk_mods_disabled").join("fix.sdkmod");
        fs::create_dir_all(parked.parent().unwrap()).unwrap();
        fs::write(&parked, b"x").unwrap();
        write(game, "fix", &Manifest { version: "1".into(), files: vec![file.clone()], replaced: None }).unwrap();
        let twin = parked.clone();
        uninstall_with(game, "fix", &root, move |f| (f == file).then(|| twin.clone())).unwrap();
        assert!(!parked.exists() && read(game, "fix").is_none());
        let _ = fs::remove_dir_all(&root);
    }
}
