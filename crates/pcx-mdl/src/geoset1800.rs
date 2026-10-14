//! Stored HD streams only; the classic/SD mesh reader keeps its typed refusal.
use super::{Chunk, Error, Geoset, MAGIC, Mdlx, read_u32, tag};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredGeosetBlock<'a> {
    /// Header offset within Geoset.data.
    pub offset: usize,
    pub tag: [u8; 4],
    pub count: u32,
    pub body: &'a [u8],
    /// Exact MATS trailer; empty for ordinary tagged blocks.
    pub trailer: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TangentStream1800<'a> {
    /// TANG header offset within Geoset.data; body starts eight bytes later.
    pub offset: usize,
    pub count: u32,
    pub data: &'a [u8],
}

impl TangentStream1800<'_> {
    /// Exact four f32 bit patterns; no handedness or normalization is inferred.
    pub fn bits(&self, index: usize) -> Option<[u32; 4]> {
        if index >= self.count as usize {
            return None;
        }
        let start = index.checked_mul(16)?;
        let raw = self.data.get(start..start.checked_add(16)?)?;
        Some(std::array::from_fn(|i| {
            u32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinRecord1800 {
    pub joint_words: [u16; 4],
    /// Authored integer weights; no division, clamping or rounding is applied.
    pub weight_words: [u16; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinStream1800<'a> {
    /// SKIN header offset within Geoset.data; body starts eight bytes later.
    pub offset: usize,
    /// Authored u16 element count, not a byte or vertex count.
    pub word_count: u32,
    pub data: &'a [u8],
}

impl SkinStream1800<'_> {
    /// Complete records bounded by both the authored count and available bytes.
    pub fn record_count(&self) -> usize {
        (self.word_count as usize / 8).min(self.data.len() / 16)
    }
    pub fn get(&self, index: usize) -> Option<SkinRecord1800> {
        if index >= self.record_count() {
            return None;
        }
        let start = index.checked_mul(16)?;
        let raw = self.data.get(start..start.checked_add(16)?)?;
        let words: [u16; 8] =
            std::array::from_fn(|i| u16::from_le_bytes(raw[i * 2..i * 2 + 2].try_into().unwrap()));
        Some(SkinRecord1800 {
            joint_words: words[..4].try_into().unwrap(),
            weight_words: words[4..].try_into().unwrap(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeosetStreams1800<'a> {
    pub version: u32,
    pub vertex_count: u32,
    /// Original block order and all stored bytes, including MATS' untagged tail.
    pub blocks: Vec<StoredGeosetBlock<'a>>,
    pub tangents: Option<TangentStream1800<'a>>,
    pub skin: Option<SkinStream1800<'a>>,
}

fn span<'a>(p: &'a [u8], at: usize, width: usize, what: &'static str) -> Result<&'a [u8], Error> {
    at.checked_add(width)
        .and_then(|end| p.get(at..end))
        .ok_or(Error::Truncated {
            what,
            at,
            need: width,
            have: p.len().saturating_sub(at),
        })
}

impl Mdlx {
    /// v1800 stored container framing, retaining opaque chunks such as FAFX/BPOS.
    /// This does not approve their semantics or enable the strict render importer.
    /// Checked import policy requires one VERS payload of exactly four bytes;
    /// this is not a claim about native malformed-envelope rejection.
    pub fn parse_stored_v1800(data: &[u8]) -> Result<Self, Error> {
        let magic: [u8; 4] = span(data, 0, 4, "magic")?.try_into().unwrap();
        if &magic != MAGIC {
            return Err(Error::BadMagic(magic));
        }
        let mut at = 4;
        let mut chunks = Vec::new();
        let mut version = None;
        while at < data.len() {
            let tag: [u8; 4] = span(data, at, 4, "chunk tag")?.try_into().unwrap();
            if !tag.iter().all(|byte| byte.is_ascii_graphic()) {
                return Err(Error::NotATag { at, tag });
            }
            let size = read_u32(data, at + 4)? as usize;
            let payload_offset = at + 8;
            let left = data.len() - payload_offset;
            if size > left {
                return Err(Error::ChunkOverruns {
                    tag: String::from_utf8_lossy(&tag).into_owned(),
                    size,
                    left,
                });
            }
            if &tag == tag::VERS {
                if size != 4 {
                    return Err(Error::RecordShape {
                        chunk: "VERS",
                        at,
                        size,
                        measured: 4,
                    });
                }
                if let Some(version) = version {
                    return Err(Error::UnsupportedVersionedChunk {
                        version,
                        chunk: "duplicate VERS",
                    });
                }
                version = Some(read_u32(data, payload_offset)?);
            }
            chunks.push(Chunk {
                tag,
                offset: at,
                payload_offset,
                size,
            });
            at = payload_offset + size;
        }
        let version = version.ok_or(Error::NoVersion)?;
        if version != 1800 {
            return Err(Error::UnsupportedVersion(version));
        }
        Ok(Self { version, chunks })
    }

    /// Checked v1800 stored streams, borrowing exact source bytes.
    /// Older u8 SKIN layouts are refused; see REFORGED_SKIN.md for width evidence.
    pub fn geoset_streams_v1800<'a>(&self, g: &'a Geoset) -> Result<GeosetStreams1800<'a>, Error> {
        if self.version != 1800 {
            return Err(Error::UnsupportedVersion(self.version));
        }
        let p = &g.data;
        let mut at = 0;
        let mut vertices = None;
        let mut tangents = None;
        let mut skin = None;
        let mut blocks = Vec::new();
        let mut pending_uvs = 0;
        while at < p.len() {
            let head = span(p, at, 8, "geoset sub-block header")?;
            let tag: [u8; 4] = head[..4].try_into().unwrap();
            let count = read_u32(head, 4)?;
            let stride = match &tag {
                b"VRTX" | b"NRMS" => 12,
                b"PTYP" | b"PCNT" | b"MTGC" | b"MATS" => 4,
                b"PVTX" | b"SKIN" => 2,
                b"GNDX" => 1,
                b"TANG" => 16,
                b"UVAS" => 0,
                b"UVBS" => 8,
                _ => return Err(Error::UnknownGeosetTag(tag)),
            };
            let width = (count as usize)
                .checked_mul(stride)
                .ok_or(Error::Truncated {
                    what: "geoset sub-block body",
                    at,
                    need: usize::MAX,
                    have: p.len().saturating_sub(at),
                })?;
            let body = span(p, at + 8, width, "geoset sub-block body")?;
            let mut end = at + 8 + width;
            let mut trailer = &p[end..end];
            if &tag == b"MATS" {
                let fixed = span(p, end, 128, "v1800 MATS trailer")?;
                let count = read_u32(fixed, 124)? as usize;
                let width = count
                    .checked_mul(28)
                    .and_then(|n| n.checked_add(128))
                    .ok_or(Error::Truncated {
                        what: "v1800 MATS seqBounds",
                        at: end,
                        need: usize::MAX,
                        have: p.len().saturating_sub(end),
                    })?;
                trailer = span(p, end, width, "v1800 MATS seqBounds")?;
                end += width;
            }
            match &tag {
                b"VRTX" => {
                    if vertices.replace(count).is_some() {
                        return Err(Error::DuplicateGeosetTag { at, tag });
                    }
                }
                b"TANG" | b"SKIN" => {
                    let vertex_count = vertices.ok_or(Error::NoVRTX)? as usize;
                    let measured = vertex_count.checked_mul(16).ok_or(Error::RecordShape {
                        chunk: "HD GEOS",
                        at,
                        size: body.len(),
                        measured: usize::MAX,
                    })?;
                    if body.len() != measured {
                        return Err(Error::RecordShape {
                            chunk: if &tag == b"TANG" { "TANG" } else { "SKIN" },
                            at,
                            size: body.len(),
                            measured,
                        });
                    }
                    if &tag == b"TANG" {
                        if tangents
                            .replace(TangentStream1800 {
                                offset: at,
                                count,
                                data: body,
                            })
                            .is_some()
                        {
                            return Err(Error::DuplicateGeosetTag { at, tag });
                        }
                    } else if skin
                        .replace(SkinStream1800 {
                            offset: at,
                            word_count: count,
                            data: body,
                        })
                        .is_some()
                    {
                        return Err(Error::DuplicateGeosetTag { at, tag });
                    }
                }
                b"UVAS" => {
                    if pending_uvs != 0 {
                        return Err(Error::RecordShape {
                            chunk: "UVAS",
                            at,
                            size: 0,
                            measured: pending_uvs as usize,
                        });
                    }
                    pending_uvs = count;
                }
                b"UVBS" => {
                    if pending_uvs == 0 {
                        return Err(Error::RecordShape {
                            chunk: "UVBS",
                            at,
                            size: count as usize,
                            measured: 0,
                        });
                    }
                    pending_uvs -= 1;
                }
                _ => {}
            }
            blocks.push(StoredGeosetBlock {
                offset: at,
                tag,
                count,
                body,
                trailer,
            });
            at = end;
        }
        if pending_uvs != 0 {
            return Err(Error::RecordShape {
                chunk: "UVAS",
                at,
                size: 0,
                measured: pending_uvs as usize,
            });
        }
        Ok(GeosetStreams1800 {
            version: self.version,
            vertex_count: vertices.ok_or(Error::NoVRTX)?,
            blocks,
            tangents,
            skin,
        })
    }
}
