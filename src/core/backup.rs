//! Snapshot backups of every file Vault Patcher is about to touch.
//!
//! Layout: `<data dir>/backups/<game id>/<timestamp>/<file name>` plus a
//! `manifest.json` recording where each file came from, so a restore puts
//! everything back even if the user moved nothing but the backup folder.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupEntry {
    pub stored_as: String,
    pub original: PathBuf,
    /// The file did not exist before; restoring deletes it.
    #[serde(default)]
    pub created: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Backup {
    pub label: String,
    pub created_at: String,
    pub files: Vec<BackupEntry>,
    #[serde(skip)]
    pub dir: PathBuf,
}

/// Where Vault Patcher keeps its settings, records and backups: a
/// `VaultPatcher Data` folder next to the exe, so the app is portable. Tests
/// use a throwaway folder so they never touch the real one, and
/// `VAULT_PATCHER_DATA_DIR` points it anywhere else.
pub fn data_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join("vaultpatcher-test-data");
    }
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(resolve_data_dir).clone()
}

/// The folder name next to the exe.
const PORTABLE_DIR: &str = "VaultPatcher Data";

/// Picks the data folder once per run. Next to the exe when that folder is
/// writable, else `%APPDATA%\VaultPatcher` (an exe in Program Files). Data an
/// older version kept in `%APPDATA%` moves next to the exe on first run.
fn resolve_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("VAULT_PATCHER_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let legacy = dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("VaultPatcher");
    let Some(portable) = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join(PORTABLE_DIR))) else {
        return legacy;
    };
    if portable.is_dir() {
        return portable;
    }
    if !legacy.is_dir() {
        return if writable_dir(&portable) { portable } else { legacy };
    }
    match move_dir(&legacy, &portable) {
        Ok(()) => portable,
        Err(_) => legacy,
    }
}

/// Creates `dir` and checks a file can be written in it.
fn writable_dir(dir: &Path) -> bool {
    let probe = dir.join(".write-test");
    let ok = std::fs::create_dir_all(dir).is_ok() && std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    if !ok {
        let _ = std::fs::remove_dir(dir);
    }
    ok
}

