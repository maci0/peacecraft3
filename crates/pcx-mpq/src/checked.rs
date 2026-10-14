//! Explicit input allocation policy reused by map member consumers.
//! No archive clone, locale priority, default budget or total-memory guarantee.
use crate::{BlockEntry, Error, HashEntry, Header};
use std::{alloc::Layout, io::Cursor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadBudget {
    pub max_archive_bytes: usize,
    pub max_archive_table_bytes: usize,
    pub max_raw_packed_bytes: usize,
    pub max_decoded_bytes: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("archive in {map:?} failed to open: {source}")]
    MpqOpen {
        map: String,
        #[source]
        source: Error,
    },
    #[error("{member:?} in {map:?} has {count} distinct locale/platform/block candidates")]
    AmbiguousMember {
        map: String,
        member: &'static str,
        count: usize,
    },
    #[error("{member:?} in {map:?}: declared {declared_bytes} bytes exceed limit {max_bytes}")]
    BudgetExceeded {
        map: String,
        member: &'static str,
        declared_bytes: usize,
        max_bytes: usize,
    },
    #[error("archive tables in {map:?}: {bytes} bytes exceed limit {max_bytes}")]
    ArchiveTableBudgetExceeded {
        map: String,
        bytes: usize,
        max_bytes: usize,
    },
    #[error("{region} in {map:?} cannot fit the allocation layout: {bytes} bytes")]
    InvalidLayout {
        map: String,
        region: &'static str,
        bytes: u64,
    },
    #[error("failed extracting {member:?} from {map:?}: {source}")]
    ExtractionFailed {
        map: String,
        member: &'static str,
        #[source]
        source: Error,
    },
}

pub(crate) fn layout<T>(
    map: &str,
    region: &'static str,
    count: usize,
    bytes: u64,
) -> Result<(), ReadError> {
    Layout::array::<T>(count)
        .map(|_| ())
        .map_err(|_| ReadError::InvalidLayout {
            map: map.into(),
            region,
            bytes,
        })
}
pub(crate) fn check_bytes(
    map: &str,
    member: &'static str,
    bytes: u64,
    max_bytes: usize,
) -> Result<usize, ReadError> {
    let declared_bytes = usize::try_from(bytes).map_err(|_| ReadError::InvalidLayout {
        map: map.into(),
        region: member,
        bytes,
    })?;
    if declared_bytes > max_bytes {
        return Err(ReadError::BudgetExceeded {
            map: map.into(),
            member,
            declared_bytes,
            max_bytes,
        });
    }
    layout::<u8>(map, member, declared_bytes, bytes)?;
    Ok(declared_bytes)
}
fn archive_error(map: &str, reason: &'static str) -> ReadError {
    ReadError::MpqOpen {
        map: map.into(),
        source: Error::Unsupported(reason),
    }
}
fn member_error(map: &str, member: &'static str, source: Error) -> ReadError {
    ReadError::ExtractionFailed {
        map: map.into(),
        member,
        source,
    }
}

pub(crate) fn sector_size(header: Header) -> Option<u32> {
    0x200u64
        .checked_shl(u32::from(header.sector_size_shift))
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n != 0)
}

