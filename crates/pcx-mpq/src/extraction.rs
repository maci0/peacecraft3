//! Explicit per-member admission before packed read and existing payload decode.
use crate::{BlockEntry, Error, HashEntry, MpqArchive, Source, checked, huffman, payload};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractionBudget {
    pub max_raw_packed_bytes: usize,
    pub max_decoded_bytes: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("member {member:?} in {archive:?}: {cause}")]
pub struct ExtractionError {
    pub archive: PathBuf,
    pub member: String,
    #[source]
    pub cause: Box<ExtractionCause>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractionCause {
    #[error("{0}")]
    Mpq(#[source] Error),
    #[error("{0}")]
    Preflight(#[source] checked::ReadError),
    #[error("member has {count} distinct locale/platform/block candidates")]
    Ambiguous { count: usize },
}

pub(crate) enum SelectionError {
    Mpq(Error),
    Ambiguous { count: usize },
}

pub(crate) fn selected_block(
    hashes: &[HashEntry],
    blocks: &[BlockEntry],
    member: &str,
) -> Result<Option<BlockEntry>, SelectionError> {
    if hashes.is_empty() {
        return Ok(None);
    }
    let (name1, name2) = crate::hash::hash_string_both(member);
    let mut slot = crate::hash::start_slot(member, hashes.len());
    let mut selected = None;
    for _ in 0..hashes.len() {
        let entry = &hashes[slot];
        if entry.is_unpopulated() {
            break;
        }
        if entry.name1 == name1 && entry.name2 == name2 {
            let block = *blocks
                .get(entry.block_index as usize)
                .ok_or(SelectionError::Mpq(Error::BadBlockIndex {
                    index: entry.block_index,
                    count: blocks.len(),
                }))?;
            if !block.exists() {
                return Err(SelectionError::Mpq(Error::Unsupported(
                    "matching member block is not a readable existing file",
                )));
            }
            let key = (entry.locale, entry.platform, entry.block_index);
            if selected.as_ref().is_some_and(|(prior, _)| *prior != key) {
                return Err(SelectionError::Ambiguous { count: 2 });
            }
            selected = Some((key, block));
        }
        slot += 1;
        if slot == hashes.len() {
            slot = 0;
        }
    }
    Ok(selected.map(|(_, block)| block))
}

impl MpqArchive {
    /// Strict ambiguity refusal and explicit codec input; no executable discovery.
    /// Tables/backing storage are already owned. Budgets do not bound decoder RSS.
    pub fn extract_bounded(
        &mut self,
        member: &str,
        budget: ExtractionBudget,
        huffman: Option<&huffman::HuffmanTable>,
    ) -> Result<Vec<u8>, ExtractionError> {
        let archive = self.path.clone();
        let context = |cause| ExtractionError {
            archive: archive.clone(),
            member: member.into(),
            cause: Box::new(cause),
        };
        let unsupported = |reason| context(ExtractionCause::Mpq(Error::Unsupported(reason)));
        let h = self.header;
        if h.format_version != 0 {
            return Err(unsupported(
                "bounded extraction supports MPQ version zero only",
            ));
        }
        if h.header_size < 0x20
            || !h.is_self_consistent()
            || h.hash_table_pos < h.header_size
            || h.hash_entries as usize != self.hashes.len()
            || h.block_entries as usize != self.blocks.len()
        {
            return Err(unsupported("header table extents are inconsistent"));
        }
        let sector_size = checked::sector_size(h)
            .ok_or_else(|| unsupported("sector size is not a nonzero u32"))?;
        let block = selected_block(&self.hashes, &self.blocks, member)
            .map_err(|error| match error {
                SelectionError::Mpq(error) => context(ExtractionCause::Mpq(error)),
                SelectionError::Ambiguous { count } => {
                    context(ExtractionCause::Ambiguous { count })
                }
            })?
            .ok_or_else(|| context(ExtractionCause::Mpq(Error::NotFound(member.into()))))?;
        let source = archive.to_string_lossy();
        let packed = checked::check_bytes(
            &source,
            "packed member",
            u64::from(block.compressed_size),
            budget.max_raw_packed_bytes,
        )
        .map_err(|error| context(ExtractionCause::Preflight(error)))?;
        checked::check_bytes(
            &source,
            "decoded member",
            u64::from(block.file_size),
            budget.max_decoded_bytes,
        )
        .map_err(|error| context(ExtractionCause::Preflight(error)))?;
        let offsets = payload::sector_count(block.file_size, sector_size) + 1;
        checked::layout::<u32>(&source, "sector offsets", offsets, offsets as u64 * 4)
            .map_err(|error| context(ExtractionCause::Preflight(error)))?;
        let actual = match &self.file {
            Source::File(file) => file.get_ref().metadata().map(|meta| meta.len()),
            Source::Memory(bytes) => Ok(bytes.len() as u64),
        }
        .map_err(|error| context(ExtractionCause::Mpq(error.into())))?;
        if u64::from(h.archive_size) > actual {
            return Err(context(ExtractionCause::Mpq(
                crate::HeaderParseError::SizeMismatch {
                    declared: h.archive_size,
                    actual,
                }
                .into(),
            )));
        }
        if u64::from(block.file_pos) + u64::from(block.compressed_size) > u64::from(h.archive_size)
        {
            return Err(unsupported(
                "packed member extends past the declared archive",
            ));
        }
        let raw = self
            .read_at(u64::from(block.file_pos), packed)
            .map_err(|error| context(ExtractionCause::Mpq(error)))?;
        let key = payload::file_key(member, block.file_pos, block.file_size, block.flags);
        payload::decode_block_with_huffman(
            &raw,
            key,
            block.file_size,
            block.flags,
            sector_size,
            huffman,
        )
        .map_err(|error| context(ExtractionCause::Mpq(error.into())))
    }
}
