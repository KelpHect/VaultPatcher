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

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("VaultPatcher")
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
    std::fs::write(dir.join("manifest.json"), serde_json::to_vec_pretty(&backup)?)?;
    Ok(backup)
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
