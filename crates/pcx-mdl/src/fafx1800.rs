//! Raw v1800 FAFX records; no text or FaceFX behavior is interpreted.
use super::{Error, Mdlx};

// Pinned WhiteoutLib parseFAFX/writeFFX and the checked HD fixture agree:
// FAFX has no count prefix. See REFORGED_FAFX.md for provenance.
const RECORD_SIZE: usize = 340;
const NAME_SIZE: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FafxRecord1800<'a> {
    /// Absolute source offset of this record and its name field.
    pub offset: usize,
    pub name: &'a [u8; 80],
    /// Raw field at offset + 80, including any NULs and trailing bytes.
    pub path: &'a [u8; 260],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FafxStream1800<'a> {
    pub version: u32,
    /// Absolute source offset of the FAFX tag.
    pub offset: usize,
    /// Absolute source offset of the first record.
    pub payload_offset: usize,
    data: &'a [u8],
}

impl<'a> FafxStream1800<'a> {
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn record_count(&self) -> usize {
        self.data.len() / RECORD_SIZE
    }

    pub fn get(&self, index: usize) -> Option<FafxRecord1800<'a>> {
        let start = index.checked_mul(RECORD_SIZE)?;
        let end = start.checked_add(RECORD_SIZE)?;
        let record = self.data.get(start..end)?;
        let offset = self.payload_offset.checked_add(start)?;
        let (name, path) = record.split_at(NAME_SIZE);
        Some(FafxRecord1800 {
            offset,
            name: name.try_into().ok()?,
            path: path.try_into().ok()?,
        })
    }
}

impl Mdlx {
    /// Borrow raw FAFX records from the same input used to parse this container.
    /// Checked v1800 import policy permits at most one FAFX chunk and requires
    /// exact 340-byte tiling. Absence is distinct from a present empty chunk.
    /// This neither changes strict parsing nor claims native rejection parity.
    pub fn fafx_stream_v1800<'a>(
        &self,
        data: &'a [u8],
    ) -> Result<Option<FafxStream1800<'a>>, Error> {
        if self.version != 1800 {
            return Err(Error::UnsupportedVersion(self.version));
        }
        let mut chunks = self.chunks.iter().filter(|chunk| &chunk.tag == b"FAFX");
        let Some(chunk) = chunks.next() else {
            return Ok(None);
        };
        if chunks.next().is_some() {
            return Err(Error::UnsupportedVersionedChunk {
                version: self.version,
                chunk: "duplicate FAFX",
            });
        }
        let count = chunk.size / RECORD_SIZE;
        let measured = count.checked_mul(RECORD_SIZE).ok_or(Error::RecordShape {
            chunk: "FAFX",
            at: chunk.payload_offset,
            size: chunk.size,
            measured: usize::MAX,
        })?;
        if measured != chunk.size {
            return Err(Error::RaggedRecords {
                chunk: "FAFX",
                size: chunk.size,
                stride: RECORD_SIZE,
            });
        }
        let payload = chunk
            .payload_offset
            .checked_add(measured)
            .and_then(|end| data.get(chunk.payload_offset..end))
            .ok_or(Error::Truncated {
                what: "FAFX payload",
                at: chunk.payload_offset,
                need: measured,
                have: data.len().saturating_sub(chunk.payload_offset),
            })?;
        Ok(Some(FafxStream1800 {
            version: self.version,
            offset: chunk.offset,
            payload_offset: chunk.payload_offset,
            data: payload,
        }))
    }
}
