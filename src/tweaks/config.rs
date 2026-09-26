//! The set of ini files a game's tweaks operate on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::Key;
use crate::core::ini::IniDoc;

#[derive(Clone)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub doc: IniDoc,
    /// Whether the file existed on disk when loaded.
    pub exists: bool,
    /// False when the file exists but couldn't be read (locked by the game,
    /// antivirus, a cloud placeholder...). Such a file is never written, so
    /// an empty stand-in can't replace the user's real settings.
    pub readable: bool,
    pub dirty: bool,
}

#[derive(Clone, Default)]
pub struct ConfigSet {
    files: BTreeMap<&'static str, ConfigFile>,
}

impl ConfigSet {
    /// Files that exist but couldn't be read.
    pub fn unreadable(&self) -> impl Iterator<Item = &ConfigFile> {
        self.files.values().filter(|f| !f.readable)
    }

    /// Loads each `(id, file name)` from `dir`. Missing files load as empty
    /// documents so tweaks can still create them.
    pub fn load(dir: &Path, files: &[(&'static str, &'static str)]) -> Self {
        let mut set = Self::default();
        for &(id, name) in files {
            let path = dir.join(name);
            let (doc, exists, readable) = match std::fs::read(&path) {
                Ok(bytes) => (IniDoc::from_bytes(&bytes), true, true),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (IniDoc::default(), false, true),
                Err(_) => (IniDoc::default(), true, false),
            };
            set.files.insert(
                id,
                ConfigFile {
                    path,
                    doc,
                    exists,
                    readable,
                    dirty: false,
                },
            );
        }
        set
    }

    /// In-memory set for tests: every document counts as an existing file.
    #[cfg(test)]
    pub fn from_docs(docs: &[(&'static str, IniDoc)]) -> Self {
        let mut set = Self::default();
        for (id, doc) in docs {
            set.files.insert(
                id,
                ConfigFile {
                    path: PathBuf::from(id),
                    doc: doc.clone(),
                    exists: true,
                    readable: true,
                    dirty: false,
                },
            );
        }
        set
    }

    pub fn file(&self, id: &str) -> Option<&ConfigFile> {
        self.files.get(id)
    }

    pub fn files(&self) -> impl Iterator<Item = (&&'static str, &ConfigFile)> {
        self.files.iter()
    }

    pub fn doc(&self, id: &str) -> Option<&IniDoc> {
        self.files.get(id).map(|f| &f.doc)
    }

    /// Mutable access marks the file dirty.
    pub fn doc_mut(&mut self, id: &str) -> Option<&mut IniDoc> {
        self.files.get_mut(id).map(|f| {
            f.dirty = true;
            &mut f.doc
        })
    }

    pub fn get(&self, k: &Key) -> Option<&str> {
        self.doc(k.file)?.get(k.section, k.key)
    }

    pub fn set(&mut self, k: &Key, value: &str) {
        if let Some(doc) = self.doc_mut(k.file) {
            doc.set(k.section, k.key, value);
        }
    }

    pub fn dirty_paths(&self) -> Vec<PathBuf> {
        self.files
            .values()
            .filter(|f| f.dirty)
            .map(|f| f.path.clone())
            .collect()
    }

    /// Writes dirty files, temporarily lifting read-only locks. Files that
    /// were locked get their lock back afterwards.
    pub fn save_dirty(&mut self) -> Result<Vec<PathBuf>> {
        if let Some(f) = self.files.values().find(|f| f.dirty && !f.readable) {
            bail!(
                "{} couldn't be read (is the game or another program using it?), so it wasn't changed",
                f.path.display()
            );
        }
        let mut written = Vec::new();
        for file in self.files.values_mut().filter(|f| f.dirty) {
            // Replaced whole (never left half-written); a read-only lock is
            // lifted for the swap and put back, even if it fails.
            file.doc.save(&file.path)?;
            file.dirty = false;
            file.exists = true;
            written.push(file.path.clone());
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_files_are_never_overwritten() {
        let dir = std::env::temp_dir().join("vaultpatcher-unreadable-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Good.ini"), "[A]\r\nB=1\r\n").unwrap();
        // A directory where the file should be: exists, but can't be read.
        std::fs::create_dir_all(dir.join("Locked.ini")).unwrap();
        let mut set = ConfigSet::load(&dir, &[("good", "Good.ini"), ("locked", "Locked.ini")]);
        assert!(set.file("locked").is_some_and(|f| f.exists && !f.readable));
        set.set(&Key { file: "good", section: "A", key: "B" }, "2");
        set.set(&Key { file: "locked", section: "A", key: "B" }, "2");
        assert!(set.save_dirty().is_err(), "must refuse to write when a target couldn't be read");
        assert_eq!(std::fs::read_to_string(dir.join("Good.ini")).unwrap(), "[A]\r\nB=1\r\n", "nothing written at all");
        assert!(dir.join("Locked.ini").is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_a_locked_file_keeps_the_lock() {
        use crate::core::backup;
        let dir = std::env::temp_dir().join("vaultpatcher-config-lock");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Engine.ini");
        std::fs::write(&path, "[A]\r\nB = 1\r\nC=2\r\n").unwrap();
        backup::set_readonly(&path, true).unwrap();
        let mut set = ConfigSet::load(&dir, &[("e", "Engine.ini")]);
        set.set(&Key { file: "e", section: "A", key: "C" }, "3");
        assert_eq!(set.save_dirty().unwrap(), vec![path.clone()]);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[A]\r\nB = 1\r\nC=3\r\n");
        assert!(backup::is_readonly(&path));
        backup::clear_readonly(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
