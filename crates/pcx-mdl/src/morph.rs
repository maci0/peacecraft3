//! Warcraft III `.mrf` morph files: per-frame vertex positions and normals for
//! the Arthas cape cinematic, `Doodads\Cinematic\ArthasIllidanFight\ArthasCape0000.mrf`
//! through `ArthasCape0005.mrf`.
//!
//! Measured on the 12 members, 6 in `War3x.mpq` and 6 in `War3xLocal.mpq`. Each
//! archive's copy is byte-identical to the other, so there are 6 distinct files,
//! 224,640 to 983,168 bytes. No version word was identified: the header is the
//! magic followed by counts, offsets and floats, and the same word layout holds
//! in all six.
//!
//! ```text
//! 0x00  "Morf"
//! 0x04  u32 frame count                  91, 239, 79, 73, 57, 54
//! 0x08  u32 vertices per frame           170 in all six
//! 0x0c  u32 index count                  822 in all six (274 triangles)
//! 0x10  f32                              1/30 in all six; meaning not established
//! 0x14  f32 x4                           centre x, y, z and radius; meaning not established
//! 0x24  u32 x8                           zero in all six; kept raw
//! 0x44  u32 texture offset               64 bytes, NUL-padded path
//! 0x48  u32 index offset                 u16 triangle list, zero-padded
//! 0x4c  u32 uv offset                    vertex-count pairs of f32 u, v, zero-padded
//! 0x50  u32 x frame count                frame offsets, zero-padded to the texture offset
//!       frames                           vertex-count x (position f32 x3, normal f32 x3),
//!                                        zero-padded to a fixed stride (4096 in all six)
//! ```
//!
//! Every section ends where the next one starts, and every padding byte is zero.
//! [`MorphFile::parse`] fails on any byte it cannot place, so a successful parse
//! means the walk landed exactly on the end of the file.
//!
//! Measured on the six distinct files:
//! - The index list addresses vertices 0 to 169 only, so every index is below the
//!   170-vertex frame. Frames hold unit-length normals.
//! - The UV table is byte-identical across all six files.
//! - Every vertex of every frame lies inside the sphere given by the 0x14 floats.
//!   This is consistent with a bounding sphere, but the engine's use of those
//!   floats is not established.
//!
//! Relation to the paired model. The `.mrf` members sit beside one `.mdx`,
//! `Doodads\Cinematic\ArthasIllidanFight\ArthasIllidanFight.mdx`. Its texture
//! list contains the path the `.mrf` names, `ArthasCloakFight.blp`, at slot 53.
//! Material 17 binds that slot, and three geosets use material 17 with 180, 100
//! and 80 vertices and 798, 357 and 336 indices. Measured against the model:
//! - No geoset has 170 vertices or 822 indices, so the frame and index counts do
//!   not equal a geoset's counts.
//! - No vertex of frame 0 in `ArthasCape0000.mrf` coincides with a vertex of
//!   those three geosets. The closest pair is 122 units apart, so the stored
//!   frame positions do not share the model's coordinates as stored.
//!
//! Not established: which geoset the morph drives, the coordinate space of the
//! positions, the meaning of the 0x10 and 0x14 floats, the zeroed words at
//! 0x24..0x44, the playback timing of the frames, and the UV semantics. Those
//! fields stay raw, and this module does not wire morphs into rendering.

use std::str;

/// The first four bytes of every `.mrf` member.
pub const MRF_MAGIC: &[u8; 4] = b"Morf";

/// Fixed header length. The frame-offset table starts here.
const HEADER_LEN: usize = 0x50;
const FRAME_COUNT_AT: usize = 0x04;
const VERTEX_COUNT_AT: usize = 0x08;
const INDEX_COUNT_AT: usize = 0x0c;
const F32_0X10_AT: usize = 0x10;
const F32S_0X14_AT: usize = 0x14;
const U32S_0X24_AT: usize = 0x24;
const TEXTURE_AT: usize = 0x44;
const INDEX_AT: usize = 0x48;
const UV_AT: usize = 0x4c;

