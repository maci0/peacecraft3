//! MPQ file payload: sector splitting, decryption and decompression.
//!
//! Three facts drive this module, each established by decoding the shipped
//! archives rather than assumed:
//!
//! 1. The file key comes from the hash of the **basename only** (the part after
//!    the last `/` or `\`), using table slice `0x300` — not the `name1`/`name2`
//!    pair used for hash-table lookups. With `FIX_KEY` set it becomes
//!    `(key + block.file_pos) ^ block.file_size`.
//! 2. The sector offset table is decrypted with `file_key - 1`, sector `i` with
//!    `file_key + i`, and a single-unit file with `file_key`.
//! 3. War3 archives are dominated by PKWare implode (`0x08`); zlib (`0x02`) and
//!    bzip2 (`0x10`) also occur.
//!
//! Solved against `war3.mpq`'s `(listfile)`: `hash("(listfile)", 0x300)` with the
//! fixed table is `0x2D2F0A94`, `FIX_KEY` yields `0x47619FC7`, the offset table
//! decrypts with `0x47619FC6`, and all 130 sectors come out type `0x08`.

use crate::crypt::decrypt;
use crate::flags;
use crate::hash;

/// Per-sector compression mask bits.
const COMPRESSION_HUFFMAN: u8 = 0x01;
const COMPRESSION_ZLIB: u8 = 0x02;
const COMPRESSION_PKWARE: u8 = 0x08;
const COMPRESSION_BZIP2: u8 = 0x10;
const COMPRESSION_SPARSE: u8 = 0x20;
const COMPRESSION_ADPCM_MONO: u8 = 0x40;
const COMPRESSION_ADPCM_STEREO: u8 = 0x80;
const COMPRESSION_LZMA: u8 = 0x12;

#[derive(Debug, thiserror::Error)]
pub enum PayloadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported compression mask 0x{mask:02x} in sector")]
    UnsupportedCompression { mask: u8 },
    #[error("zlib sector is corrupt")]
    Zlib,
    #[error("bzip2 sector is corrupt")]
    Bzip2,
    #[error("PKWARE sector is corrupt")]
    Pkware,
    #[error("sector {index} of {count} lies outside the packed data")]
    SectorOutOfRange { index: usize, count: usize },
    #[error("block claims {file_size} bytes but sectors produced {produced}")]
    SizeMismatch { file_size: usize, produced: usize },
}

/// The key Storm uses for a file's payload.
pub fn file_key(path: &str, file_pos: u32, file_size: u32, block_flags: u32) -> u32 {
    let basename = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let mut key = hash::hash_bytes(basename.as_bytes(), 0x300);
    if block_flags & flags::FIX_KEY != 0 {
        key = key.wrapping_add(file_pos) ^ file_size;
    }
    key
}

/// Number of sectors a multi-sector block has.
pub fn sector_count(file_size: u32, sector_size: u32) -> usize {
    if file_size == 0 {
        return 0;
    }
    (file_size - 1) as usize / sector_size as usize + 1
}

/// A sector is stored verbatim when it occupies the whole sector size, or when
/// it exactly fills the output space still owed. Such a sector carries **no**
/// compression-mask byte; treating its first byte as a mask feeds raw map data
/// to the imploder, which is exactly how `war3.mpq`'s `.w3m` entries break.
///
/// Nearly-incompressible files (`cs ~= fs`) hit this on nearly every sector,
/// while highly compressible ones like `(listfile)` never do.
fn is_stored_verbatim(sector_len: usize, remaining_out: usize, sector_size: usize) -> bool {
    sector_len == sector_size || sector_len == remaining_out
}

/// Decompress one sector, which begins with its compression mask byte.
pub fn decompress_sector(sector: &[u8], out: &mut [u8]) -> Result<usize, PayloadError> {
    let Some(&mask) = sector.first() else {
        return Ok(0);
    };
    let body = &sector[1..];

    // 0x12 (LZMA) is an exact mask, not a bit set: testing it bitwise also
    // matches plain zlib (0x12 & 0x02), which is very common in these archives.
    if mask == COMPRESSION_LZMA || mask & COMPRESSION_BZIP2 != 0 {
        return Err(PayloadError::UnsupportedCompression { mask });
    }
    if mask == COMPRESSION_ZLIB {
        let mut z = flate2::Decompress::new(true);
        match z.decompress(body, out, flate2::FlushDecompress::Finish) {
            Ok(flate2::Status::StreamEnd) if z.total_in() == body.len() as u64 => {}
            _ => return Err(PayloadError::Zlib),
        }
        return Ok(z.total_out() as usize);
    }
    if mask == COMPRESSION_PKWARE {
        return decompress_implode(body, out);
    }
    // Masks compose: 0x41/0x81 need Huffman followed by ADPCM.
    // Refuse unsupported chains rather than silently decode only one stage.
    if mask != 0
        || mask
            & (COMPRESSION_ADPCM_STEREO
                | COMPRESSION_ADPCM_MONO
                | COMPRESSION_SPARSE
                | COMPRESSION_HUFFMAN)
            != 0
    {
        return Err(PayloadError::UnsupportedCompression { mask });
    }

    let take = body.len().min(out.len());
    out[..take].copy_from_slice(&body[..take]);
    Ok(take)
}

