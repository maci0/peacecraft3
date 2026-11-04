//! `war3map.ini` — the map init file: script source selection, safe mode
//! and forced flags.
//!
//! The file is plain INI text (`[section]` + `key=value`) read from the map's
//! embedded archive. No shipped-byte layout beyond that is claimed: keys and
//! sections are retained verbatim with provenance, and script selection is
//! resolved only against members the archive actually carries. A missing native
//! meaning for a key is a loud refusal at use, never a silent default.

use std::collections::BTreeMap;

/// `war3map.ini`, the only name the game reads this file under.
pub const INI_NAME: &str = "war3map.ini";

/// Known script members the init file may select between. These are file
/// names, not behavior claims.
pub const SCRIPT_MEMBERS: [&str; 3] = ["war3map.j", "war3map.wtg", "war3map.wct"];

/// One parsed `war3map.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ini {
    raw: Vec<u8>,
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl Ini {
    /// Parse INI bytes, retaining every section and key verbatim.
    ///
    /// Lines are split on LF with an optional trailing CR. A section header is
    /// `[name]`; a value line is `key=value` (first `=` splits). Blank lines
    /// and `;`/`#` comments are skipped. Anything else is refused with its
    /// 1-based line number rather than guessed.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| format!("{INI_NAME}: not UTF-8 at byte {}", e.valid_up_to()))?;
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut current: Option<String> = None;
        for (index, raw_line) in text.split('\n').enumerate() {
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                continue;
            }
            if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Err(format!("{INI_NAME}: empty section at line {}", index + 1));
                }
                sections.entry(name.clone()).or_default();
                current = Some(name);
                continue;
            }
            let Some((key, value)) = trimmed.split_once('=') else {
                return Err(format!(
                    "{INI_NAME}: line {} is not [section] or key=value: {trimmed:?}",
                    index + 1
                ));
            };
            let section = current.as_ref().ok_or_else(|| {
                format!(
                    "{INI_NAME}: key=value before any section at line {}",
                    index + 1
                )
            })?;
            sections
                .get_mut(section)
                .expect("current section is retained")
                .insert(key.trim().to_string(), value.trim().to_string());
        }
        Ok(Self {
            raw: bytes,
            sections,
        })
    }

    /// The file's own bytes.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Every section and key verbatim, in sorted order.
    pub fn sections(&self) -> &BTreeMap<String, BTreeMap<String, String>> {
        &self.sections
    }

    /// Look up one key in one section.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .get(section)
            .and_then(|s| s.get(key))
            .map(String::as_str)
    }

    /// FNV-1a 64 over the raw file.
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.raw)
    }
}

/// Which script source the map wants, resolved only against members the
/// archive actually carries.
///
/// The init file may name a script member in any value; this scans every key
/// and value for a known script member spelling (case-insensitive, `/` and
/// `\` unified). When the init file names a member the archive carries, that
/// member wins. Otherwise the archive decides: `war3map.j` when present
/// (JASS-first maps), else the `wtg`+`wct` editor pair. Nothing is invented:
/// an init file that names no carried member leaves the archive order in
/// charge, and a map with neither path is reported by the caller.
pub fn preferred_script(ini: Option<&Ini>, present: &[&str]) -> Option<&'static str> {
    let has = |name: &str| present.iter().any(|p| p.eq_ignore_ascii_case(name));
    if let Some(ini) = ini {
        for section in ini.sections.values() {
            for (key, value) in section {
                for candidate in SCRIPT_MEMBERS {
                    if (key.eq_ignore_ascii_case(candidate)
                        || value.eq_ignore_ascii_case(candidate))
                        && has(candidate)
                    {
                        return Some(match candidate {
                            "war3map.j" => "war3map.j",
                            "war3map.wtg" => "war3map.wtg",
                            _ => "war3map.wtg",
                        });
                    }
                    // Substring match for values like `war3map.j` with a path prefix.
                    let lower = value.replace('\\', "/");
                    if lower.len() >= candidate.len()
                        && lower
                            .as_bytes()
                            .windows(candidate.len())
                            .any(|w| w.eq_ignore_ascii_case(candidate.as_bytes()))
                        && has(candidate)
                    {
                        return Some(match candidate {
                            "war3map.j" => "war3map.j",
                            _ => "war3map.wtg",
                        });
                    }
                }
            }
        }
    }
    if has("war3map.j") {
        return Some("war3map.j");
    }
    if has("war3map.wtg") && has("war3map.wct") {
        return Some("war3map.wtg");
    }
    None
}
