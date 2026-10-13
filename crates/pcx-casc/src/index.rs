//! V2 contiguous guarded index blocks. Other revisions/layouts are refused.
use crate::{Error, Evidence, Failure, Provenance, bytes, error, integer, jenkins};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexFlags {
    NoFlags,
    Unsupported(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub revision: u16,
    pub bucket: u8,
    pub flags: IndexFlags,
    pub size_bytes: u8,
    pub offset_bytes: u8,
    pub key_bytes: u8,
    pub segment_bits: u8,
    /// Preserved word: its operational limit semantics are unverified.
    pub opaque_max_offset: u64,
    pub header_checksum: u32,
    pub table_checksum: u32,
    pub table_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key_prefix: Vec<u8>,
    pub packed_offset: u64,
    pub segment: u64,
    pub offset: u64,
    /// Includes the local archive envelope, not just the BLTE stream.
    pub encoded_size: u64,
    pub index_offset: usize,
}

#[derive(Debug, Clone)]
pub struct Index {
    pub evidence: Evidence,
    pub header: Header,
    pub entries: Vec<Entry>,
    pub table_offset: usize,
    pub zero_tail_bytes: usize,
}

impl Index {
    pub fn max_offset_semantics(&self) -> Error {
        error(
            &self.evidence.source.member,
            16,
            Failure::Unsupported {
                feature: "index maximum-offset semantics",
                evidence: format!("preserved word {:#x}", self.header.opaque_max_offset),
                next_probe: "trace the maximum-offset word's consumer; physical bounds are verified from data-part file sizes",
            },
        )
    }

    pub fn parse(data: &[u8], source: Provenance) -> Result<Self, Error> {
        let name = &source.member;
        let header_length = integer(bytes(data, 0, 4, name)?, false) as usize;
        let header_checksum = integer(bytes(data, 4, 4, name)?, false) as u32;
        let payload = bytes(data, 8, header_length, name)?;
        if header_length != 16 {
            return Err(error(
                name,
                0,
                Failure::Unsupported {
                    feature: "index header length",
                    evidence: header_length.to_string(),
                    next_probe: "identify the revision-specific header in the supplied index",
                },
            ));
        }
        check_hash(jenkins::hash(payload, 0, 0).0, header_checksum, name, 8)?;
        let revision = integer(&payload[..2], false) as u16;
        if revision != 7 {
            return Err(error(
                name,
                8,
                Failure::Unsupported {
                    feature: "index revision",
                    evidence: revision.to_string(),
                    next_probe: "verify the revision's guarded-block and entry framing",
                },
            ));
        }
        let flags = if payload[3] == 0 {
            IndexFlags::NoFlags
        } else {
            IndexFlags::Unsupported(payload[3])
        };
        if matches!(flags, IndexFlags::Unsupported(_)) {
            return Err(error(
                name,
                11,
                Failure::Unsupported {
                    feature: "index flags",
                    evidence: format!("{flags:?}"),
                    next_probe: "trace the flagged index layout in the local loader",
                },
            ));
        }
        let (size_bytes, offset_bytes, key_bytes, segment_bits) =
            (payload[4], payload[5], payload[6], payload[7]);
        if !(1..=8).contains(&size_bytes)
            || !(1..=8).contains(&offset_bytes)
            || !(1..=16).contains(&key_bytes)
            || segment_bits == 0
            || segment_bits >= 64
            || u16::from(segment_bits) >= u16::from(offset_bytes) * 8
        {
            return Err(error(
                name,
                12,
                Failure::Malformed("invalid entry widths or segment split".into()),
            ));
        }
        let padding_offset = 8 + header_length;
        let padding = bytes(data, padding_offset, 8, name)?;
        if padding.iter().any(|&byte| byte != 0) {
            return Err(error(
                name,
                padding_offset,
                Failure::Unsupported {
                    feature: "index alignment bytes",
                    evidence: crate::hex(padding),
                    next_probe: "establish nonzero alignment-field semantics",
                },
            ));
        }
        let guard_offset = padding_offset + padding.len();
        let table_bytes = integer(bytes(data, guard_offset, 4, name)?, false) as usize;
        let table_checksum = integer(bytes(data, guard_offset + 4, 4, name)?, false) as u32;
        let table_offset = guard_offset + 8;
        let table = bytes(data, table_offset, table_bytes, name)?;
        let width = usize::from(size_bytes) + usize::from(offset_bytes) + usize::from(key_bytes);
        if !table_bytes.is_multiple_of(width) {
            return Err(error(
                name,
                guard_offset,
                Failure::Malformed("table length is not an integral entry count".into()),
            ));
        }
        let mut checksum = (0, 0);
        let mut entries = Vec::with_capacity(table.len() / width);
        for (i, raw) in table.chunks_exact(width).enumerate() {
            checksum = jenkins::hash(raw, checksum.0, checksum.1);
            let split = usize::from(key_bytes);
            let size_offset = split + usize::from(offset_bytes);
            let packed_offset = integer(&raw[split..size_offset], true);
            entries.push(Entry {
                key_prefix: raw[..split].to_vec(),
                packed_offset,
                segment: packed_offset >> segment_bits,
                offset: packed_offset & ((1u64 << segment_bits) - 1),
                encoded_size: integer(&raw[size_offset..], false),
                index_offset: table_offset + i * width,
            });
        }
        check_hash(checksum.0, table_checksum, name, table_offset)?;
        let tail_offset = table_offset + table_bytes;
        let tail = &data[tail_offset..];
        if tail.iter().any(|&byte| byte != 0) {
            return Err(error(
                name,
                tail_offset,
                Failure::Unsupported {
                    feature: "index trailing/paged data",
                    evidence: format!("{} trailing bytes, nonzero", tail.len()),
                    next_probe: "verify guarded page entries instead of assuming contiguous padding",
                },
            ));
        }
        let header = Header {
            revision,
            bucket: payload[2],
            flags,
            size_bytes,
            offset_bytes,
            key_bytes,
            segment_bits,
            opaque_max_offset: integer(&payload[8..16], false),
            header_checksum,
            table_checksum,
            table_bytes,
        };
        Ok(Self {
            evidence: Evidence::of(source, data),
            header,
            entries,
            table_offset,
            zero_tail_bytes: tail.len(),
        })
    }
}

fn check_hash(actual: u32, expected: u32, source: &str, offset: usize) -> Result<(), Error> {
    if actual != expected {
        return Err(error(
            source,
            offset,
            Failure::Checksum {
                algorithm: "Jenkins lookup3",
                expected: format!("{expected:08x}"),
                actual: format!("{actual:08x}"),
            },
        ));
    }
    Ok(())
}
