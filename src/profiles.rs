//! Named settings profiles: a snapshot of every setting's value, stored as
//! JSON so it can be shared (export/import) and loaded back later.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

use crate::games::GameDef;
use crate::tweaks::{Control, Tweak, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    /// Game id the profile was made for (`bl2`, `tps`, …).
    pub game: String,
    pub created: String,
    /// Tweak id → value: bool, number, or a choice's option value.
    pub values: BTreeMap<String, serde_json::Value>,
}

/// A saved profile on disk.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub count: usize,
    pub created: String,
}

pub fn dir(game_id: &str) -> PathBuf {
    crate::core::backup::data_dir().join("profiles").join(game_id)
}

pub fn list(game_id: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = std::fs::read_dir(dir(game_id))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|path| {
            let profile = read(&path).ok()?;
            Some(Entry { name: profile.name, count: profile.values.len(), created: profile.created, path })
        })
        .collect();
    out.sort_by_key(|e| e.name.to_lowercase());
    out
}

pub fn read(path: &Path) -> Result<Profile> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).context("not a Vault Patcher profile")
}

pub fn write(profile: &Profile, path: &Path) -> Result<()> {
    crate::core::atomic::write(path, &serde_json::to_vec_pretty(profile)?)
}

/// The saved profile called `name` (ignoring case), if there is one.
pub fn find(game_id: &str, name: &str) -> Option<PathBuf> {
    find_in(&dir(game_id), name)
}

fn find_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let name = name.trim();
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .find(|p| read(p).is_ok_and(|profile| profile.name.trim().to_lowercase() == name.to_lowercase()))
}

/// Where a profile with this name is stored: the file of the profile already
/// called that (so saving again updates it), else a new file whose name no
/// other profile uses.
pub fn path_for(game_id: &str, name: &str) -> PathBuf {
    path_in(&dir(game_id), name)
}

fn path_in(dir: &Path, name: &str) -> PathBuf {
    if let Some(existing) = find_in(dir, name) {
        return existing;
    }
    let slug = slug(name);
    (1..)
        .map(|n| dir.join(if n == 1 { format!("{slug}.json") } else { format!("{slug}-{n}.json") }))
        .find(|p| !p.exists())
        .expect("some file name is free")
}

/// A safe file name for a profile: letters and digits of any script,
/// lowercased, with dashes between words; never a reserved Windows name.
fn slug(name: &str) -> String {
    let slug = name
        .chars()
        .flat_map(|c| if c.is_alphanumeric() { c.to_lowercase().collect::<Vec<_>>() } else { vec!['-'] })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let reserved = ["con", "prn", "aux", "nul"].contains(&slug.as_str())
        || (slug.len() == 4 && (slug.starts_with("com") || slug.starts_with("lpt")) && slug.ends_with(|c: char| c.is_ascii_digit()));
    match slug.as_str() {
        "" => "profile".into(),
        _ if reserved => format!("{slug}-profile"),
        _ => slug,
    }
}

/// `name`, or `name (2)`, `name (3)`…: the first no saved profile uses.
pub fn unused_name(game_id: &str, name: &str) -> String {
    let dir = dir(game_id);
    let name = name.trim();
    (1..)
        .map(|n| if n == 1 { name.to_string() } else { format!("{name} ({n})") })
        .find(|n| find_in(&dir, n).is_none())
        .expect("some name is free")
}

pub fn snapshot(def: &GameDef, name: &str, value_of: impl Fn(&'static Tweak) -> Value) -> Profile {
    let values = def
        .visible_tweaks()
        .filter_map(|t| encode(&value_of(t)).map(|v| (t.id.to_string(), v)))
        .collect();
    Profile {
        name: name.trim().to_string(),
        game: def.id.to_string(),
        created: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        values,
    }
}

pub(crate) fn encode(value: &Value) -> Option<serde_json::Value> {
    Some(match value {
        Value::Bool(b) => (*b).into(),
        Value::Num(n) => serde_json::Number::from_f64(*n)?.into(),
        Value::Choice(c) => (*c).into(),
        // Custom values we don't model can't be restored faithfully.
        Value::Unknown(_) => return None,
    })
}

/// Turns a profile back into tweak values, skipping anything that doesn't
/// fit this game (unknown ids, wrong types, options that don't exist).
pub fn resolve(def: &GameDef, profile: &Profile) -> Result<(Vec<(&'static Tweak, Value)>, usize)> {
    if profile.game != def.id {
        bail!("this profile is for {}, not {}", profile.game.to_uppercase(), def.short);
    }
    let mut out = Vec::new();
    let mut skipped = 0;
    for (id, raw) in &profile.values {
        let decoded = def.tweak(id).and_then(|t| decode(t, raw).map(|v| (t, t.clamp(v))));
        match decoded {
            Some(pair) => out.push(pair),
            None => skipped += 1,
        }
    }
    Ok((out, skipped))
}

pub(crate) fn decode(tweak: &Tweak, raw: &serde_json::Value) -> Option<Value> {
    match tweak.control {
        Control::Toggle => raw.as_bool().map(Value::Bool),
        Control::Slider { .. } => raw.as_f64().map(Value::Num),
        Control::Choice(options) => {
            let s = raw.as_str()?;
            options.iter().find(|o| o.value == s).map(|o| Value::Choice(o.value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_round_trip_and_reject_other_games() {
        let def = &crate::games::bl2::GAME;
        let profile = snapshot(def, "  My Look ", |t| t.default.to_value());
        assert_eq!(profile.name, "My Look");
        assert!(profile.values.len() > 50);
        let json = serde_json::to_string(&profile).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        let (values, skipped) = resolve(def, &back).unwrap();
        assert_eq!(skipped, 0);
        assert!(values.iter().all(|(t, v)| *v == t.clamp(t.default.to_value())));

        let mut other = back.clone();
        other.game = "tps".into();
        assert!(resolve(def, &other).is_err());

        let mut junk = back;
        junk.values.insert("not_a_tweak".into(), true.into());
        junk.values.insert("post_chain".into(), "nonsense".into());
        let (_, skipped) = resolve(def, &junk).unwrap();
        assert_eq!(skipped, 2);
        assert_eq!(slug("My Look!"), "my-look");
    }

    #[test]
    fn profile_names_never_share_a_file() {
        let dir = std::env::temp_dir().join("vaultpatcher-profile-names");
        let _ = std::fs::remove_dir_all(&dir);
        let def = &crate::games::bl2::GAME;
        let save = |name: &str| {
            let path = path_in(&dir, name);
            write(&snapshot(def, name, |t| t.default.to_value()), &path).unwrap();
            path
        };
        let look = save("My Look");
        assert!(look.ends_with("my-look.json"));
        let other = save("my-look!");
        assert_ne!(look, other, "a different name that slugs the same gets its own file");
        assert_eq!(save("MY LOOK"), look, "the same name (any case) updates its file");
        let (ru, jp) = (save("Ультра"), save("高画質"));
        assert!(ru.ends_with("ультра.json") && jp.ends_with("高画質.json"));
        assert!(save("!!!").ends_with("profile.json"));
        assert!(save("???").ends_with("profile-2.json"));
        assert_eq!(slug("CON"), "con-profile");
        let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| read(&e.path()).unwrap().name).collect();
        assert_eq!(names.len(), 6);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
