//! The values Vaulter last wrote, remembered per game.
//!
//! The game, its launcher, an in-game menu or a file check can rewrite the
//! ini files (or revert the exe) behind our back. This record is what
//! "Re-apply" puts back: every tweak value last written through a config
//! save, plus the exe patches meant to stay on. It's a merge: each write
//! updates the tweaks it touched and leaves the rest of the record alone.
//! Values are encoded the same way profiles encode them, so anything a
//! profile can't carry (custom ini values we don't model) isn't remembered
//! either. The record belongs to one settings folder and one install: when
//! either changes, the part that no longer applies is dropped.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::core::backup;
use crate::core::binpatch::PatchState;
use crate::games::GameDef;
use crate::tweaks::{ConfigSet, Tweak, Value};

/// What Vaulter currently has down on disk, or believes it has.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Applied {
    /// Tweak id → last written value (encoded like a profile).
    pub values: BTreeMap<String, serde_json::Value>,
    /// Exe patch ids meant to stay applied.
    pub patches: BTreeSet<String>,
    /// The settings folder `values` were written to.
    pub config_dir: Option<PathBuf>,
    /// The install whose exe `patches` were applied to.
    pub install: Option<PathBuf>,
}

impl Applied {
    /// Drops what was recorded for another settings folder or install, and
    /// adopts the current ones. An unknown folder (not found right now)
    /// keeps the record as it is.
    pub fn fit(mut self, config_dir: Option<&Path>, install: Option<&Path>) -> Self {
        if let Some(dir) = config_dir {
            if self.config_dir.as_deref().is_some_and(|d| !same_dir(d, dir)) {
                self.values.clear();
            }
            self.config_dir = Some(dir.to_path_buf());
        }
        if let Some(root) = install {
            if self.install.as_deref().is_some_and(|r| !same_dir(r, root)) {
                self.patches.clear();
            }
            self.install = Some(root.to_path_buf());
        }
        self
    }

