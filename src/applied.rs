//! The values Vault Patcher last wrote, remembered per game.
//!
//! The game, its launcher, an in-game menu or a file check can rewrite the
//! ini files (or revert the exe) behind our back. This record is what
//! "Re-apply" puts back: every tweak value last written through a config
//! save, plus the exe patches meant to stay on. It's a merge — each write
//! updates the tweaks it touched and leaves the rest of the record alone.
//! Values are encoded the same way profiles encode them, so anything a
//! profile can't carry (custom ini values we don't model) isn't remembered
//! either.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::backup;
use crate::games::GameDef;
use crate::tweaks::{ConfigSet, Value};

/// What Vault Patcher currently has down on disk — or believes it has.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Applied {
    /// Tweak id → last written value (encoded like a profile).
    pub values: BTreeMap<String, serde_json::Value>,
    /// Exe patch ids meant to stay applied.
    pub patches: BTreeSet<String>,
}

impl Applied {
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.patches.is_empty()
    }
}

fn path(game_id: &str) -> PathBuf {
    backup::data_dir().join("applied").join(format!("{game_id}.json"))
}

pub fn load(game_id: &str) -> Applied {
    std::fs::read_to_string(path(game_id))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(game_id: &str, applied: &Applied) {
    let path = path(game_id);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, serde_json::to_vec_pretty(applied).unwrap_or_default());
}

/// Merges what a config write changed: a tweak that ended up with a known
/// value is remembered; one that can't be encoded is forgotten (there's
/// nothing we could put back later).
pub fn record(game_id: &str, changes: &[(&'static str, Option<Value>)]) {
    if changes.is_empty() {
        return;
    }
    let mut applied = load(game_id);
    for (id, value) in changes {
        match value.as_ref().and_then(crate::profiles::encode) {
            Some(v) => {
                applied.values.insert((*id).to_string(), v);
            }
            None => {
                applied.values.remove(*id);
            }
        }
    }
    save(game_id, &applied);
}

/// Marks an exe patch as meant to stay applied (or forgets it, when the
/// user reverted it).
pub fn set_patch(game_id: &str, patch: &str, on: bool) {
    let mut applied = load(game_id);
    let changed = if on {
        applied.patches.insert(patch.to_string())
    } else {
        applied.patches.remove(patch)
    };
    if changed {
        save(game_id, &applied);
    }
}

/// After a backup restore the intent is whatever is now on disk: rebuild
/// `values` from the current file contents so re-apply doesn't resurrect
/// the state the restore replaced. Exe patch intent is kept.
pub fn sync_current(game_id: &str, def: &GameDef, config: &ConfigSet) {
    let mut applied = load(game_id);
    applied.values = def
        .visible_tweaks()
        .filter_map(|t| {
            let value = t.read(config)?;
            crate::profiles::encode(&value).map(|e| (t.id.to_string(), e))
        })
        .collect();
    save(game_id, &applied);
}

/// Every remembered value that still maps to a real tweak of this game,
/// decoded and clamped (a stale id or removed option is skipped).
pub fn resolve(def: &GameDef, applied: &Applied) -> Vec<(&'static crate::tweaks::Tweak, Value)> {
    applied
        .values
        .iter()
        .filter_map(|(id, raw)| {
            def.tweak(id)
                .and_then(|t| crate::profiles::decode(t, raw).map(|v| (t, t.clamp(v))))
        })
        .collect()
}
