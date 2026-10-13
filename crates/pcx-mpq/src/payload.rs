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
    if mask & COMPRESSION_ZLIB != 0 {
        let mut z = flate2::Decompress::new(true);
        // A sector may stop short of the output buffer; take whatever the
        // decompressor produced rather than treating a partial flush as fatal.
        match z.decompress(body, out, flate2::FlushDecompress::None) {
            Ok(flate2::Status::StreamEnd) => {}
            Ok(_) => {
                // Ran out of output room; the sector decodes fine, it just does
                // not fill the whole remaining buffer.
            }
            // A short output buffer surfaces as a general error; the bytes
            // already produced are still valid, so accept whatever landed.
            Err(_) if z.total_out() > 0 => {}
            Err(_) => return Err(PayloadError::Zlib),
        }
        return Ok(z.total_out() as usize);
    }
    if mask & COMPRESSION_PKWARE != 0 {
        use implode::exploder::Exploder;
        use implode::symbol::DEFAULT_CODE_TABLE;
        let mut ex = Exploder::new(&DEFAULT_CODE_TABLE);
        let mut read = 0usize;
        let mut written = 0usize;
        while !ex.ended && read < body.len() && written < out.len() {
            let (consumed, block) = ex
                .explode_block(&body[read..])
                .map_err(|_| PayloadError::Pkware)?;
            if consumed == 0 && block.is_empty() {
                return Err(PayloadError::Pkware);
            }
            read += consumed;
            let take = block.len().min(out.len() - written);
            out[written..written + take].copy_from_slice(&block[..take]);
            written += take;
        }
        return Ok(written);
    }
    // WC3 writes `0x41` / `0x81` for ADPCM: the low bit belongs to the ADPCM
    // block header, it is not a separate Huffman stream. Mask 0x40/0x80 remains
    // unsupported (RE-Q10) — see docs/re-notes/000-mpq.md.
    if mask
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
            && !is_stored_verbatim(buf.len(), file_size as usize, buf.len())
        {
            let mut plain = vec![0u8; file_size as usize];
            let n = decompress_sector(&buf, &mut plain)?;
            plain.truncate(n);
            return Ok(plain);
        }
        buf.truncate(file_size as usize);
        return Ok(buf);
    }

    let nsec = sector_count(file_size, sector_size);

    // A sector offset table exists only when the block is compressed or
    // encrypted. A plain, stored block is just consecutive fixed-size sectors —
    // reading a table where there is none yields offsets that point past the end
    // of the packed data, which is how most of `war3.mpq` fails.
    let has_offset_table = block_flags & (flags::COMPRESS | flags::ENCRYPTED) != 0;
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
        let start = (offsets[i] as usize).min(raw.len());
        let end = (offsets[i + 1] as usize).min(raw.len());
        if start > end {
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
        let remaining = (file_size as usize).saturating_sub(out.len());
        if (block_flags & (flags::COMPRESS | flags::IMPLODE) != 0)
            && !is_stored_verbatim(sector.len(), remaining, sector_size as usize)
        {
            let mut buf = vec![0u8; remaining];
            let n = decompress_sector(&sector, &mut buf)?;
            buf.truncate(n);
            out.extend_from_slice(&buf);
        } else {
            out.extend_from_slice(&sector);
        }
    }

    if out.len() < file_size as usize {
        return Err(PayloadError::SizeMismatch {
            file_size: file_size as usize,
            produced: out.len(),
        });
    }
    out.truncate(file_size as usize);
    Ok(out)
}
