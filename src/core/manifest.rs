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
    fs::create_dir_all(path.parent().expect("manifest path has a parent"))?;
    fs::write(path, serde_json::to_vec_pretty(manifest)?)?;
    Ok(())
}

/// Deletes the recorded files (only those under `root`), then any folders the
/// install created that are now empty, then the manifest itself.
pub fn uninstall(game_id: &str, component: &str, root: &Path) -> Result<()> {
    let Some(manifest) = read(game_id, component) else {
        anyhow::bail!("no install record for {component}");
    };
    for file in &manifest.files {
        if file.starts_with(root) && file.is_file() {
            backup::clear_readonly(file);
            fs::remove_file(file)
                .with_context(|| format!("removing {} (close the game first)", file.display()))?;
        }
    }
    let mut dirs: Vec<PathBuf> = manifest
        .files
        .iter()
        .flat_map(|f| f.ancestors().skip(1).map(Path::to_path_buf).collect::<Vec<_>>())
        .filter(|d| d.starts_with(root) && d != root)
        .collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dirs.dedup();
    for dir in dirs {
        fs::remove_dir(&dir).ok();
    }
    fs::remove_file(path(game_id, component)).ok();
    Ok(())
}
