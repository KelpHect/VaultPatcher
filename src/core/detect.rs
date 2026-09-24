//! Locating game installs through Steam and the Epic Games Launcher.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    Steam,
    Epic,
    Manual,
}

impl Store {
    pub fn label(self) -> &'static str {
        match self {
            Store::Steam => "Steam",
            Store::Epic => "Epic Games",
            Store::Manual => "Manual",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Install {
    pub root: PathBuf,
    pub store: Store,
}

/// What a game definition needs to be found on disk.
pub struct DetectSpec<'a> {
    pub steam_app_ids: &'a [u32],
    /// Case-insensitive substrings matched against Epic's `DisplayName`.
    pub epic_names: &'a [&'a str],
    /// Executable relative to the install root; used to validate candidates.
    pub exe: &'a str,
}

pub fn detect(spec: &DetectSpec) -> Option<Install> {
    for library in steam_libraries() {
        for &id in spec.steam_app_ids {
            if let Some(dir) = steam_app_dir(&library, id)
                && dir.join(spec.exe).is_file() {
                    return Some(Install {
                        root: dir,
                        store: Store::Steam,
                    });
                }
        }
    }
    for (name, location) in epic_installs() {
        let lower = name.to_ascii_lowercase();
        if spec.epic_names.iter().any(|n| lower.contains(&n.to_ascii_lowercase())) {
            let dir = PathBuf::from(location);
            if dir.join(spec.exe).is_file() {
                return Some(Install {
                    root: dir,
                    store: Store::Epic,
                });
            }
        }
    }
    None
}

/// Accepts either the install root or any folder inside it (e.g. the exe's
/// folder) and walks up until the executable resolves.
pub fn validate_manual(path: &Path, exe: &str) -> Option<PathBuf> {
    let mut dir = if path.is_file() { path.parent()? } else { path };
    loop {
        if dir.join(exe).is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

fn steam_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam")
            && let Ok(path) = key.get_value::<String, _>("SteamPath") {
                let path = PathBuf::from(path.replace('/', "\\"));
                if path.is_dir() {
                    return Some(path);
                }
            }
        for sub in ["SOFTWARE\\WOW6432Node\\Valve\\Steam", "SOFTWARE\\Valve\\Steam"] {
            if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(sub)
                && let Ok(path) = key.get_value::<String, _>("InstallPath") {
                    let path = PathBuf::from(path);
                    if path.is_dir() {
                        return Some(path);
                    }
                }
        }
        let fallback = PathBuf::from("C:\\Program Files (x86)\\Steam");
        fallback.is_dir().then_some(fallback)
    }
    #[cfg(not(windows))]
    {
        let home = dirs::home_dir()?;
        [".steam/steam", ".local/share/Steam"]
            .iter()
            .map(|p| home.join(p))
            .find(|p| p.is_dir())
    }
}

pub fn steam_libraries() -> Vec<PathBuf> {
    let Some(root) = steam_root() else {
        return Vec::new();
    };
    let mut libraries = vec![root.clone()];
    let vdf = root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = std::fs::read_to_string(vdf) {
        for value in vdf_values(&text, "path") {
            let path = PathBuf::from(value.replace("\\\\", "\\"));
            if path.is_dir() && !libraries.iter().any(|l| same_path(l, &path)) {
                libraries.push(path);
            }
        }
    }
    libraries
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

fn steam_app_dir(library: &Path, app_id: u32) -> Option<PathBuf> {
    let steamapps = library.join("steamapps");
    let manifest = std::fs::read_to_string(steamapps.join(format!("appmanifest_{app_id}.acf"))).ok()?;
    let installdir = vdf_values(&manifest, "installdir").into_iter().next()?;
    let dir = steamapps.join("common").join(installdir);
    dir.is_dir().then_some(dir)
}

/// Minimal extraction of `"key"  "value"` pairs from Valve's KeyValues text.
fn vdf_values(text: &str, wanted: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        // A key/value line splits into: "", key, ws, value, ""
        if parts.len() >= 5 && parts[1].eq_ignore_ascii_case(wanted) {
            out.push(parts[3].to_string());
        }
    }
    out
}

fn epic_installs() -> Vec<(String, String)> {
    let dir = PathBuf::from("C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")))
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path()).ok()?;
            let json: serde_json::Value = serde_json::from_str(&text).ok()?;
            Some((
                json.get("DisplayName")?.as_str()?.to_string(),
                json.get("InstallLocation")?.as_str()?.to_string(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_library_folders() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"49520\"\t\t\"21587485296\"\n\t\t}\n\t}\n}";
        assert_eq!(vdf_values(vdf, "path"), vec!["C:\\\\Program Files (x86)\\\\Steam"]);
    }
}
