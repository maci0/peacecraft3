//! `war3map.imp`: the map's import list.
//!
//! ```text
//! 0  4  version, u32 LE: 0 or 1
//! 4  4  entry count, u32 LE
//! 8  n  entries
//!         version 0: NUL-terminated path
//!         version 1: flag byte, then NUL-terminated path
//! ```
//!
//! Flag bytes are kept raw. No shipped byte establishes what a flag value
//! means.
//!
//! # Measured
//!
//! Read with `mpqxtract` from the Warcraft III (Legacy) install: 284 maps, 187 loose
//! files under `Maps/` (45 `.w3m`, 142 `.w3x`), 44 `.w3m` in `War3Local.mpq`
//! and 53 `.w3x` in `War3x.mpq`. `War3xLocal.mpq` lists the same 53 members
//! with byte-identical `war3map.imp` files and is not counted again.
//!
//! * Present in 23 maps: 12 loose `.w3x`, 5 in `War3Local.mpq`, 6 in
//!   `War3x.mpq`. The other 261 have no member. Absence was checked twice,
//!   once by listing and once by a by-name lookup; both report not found.
//! * Version 0 in 5 maps. All five hold the same 31-byte file: one entry,
//!   `LargeCityGateOpen1.wav`, with no flag byte.
//! * Version 1 in 18 maps, 387 entries. Flag values seen: `0` (7 entries,
//!   all in `War3x.mpq`), `8` (236), `13` (144).
//! * Every file ends exactly at its last entry. None has trailing bytes.
//!
//! Versions 2 and up have no measured layout and are refused.

use crate::{Error, Map};
use thiserror::Error;

/// File name inside the embedded map archive.
pub const IMPORTS_NAME: &str = "war3map.imp";

/// Highest version seen in shipped maps.
pub const IMPORTS_MAX_VERSION: u32 = 1;

/// First version whose entries carry a flag byte before the path.
pub const IMPORTS_FLAG_VERSION: u32 = 1;

/// Version and entry count, before the first entry.
const HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImportError {
    #[error("imp needs {need} bytes, has {have}")]
    Truncated { have: usize, need: usize },
    #[error("unsupported imp version {version}; shipped maps use 0 and 1")]
    UnsupportedVersion { version: u32 },
    #[error("imp path at {at} has no NUL terminator")]
    UnterminatedPath { at: usize },
    #[error("imp declares {count} entries, but at most {max} fit in {length} bytes")]
    TooManyEntries {
        count: u32,
        max: usize,
        length: usize,
    },
    #[error("imp entries consume {end} bytes of {length}")]
    TrailingBytes { end: usize, length: usize },
}

/// One import entry, with the bytes as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    /// The flag byte. `None` for version 0, which stores none.
    pub flag: Option<u8>,
    /// The authored path (raw bytes, without the NUL).
    pub path: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imports {
    pub version: u32,
    pub entries: Vec<ImportEntry>,
}

impl Imports {
    pub fn parse(data: &[u8]) -> Result<Self, ImportError> {
        if data.len() < HEADER_LEN {
            return Err(ImportError::Truncated {
                have: data.len(),
                need: HEADER_LEN,
            });
        }
        let version = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let count = u32::from_le_bytes(data[4..8].try_into().unwrap());

        if version > IMPORTS_MAX_VERSION {
            return Err(ImportError::UnsupportedVersion { version });
        }
        let flagged = version >= IMPORTS_FLAG_VERSION;

        // Refuse a count that cannot fit before allocating. Each entry is at
        // least its NUL, plus the flag byte when the version stores one.
        let min_entry_len = 1 + usize::from(flagged);
        let max_entries = (data.len() - HEADER_LEN) / min_entry_len;
        if count as usize > max_entries {
            return Err(ImportError::TooManyEntries {
                count,
                max: max_entries,
                length: data.len(),
            });
        }

        let mut cursor = HEADER_LEN;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let flag = if flagged {
                let byte = *data.get(cursor).ok_or(ImportError::Truncated {
                    have: data.len(),
                    need: cursor + 1,
                })?;
                cursor += 1;
                Some(byte)
            } else {
                None
            };
            let path = read_path(data, &mut cursor)?;
            entries.push(ImportEntry { flag, path });
        }