const WORD: usize = 4;
const VERTEX_BYTES: usize = 24;
const INDEX_BYTES: usize = 2;
const UV_BYTES: usize = 8;
const TRIANGLE_INDICES: usize = 3;

/// One frame's vertex: position, then normal, as stored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MorphVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// A parsed `.mrf` member. Every field is a stored value; nothing is scaled or
/// reinterpreted.
#[derive(Debug, Clone, PartialEq)]
pub struct MorphFile {
    /// The texture path the morph names, without its NUL padding.
    pub texture: String,
    /// The `f32` at 0x10. Measured as 1/30 in all six files; meaning not established.
    pub f32_0x10: f32,
    /// The four `f32`s at 0x14. Meaning not established.
    pub f32s_0x14: [f32; 4],
    /// The eight `u32`s at 0x24. Zero in all six files; meaning not established.
    pub u32s_0x24: [u32; 8],
    /// One `[u, v]` per vertex.
    pub uvs: Vec<[f32; 2]>,
    /// Triangle list: three vertex indices per triangle, each below the vertex count.
    pub indices: Vec<u16>,
    /// Bytes from one frame's start to the next.
    pub frame_stride: usize,
    /// Frames in file order, each with one vertex per UV pair.
    pub frames: Vec<Vec<MorphVertex>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MorphError {
    #[error("{what} at +{at} needs {need} bytes, file has {have}")]
    Truncated {
        what: &'static str,
        at: usize,
        need: usize,
        have: usize,
    },
    #[error("bad magic {0:?}, expected Morf")]
    BadMagic([u8; 4]),
    #[error("{what} at +{at} is not zero")]
    NonZeroPadding { what: &'static str, at: usize },
    #[error("{what} at +{at} starts before +{before}, where the previous section ends")]
    Overlap {
        what: &'static str,
        at: usize,
        before: usize,
    },
    #[error("{what} overflows the address space")]
    Overflow { what: &'static str },
    #[error("texture path at +{at} has no NUL terminator")]
    UnterminatedTexture { at: usize },
    #[error("texture path at +{at} is not UTF-8")]
    TextureEncoding { at: usize },
    #[error("index count {0} is not a whole number of triangles")]
    PartialTriangle(u32),
    #[error("index {index} at +{at} addresses a vertex outside the {vertices}-vertex frame")]
    IndexRange {
        at: usize,
        index: u16,
        vertices: usize,
    },
    #[error("frame {frame} starts at +{at}, expected +{expected}")]
    FrameOffset {
        frame: usize,
        at: usize,
        expected: usize,
    },
    #[error("frame stride {stride} is below the {need}-byte vertex block")]
    FrameStride { stride: usize, need: usize },
    #[error("frames end at +{end}, file is {len} bytes")]
    FrameTiling { end: usize, len: usize },
}

impl MorphFile {
    /// Parses one `.mrf` member. Fails on any byte outside the layout above, on
    /// an index past the frame's vertices, and on a frame table that does not
    /// tile the file.
    pub fn parse(data: &[u8]) -> Result<Self, MorphError> {
        let magic = field(data, 0, MRF_MAGIC.len(), "magic")?;
        if magic != MRF_MAGIC {
            return Err(MorphError::BadMagic(magic.try_into().unwrap()));
        }
        let header = field(data, 0, HEADER_LEN, "header")?;
        let frame_count = word(header, FRAME_COUNT_AT) as usize;
        let vertex_count = word(header, VERTEX_COUNT_AT) as usize;
        let index_count = word(header, INDEX_COUNT_AT);
        if !(index_count as usize).is_multiple_of(TRIANGLE_INDICES) {
            return Err(MorphError::PartialTriangle(index_count));
        }
        let index_count = index_count as usize;
        let texture_at = word(header, TEXTURE_AT) as usize;
        let index_at = word(header, INDEX_AT) as usize;
        let uv_at = word(header, UV_AT) as usize;

        // Frame-offset table, then padding to the texture field.
        let table_end = frame_count
            .checked_mul(WORD)
            .and_then(|bytes| bytes.checked_add(HEADER_LEN))
            .ok_or(MorphError::Overflow {
                what: "frame table",
            })?;
        let table = field(data, HEADER_LEN, table_end - HEADER_LEN, "frame table")?;
        let frame_offsets: Vec<usize> = table
            .as_chunks::<WORD>()
            .0
            .iter()
            .map(|&w| u32::from_le_bytes(w) as usize)
            .collect();
        zero_run(data, table_end, texture_at, "frame table padding")?;

        // NUL-padded texture path, from texture offset to index offset.
        let path = section(data, texture_at, index_at, table_end, "texture path")?;
        let nul = path
            .iter()
            .position(|&b| b == 0)
            .ok_or(MorphError::UnterminatedTexture { at: texture_at })?;
        zero_run(data, texture_at + nul, index_at, "texture path padding")?;
        let texture = str::from_utf8(&path[..nul])
            .map_err(|_| MorphError::TextureEncoding { at: texture_at })?
            .to_owned();

        // u16 triangle list, zero-padded to the UV table.
        let index_bytes = index_count
            .checked_mul(INDEX_BYTES)
            .ok_or(MorphError::Overflow { what: "index list" })?;
        let index_end = index_at
            .checked_add(index_bytes)
            .ok_or(MorphError::Overflow { what: "index list" })?;
        let index_region = section(data, index_at, index_end, index_at, "index list")?;
        let indices: Vec<u16> = index_region
            .as_chunks::<INDEX_BYTES>()
            .0
            .iter()
            .map(|&b| u16::from_le_bytes(b))
            .collect();
        zero_run(data, index_end, uv_at, "index list padding")?;
        for (i, &index) in indices.iter().enumerate() {
            if usize::from(index) >= vertex_count {
                return Err(MorphError::IndexRange {
                    at: index_at + i * INDEX_BYTES,
                    index,
                    vertices: vertex_count,
                });
            }
        }

        // UV pairs, then padding to the first frame (or to the end when there are none).
        let uv_bytes = vertex_count
            .checked_mul(UV_BYTES)
            .ok_or(MorphError::Overflow { what: "UV table" })?;
        let uv_end = uv_at
            .checked_add(uv_bytes)
            .ok_or(MorphError::Overflow { what: "UV table" })?;
        let uv_region = section(data, uv_at, uv_end, index_end, "UV table")?;
        let uvs: Vec<[f32; 2]> = uv_region
            .as_chunks::<UV_BYTES>()
            .0
            .iter()
            .map(|b| [f32_at(b, 0), f32_at(b, WORD)])
            .collect();

        let frames_at = frame_offsets.first().copied().unwrap_or(data.len());
        zero_run(data, uv_end, frames_at, "UV table padding")?;

        let (frame_stride, frames) = read_frames(data, &frame_offsets, frames_at, vertex_count)?;

        Ok(Self {
            texture,
            f32_0x10: f32_at(header, F32_0X10_AT),
            f32s_0x14: std::array::from_fn(|i| f32_at(header, F32S_0X14_AT + i * WORD)),
            u32s_0x24: std::array::from_fn(|i| word(header, U32S_0X24_AT + i * WORD)),
            uvs,
            indices,
            frame_stride,
            frames,
        })
    }
}

/// Reads the frames at `frames_at`. Stride is the distance between the first
/// two table entries (or the rest of the file for one frame). Every table entry
/// must sit on that stride, and the frames must end exactly at the file end.
fn read_frames(
    data: &[u8],
    offsets: &[usize],
    frames_at: usize,
    vertex_count: usize,
) -> Result<(usize, Vec<Vec<MorphVertex>>), MorphError> {
    let Some(&first) = offsets.first() else {
        return Ok((0, Vec::new()));
    };
    let overflow = || MorphError::Overflow {
        what: "frame table",
    };
    let stride = match offsets {
        [_, second, ..] => second.checked_sub(first).ok_or(MorphError::Overlap {
            what: "frame table",
            at: *second,
            before: first,
        })?,
        _ => data.len().checked_sub(first).ok_or_else(overflow)?,
    };
    let frame_bytes = vertex_count
        .checked_mul(VERTEX_BYTES)
        .ok_or(MorphError::Overflow {
            what: "frame vertices",
        })?;
    if stride < frame_bytes {
        return Err(MorphError::FrameStride {
            stride,
            need: frame_bytes,
        });
    }
    let end = offsets
        .len()
        .checked_mul(stride)
        .and_then(|bytes| bytes.checked_add(frames_at))
        .ok_or_else(overflow)?;
    if end != data.len() {
        return Err(MorphError::FrameTiling {
            end,
            len: data.len(),
        });
    }
    for (frame, &at) in offsets.iter().enumerate() {
        let expected = frames_at + frame * stride;
        if at != expected {
            return Err(MorphError::FrameOffset {
                frame,
                at,
                expected,
            });
        }
    }
    let frames = offsets
        .iter()
        .map(|&at| {
            let vertices = field(data, at, frame_bytes, "frame vertices")?;
            zero_run(data, at + frame_bytes, at + stride, "frame padding")?;
            Ok(vertices
                .as_chunks::<VERTEX_BYTES>()
                .0
                .iter()
                .map(|v| MorphVertex {
                    position: [f32_at(v, 0), f32_at(v, WORD), f32_at(v, 2 * WORD)],
                    normal: [
                        f32_at(v, 3 * WORD),
                        f32_at(v, 4 * WORD),
                        f32_at(v, 5 * WORD),
                    ],
                })
                .collect())
        })
        .collect::<Result<Vec<_>, MorphError>>()?;
    Ok((stride, frames))
}

/// `data[at..at + len]`, or a truncation error naming the section.
fn field<'a>(
    data: &'a [u8],
    at: usize,
    len: usize,
    what: &'static str,
) -> Result<&'a [u8], MorphError> {
    let end = at.checked_add(len).ok_or(MorphError::Overflow { what })?;
    data.get(at..end).ok_or(MorphError::Truncated {
        what,
        at,
        need: len,
        have: data.len().saturating_sub(at),
    })
}

/// `data[start..end]` after checking that `start` does not precede the
/// previous section's end at `before`.
fn section<'a>(
    data: &'a [u8],
    start: usize,
    end: usize,
    before: usize,
    what: &'static str,
) -> Result<&'a [u8], MorphError> {
    if start < before {
        return Err(MorphError::Overlap {
            what,
            at: start,
            before,
        });
    }
    let len = end.checked_sub(start).ok_or(MorphError::Overlap {
        what,
        at: end,
        before: start,
    })?;
    field(data, start, len, what)
}

