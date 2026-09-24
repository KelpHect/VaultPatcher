//! The set of ini files a game's tweaks operate on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::Key;
use crate::core::backup;
use crate::core::ini::IniDoc;

#[derive(Clone)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub doc: IniDoc,
    /// Whether the file existed on disk when loaded.
    pub exists: bool,
    pub dirty: bool,
}

#[derive(Clone, Default)]
pub struct ConfigSet {
    files: BTreeMap<&'static str, ConfigFile>,
}

impl ConfigSet {
    /// Loads each `(id, file name)` from `dir`. Missing files load as empty
    /// documents so tweaks can still create them.
    pub fn load(dir: &Path, files: &[(&'static str, &'static str)]) -> Self {
        let mut set = Self::default();
        for &(id, name) in files {
            let path = dir.join(name);
            let (doc, exists) = match IniDoc::load(&path) {
                Ok(doc) => (doc, true),
                Err(_) => (IniDoc::default(), false),
            };
            set.files.insert(
                id,
                ConfigFile {
                    path,
                    doc,
                    exists,
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
        let mut written = Vec::new();
        for file in self.files.values_mut().filter(|f| f.dirty) {
            let was_locked = backup::is_readonly(&file.path);
            backup::clear_readonly(&file.path);
            if let Some(parent) = file.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            file.doc.save(&file.path)?;
            if was_locked {
                backup::set_readonly(&file.path, true)?;
            }
            file.dirty = false;
            file.exists = true;
            written.push(file.path.clone());
        }
        Ok(written)
    }
}