        if cursor != data.len() {
            return Err(ImportError::TrailingBytes {
                end: cursor,
                length: data.len(),
            });
        }
        Ok(Self { version, entries })
    }
}

fn read_path(data: &[u8], cursor: &mut usize) -> Result<Vec<u8>, ImportError> {
    let tail = data.get(*cursor..).ok_or(ImportError::Truncated {
        have: data.len(),
        need: *cursor + 1,
    })?;
    let nul = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or(ImportError::UnterminatedPath { at: *cursor })?;
    let path = tail[..nul].to_vec();
    *cursor += nul + 1;
    Ok(path)
}

impl Map<'_> {
    /// Decode `war3map.imp` when the archive carries it. Most shipped maps do
    /// not, so a missing member is `Ok(None)`. Any other extraction failure
    /// and a present but malformed file are errors.
    pub fn imports(&self) -> Result<Option<Imports>, Error> {
        match self.extract(IMPORTS_NAME) {
            Ok(bytes) => Ok(Some(Imports::parse(&bytes)?)),
            Err(pcx_mpq::Error::NotFound(_)) => Ok(None),
            Err(e) => Err(Error::Extraction(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Builds an imp file with the given version and paths. Version 1 entries
    /// get flag `0x08`, the value most common in shipped files.
    fn fixture(version: u32, paths: &[&[u8]]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend(version.to_le_bytes());
        data.extend((paths.len() as u32).to_le_bytes());
        for path in paths {
            if version >= IMPORTS_FLAG_VERSION {
                data.push(0x08);
            }
            data.extend(*path);
            data.push(0);
        }
        data
    }

    #[test]
    fn version_zero_has_no_flag_byte() {
        let parsed = Imports::parse(&fixture(0, &[b"a.wav", b"b.blp"])).unwrap();
        assert_eq!(parsed.version, 0);
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].flag, None);
        assert_eq!(parsed.entries[0].path, b"a.wav");
        assert_eq!(parsed.entries[1].path, b"b.blp");
    }

    #[test]
    fn version_one_stores_a_raw_flag_per_entry() {
        let mut data = fixture(1, &[b"a.wav"]);
        data.extend([0x0D, b'b', b'.', b't', b'g', b'a', 0]);
        data[4] = 2;
        let parsed = Imports::parse(&data).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.entries[0].flag, Some(0x08));
        assert_eq!(parsed.entries[0].path, b"a.wav");
        assert_eq!(parsed.entries[1].flag, Some(0x0D));
        assert_eq!(parsed.entries[1].path, b"b.tga");
    }

    #[test]
    fn zero_entries_is_the_bare_header() {
        let parsed = Imports::parse(&fixture(1, &[])).unwrap();
        assert!(parsed.entries.is_empty());
        let mut longer = fixture(0, &[]);
        longer.push(0);
        assert!(matches!(
            Imports::parse(&longer),
            Err(ImportError::TrailingBytes { .. })
        ));
    }

    #[test]
    fn short_headers_are_refused() {
        for length in 0..HEADER_LEN {
            let data = vec![0u8; length];
            assert!(matches!(
                Imports::parse(&data),
                Err(ImportError::Truncated {
                    need: HEADER_LEN,
                    ..
                })
            ));
        }
    }

    #[test]
    fn incomplete_and_trailing_entries_are_refused() {
        for version in 0..=IMPORTS_MAX_VERSION {
            let data = fixture(version, &[b"one.wav", b"two.blp"]);
            for length in 0..data.len() {
                assert!(
                    Imports::parse(&data[..length]).is_err(),
                    "version {version} prefix of {length} bytes parsed"
                );
            }
            let mut longer = data;
            longer.push(0);
            assert!(matches!(
                Imports::parse(&longer),
                Err(ImportError::TrailingBytes { .. })
            ));
        }
    }

    #[test]
    fn unterminated_path_is_refused() {
        let mut data = fixture(1, &[b"x"]);
        data.truncate(data.len() - 1);
        assert!(matches!(
            Imports::parse(&data),
            Err(ImportError::UnterminatedPath { at: 9 })
        ));
    }

    #[test]
    fn unsupported_versions_are_refused() {
        for version in [2, u32::MAX] {
            assert!(matches!(
                Imports::parse(&fixture(version, &[b"x"])),
                Err(ImportError::UnsupportedVersion { version: v }) if v == version
            ));
        }
    }

    #[test]
    fn count_is_bounded_by_the_bytes_that_follow() {
        // Version 1 needs two bytes per entry. Three fit in eight bytes plus
        // six, and four do not.
        let fits = fixture(1, &[b"", b"", b""]);
        assert_eq!(Imports::parse(&fits).unwrap().entries.len(), 3);

        let mut too_many = fits;
        too_many[4] = 4;
        assert!(matches!(
            Imports::parse(&too_many),
            Err(ImportError::TooManyEntries {
                count: 4,
                max: 3,
                ..
            })
        ));

        let mut huge = fixture(0, &[b"x"]);
        huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Imports::parse(&huge),
            Err(ImportError::TooManyEntries {
                count: u32::MAX,
                ..
            })
        ));
    }

    #[test]
    fn mutated_fixture_never_panics() {
        let base = fixture(1, &[b"unit.mdx", b"sound.wav"]);
        for at in 0..base.len() {
            for value in [0u8, 0x01, 0x7F, 0xFF] {
                let mut data = base.clone();
                data[at] = value;
                let _ = Imports::parse(&data);
            }
        }
    }

    /// Reads the real install named by `PEACECRAFT3_INSTALL`, the Warcraft III
    /// (Legacy) directory. Fails when the variable is unset or the install
    /// lacks a member the measurements name.
    #[test]
    #[ignore = "needs PEACECRAFT3_INSTALL pointing at the Warcraft III (Legacy) directory"]
    fn shipped_import_lists_match_the_measurements() {
        let dir = PathBuf::from(
            std::env::var_os("PEACECRAFT3_INSTALL")
                .expect("PEACECRAFT3_INSTALL must name the Warcraft III (Legacy) directory"),
        );
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();

        // Version 0, from War3Local.mpq.
        let bytes = set.extract(r"Maps\Campaign\BonusCredits.w3m").unwrap();
        let map = Map::parse(&bytes).unwrap();
        let list = map
            .imports()
            .unwrap()
            .expect("BonusCredits carries war3map.imp");
        assert_eq!(list.version, 0);
        assert_eq!(list.entries.len(), 1);
        assert_eq!(list.entries[0].flag, None);
        assert_eq!(list.entries[0].path, b"LargeCityGateOpen1.wav");

        // Version 1, from War3x.mpq.
        let bytes = set
            .extract(r"Maps\FrozenThrone\Campaign\HumanX03.w3x")
            .unwrap();
        let map = Map::parse(&bytes).unwrap();
        let list = map
            .imports()
            .unwrap()
            .expect("HumanX03 carries war3map.imp");
        assert_eq!(list.version, 1);
        assert_eq!(list.entries.len(), 5);
        assert!(list.entries.iter().all(|e| e.flag == Some(0x08)));
        assert_eq!(list.entries[0].path, b"Shpisd1.wav");

        // A loose map without the member.
        let bytes = std::fs::read(dir.join("Maps").join("(2)BootyBay.w3m")).unwrap();
        let map = Map::parse(&bytes).unwrap();
        assert!(map.imports().unwrap().is_none());
    }
}
