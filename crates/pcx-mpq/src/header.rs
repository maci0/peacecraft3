//! MPQ header.
//!
//! Layout confirmed against all four shipped archives and against Storm.dll
//! `FUN_1501CFC0` (evidence `ev_faddef74215f57387b18fbeb347d85a10681eddb1c5e7565647acd1dc08b0048`):
//!
//! ```text
//! offset  size  field
//! 0x00    4     magic "MPQ\x1a"
//! 0x04    4     header size (0x20)
//! 0x08    4     archive size
//! 0x0C    2     format version (0)
//! 0x0E    1     sector size shift; sector size = 0x200 << shift
//! 0x10    4     hash table position
//! 0x14    4     block table position
//! 0x18    4     hash table entry count
//! 0x1C    4     block table entry count
//! 0x20    4     word 0x20 (not yet attributed)
//! 0x24    4     word 0x24 (not yet attributed)
//! ```
//!
//! Two invariants hold in all four archives and are asserted in the tests:
//!
//! * `block_table_pos == hash_table_pos + hash_entries * 16`
//! * `block_table_pos + block_entries * 16 == archive_size`
//!
//! The `0x0C` word reads `0x00030000` everywhere, which the split above renders
//! as format version `0` and sector shift `3`, i.e. 4096-byte sectors.

use std::io::{Read, Seek, SeekFrom};

pub const MAGIC: [u8; 4] = [b'M', b'P', b'Q', 0x1A];

/// Bytes per hash- and block-table entry.
pub const TABLE_ENTRY_SIZE: u32 = 16;

#[derive(Debug, thiserror::Error)]
pub enum HeaderParseError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad magic: expected 4D 50 51 1A, got {0:02X?}")]
    BadMagic([u8; 4]),
    #[error("unsupported header size {0}")]
    UnsupportedHeaderSize(u32),
    #[error("archive size {declared} exceeds file length {actual}")]
    SizeMismatch { declared: u32, actual: u64 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Header {
    pub header_size: u32,
    pub archive_size: u32,
    pub format_version: u16,
    pub sector_size_shift: u8,
    pub hash_table_pos: u32,
    pub block_table_pos: u32,
    pub hash_entries: u32,
    pub block_entries: u32,
    /// Bytes `0x20..=0x23`; present in all four archives, semantics unconfirmed.
    pub w20: u32,
    /// Bytes `0x24..=0x27`; present in all four archives, semantics unconfirmed.
    pub w24: u32,
}

impl Header {
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self, HeaderParseError> {
        let declared_len = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(0))?;
        let mut buf = [0u8; 0x28];
        reader.read_exact(&mut buf)?;

        let magic = [buf[0], buf[1], buf[2], buf[3]];
        if magic != MAGIC {
            return Err(HeaderParseError::BadMagic(magic));
        }

        let header_size = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        if header_size < 0x20 {
            return Err(HeaderParseError::UnsupportedHeaderSize(header_size));
        }
        let archive_size = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        if u64::from(archive_size) > declared_len {
            return Err(HeaderParseError::SizeMismatch {
                declared: archive_size,
                actual: declared_len,
            });
        }

        Ok(Self {
            header_size,
            archive_size,
            format_version: u16::from_le_bytes(buf[0x0C..0x0E].try_into().unwrap()),
            sector_size_shift: buf[0x0E],
            hash_table_pos: u32::from_le_bytes(buf[0x10..0x14].try_into().unwrap()),
            block_table_pos: u32::from_le_bytes(buf[0x14..0x18].try_into().unwrap()),
            hash_entries: u32::from_le_bytes(buf[0x18..0x1C].try_into().unwrap()),
            block_entries: u32::from_le_bytes(buf[0x1C..0x20].try_into().unwrap()),
            w20: u32::from_le_bytes(buf[0x20..0x24].try_into().unwrap()),
            w24: u32::from_le_bytes(buf[0x24..0x28].try_into().unwrap()),
        })
    }

    /// `0x200 << sector_size_shift`. All four shipped archives use `3`.
    pub fn sector_size(&self) -> u32 {
        0x200u32 << self.sector_size_shift
    }

    /// Byte length of the hash table.
    pub fn hash_table_bytes(&self) -> u64 {
        u64::from(self.hash_entries) * u64::from(TABLE_ENTRY_SIZE)
    }

    /// Byte length of the block table.
    pub fn block_table_bytes(&self) -> u64 {
        u64::from(self.block_entries) * u64::from(TABLE_ENTRY_SIZE)
    }

    /// `block_table_pos == hash_table_pos + hash_entries * 16`.
    pub fn tables_are_adjacent(&self) -> bool {
        u64::from(self.block_table_pos) == u64::from(self.hash_table_pos) + self.hash_table_bytes()
    }

    /// `block_table_pos + block_entries * 16 == archive_size`, i.e. the tables
    /// are the last thing in the file.
    pub fn tables_end_at_archive_end(&self) -> bool {
        u64::from(self.block_table_pos) + self.block_table_bytes() == u64::from(self.archive_size)
    }

    /// Both structural invariants.
    pub fn is_self_consistent(&self) -> bool {
        self.tables_are_adjacent() && self.tables_end_at_archive_end()
    }

    /// Round-trip helper for RE diffing against the shipped files.
    pub fn as_words(&self) -> [u32; 8] {
        [
            self.header_size,
            self.archive_size,
            (self.format_version as u32) | ((self.sector_size_shift as u32) << 16),
            self.hash_table_pos,
            self.block_table_pos,
            self.hash_entries,
            self.block_entries,
            self.w20 ^ self.w24,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install_dir() -> Option<std::path::PathBuf> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    #[test]
    fn parses_every_shipped_archive() {
        let Some(dir) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        for name in ["war3.mpq", "War3x.mpq", "War3xlocal.mpq", "War3Patch.mpq"] {
            let mut f = std::fs::File::open(dir.join(name)).unwrap();
            let h = Header::parse(&mut f).unwrap();
            assert_eq!(h.header_size, 0x20, "{name}");
            assert_eq!(h.archive_size as u64, f.metadata().unwrap().len(), "{name}");
            assert_eq!(h.format_version, 0, "{name}");
            assert_eq!(h.sector_size_shift, 3, "{name}");
            assert_eq!(h.sector_size(), 4096, "{name}");
            assert!(
                h.tables_are_adjacent(),
                "{name}: block_pos != hash_pos + hash_count*16"
            );
            assert!(
                h.tables_end_at_archive_end(),
                "{name}: tables do not end at archive end"
            );
        }
    }

    #[test]
    fn rejects_garbage() {
        let junk = [0u8; 64];
        let mut cur = std::io::Cursor::new(&junk);
        assert!(matches!(
            Header::parse(&mut cur),
            Err(HeaderParseError::BadMagic(_))
        ));
    }
}
