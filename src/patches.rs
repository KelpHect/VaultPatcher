//! Executable patches offered on a game's Patches page.

use std::path::Path;

use anyhow::{Context as _, Result};

use crate::core::binpatch::{self, HexPatch, PatchState};
use crate::theme::Rarity;

#[derive(Clone, Copy)]
pub enum ExePatchKind {
    /// Sets IMAGE_FILE_LARGE_ADDRESS_AWARE so the 32-bit exe can use 4 GB.
    LargeAddressAware,
    /// Signature swap. Both sides must match exactly once for the patch to be
    /// offered, which protects unknown exe builds.
    Hex {
        original: &'static str,
        patched: &'static str,
    },
}

#[derive(Clone, Copy)]
pub struct ExePatch {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Not shown: a rarity stripe on a patch meant nothing to people.
    #[allow(dead_code)]
    pub rarity: Rarity,
    pub kind: ExePatchKind,
    /// Shown as a caveat under the description.
    pub note: Option<&'static str>,
    /// False when the "patched" state may be how the game ships (e.g. LAA),
    /// so offering a revert could break a stock exe.
    pub revertible: bool,
}

impl ExePatch {
    pub fn state(&self, exe: &[u8]) -> PatchState {
        match self.kind {
            ExePatchKind::LargeAddressAware => match binpatch::is_large_address_aware(exe) {
                Ok(true) => PatchState::Patched,
                Ok(false) => PatchState::Unpatched,
                Err(_) => PatchState::Unsupported,
            },
            ExePatchKind::Hex { original, patched } => match HexPatch::new(original, patched) {
                Ok(p) => p.state(exe),
                Err(_) => PatchState::Unsupported,
            },
        }
    }

    pub fn set(&self, exe: &mut [u8], enabled: bool) -> Result<()> {
        match self.kind {
            ExePatchKind::LargeAddressAware => binpatch::set_large_address_aware(exe, enabled),
            ExePatchKind::Hex { original, patched } => {
                let p = HexPatch::new(original, patched)?;
                if enabled { p.apply(exe) } else { p.revert(exe) }
            }
        }
    }
}

pub fn read_exe(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("reading {}", path.display()))
}
