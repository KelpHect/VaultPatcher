//! Format-preserving reader/writer for Unreal Engine 3 style ini files.
//!
//! UE3 ini files differ from "classic" ini files in a few ways that matter
//! when patching them:
//! * keys may repeat inside a section (arrays such as `StartupMovies=`),
//! * keys may carry an operator prefix (`+Key=`, `-Key=`, `.Key=`, `!Key=`),
//! * sections may repeat, and the game reads them as one merged section,
//! * comments start with `;` or `//`.
//!
//! Every edit here touches only the lines it has to, so a patched file diffs
//! cleanly against the original and anything we don't understand survives.

use std::path::Path;

use anyhow::{Context as _, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    /// Not valid UTF-8 (e.g. Windows-1252): each byte maps to one char and
    /// back, so the file round-trips byte for byte.
    Latin1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    /// A `[name]` header; `raw` is the whole line as written (spacing,
    /// a trailing comment), reproduced as is.
    Section { name: String, raw: String },
    /// `key` is trimmed for lookups; `raw_key` is everything before the `=`
    /// as written, so untouched spacing survives a save.
    Entry { key: String, raw_key: String, value: String },
    Other(String),
}

impl Line {
    fn entry(key: &str, value: &str) -> Self {
        Line::Entry { key: key.to_string(), raw_key: key.to_string(), value: value.to_string() }
    }
}

/// The name of a `[Section]` header line, which may carry a trailing comment.
fn header_name(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix('[')?;
    let end = rest.find(']')?;
    let after = rest[end + 1..].trim();
    (after.is_empty() || is_comment(after)).then(|| &rest[..end])
}

#[derive(Clone, Debug)]
pub struct IniDoc {
    lines: Vec<Line>,
    encoding: Encoding,
    newline: &'static str,
    trailing_newline: bool,
}

impl Default for IniDoc {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            encoding: Encoding::Utf8,
            newline: "\r\n",
            trailing_newline: true,
        }
    }
}

fn is_comment(trimmed: &str) -> bool {
    trimmed.starts_with(';') || trimmed.starts_with("//") || trimmed.starts_with('#')
}

/// Splits `+Key` style names into (operator, bare key).
fn split_op(key: &str) -> (Option<char>, &str) {
    match key.chars().next() {
        Some(c @ ('+' | '-' | '.' | '!')) => (Some(c), &key[1..]),
        _ => (None, key),
    }
}

