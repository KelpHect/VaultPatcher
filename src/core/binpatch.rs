//! Executable patching: PE header flags and signature-based byte patches.

use anyhow::{Result, bail};

const IMAGE_FILE_LARGE_ADDRESS_AWARE: u16 = 0x0020;

fn pe_offset(bytes: &[u8]) -> Result<usize> {
    if bytes.len() < 0x40 || &bytes[..2] != b"MZ" {
        bail!("not a Windows executable (missing MZ header)");
    }
    let off = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize;
    if bytes.len() < off + 24 || &bytes[off..off + 4] != b"PE\0\0" {
        bail!("not a Windows executable (missing PE header)");
    }
    Ok(off)
}

pub fn is_large_address_aware(bytes: &[u8]) -> Result<bool> {
    let off = pe_offset(bytes)?;
    let chars = u16::from_le_bytes([bytes[off + 22], bytes[off + 23]]);
    Ok(chars & IMAGE_FILE_LARGE_ADDRESS_AWARE != 0)
}

/// Sets or clears the LAA flag and refreshes the optional-header checksum.
pub fn set_large_address_aware(bytes: &mut [u8], enabled: bool) -> Result<()> {
    let off = pe_offset(bytes)?;
    let mut chars = u16::from_le_bytes([bytes[off + 22], bytes[off + 23]]);
    if enabled {
        chars |= IMAGE_FILE_LARGE_ADDRESS_AWARE;
    } else {
        chars &= !IMAGE_FILE_LARGE_ADDRESS_AWARE;
    }
    bytes[off + 22..off + 24].copy_from_slice(&chars.to_le_bytes());
    update_checksum(bytes, off);
    Ok(())
}

/// Recomputes `OptionalHeader.CheckSum` the same way `CheckSumMappedFile` does.
fn update_checksum(bytes: &mut [u8], pe: usize) {
    let checksum_at = pe + 24 + 64;
    if bytes.len() < checksum_at + 4 {
        return;
    }
    bytes[checksum_at..checksum_at + 4].fill(0);
    let mut sum: u64 = 0;
    for chunk in bytes.chunks(2) {
        let word = if chunk.len() == 2 {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            chunk[0] as u16
        };
        sum += word as u64;
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    sum = (sum & 0xFFFF) + (sum >> 16);
    let checksum = (sum as u32).wrapping_add(bytes.len() as u32);
    bytes[checksum_at..checksum_at + 4].copy_from_slice(&checksum.to_le_bytes());
}

/// A byte signature such as `"8B 45 ?? 3D"` where `??` matches anything.
#[derive(Clone, Debug)]
pub struct Pattern(Vec<Option<u8>>);

impl Pattern {
    pub fn parse(text: &str) -> Result<Self> {
        let mut out = Vec::new();
        for token in text.split_whitespace() {
            if token == "??" || token == "?" {
                out.push(None);
            } else {
                out.push(Some(u8::from_str_radix(token, 16)?));
            }
        }
        if out.is_empty() {
            bail!("empty pattern");
        }
        Ok(Self(out))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    fn matches_at(&self, bytes: &[u8], at: usize) -> bool {
        self.0
            .iter()
            .enumerate()
            .all(|(i, b)| b.is_none_or(|b| bytes[at + i] == b))
    }

    pub fn find_all(&self, bytes: &[u8]) -> Vec<usize> {
        if bytes.len() < self.0.len() {
            return Vec::new();
        }
        (0..=bytes.len() - self.0.len())
            .filter(|&at| self.matches_at(bytes, at))
            .collect()
    }
}

/// The state of a signature patch within a given executable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchState {
    Unpatched,
    Patched,
    /// Neither signature (or several matches) was found: an unknown exe build.
    Unsupported,
}

pub struct HexPatch {
    pub original: Pattern,
    pub patched: Pattern,
}

impl HexPatch {
    pub fn new(original: &str, patched: &str) -> Result<Self> {
        let original = Pattern::parse(original)?;
        let patched = Pattern::parse(patched)?;
        if original.len() != patched.len() {
            bail!("original and patched signatures differ in length");
        }
        Ok(Self { original, patched })
    }

    pub fn state(&self, bytes: &[u8]) -> PatchState {
        match (self.original.find_all(bytes).len(), self.patched.find_all(bytes).len()) {
            (1, 0) => PatchState::Unpatched,
            (0, 1) => PatchState::Patched,
            _ => PatchState::Unsupported,
        }
    }

    /// Rewrites the unique match of `from` with the fixed bytes of `to`.
    fn rewrite(from: &Pattern, to: &Pattern, bytes: &mut [u8]) -> Result<()> {
        let hits = from.find_all(bytes);
        if hits.len() != 1 {
            bail!("expected exactly one signature match, found {}", hits.len());
        }
        for (i, b) in to.0.iter().enumerate() {
            if let Some(b) = b {
                bytes[hits[0] + i] = *b;
            }
        }
        Ok(())
    }

    pub fn apply(&self, bytes: &mut [u8]) -> Result<()> {
        Self::rewrite(&self.original, &self.patched, bytes)
    }

    pub fn revert(&self, bytes: &mut [u8]) -> Result<()> {
        Self::rewrite(&self.patched, &self.original, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_patch_round_trip() {
        let patch = HexPatch::new("73 61 79 20", "65 78 65 63").unwrap();
        let mut bytes = b"xxsay yy".to_vec();
        assert_eq!(patch.state(&bytes), PatchState::Unpatched);
        patch.apply(&mut bytes).unwrap();
        assert_eq!(&bytes, b"xxexecyy");
        assert_eq!(patch.state(&bytes), PatchState::Patched);
        patch.revert(&mut bytes).unwrap();
        assert_eq!(&bytes, b"xxsay yy");
    }

    #[test]
    fn wildcards_match_anything() {
        let p = Pattern::parse("01 ?? 03").unwrap();
        assert_eq!(p.find_all(&[0, 1, 9, 3, 1, 2, 3]), vec![1, 4]);
    }
}
