//! Sparse (run-length) decompression, per StormLib's `src/sparse/sparse.cpp`.
//!
//! A sparse stream begins with the original data length as a big-endian `u32`.
//! Each following marker byte selects a run:
//!
//! - `0x80..=0xff`: a non-zero run of `(marker & 0x7f) + 1` bytes, followed by
//!   that many literal bytes.
//! - `0x00..=0x7f`: a zero run of `(marker & 0x7f) + 3` bytes (so `0x7f` is
//!   130 bytes, the format's maximum zero run).
//!
//! The sum of every run's length must equal the declared original size.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SparseError {
    #[error("sparse input is truncated: need {need} bytes, have {have}")]
    Truncated { need: usize, have: usize },
    #[error("sparse decoded size {decoded} exceeds output capacity {capacity}")]
    DecodedSizeExceeds { decoded: usize, capacity: usize },
    #[error("sparse produced {produced} bytes, declared {decoded}")]
    SizeMismatch { decoded: usize, produced: usize },
}

/// Decompress a sparse stream into `out`, returning the number of bytes written.
pub fn decompress(input: &[u8], out: &mut [u8]) -> Result<usize, SparseError> {
    if input.len() < 4 {
        return Err(SparseError::Truncated {
            need: 4,
            have: input.len(),
        });
    }
    let decoded = u32::from_be_bytes([input[0], input[1], input[2], input[3]]) as usize;
    if decoded > out.len() {
        return Err(SparseError::DecodedSizeExceeds {
            decoded,
            capacity: out.len(),
        });
    }
    if decoded == 0 {
        // An empty declared size still owns the whole header; any trailing marker
        // would be framing without content and is rejected below.
        if input.len() > 4 {
            return Err(SparseError::SizeMismatch {
                decoded: 0,
                produced: 0,
            });
        }
        return Ok(0);
    }

    let mut in_pos = 4usize;
    let mut out_pos = 0usize;
    let end = input.len();
    while in_pos < end {
        if out_pos >= decoded {
            return Err(SparseError::SizeMismatch {
                decoded,
                produced: out_pos,
            });
        }
        let marker = input[in_pos];
        in_pos += 1;
        if marker & 0x80 != 0 {
            let requested = (marker & 0x7f) as usize + 1;
            let run = requested.min(decoded - out_pos);
            if in_pos + requested > end {
                return Err(SparseError::Truncated {
                    need: in_pos + requested,
                    have: end,
                });
            }
            // A cap to the declared size must not silently drop authored bytes.
            if run != requested {
                return Err(SparseError::SizeMismatch {
                    decoded,
                    produced: out_pos + requested,
                });
            }
            out[out_pos..out_pos + run].copy_from_slice(&input[in_pos..in_pos + run]);
            in_pos += run;
            out_pos += run;
        } else {
            let requested = (marker & 0x7f) as usize + 3;
            let run = requested.min(decoded - out_pos);
            if run != requested {
                return Err(SparseError::SizeMismatch {
                    decoded,
                    produced: out_pos + requested,
                });
            }
            for byte in &mut out[out_pos..out_pos + run] {
                *byte = 0;
            }
            out_pos += run;
        }
    }

    if out_pos != decoded {
        return Err(SparseError::SizeMismatch {
            decoded,
            produced: out_pos,
        });
    }
    Ok(decoded)
}