/// Legacy IMPLODE blocks have no compression-mask prefix.
fn decompress_implode(body: &[u8], out: &mut [u8]) -> Result<usize, PayloadError> {
    use implode::exploder::Exploder;
    use implode::symbol::DEFAULT_CODE_TABLE;
    // The dependency supports binary literals and dictionary widths 4..=6.
    if body.len() < 2 || body[0] != 0 || !(4..=6).contains(&body[1]) {
        return Err(PayloadError::Pkware);
    }
    let mut ex = Exploder::new(&DEFAULT_CODE_TABLE);
    let mut read = 0;
    let mut written = 0;
    while !ex.ended {
        let (consumed, block) = ex
            .explode_block(&body[read..])
            .map_err(|_| PayloadError::Pkware)?;
        let stalled = consumed == 0 && block.is_empty();
        read += consumed;
        if block.len() > out.len() - written {
            return Err(PayloadError::Pkware);
        }
        out[written..written + block.len()].copy_from_slice(block);
        written += block.len();
        if stalled && !ex.ended {
            return Err(PayloadError::Pkware);
        }
    }
    Ok(written)
}

fn decompress_payload(
    data: &[u8],
    out: &mut [u8],
    block_flags: u32,
) -> Result<usize, PayloadError> {
    if block_flags & flags::COMPRESS != 0 {
        decompress_sector(data, out)
    } else {
        decompress_implode(data, out)
    }
}