/// Requires `data[from..to]` to exist and be all zero.
fn zero_run(data: &[u8], from: usize, to: usize, what: &'static str) -> Result<(), MorphError> {
    let len = to.checked_sub(from).ok_or(MorphError::Overlap {
        what,
        at: to,
        before: from,
    })?;
    let run = field(data, from, len, what)?;
    match run.iter().position(|&b| b != 0) {
        Some(at) => Err(MorphError::NonZeroPadding {
            what,
            at: from + at,
        }),
        None => Ok(()),
    }
}

/// Little-endian `u32` at `at`. Callers pass a header slice of fixed length.
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + WORD].try_into().unwrap())
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mdlx;

    /// One vertex as `(position, normal)`.
    type Vert = ([f32; 3], [f32; 3]);

    const TEXTURE_FIELD: usize = 64;
    const FRAME_PAD: usize = 16;
    const TEXTURE: &str = "Doodads\\Test\\cape.blp";
    const TRIANGLE: [u16; 3] = [0, 1, 2];
    const UVS: [[f32; 2]; 3] = [[0.0, 1.0], [0.5, 0.25], [1.0, 0.0]];

    fn put_word(bytes: &mut [u8], at: usize, v: u32) {
        bytes[at..at + WORD].copy_from_slice(&v.to_le_bytes());
    }

    fn put_f32(bytes: &mut [u8], at: usize, v: f32) {
        put_word(bytes, at, v.to_bits());
    }

    /// Writes a member in the layout from the module doc. Frames sit at a stride
    /// of one vertex block plus [`FRAME_PAD`] zero bytes.
    fn build(texture: &str, frames: &[Vec<Vert>], indices: &[u16], uvs: &[[f32; 2]]) -> Vec<u8> {
        let vertex_count = uvs.len();
        let table_end = HEADER_LEN + frames.len() * WORD;
        let index_at = table_end + TEXTURE_FIELD;
        let uv_at = index_at + indices.len() * INDEX_BYTES;
        let frames_at = uv_at + vertex_count * UV_BYTES;
        let stride = vertex_count * VERTEX_BYTES + FRAME_PAD;
        let mut out = vec![0u8; frames_at + frames.len() * stride];
        out[..MRF_MAGIC.len()].copy_from_slice(MRF_MAGIC);
        put_word(&mut out, FRAME_COUNT_AT, frames.len() as u32);
        put_word(&mut out, VERTEX_COUNT_AT, vertex_count as u32);
        put_word(&mut out, INDEX_COUNT_AT, indices.len() as u32);
        put_f32(&mut out, F32_0X10_AT, 1.0 / 30.0);
        for (i, v) in [-107.5, 6.25, 46.0, 264.0].into_iter().enumerate() {
            put_f32(&mut out, F32S_0X14_AT + i * WORD, v);
        }
        put_word(&mut out, TEXTURE_AT, table_end as u32);
        put_word(&mut out, INDEX_AT, index_at as u32);
        put_word(&mut out, UV_AT, uv_at as u32);
        for k in 0..frames.len() {
            put_word(
                &mut out,
                HEADER_LEN + k * WORD,
                (frames_at + k * stride) as u32,
            );
        }
        let path = &texture.as_bytes()[..texture.len().min(TEXTURE_FIELD)];
        out[table_end..table_end + path.len()].copy_from_slice(path);
        for (i, index) in indices.iter().enumerate() {
            let at = index_at + i * INDEX_BYTES;
            out[at..at + INDEX_BYTES].copy_from_slice(&index.to_le_bytes());
        }
        for (i, uv) in uvs.iter().enumerate() {
            let at = uv_at + i * UV_BYTES;
            put_f32(&mut out, at, uv[0]);
            put_f32(&mut out, at + WORD, uv[1]);
        }
        for (k, frame) in frames.iter().enumerate() {
            assert_eq!(frame.len(), vertex_count, "builder: frame {k} vertex count");
            for (v, (position, normal)) in frame.iter().enumerate() {
                let at = frames_at + k * stride + v * VERTEX_BYTES;
                for (c, &f) in position.iter().chain(normal).enumerate() {
                    put_f32(&mut out, at + c * WORD, f);
                }
            }
        }
        out
    }

    fn triangle_frames() -> Vec<Vec<Vert>> {
        let frame = |z: f32| {
            vec![
                ([0.0, 0.0, z], [0.0, 0.0, 1.0]),
                ([1.0, 0.0, z], [0.0, 1.0, 0.0]),
                ([0.0, 1.0, z], [1.0, 0.0, 0.0]),
            ]
        };
        vec![frame(0.0), frame(2.5)]
    }

    fn valid() -> Vec<u8> {
        build(TEXTURE, &triangle_frames(), &TRIANGLE, &UVS)
    }

    #[test]
    fn parses_header_texture_uvs_indices_and_frames() {
        let m = MorphFile::parse(&valid()).unwrap();
        assert_eq!(m.texture, TEXTURE);
        assert_eq!(m.f32_0x10, 1.0 / 30.0);
        assert_eq!(m.f32s_0x14, [-107.5, 6.25, 46.0, 264.0]);
        assert_eq!(m.u32s_0x24, [0; 8]);
        assert_eq!(m.uvs, UVS);
        assert_eq!(m.indices, TRIANGLE);
        assert_eq!(m.frame_stride, 3 * VERTEX_BYTES + FRAME_PAD);
        let pairs: Vec<Vec<Vert>> = m
            .frames
            .iter()
            .map(|f| f.iter().map(|v| (v.position, v.normal)).collect())
            .collect();
        assert_eq!(pairs, triangle_frames());
    }

    #[test]
    fn zero_frame_member_has_no_frames() {
        let m = MorphFile::parse(&build(TEXTURE, &[], &TRIANGLE, &UVS)).unwrap();
        assert!(m.frames.is_empty());
        assert_eq!(m.frame_stride, 0);
        assert_eq!(m.indices, TRIANGLE);
    }

    #[test]
    fn empty_input_is_truncated_not_a_panic() {
        assert!(matches!(
            MorphFile::parse(&[]),
            Err(MorphError::Truncated { what: "magic", .. })
        ));
    }

    #[test]
    fn every_truncation_is_refused() {
        let data = valid();
        for len in 0..data.len() {
            assert!(
                MorphFile::parse(&data[..len]).is_err(),
                "prefix {len} parsed"
            );
        }
    }

    #[test]
    fn trailing_byte_is_refused() {
        let mut data = valid();
        data.push(0);
        assert!(matches!(
            MorphFile::parse(&data),
            Err(MorphError::FrameTiling { .. })
        ));
    }

    #[test]
    fn wrong_magic_is_refused() {
        let mut data = valid();
        data[..4].copy_from_slice(b"Morg");
        assert_eq!(MorphFile::parse(&data), Err(MorphError::BadMagic(*b"Morg")));
    }

    #[test]
    fn maximal_counts_are_refused_before_allocation() {
        for at in [FRAME_COUNT_AT, VERTEX_COUNT_AT, INDEX_COUNT_AT] {
            let mut data = valid();
            put_word(&mut data, at, u32::MAX);
            assert!(
                matches!(
                    MorphFile::parse(&data),
                    Err(MorphError::Truncated { .. } | MorphError::Overflow { .. })
                ),
                "count at {at:#x} did not fail as truncated"
            );
        }
    }

    #[test]
    fn index_past_the_frame_is_refused() {
        let data = build(TEXTURE, &triangle_frames(), &[0, 1, 3], &UVS);
        assert!(matches!(
            MorphFile::parse(&data),
            Err(MorphError::IndexRange {
                index: 3,
                vertices: 3,
                ..
            })
        ));
    }

    #[test]
    fn index_into_an_empty_frame_is_refused() {
        let data = build(TEXTURE, &[vec![]], &TRIANGLE, &[]);
        assert!(matches!(
            MorphFile::parse(&data),
            Err(MorphError::IndexRange {
                index: 0,
                vertices: 0,
                ..
            })
        ));
    }

    #[test]
    fn partial_triangle_is_refused() {
        let data = build(TEXTURE, &triangle_frames(), &[0, 1], &UVS);
        assert_eq!(MorphFile::parse(&data), Err(MorphError::PartialTriangle(2)));
    }

    #[test]
    fn unterminated_texture_is_refused() {
        let path = "x".repeat(TEXTURE_FIELD);
        let data = build(&path, &triangle_frames(), &TRIANGLE, &UVS);
        assert!(matches!(
            MorphFile::parse(&data),
            Err(MorphError::UnterminatedTexture { .. })
        ));
    }

    #[test]
    fn non_utf8_texture_is_refused() {
        let mut data = valid();
        let texture_at = HEADER_LEN + 2 * WORD;
        data[texture_at] = 0xff;
        assert_eq!(
            MorphFile::parse(&data),
            Err(MorphError::TextureEncoding { at: texture_at })
        );
    }

    #[test]
    fn texture_padding_must_be_zero() {
        let mut data = valid();
        let at = HEADER_LEN + 2 * WORD + TEXTURE.len() + 3;
        data[at] = 1;
        assert_eq!(
            MorphFile::parse(&data),
            Err(MorphError::NonZeroPadding {
                what: "texture path padding",
                at,
            })
        );
    }

    #[test]
    fn frame_padding_must_be_zero() {
        let mut data = valid();
        let last_frame = data.len() - (3 * VERTEX_BYTES + FRAME_PAD);
        let at = last_frame + 3 * VERTEX_BYTES;
        data[at] = 1;
        assert_eq!(
            MorphFile::parse(&data),
            Err(MorphError::NonZeroPadding {
                what: "frame padding",
                at,
            })
        );
    }

    #[test]
    fn texture_field_inside_the_frame_table_is_refused() {
        let mut data = valid();
        put_word(&mut data, TEXTURE_AT, HEADER_LEN as u32);
        assert!(matches!(
            MorphFile::parse(&data),
            Err(MorphError::Overlap { .. })
        ));
    }

    #[test]
    fn frame_stride_below_the_vertex_block_is_refused() {
        let mut data = valid();
        let first = word(&data, HEADER_LEN) as usize;
        put_word(&mut data, HEADER_LEN + WORD, (first + 10) as u32);
        assert_eq!(
            MorphFile::parse(&data),
            Err(MorphError::FrameStride {
                stride: 10,
                need: 3 * VERTEX_BYTES,
            })
        );
    }

    #[test]
    fn uneven_frame_table_is_refused() {
        let mut data = valid();
        let first = word(&data, HEADER_LEN) as usize;
        let stride = 3 * VERTEX_BYTES + FRAME_PAD;
        put_word(&mut data, HEADER_LEN + WORD, (first + stride + 4) as u32);
        assert!(matches!(
            MorphFile::parse(&data),
            Err(MorphError::FrameTiling { .. })
        ));
    }

    #[test]
    fn frame_offset_off_the_stride_is_refused() {
        let frames = vec![triangle_frames()[0].clone(); 3];
        let mut data = build(TEXTURE, &frames, &TRIANGLE, &UVS);
        let first = word(&data, HEADER_LEN) as usize;
        let stride = 3 * VERTEX_BYTES + FRAME_PAD;
        put_word(
            &mut data,
            HEADER_LEN + 2 * WORD,
            (first + 2 * stride + 4) as u32,
        );
        assert!(matches!(
            MorphFile::parse(&data),
            Err(MorphError::FrameOffset { frame: 2, .. })
        ));
    }

    #[test]
    fn member_at_the_measured_geometry_parses_with_many_frames() {
        // The measured vertex and index counts with 500 frames, past the largest
        // frame count seen in the shipped members.
        const VERTICES: usize = 170;
        const INDICES: usize = 822;
        let frame: Vec<Vert> = (0..VERTICES)
            .map(|v| ([v as f32, 0.0, 1.0], [0.0, 0.0, 1.0]))
            .collect();
        let frames = vec![frame; 500];
        let indices: Vec<u16> = (0..INDICES).map(|i| (i % VERTICES) as u16).collect();
        let uvs = vec![[0.5, 0.5]; VERTICES];
        let m = MorphFile::parse(&build(TEXTURE, &frames, &indices, &uvs)).unwrap();
        assert_eq!(m.frames.len(), 500);
        assert_eq!(m.indices.len(), INDICES);
        assert_eq!(m.frame_stride, 4096);
        assert_eq!(m.frames[499][169].position, [169.0, 0.0, 1.0]);
    }

    #[test]
    fn single_byte_mutations_never_panic() {
        let data = valid();
        for at in 0..data.len() {
            for value in [0x00, 0x7f, 0x80, 0xff] {
                let mut mutated = data.clone();
                mutated[at] = value;
                let _ = MorphFile::parse(&mutated);
            }
        }
    }

    /// The six `ArthasCape` members in one archive, as `(path, bytes)`.
    fn cape_members(set: &mut pcx_mpq::MpqSet, listed: &[String]) -> Vec<(String, Vec<u8>)> {
        listed
            .iter()
            .filter(|p| p.starts_with(CAPE_DIR) && p.ends_with(".mrf"))
            .map(|p| (p.clone(), set.extract(p).unwrap()))
            .collect()
    }

    const CAPE_DIR: &str = r"Doodads\Cinematic\ArthasIllidanFight\";

    #[test]
    #[ignore = "needs the original install: set PEACECRAFT3_INSTALL to the Warcraft III directory"]
    fn real_cape_members_parse_exactly_and_bind_the_paired_model() {
        let dir = std::env::var("PEACECRAFT3_INSTALL")
            .expect("PEACECRAFT3_INSTALL names the Warcraft III install directory");
        let mut war3x = pcx_mpq::MpqSet::open_named(&dir, &["War3x.mpq"]).unwrap();
        let mut local = pcx_mpq::MpqSet::open_named(&dir, &["War3xLocal.mpq"]).unwrap();
        let listed = war3x.file_list().unwrap();

        let shipped = cape_members(&mut war3x, &listed);
        let localised = cape_members(&mut local, &listed);
        assert_eq!(shipped.len(), 6, "War3x.mpq cape members");
        assert_eq!(localised.len(), 6, "War3xLocal.mpq cape members");

        // The paired model is the only .mdx beside the members.
        let models: Vec<&String> = listed
            .iter()
            .filter(|p| p.starts_with(CAPE_DIR) && p.ends_with(".mdx"))
            .collect();
        assert_eq!(models.len(), 1, "paired model beside the cape members");
        let model_bytes = war3x.extract(models[0]).unwrap();
        let model = Mdlx::parse(&model_bytes).unwrap();
        let textures = model.textures(&model_bytes).unwrap();
        let materials = model.materials(&model_bytes).unwrap();
        let geosets = model.geosets(&model_bytes).unwrap();

        let mut total = 0;
        for ((path, bytes), (_, copy)) in shipped.iter().zip(&localised) {
            assert!(bytes == copy, "{path}: archive copies differ");
            for data in [bytes, copy] {
                // Parsing succeeds only when every byte is placed, so Ok is the
                // consumed-exactly check.
                let morph = MorphFile::parse(data).unwrap_or_else(|e| panic!("{path}: {e}"));
                total += 1;
                let slot = textures
                    .iter()
                    .position(|t| t.path.eq_ignore_ascii_case(&morph.texture))
                    .unwrap_or_else(|| {
                        panic!("{path}: {:?} is not a model texture", morph.texture)
                    });
                let bound: Vec<usize> = materials
                    .iter()
                    .enumerate()
                    .filter(|(_, mt)| {
                        mt.checked_layers().is_ok_and(|layers| {
                            layers
                                .iter()
                                .any(|l| l.properties.texture_id as usize == slot)
                        })
                    })
                    .map(|(i, _)| i)
                    .collect();
                let mut users = Vec::new();
                for (i, g) in geosets.iter().enumerate() {
                    let mesh = model.mesh(&model_bytes, g).unwrap();
                    if bound.contains(&(mesh.material_id as usize)) {
                        users.push((i, mesh.vertices.count, mesh.triangles.count));
                    }
                }
                assert!(
                    !users.is_empty(),
                    "{path}: no geoset draws the morph texture"
                );
                println!(
                    "{path}: frames {}, vertices {}, indices {}, texture slot {slot}, \
                     materials {bound:?}, geosets (index, vertices, indices) {users:?}",
                    morph.frames.len(),
                    morph.frames.first().map_or(0, Vec::len),
                    morph.indices.len(),
                );
            }
        }
        assert_eq!(total, 12, "members parsed across both archives");
    }
}