impl IniDoc {
    pub fn parse(text: &str) -> Self {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let trailing_newline = text.ends_with('\n');
        let mut lines = Vec::new();
        for raw in text.lines() {
            let raw = raw.strip_suffix('\r').unwrap_or(raw);
            let trimmed = raw.trim();
            if let Some(name) = header_name(trimmed) {
                lines.push(Line::Section { name: name.to_string(), raw: raw.to_string() });
            } else if !trimmed.is_empty() && !is_comment(trimmed) {
                if let Some((k, v)) = raw.split_once('=') {
                    lines.push(Line::Entry {
                        key: k.trim().to_string(),
                        raw_key: k.to_string(),
                        value: v.to_string(),
                    });
                    continue;
                }
                lines.push(Line::Other(raw.to_string()));
            } else {
                lines.push(Line::Other(raw.to_string()));
            }
        }
        Self {
            lines,
            encoding: Encoding::Utf8,
            newline,
            trailing_newline,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let (text, encoding) = if bytes.starts_with(&[0xFF, 0xFE]) {
            let units: Vec<u16> = bytes[2..]
                .as_chunks::<2>().0.iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            (String::from_utf16_lossy(&units), Encoding::Utf16Le)
        } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            (String::from_utf8_lossy(&bytes[3..]).into_owned(), Encoding::Utf8Bom)
        } else {
            match std::str::from_utf8(bytes) {
                Ok(text) => (text.to_string(), Encoding::Utf8),
                Err(_) => (bytes.iter().map(|&b| b as char).collect(), Encoding::Latin1),
            }
        };
        let mut doc = Self::parse(&text);
        doc.encoding = encoding;
        doc
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push_str(self.newline);
            }
            match line {
                Line::Section { raw, .. } => out.push_str(raw),
                Line::Entry { raw_key, value, .. } => {
                    out.push_str(raw_key);
                    out.push('=');
                    out.push_str(value);
                }
                Line::Other(raw) => out.push_str(raw),
            }
        }
        if self.trailing_newline && !self.lines.is_empty() {
            out.push_str(self.newline);
        }
        out
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let text = self.to_text();
        match self.encoding {
            Encoding::Utf8 => text.into_bytes(),
            Encoding::Utf8Bom => {
                let mut out = vec![0xEF, 0xBB, 0xBF];
                out.extend_from_slice(text.as_bytes());
                out
            }
            Encoding::Latin1 => text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect(),
            Encoding::Utf16Le => {
                let mut out = vec![0xFF, 0xFE];
                for unit in text.encode_utf16() {
                    out.extend_from_slice(&unit.to_le_bytes());
                }
                out
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        crate::core::atomic::write(path, &self.to_bytes()).with_context(|| format!("saving {}", path.display()))
    }

    /// Index ranges `(header, end)` of every occurrence of `section`, where
    /// `end` is exclusive and points at the next section header (or EOF).
    fn section_ranges(&self, section: &str) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let mut current: Option<usize> = None;
        for (i, line) in self.lines.iter().enumerate() {
            if let Line::Section { name, .. } = line {
                if let Some(start) = current.take() {
                    ranges.push((start, i));
                }
                if name.eq_ignore_ascii_case(section) {
                    current = Some(i);
                }
            }
        }
        if let Some(start) = current {
            ranges.push((start, self.lines.len()));
        }
        ranges
    }

    fn entry_indices(&self, section: &str, key: &str) -> Vec<usize> {
        let mut out = Vec::new();
        for (start, end) in self.section_ranges(section) {
            for i in start + 1..end {
                if let Line::Entry { key: k, .. } = &self.lines[i] {
                    let (op, bare) = split_op(k);
                    if matches!(op, None | Some('+') | Some('.')) && bare.eq_ignore_ascii_case(key) {
                        out.push(i);
                    }
                }
            }
        }
        out
    }

    /// Last value wins, matching how UE3 resolves a repeated scalar key.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.entry_indices(section, key).last().map(|&i| match &self.lines[i] {
            Line::Entry { value, .. } => value.trim(),
            _ => unreachable!(),
        })
    }

    pub fn get_all(&self, section: &str, key: &str) -> Vec<&str> {
        self.entry_indices(section, key)
            .into_iter()
            .map(|i| match &self.lines[i] {
                Line::Entry { value, .. } => value.trim(),
                _ => unreachable!(),
            })
            .collect()
    }

    /// Sets a scalar key: rewrites the last occurrence in place and drops any
    /// earlier duplicates, or appends the key to the section (creating the
    /// section at the end of the file if needed).
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let indices = self.entry_indices(section, key);
        if let Some((&last, earlier)) = indices.split_last() {
            if let Line::Entry { value: v, .. } = &mut self.lines[last] {
                // Keep the spacing after `=` the file already uses.
                let lead = v.len() - v.trim_start().len();
                *v = format!("{}{value}", &v[..lead]);
            }
            for &i in earlier.iter().rev() {
                self.lines.remove(i);
            }
            return;
        }
        let at = self.insertion_point(section);
        self.lines.insert(at, Line::entry(key, value));
    }

    /// Replaces every value of an array key with `values`, keeping them where
    /// the first old value was so the file layout stays familiar.
    pub fn set_all(&mut self, section: &str, key: &str, values: &[&str]) {
        let indices = self.entry_indices(section, key);
        let at = match indices.first() {
            Some(&first) => first,
            None => self.insertion_point(section),
        };
        for &i in indices.iter().rev() {
            self.lines.remove(i);
        }
        for (n, value) in values.iter().enumerate() {
            self.lines.insert(at + n, Line::entry(key, value));
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn remove(&mut self, section: &str, key: &str) -> usize {
        let indices = self.entry_indices(section, key);
        for &i in indices.iter().rev() {
            self.lines.remove(i);
        }
        indices.len()
    }

    /// Removes one specific `key=value` pair from an array key.
    pub fn remove_value(&mut self, section: &str, key: &str, value: &str) -> bool {
        let found = self.entry_indices(section, key).into_iter().rev().find(|&i| {
            matches!(&self.lines[i], Line::Entry { value: v, .. } if v.trim().eq_ignore_ascii_case(value))
        });
        if let Some(i) = found {
            self.lines.remove(i);
        }
        found.is_some()
    }

    /// First value of an array key accepted by `pred` (e.g. one `Bindings=` entry).
    pub fn find_value(&self, section: &str, key: &str, pred: impl Fn(&str) -> bool) -> Option<&str> {
        self.get_all(section, key).into_iter().find(|v| pred(v))
    }

    /// Removes every value of an array key accepted by `pred`; returns how many.
    pub fn remove_values_where(&mut self, section: &str, key: &str, pred: impl Fn(&str) -> bool) -> usize {
        let doomed: Vec<usize> = self
            .entry_indices(section, key)
            .into_iter()
            .filter(|&i| matches!(&self.lines[i], Line::Entry { value, .. } if pred(value.trim())))
            .collect();
        for &i in doomed.iter().rev() {
            self.lines.remove(i);
        }
        doomed.len()
    }

    /// Adds `key=value` to the end of the section unless already present.
    pub fn add_value(&mut self, section: &str, key: &str, value: &str) {
        let exists = self
            .get_all(section, key)
            .iter()
            .any(|v| v.eq_ignore_ascii_case(value));
        if !exists {
            let at = self.insertion_point(section);
            self.lines.insert(at, Line::entry(key, value));
        }
    }

    /// Where a new key for `section` belongs: after the last non-blank line of
    /// its final occurrence, so blank separator lines stay between sections.
    fn insertion_point(&mut self, section: &str) -> usize {
        if let Some(&(start, end)) = self.section_ranges(section).last() {
            let mut at = end;
            while at > start + 1 {
                match &self.lines[at - 1] {
                    Line::Other(raw) if raw.trim().is_empty() => at -= 1,
                    _ => break,
                }
            }
            return at;
        }
        if let Some(Line::Other(raw)) = self.lines.last() {
            if !raw.trim().is_empty() {
                self.lines.push(Line::Other(String::new()));
            }
        } else if !self.lines.is_empty() {
            self.lines.push(Line::Other(String::new()));
        }
        self.lines.push(Line::Section { name: section.to_string(), raw: format!("[{section}]") });
        self.lines.len()
    }
}

/// Parses UE3 booleans, which show up as `True`, `TRUE`, `true`, `1`, ...
pub fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[SystemSettings]\r\nBloom=True\r\nResX=1920\r\n\r\n[FullScreenMovie]\r\nbForceNoMovies=FALSE\r\nStartupMovies=2K_logo\r\nStartupMovies=Gearbox_logo\r\nStartupMovies=Loading\r\n\r\n[Engine.Console]\r\n; comment=ignored\r\n+ConsoleKey=Tilde\r\n";

    #[test]
    fn round_trips_untouched_files_byte_for_byte() {
        let doc = IniDoc::parse(SAMPLE);
        assert_eq!(doc.to_text(), SAMPLE);
    }

    #[test]
    fn reads_scalars_arrays_and_prefixed_keys() {
        let doc = IniDoc::parse(SAMPLE);
        assert_eq!(doc.get("systemsettings", "bloom"), Some("True"));
        assert_eq!(doc.get_all("FullScreenMovie", "StartupMovies").len(), 3);
        assert_eq!(doc.get("Engine.Console", "ConsoleKey"), Some("Tilde"));
        assert_eq!(doc.get("Engine.Console", "comment"), None);
    }

    #[test]
    fn set_edits_in_place_and_appends_missing_keys() {
        let mut doc = IniDoc::parse(SAMPLE);
        doc.set("SystemSettings", "Bloom", "False");
        doc.set("SystemSettings", "ResY", "1080");
        let text = doc.to_text();
        assert!(text.starts_with("[SystemSettings]\r\nBloom=False\r\nResX=1920\r\nResY=1080\r\n\r\n[FullScreenMovie]"));
    }

    #[test]
    fn edits_keep_untouched_lines_exactly() {
        let text = "[A]\r\nKey = Value\r\n  Other=1\r\n\t[B] ; note\r\nX=1\r\n";
        let mut doc = IniDoc::parse(text);
        assert_eq!(doc.get("B", "X"), Some("1"), "a header with a comment is still a header");
        doc.set("A", "Key", "New");
        doc.set("A", "Added", "2");
        assert_eq!(doc.to_text(), "[A]\r\nKey = New\r\n  Other=1\r\nAdded=2\r\n\t[B] ; note\r\nX=1\r\n");
        let mut round = IniDoc::parse(text);
        round.set("B", "X", "1");
        assert_eq!(round.to_text(), text, "rewriting the same value changes nothing");
    }

    #[test]
    fn set_creates_missing_sections() {
        let mut doc = IniDoc::parse(SAMPLE);
        doc.set("Engine.Engine", "bSmoothFrameRate", "FALSE");
        assert_eq!(doc.get("Engine.Engine", "bSmoothFrameRate"), Some("FALSE"));
        assert!(doc.to_text().ends_with("\r\n\r\n[Engine.Engine]\r\nbSmoothFrameRate=FALSE\r\n"));
    }

    #[test]
    fn array_helpers() {
        let mut doc = IniDoc::parse(SAMPLE);
        doc.set_all("FullScreenMovie", "StartupMovies", &["Loading"]);
        assert_eq!(doc.get_all("FullScreenMovie", "StartupMovies"), vec!["Loading"]);
        doc.add_value("FullScreenMovie", "StartupMovies", "2K_logo");
        doc.add_value("FullScreenMovie", "StartupMovies", "2k_LOGO");
        assert_eq!(doc.get_all("FullScreenMovie", "StartupMovies").len(), 2);
        assert!(doc.remove_value("FullScreenMovie", "StartupMovies", "loading"));
        assert_eq!(doc.remove("FullScreenMovie", "StartupMovies"), 1);
    }

    #[test]
    fn ansi_files_round_trip_byte_for_byte() {
        let bytes = b"[A]\r\nName=Caf\xe9 \xa9 2K\r\n".to_vec();
        let mut doc = IniDoc::from_bytes(&bytes);
        assert_eq!(doc.to_bytes(), bytes);
        doc.set("A", "Other", "1");
        let out = doc.to_bytes();
        assert!(out.windows(4).any(|w| w == b"Caf\xe9"));
    }

    #[test]
    fn utf16_round_trip() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "[A]\r\nB=1\r\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let mut doc = IniDoc::from_bytes(&bytes);
        assert_eq!(doc.get("A", "B"), Some("1"));
        assert_eq!(doc.to_bytes(), bytes);
        doc.set("A", "B", "2");
        assert_eq!(IniDoc::from_bytes(&doc.to_bytes()).get("A", "B"), Some("2"));
    }
}