/// Reassemble a block from its raw packed bytes.
///
/// `raw` must be exactly `compressed_size` bytes starting at `file_pos`.
pub fn decode_block(
    raw: &[u8],
    key: u32,
    file_size: u32,
    block_flags: u32,
    sector_size: u32,
) -> Result<Vec<u8>, PayloadError> {
    let mut out = Vec::with_capacity(file_size as usize);
    let encrypted = block_flags & flags::ENCRYPTED != 0;

    if block_flags & flags::SINGLE_UNIT != 0 {
        let mut buf = raw.to_vec();
        if encrypted {
            decrypt(&mut buf, key, crate::crypt::table());
        }
        if (block_flags & flags::COMPRESS != 0 || block_flags & flags::IMPLODE != 0)
            && buf.len() != file_size as usize
        {
            let mut plain = vec![0u8; file_size as usize];
            let n = decompress_payload(&buf, &mut plain, block_flags)?;
            if n != file_size as usize {
                return Err(PayloadError::SizeMismatch {
                    file_size: file_size as usize,
                    produced: n,
                });
            }
            plain.truncate(n);
            return Ok(plain);
        }
        if buf.len() != file_size as usize {
            return Err(PayloadError::SizeMismatch {
                file_size: file_size as usize,
                produced: buf.len(),
            });
        }
        return Ok(buf);
    }

    let nsec = sector_count(file_size, sector_size);

    // Compression (including legacy IMPLODE) needs an offset table.
    // Stored blocks, even encrypted ones, use fixed-size sectors —
    // reading a table where there is none yields offsets that point past the end
    // of the packed data, which is how most of `war3.mpq` fails.
    let has_offset_table = block_flags & (flags::COMPRESS | flags::IMPLODE) != 0;
    let offsets: Vec<u32> = if has_offset_table {
        let table_bytes = (nsec + 1) * 4;
        if raw.len() < table_bytes {
            return Err(PayloadError::SizeMismatch {
                file_size: file_size as usize,
                produced: 0,
            });
        }
        let mut table = raw[..table_bytes].to_vec();
        if encrypted {
            decrypt(&mut table, key.wrapping_sub(1), crate::crypt::table());
        }
        table
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    } else {
        // The final synthetic offset must not run past the packed data: a
        // stored block's last sector is short, and `cs` is usually not a
        // multiple of the sector size.
        (0..=nsec)
            .map(|i| (i as u32).saturating_mul(sector_size).min(raw.len() as u32))
            .collect()
    };

    out.reserve(file_size as usize);
    for i in 0..nsec {
        let start = offsets[i] as usize;
        let end = offsets[i + 1] as usize;
        if start > end || end > raw.len() || (has_offset_table && start < (nsec + 1) * 4) {
            return Err(PayloadError::SectorOutOfRange {
                index: i,
                count: nsec,
            });
        }
        let mut sector = raw[start..end].to_vec();
        if encrypted {
            decrypt(
                &mut sector,
                key.wrapping_add(i as u32),
                crate::crypt::table(),
            );
        }
        let remaining = ((file_size as usize).saturating_sub(out.len())).min(sector_size as usize);
        if (block_flags & (flags::COMPRESS | flags::IMPLODE) != 0)
            && !is_stored_verbatim(sector.len(), remaining, sector_size as usize)
        {
            let mut buf = vec![0u8; remaining];
            let n = decompress_payload(&sector, &mut buf, block_flags)?;
            if n != remaining {
                return Err(PayloadError::SizeMismatch {
                    file_size: remaining,
                    produced: n,
                });
            }
            out.extend_from_slice(&buf);
        } else {
            if sector.len() != remaining {
                return Err(PayloadError::SizeMismatch {
                    file_size: remaining,
                    produced: sector.len(),
                });
            }
            out.extend_from_slice(&sector);
        }
    }

    if out.len() != file_size as usize {
        return Err(PayloadError::SizeMismatch {
            file_size: file_size as usize,
            produced: out.len(),
        });
    }
    out.truncate(file_size as usize);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        let mut packed = vec![COMPRESSION_ZLIB];
        packed.extend(encoder.finish().unwrap());
        packed
    }

    #[test]
    fn compressed_single_unit_is_decoded() {
        let plain = vec![b'Q'; 256];
        let packed = zlib(&plain);
        assert!(packed.len() < plain.len());
        assert_eq!(
            decode_block(
                &packed,
                0,
                plain.len() as u32,
                flags::SINGLE_UNIT | flags::COMPRESS,
                4096
            )
            .unwrap(),
            plain
        );
    }

    #[test]
    fn legacy_implode_has_no_mask_byte() {
        // Synthetic PKWARE stream: binary literals, 1 KiB dictionary,
        // two literals, a back-reference and the end marker.
        let packed = [0, 4, 0x82, 0x24, 0x25, 0x8f, 0x80, 0x7f];
        let plain = b"AIAIAIAIAIAIA";
        assert_eq!(
            decode_block(
                &packed,
                0,
                plain.len() as u32,
                flags::SINGLE_UNIT | flags::IMPLODE,
                4096
            )
            .unwrap(),
            plain
        );
        let mut sectors = Vec::new();
        sectors.extend(8u32.to_le_bytes());
        sectors.extend((8u32 + packed.len() as u32).to_le_bytes());
        sectors.extend(packed);
        assert_eq!(
            decode_block(&sectors, 0, plain.len() as u32, flags::IMPLODE, 4096).unwrap(),
            plain
        );
    }

    #[test]
    fn sector_offsets_are_rejected_before_slicing() {
        for offsets in [[8u32, 99], [4, 9], [9, 8]] {
            let mut packed = Vec::new();
            for offset in offsets {
                packed.extend(offset.to_le_bytes());
            }
            packed.push(b'x');
            assert!(matches!(
                decode_block(&packed, 0, 1, flags::COMPRESS, 4096),
                Err(PayloadError::SectorOutOfRange { .. })
            ));
        }
    }

    #[test]
    fn single_unit_short_output_is_an_error() {
        assert!(matches!(
            decode_block(b"short", 0, 7, flags::SINGLE_UNIT, 4096),
            Err(PayloadError::SizeMismatch { .. })
        ));
        assert!(matches!(
            decode_block(
                &zlib(b"short"),
                0,
                30,
                flags::SINGLE_UNIT | flags::COMPRESS,
                4096
            ),
            Err(PayloadError::SizeMismatch { .. })
        ));
    }

    #[test]
    fn unsupported_compression_chain_is_not_partially_decoded() {
        let mut packed = zlib(&[1; 128]);
        packed[0] |= COMPRESSION_HUFFMAN;
        assert!(matches!(
            decompress_sector(&packed, &mut [0; 128]),
            Err(PayloadError::UnsupportedCompression { mask: 3 })
        ));
    }

    #[test]
    fn compressed_sector_cannot_borrow_next_sectors_output_space() {
        let packed = zlib(&[1; 80]);
        let mut raw = Vec::new();
        raw.extend(12u32.to_le_bytes());
        raw.extend((12u32 + packed.len() as u32).to_le_bytes());
        raw.extend((13u32 + packed.len() as u32).to_le_bytes());
        raw.extend(packed);
        raw.push(2);
        assert!(decode_block(&raw, 0, 65, flags::COMPRESS, 64).is_err());
    }
}