/// Table buffers are decoded after their caller budget and span checks.
/// The member payload stays borrowed until its own packed/decoded preflight.
pub struct CheckedArchive<'a> {
    raw: &'a [u8],
    source: &'a str,
    header: Header,
    hashes: Vec<HashEntry>,
    blocks: Vec<BlockEntry>,
    budget: ReadBudget,
}
impl<'a> CheckedArchive<'a> {
    pub fn parse(raw: &'a [u8], source: &'a str, budget: ReadBudget) -> Result<Self, ReadError> {
        check_bytes(
            source,
            "embedded archive",
            raw.len() as u64,
            budget.max_archive_bytes,
        )?;
        let h =
            Header::parse(&mut Cursor::new(raw)).map_err(|source_error| ReadError::MpqOpen {
                map: source.into(),
                source: source_error.into(),
            })?;
        let table_bytes = h
            .hash_table_bytes()
            .checked_add(h.block_table_bytes())
            .ok_or_else(|| archive_error(source, "table byte sum overflows"))?;
        let bytes = usize::try_from(table_bytes).map_err(|_| ReadError::InvalidLayout {
            map: source.into(),
            region: "archive tables",
            bytes: table_bytes,
        })?;
        if bytes > budget.max_archive_table_bytes {
            return Err(ReadError::ArchiveTableBudgetExceeded {
                map: source.into(),
                bytes,
                max_bytes: budget.max_archive_table_bytes,
            });
        }
        layout::<u8>(source, "archive tables", bytes, table_bytes)?;
        layout::<HashEntry>(
            source,
            "hash table",
            h.hash_entries as usize,
            h.hash_table_bytes(),
        )?;
        layout::<BlockEntry>(
            source,
            "block table",
            h.block_entries as usize,
            h.block_table_bytes(),
        )?;
        if h.format_version != 0 {
            return Err(archive_error(
                source,
                "map import supports MPQ version zero only",
            ));
        }
        if !h.is_self_consistent() || h.hash_table_pos < h.header_size {
            return Err(archive_error(
                source,
                "header table extents are inconsistent",
            ));
        }
        if sector_size(h).is_none() {
            return Err(archive_error(source, "sector size is not a nonzero u32"));
        }
        let (hashes, blocks) =
            crate::read_tables(&mut Cursor::new(raw), h).map_err(|error| ReadError::MpqOpen {
                map: source.into(),
                source: error,
            })?;
        Ok(Self {
            raw,
            source,
            header: h,
            hashes,
            blocks,
            budget,
        })
    }
    /// Bounded Storm probe; identical keys are allowed, distinct candidates refused.
    pub fn selected_block(&self, member: &'static str) -> Result<Option<BlockEntry>, ReadError> {
        crate::extraction::selected_block(&self.hashes, &self.blocks, member).map_err(|error| {
            match error {
                crate::extraction::SelectionError::Mpq(error) => {
                    member_error(self.source, member, error)
                }
                crate::extraction::SelectionError::Ambiguous { count } => {
                    ReadError::AmbiguousMember {
                        map: self.source.into(),
                        member,
                        count,
                    }
                }
            }
        })
    }

    pub fn preflight_member(
        &self,
        member: &'static str,
        limit: usize,
    ) -> Result<Option<usize>, ReadError> {
        let Some(block) = self.selected_block(member)? else {
            return Ok(None);
        };
        let size = check_bytes(self.source, member, u64::from(block.file_size), limit)?;
        check_bytes(
            self.source,
            member,
            u64::from(block.file_size),
            self.budget.max_decoded_bytes,
        )?;
        check_bytes(
            self.source,
            member,
            u64::from(block.compressed_size),
            self.budget.max_raw_packed_bytes,
        )?;
        if u64::from(block.file_pos) + u64::from(block.compressed_size)
            > u64::from(self.header.archive_size)
        {
            return Err(member_error(
                self.source,
                member,
                Error::Unsupported("packed member extends past the declared archive"),
            ));
        }
        Ok(Some(size))
    }
    /// Rechecks the immutable selection and budgets; no caller can forge a block.
    /// Uses existing memory-archive decoding with no inferred Huffman table.
    pub fn extract(
        &self,
        member: &'static str,
        limit: usize,
    ) -> Result<Option<Vec<u8>>, ReadError> {
        if self.preflight_member(member, limit)?.is_none() {
            return Ok(None);
        }
        let block = self
            .selected_block(member)?
            .expect("immutable selected member");
        let start = block.file_pos as usize;
        let packed = &self.raw[start..start + block.compressed_size as usize];
        let key = crate::payload::file_key(member, block.file_pos, block.file_size, block.flags);
        crate::payload::decode_block_with_huffman(
            packed,
            key,
            block.file_size,
            block.flags,
            self.header.sector_size(),
            None,
        )
        .map(Some)
        .map_err(|error| member_error(self.source, member, error.into()))
    }
}