/// Moves a folder: a rename on the same drive, otherwise a full copy that
/// only deletes the original once everything arrived. A failed copy is
/// removed again, so the original stays the one in use.
fn move_dir(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if let Err(e) = copy_tree(from, to) {
        let _ = std::fs::remove_dir_all(to);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(from);
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn game_dir(game_id: &str) -> PathBuf {
    data_dir().join("backups").join(game_id)
}

/// Copies `files` into a new snapshot. Missing files are recorded as
/// "created" so restoring removes whatever the patch added.
pub fn create(game_id: &str, label: &str, files: &[PathBuf]) -> Result<Backup> {
    let now = chrono::Local::now();
    let mut dir = game_dir(game_id).join(now.format("%Y%m%d-%H%M%S").to_string());
    let mut n = 1;
    while dir.exists() {
        dir = game_dir(game_id).join(format!("{}-{n}", now.format("%Y%m%d-%H%M%S")));
        n += 1;
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let mut entries = Vec::new();
    for (i, original) in files.iter().enumerate() {
        let name = original
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        let stored_as = format!("{i:02}_{name}");
        let created = !original.exists();
        if !created {
            std::fs::copy(original, dir.join(&stored_as))
                .with_context(|| format!("backing up {}", original.display()))?;
        }
        entries.push(BackupEntry {
            stored_as,
            original: original.clone(),
            created,
        });
    }
    let backup = Backup {
        label: label.to_string(),
        created_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        files: entries,
        dir: dir.clone(),
    };
    super::atomic::write(&dir.join("manifest.json"), &serde_json::to_vec_pretty(&backup)?)?;
    Ok(backup)
}

/// A path as Windows compares it, for matching backup entries.
fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// Adds the `files` a snapshot doesn't hold yet to it, so restoring it puts
/// them back too. Missing files are recorded as created.
pub fn add_files(backup: &mut Backup, files: &[PathBuf]) -> Result<()> {
    for original in files {
        if backup.files.iter().any(|f| path_key(&f.original) == path_key(original)) {
            continue;
        }
        let name = original.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
        let stored_as = format!("{:02}_{name}", backup.files.len());
        let created = !original.exists();
        if !created {
            std::fs::copy(original, backup.dir.join(&stored_as))
                .with_context(|| format!("backing up {}", original.display()))?;
        }
        backup.files.push(BackupEntry { stored_as, original: original.clone(), created });
    }
    super::atomic::write(&backup.dir.join("manifest.json"), &serde_json::to_vec_pretty(&*backup)?)
}

/// The `files` a snapshot holds no copy of.
pub fn missing_from(backup: &Backup, files: &[PathBuf]) -> Vec<PathBuf> {
    files
        .iter()
        .filter(|f| !backup.files.iter().any(|b| path_key(&b.original) == path_key(f)))
        .cloned()
        .collect()
}

/// Label of the permanent snapshot taken before Vault Patcher first changes
/// a game's settings. Never pruned.
pub const ORIGINAL_LABEL: &str = "Original settings (before Vault Patcher)";

pub fn load(dir: &Path) -> Option<Backup> {
    let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
    let mut backup: Backup = serde_json::from_str(&text).ok()?;
    backup.dir = dir.to_path_buf();
    Some(backup)
}

pub fn list(game_id: &str) -> Vec<Backup> {
    let Ok(entries) = std::fs::read_dir(game_dir(game_id)) else {
        return Vec::new();
    };
    let mut backups: Vec<Backup> = entries
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
            let mut backup: Backup = serde_json::from_str(&text).ok()?;
            backup.dir = dir;
            Some(backup)
        })
        .collect();
    backups.sort_by(|a, b| b.dir.cmp(&a.dir));
    backups
}

pub fn restore(backup: &Backup) -> Result<()> {
    for entry in &backup.files {
        clear_readonly(&entry.original);
        if entry.created {
            if entry.original.is_dir() {
                std::fs::remove_dir_all(&entry.original).ok();
            } else if entry.original.exists() {
                std::fs::remove_file(&entry.original)
                    .with_context(|| format!("removing {}", entry.original.display()))?;
            }
        } else {
            if let Some(parent) = entry.original.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::copy(backup.dir.join(&entry.stored_as), &entry.original)
                .with_context(|| format!("restoring {}", entry.original.display()))?;
        }
    }
    Ok(())
}

pub fn delete(backup: &Backup) -> Result<()> {
    std::fs::remove_dir_all(&backup.dir).with_context(|| format!("deleting {}", backup.dir.display()))
}

pub fn clear_readonly(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            std::fs::set_permissions(path, perms).ok();
        }
    }
}

pub fn set_readonly(path: &Path, readonly: bool) -> Result<()> {
    let mut perms = std::fs::metadata(path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(readonly);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

pub fn is_readonly(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_the_data_folder_keeps_everything() {
        let root = std::env::temp_dir().join(format!("vp-move-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let from = root.join("old");
        std::fs::create_dir_all(from.join("backups").join("bl2")).unwrap();
        std::fs::write(from.join("settings.json"), b"{}").unwrap();
        std::fs::write(from.join("backups").join("bl2").join("a.ini"), b"x").unwrap();

        let to = root.join("new").join(PORTABLE_DIR);
        move_dir(&from, &to).unwrap();
        assert!(!from.exists());
        assert_eq!(std::fs::read(to.join("settings.json")).unwrap(), b"{}");
        assert_eq!(std::fs::read(to.join("backups").join("bl2").join("a.ini")).unwrap(), b"x");

        // The copy path used across drives gives the same result.
        let copy = root.join("copy");
        copy_tree(&to, &copy).unwrap();
        assert_eq!(std::fs::read(copy.join("backups").join("bl2").join("a.ini")).unwrap(), b"x");
        let _ = std::fs::remove_dir_all(&root);
    }
}
