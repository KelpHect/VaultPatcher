//! Crash-safe file replacement for everything Vaulter writes: game
//! configs, the game exe, installed files and its own records.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context as _, Result};

use super::backup;

/// Replaces `path` with `bytes` so that either the old or the new file is
/// there afterwards, never half of one: the bytes go to a temp file in the
/// same folder, are flushed to disk, and the temp file is renamed over the
/// original. A read-only original (a "locked" config) is unlocked for the
/// swap and the new file is locked again.
pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let tmp = temp_beside(path);
    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(e) = written {
        fs::remove_file(&tmp).ok();
        return Err(e).with_context(|| format!("writing {}", path.display()));
    }
    let locked = backup::is_readonly(path);
    if locked {
        backup::clear_readonly(path);
    }
    match fs::rename(&tmp, path) {
        Ok(()) => {
            if locked {
                backup::set_readonly(path, true)?;
            }
            Ok(())
        }
        Err(e) => {
            fs::remove_file(&tmp).ok();
            if locked {
                backup::set_readonly(path, true).ok();
            }
            Err(e).with_context(|| format!("writing {}", path.display()))
        }
    }
}

/// A temp file name next to `path` that no other write (in this process or
/// another copy of the app) is using.
fn temp_beside(path: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{name}.{}-{n}.vp-tmp", std::process::id()))
}

/// A fresh path in the system temp folder for a download, unique per call
/// so two jobs never share (or delete) each other's file.
pub fn temp_file(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("vaulter-{}-{n}-{name}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaulter-atomic-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".vp-tmp")).collect()
    }

    #[test]
    fn replaces_and_creates_files_without_leftovers() {
        let dir = sandbox("plain");
        let file = dir.join("WillowEngine.ini");
        write(&file, b"one").unwrap();
        write(&file, b"two").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"two");
        write(&dir.join("new/nested.txt"), b"x").unwrap();
        assert!(dir.join("new/nested.txt").is_file());
        assert!(leftovers(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn keeps_a_read_only_lock() {
        let dir = sandbox("locked");
        let file = dir.join("WillowEngine.ini");
        fs::write(&file, b"old").unwrap();
        backup::set_readonly(&file, true).unwrap();
        write(&file, b"new").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"new");
        assert!(backup::is_readonly(&file), "the lock comes back");
        assert!(leftovers(&dir).is_empty());
        backup::clear_readonly(&file);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_write_leaves_the_original_alone() {
        let dir = sandbox("fail");
        // A folder where the file should be: the rename can't replace it.
        let target = dir.join("Binaries");
        fs::create_dir_all(target.join("inside")).unwrap();
        assert!(write(&target, b"x").is_err());
        assert!(target.join("inside").is_dir());
        assert!(leftovers(&dir).is_empty(), "the temp file is cleaned up");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn temp_files_are_unique() {
        assert_ne!(temp_file("sdk.zip"), temp_file("sdk.zip"));
    }
}