    /// Merges what a config write changed: a tweak that ended up with a
    /// known value is remembered; one that can't be encoded is forgotten
    /// (there's nothing we could put back later).
    pub fn merge(&mut self, changes: &[(&'static str, Option<Value>)]) {
        for (id, value) in changes {
            match value.as_ref().and_then(crate::profiles::encode) {
                Some(v) => {
                    self.values.insert((*id).to_string(), v);
                }
                None => {
                    self.values.remove(*id);
                }
            }
        }
    }

    /// Stops remembering these tweaks (their bundle was removed).
    pub fn forget(&mut self, ids: impl IntoIterator<Item = &'static str>) {
        for id in ids {
            self.values.remove(id);
        }
    }

    /// Marks an exe patch as meant to stay applied, or forgets it.
    pub fn set_patch(&mut self, patch: &str, on: bool) {
        if on {
            self.patches.insert(patch.to_string());
        } else {
            self.patches.remove(patch);
        }
    }

    /// Whether `tweak`'s remembered value differs from `value`, i.e. the
    /// user just picked the value that's on disk over the remembered one.
    pub fn differs(&self, tweak: &Tweak, value: &Value) -> bool {
        self.values
            .get(tweak.id)
            .is_some_and(|raw| crate::profiles::decode(tweak, raw).as_ref() != Some(value))
    }

    /// Takes what's on disk now as the intent, for the tweaks already
    /// remembered (after a backup restore, or "keep current values").
    /// Nothing new gets pinned: tweaks Vaulter never wrote stay free.
    pub fn sync_values(&mut self, def: &GameDef, config: &ConfigSet) {
        self.values.retain(|id, raw| {
            let now = def.tweak(id).and_then(|t| t.read(config)).and_then(|v| crate::profiles::encode(&v));
            match now {
                Some(v) => {
                    *raw = v;
                    true
                }
                None => false,
            }
        });
    }

    /// Forgets exe patches that aren't applied any more (the exe was
    /// restored from a backup, or the user kept the reverted exe).
    pub fn sync_patches(&mut self, states: &std::collections::HashMap<&'static str, PatchState>) {
        self.patches.retain(|id| states.get(id.as_str()) != Some(&PatchState::Unpatched));
    }
}

/// Folder paths compared the way Windows does: case-insensitive, either
/// slash, trailing separators ignored.
fn same_dir(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

fn path(game_id: &str) -> PathBuf {
    backup::data_dir().join("applied").join(format!("{game_id}.json"))
}

/// The raw record on disk. A missing or unreadable file reads as empty.
pub fn load(game_id: &str) -> Applied {
    load_from(&path(game_id))
}

fn load_from(path: &Path) -> Applied {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Writes via a temp file and a rename, so a crash or a full disk can't
/// leave half a record behind.
fn save_to(path: &Path, applied: &Applied) -> Result<()> {
    crate::core::atomic::write(path, &serde_json::to_vec_pretty(applied)?)
}

/// Loads the record for this settings folder and install, changes it with
/// `f` and saves it. Returns the saved record.
pub fn update(game_id: &str, config_dir: Option<&Path>, install: Option<&Path>, f: impl FnOnce(&mut Applied)) -> Result<Applied> {
    update_at(&path(game_id), config_dir, install, f)
}

fn update_at(path: &Path, config_dir: Option<&Path>, install: Option<&Path>, f: impl FnOnce(&mut Applied)) -> Result<Applied> {
    let before = load_from(path);
    let mut applied = before.clone().fit(config_dir, install);
    f(&mut applied);
    if applied != before {
        save_to(path, &applied).context("remembering what Vaulter wrote, for Re-apply")?;
    }
    Ok(applied)
}

/// Every remembered value that still maps to a real tweak of this game (a
/// stale id or removed option is skipped). Values are put back exactly as
/// they were written, not snapped to the slider grid, so a hand-edited
/// `PoolSize=1024` stays 1024.
pub fn resolve(def: &GameDef, applied: &Applied) -> Vec<(&'static Tweak, Value)> {
    applied
        .values
        .iter()
        .filter_map(|(id, raw)| def.tweak(id).and_then(|t| crate::profiles::decode(t, raw).map(|v| (t, v))))
        .collect()
}

/// How many remembered values differ from what `current` reads now.
pub fn drifted(def: &GameDef, applied: &Applied, current: impl Fn(&Tweak) -> Option<Value>) -> usize {
    resolve(def, applied).iter().filter(|(t, want)| current(t).as_ref() != Some(want)).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ini::IniDoc;

    fn def() -> &'static GameDef {
        &crate::games::bl2::GAME
    }

    fn config() -> ConfigSet {
        let docs: Vec<(&'static str, IniDoc)> = def().ini_files.iter().map(|(id, _)| (*id, IniDoc::default())).collect();
        ConfigSet::from_docs(&docs)
    }

    fn ids(resolved: Vec<(&'static Tweak, Value)>) -> Vec<(&'static str, Value)> {
        resolved.into_iter().map(|(t, v)| (t.id, v)).collect()
    }

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaulter-applied-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("bl2.json")
    }

    #[test]
    fn off_grid_values_are_put_back_as_written() {
        let pool = def().tweak("pool_size").unwrap();
        let mut config = config();
        pool.write(&mut config, &Value::Num(1024.0));
        let mut applied = Applied::default();
        applied.merge(&[(pool.id, pool.read(&config))]);
        assert_eq!(ids(resolve(def(), &applied)), vec![("pool_size", Value::Num(1024.0))]);
        assert_eq!(drifted(def(), &applied, |t| t.read(&config)), 0);
        pool.write(&mut config, &Value::Num(600.0));
        assert_eq!(drifted(def(), &applied, |t| t.read(&config)), 1);
    }

    #[test]
    fn accepting_the_value_on_disk_clears_drift() {
        let pool = def().tweak("pool_size").unwrap();
        let mut config = config();
        let mut applied = Applied::default();
        applied.merge(&[(pool.id, Some(Value::Num(600.0)))]);
        pool.write(&mut config, &Value::Num(800.0));
        assert!(applied.differs(pool, &Value::Num(800.0)));
        assert!(!applied.differs(pool, &Value::Num(600.0)));
        applied.merge(&[(pool.id, Some(Value::Num(800.0)))]);
        assert_eq!(drifted(def(), &applied, |t| t.read(&config)), 0);
        // Tweaks that were never written aren't "different".
        let aniso = def().tweak("aniso").unwrap();
        assert!(!applied.differs(aniso, &aniso.default.to_value()));
    }

    #[test]
    fn syncing_after_a_restore_pins_nothing_new() {
        let pool = def().tweak("pool_size").unwrap();
        let mut config = config();
        for t in def().visible_tweaks() {
            t.write(&mut config, &t.default.to_value());
        }
        pool.write(&mut config, &Value::Num(1024.0));
        let mut applied = Applied::default();
        applied.merge(&[(pool.id, Some(Value::Num(600.0)))]);
        applied.values.insert("gone_tweak".into(), true.into());
        applied.sync_values(def(), &config);
        assert_eq!(applied.values.len(), 1, "only tweaks already remembered stay");
        assert_eq!(ids(resolve(def(), &applied)), vec![("pool_size", Value::Num(1024.0))]);
    }

    #[test]
    fn restored_exe_forgets_reverted_patches() {
        let mut applied = Applied::default();
        applied.set_patch("laa", true);
        applied.set_patch("other", true);
        let states = [("laa", PatchState::Unpatched), ("other", PatchState::Patched)].into_iter().collect();
        applied.sync_patches(&states);
        assert_eq!(applied.patches.iter().collect::<Vec<_>>(), ["other"]);
        applied.set_patch("other", false);
        assert!(applied.patches.is_empty());
    }

    #[test]
    fn record_belongs_to_one_folder_and_install() {
        let mut applied = Applied::default().fit(Some(Path::new("C:\\Cfg")), Some(Path::new("D:\\Game")));
        applied.values.insert("aniso".into(), "4".into());
        applied.set_patch("laa", true);
        let same = applied.clone().fit(Some(Path::new("c:/cfg/")), None);
        assert_eq!((&same.values, &same.patches), (&applied.values, &applied.patches), "same folder, other spelling; unknown install keeps patches");
        let moved = applied.clone().fit(Some(Path::new("E:\\Other")), Some(Path::new("D:\\Game")));
        assert!(moved.values.is_empty());
        assert_eq!(moved.patches.len(), 1);
        assert_eq!(moved.config_dir.as_deref(), Some(Path::new("E:\\Other")));
    }

    #[test]
    fn updates_save_atomically_and_merge() {
        let path = sandbox("update");
        let dir = Path::new("C:\\Cfg");
        let a = update_at(&path, Some(dir), None, |a| a.merge(&[("aniso", Some(Value::Choice("4")))])).unwrap();
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(load_from(&path), a);
        let b = update_at(&path, Some(dir), None, |a| a.set_patch("laa", true)).unwrap();
        assert_eq!(b.values.len(), 1);
        assert_eq!(load_from(&path).patches.len(), 1);
        let c = update_at(&path, Some(dir), None, |a| a.forget(["aniso"])).unwrap();
        assert!(c.values.is_empty());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
