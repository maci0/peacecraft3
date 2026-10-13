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

// Storm.dll eecc42b0…1bf97d0a, VA 0x15042ad8 (89 LE dwords),
// ev_44af6eaa20915be4d5dad55689e927b483599dab5bbe95f28653b900f5fa3b3e.
const ADPCM_STEPS: [u32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
// VA 0x15042a58, ev_105707040ec97e135125729304f99e50526576997387258cf586e90e020f5891.
// Preserve the shipped schedule literally, including every even entry being -1.
const ADPCM_INDEX_ADJUST: [i32; 32] = [
    -1, 0, -1, 4, -1, 2, -1, 6, -1, 1, -1, 5, -1, 3, -1, 7, -1, 1, -1, 5, -1, 3, -1, 7, -1, 2, -1,
    4, -1, 6, -1, 8,
];

#[derive(Debug, thiserror::Error)]
pub enum PayloadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported compression mask 0x{mask:02x} in sector")]
    UnsupportedCompression { mask: u8 },
    #[error(transparent)]
    Huffman(#[from] crate::huffman::HuffmanError),
    #[error("zlib sector is corrupt")]
    Zlib,
    #[error("bzip2 sector is corrupt")]
    Bzip2,
    #[error("ADPCM sector is truncated or exceeds its output buffer")]
    Adpcm,
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
    decompress_sector_with_huffman(sector, out, None)
}

/// Decode with Huffman weights imported from the caller's original Storm.dll.
pub fn decompress_sector_with_huffman(
    sector: &[u8],
    out: &mut [u8],
    table: Option<&crate::huffman::HuffmanTable>,
) -> Result<usize, PayloadError> {
    let Some(&mask) = sector.first() else {
        return Ok(0);
    };
    let allowed = COMPRESSION_PKWARE
        | COMPRESSION_ZLIB
        | COMPRESSION_HUFFMAN
        | COMPRESSION_ADPCM_MONO
        | COMPRESSION_ADPCM_STEREO;
    if mask == COMPRESSION_LZMA
        || mask & (COMPRESSION_BZIP2 | COMPRESSION_SPARSE) != 0
        || mask & !allowed != 0
        || mask & COMPRESSION_HUFFMAN != 0 && table.is_none()
    {
        return Err(PayloadError::UnsupportedCompression { mask });
    }
    let body = &sector[1..];
    if mask == 0 {
        if body.len() > out.len() {
            return Err(PayloadError::SizeMismatch {
                file_size: out.len(),
                produced: body.len(),
            });
        }
        out[..body.len()].copy_from_slice(body);
        return Ok(body.len());
    }
    // FUN_1500dd30 visits the runtime dispatcher table backwards:
    // ev_7be596f3cf153d1fe4da9a3529555c088e88377b6c47cf10ab20d521226f82e0.
    let stages: Vec<_> = [
        COMPRESSION_PKWARE,
        COMPRESSION_ZLIB,
        COMPRESSION_HUFFMAN,
        COMPRESSION_ADPCM_STEREO,
        COMPRESSION_ADPCM_MONO,
    ]
    .into_iter()
    .filter(|stage| mask & stage != 0)
    .collect();
    let mut input = body.to_vec();
    for (index, stage) in stages.iter().enumerate() {
        if index + 1 == stages.len() {
            return decompress_stage(*stage, &input, out, table);
        }
        let mut next = vec![0; out.len()];
        let produced = decompress_stage(*stage, &input, &mut next, table)?;
        next.truncate(produced);
        input = next;
    }
    unreachable!("nonzero supported mask has a decoding stage")
}

fn decompress_stage(
    mask: u8,
    body: &[u8],
    out: &mut [u8],
    table: Option<&crate::huffman::HuffmanTable>,
) -> Result<usize, PayloadError> {
    match mask {
        COMPRESSION_ZLIB => {
            let mut z = flate2::Decompress::new(true);
            match z.decompress(body, out, flate2::FlushDecompress::Finish) {
                Ok(flate2::Status::StreamEnd) if z.total_in() == body.len() as u64 => {
                    Ok(z.total_out() as usize)
                }
                _ => Err(PayloadError::Zlib),
            }
        }
        COMPRESSION_PKWARE => decompress_implode(body, out),
        COMPRESSION_HUFFMAN => Ok(crate::huffman::decompress(
            body,
            out,
            table.ok_or(PayloadError::UnsupportedCompression { mask })?,
        )?),
        COMPRESSION_ADPCM_MONO => decompress_adpcm(body, out, 1),
        COMPRESSION_ADPCM_STEREO => decompress_adpcm(body, out, 2),
        _ => Err(PayloadError::UnsupportedCompression { mask }),
    }
}

/// Storm FUN_1500d240, ev_5abf352abd1291d6df0c852c62634902616f2b08066b7ae35baf05e0173c3c6a
/// (coordinator ev_5884eec95cd29ec921db15f15b27b0cab0bf6e7ccd8256db30061e5852272a21).
/// Mono/stereo wrappers at 0x1500d960/0x1500d9f0 supply channel counts 1/2.
fn decompress_adpcm(body: &[u8], out: &mut [u8], channels: usize) -> Result<usize, PayloadError> {
    let header_size = 2 + channels * 2;
    if body.len() < header_size {
        return Err(PayloadError::Adpcm);
    }
    let shift = body[1] & 31; // x86 SHR masks the authored shift count.
    let mut indices = [44i32; 2];
    let mut samples = [0i32; 2];
    let mut written = 0;
    let emit = |value: i32, written: &mut usize, out: &mut [u8]| {
        let target = out
            .get_mut(*written..*written + 2)
            .ok_or(PayloadError::Adpcm)?;
        target.copy_from_slice(&(value as i16).to_le_bytes());
        *written += 2;
        Ok::<_, PayloadError>(())
    };
    for (channel, sample) in body[2..header_size].as_chunks::<2>().0.iter().enumerate() {
        samples[channel] = i16::from_le_bytes(*sample) as i32;
        emit(samples[channel], &mut written, out)?;
    }
    let mut channel = channels - 1;
    for &byte in &body[header_size..] {
        if channels == 2 {
            channel ^= 1;
        }
        if byte & 0x80 != 0 {
            match byte & 0x7f {
                0 => {
                    indices[channel] = (indices[channel] - 1).max(0);
                    emit(samples[channel], &mut written, out)?;
                }
                1 | 2 => {
                    let adjustment = if byte & 0x7f == 1 { 8 } else { -8 };
                    indices[channel] =
                        (indices[channel] + adjustment).clamp(0, ADPCM_STEPS.len() as i32 - 1);
                    if channels == 2 {
                        channel ^= 1;
                    }
                }
                _ => {}
            }
            continue;
        }
        let step = ADPCM_STEPS[indices[channel] as usize];
        let mut delta = step >> shift;
        for bit in 0..6 {
            if byte & (1 << bit) != 0 {
                delta += step >> bit;
            }
        }
        let delta = delta as i32;
        samples[channel] = if byte & 0x40 == 0 {
            (samples[channel] + delta).min(i16::MAX as i32)
        } else {
            (samples[channel] - delta).max(i16::MIN as i32)
        };
        emit(samples[channel], &mut written, out)?;
        indices[channel] = (indices[channel] + ADPCM_INDEX_ADJUST[(byte & 0x1f) as usize])
            .clamp(0, ADPCM_STEPS.len() as i32 - 1);
    }
    Ok(written)
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
    table: Option<&crate::huffman::HuffmanTable>,
) -> Result<usize, PayloadError> {
    if block_flags & flags::COMPRESS != 0 {
        decompress_sector_with_huffman(data, out, table)
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
    decode_block_with_huffman(raw, key, file_size, block_flags, sector_size, None)
}

/// Reassemble with locally imported Huffman weights; the ordinary entry point refuses Huffman.
pub fn decode_block_with_huffman(
    raw: &[u8],
    key: u32,
    file_size: u32,
    block_flags: u32,
    sector_size: u32,
    table: Option<&crate::huffman::HuffmanTable>,
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
            let n = decompress_payload(&buf, &mut plain, block_flags, table)?;
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
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
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
            let n = decompress_payload(&sector, &mut buf, block_flags, table)?;
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
    fn adpcm_mono_controls_and_literal_schedule() {
        let packed = [
            COMPRESSION_ADPCM_MONO,
            0,
            1,
            0xe8,
            3,
            0,
            0x40,
            0x80,
            1,
            0x81,
            0,
            0x82,
            0,
            0x83,
        ];
        let mut out = [0; 16];
        let n = decompress_sector(&packed, &mut out).unwrap();
        let samples: Vec<_> = out[..n]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|s| i16::from_le_bytes(*s))
            .collect();
        assert_eq!(samples, [1000, 1247, 1023, 1023, 1579, 1977, 2145]);
    }

    #[test]
    fn adpcm_stereo_controls_preserve_channel_schedule() {
        let packed = [
            COMPRESSION_ADPCM_STEREO,
            0,
            1,
            100,
            0,
            0x18,
            0xfc,
            0x80,
            0x81,
            0,
            0x83,
            0,
            0x82,
            0,
        ];
        let mut out = [0; 16];
        let n = decompress_sector(&packed, &mut out).unwrap();
        let samples: Vec<_> = out[..n]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|s| i16::from_le_bytes(*s))
            .collect();
        assert_eq!(samples, [100, -1000, 100, -470, 11, 204]);
    }

    #[test]
    fn adpcm_saturates_samples_and_masks_shift_count() {
        for (initial, byte, expected) in [(i16::MAX, 0x3f, i16::MAX), (i16::MIN, 0x7f, i16::MIN)] {
            let mut body = vec![0, 33];
            body.extend(initial.to_le_bytes());
            body.push(byte);
            let mut out = [0; 4];
            assert_eq!(decompress_adpcm(&body, &mut out, 1).unwrap(), 4);
            assert_eq!(i16::from_le_bytes(out[2..].try_into().unwrap()), expected);
        }
    }

    #[test]
    fn adpcm_truncation_overflow_and_huffman_chains_are_explicit() {
        for (mask, header_size) in [(COMPRESSION_ADPCM_MONO, 4), (COMPRESSION_ADPCM_STEREO, 6)] {
            for length in 0..header_size {
                let mut packed = vec![mask];
                packed.resize(length + 1, 0);
                assert!(matches!(
                    decompress_sector(&packed, &mut [0; 16]),
                    Err(PayloadError::Adpcm)
                ));
            }
            let mut packed = vec![mask];
            packed.resize(header_size + 1, 0);
            assert!(matches!(
                decompress_sector(&packed, &mut []),
                Err(PayloadError::Adpcm)
            ));
            packed[0] |= COMPRESSION_HUFFMAN;
            assert!(
                matches!(decompress_sector(&packed, &mut [0; 16]), Err(PayloadError::UnsupportedCompression { mask: m }) if m == mask | COMPRESSION_HUFFMAN)
            );
        }
        for mask in [COMPRESSION_HUFFMAN, COMPRESSION_SPARSE] {
            assert!(
                matches!(decompress_sector(&[mask], &mut [0; 16]), Err(PayloadError::UnsupportedCompression { mask: m }) if m == mask)
            );
        }
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
