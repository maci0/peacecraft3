//! Checked primary-source convention and raw metadata, not native-verified skinning.
use super::{
    BONE_FIXED, BONE_TRAILER, Chunk, Error, Geoset, GeosetStreams1800, Mdlx, NODE_NAME,
    SkinRecord1800, SkinStream1800, read_u32, tag,
};
use std::collections::BTreeMap;

const BIND_POSE_SIZE: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindPoseRecord1800<'a> {
    pub index: usize,
    /// Absolute source offset of the twelve stored f32 words.
    pub offset: usize,
    pub data: &'a [u8; 48],
}

impl BindPoseRecord1800<'_> {
    /// Stored bits only; no matrix basis, inverse or pose space is inferred.
    pub fn bits(&self) -> [u32; 12] {
        std::array::from_fn(|i| u32::from_le_bytes(self.data[i * 4..i * 4 + 4].try_into().unwrap()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindPoseStream1800<'a> {
    pub version: u32,
    /// Absolute BPOS tag and count-word offsets, respectively.
    pub offset: usize,
    pub payload_offset: usize,
    pub count: u32,
    data: &'a [u8],
}

impl<'a> BindPoseStream1800<'a> {
    /// Complete raw payload, including the count word.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
    pub fn record_count(&self) -> usize {
        (self.data.len() - 4) / BIND_POSE_SIZE
    }
    pub fn get(&self, index: usize) -> Option<BindPoseRecord1800<'a>> {
        let start = index.checked_mul(BIND_POSE_SIZE)?.checked_add(4)?;
        let raw = self.data.get(start..start.checked_add(BIND_POSE_SIZE)?)?;
        Some(BindPoseRecord1800 {
            index,
            offset: self.payload_offset.checked_add(start)?,
            data: raw.try_into().ok()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixPaletteEntry1800<'a> {
    /// Object ID under the pinned primary convention, not a runtime bone ordinal.
    pub object_id: u32,
    pub offset: usize,
    pub data: &'a [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixPalette1800<'a> {
    pub version: u32,
    /// Absolute source offsets of the geoset size word, MATS tag and body.
    pub geoset_offset: usize,
    pub offset: usize,
    pub payload_offset: usize,
    pub count: u32,
    data: &'a [u8],
}

impl<'a> MatrixPalette1800<'a> {
    /// Raw MATS words, excluding its separately preserved geoset trailer.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
    pub fn record_count(&self) -> usize {
        self.data.len() / 4
    }
    pub fn get(&self, index: usize) -> Option<MatrixPaletteEntry1800<'a>> {
        let start = index.checked_mul(4)?;
        let raw: &'a [u8; 4] = self
            .data
            .get(start..start.checked_add(4)?)?
            .try_into()
            .ok()?;
        Some(MatrixPaletteEntry1800 {
            object_id: u32::from_le_bytes(*raw),
            offset: self.payload_offset.checked_add(start)?,
            data: raw,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinWeightDiagnostics1800 {
    pub sum: u32,
    pub nonzero_lanes: u8,
    pub max_word: u16,
}

impl SkinRecord1800 {
    /// Integer diagnostics only; every raw lane remains unchanged.
    pub fn weight_diagnostics(&self) -> SkinWeightDiagnostics1800 {
        SkinWeightDiagnostics1800 {
            sum: self.weight_words.iter().map(|word| u32::from(*word)).sum(),
            nonzero_lanes: self.weight_words.iter().filter(|word| **word != 0).count() as u8,
            max_word: self.weight_words.iter().copied().max().unwrap(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimaryJointRefusal1800 {
    MissingSkin,
    MissingPalette,
    InvalidVertex { record_count: usize },
    InvalidLane,
    JointOutsidePalette { joint: u16, palette_count: usize },
    ObjectNotInBoneInventory { object_id: u32 },
    AmbiguousBoneObject { object_id: u32 },
    MissingBindPoses,
    MissingBindPoseRow { object_id: u32, record_count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryJointReference1800<'a> {
    pub version: u32,
    pub raw_joint: u16,
    pub raw_weight: u16,
    pub object_id: u32,
    pub joint_offset: usize,
    pub weight_offset: usize,
    pub mats_offset: usize,
    pub bone_object_id_offset: usize,
    pub bind_pose: BindPoseRecord1800<'a>,
}

/// Explicit primary convention: SKIN ordinal -> MATS object ID -> BPOS row.
/// This local import policy is not native-verified and enables no render semantics.
#[derive(Debug, Clone)]
pub struct PrimaryJointJoin1800<'a> {
    geoset_offset: usize,
    skin_offset: usize,
    skin: Option<SkinStream1800<'a>>,
    palette: Option<MatrixPalette1800<'a>>,
    poses: Option<BindPoseStream1800<'a>>,
    // None means the source BONE inventory contains this object ID more than once.
    bones: BTreeMap<u32, Option<usize>>,
}

impl<'a> PrimaryJointJoin1800<'a> {
    /// Every lane, including zero weights and repeated joints, is checked separately.
    /// Direct-ID fallback, non-BONE targets and ambiguous references are refused.
    pub fn get(&self, vertex: usize, lane: usize) -> Result<PrimaryJointReference1800<'a>, Error> {
        let mut at = self.skin_offset;
        let refuse = |at, reason| Error::PrimaryJointRefusal {
            version: 1800,
            geoset_offset: self.geoset_offset,
            vertex,
            lane,
            at,
            reason,
        };
        let skin = self
            .skin
            .as_ref()
            .ok_or_else(|| refuse(at, PrimaryJointRefusal1800::MissingSkin))?;
        if lane >= 4 {
            return Err(refuse(at, PrimaryJointRefusal1800::InvalidLane));
        }
        let raw = skin.get(vertex).ok_or_else(|| {
            refuse(
                at,
                PrimaryJointRefusal1800::InvalidVertex {
                    record_count: skin.record_count(),
                },
            )
        })?;
        // get() bounded the 16-byte record; the stream's entire source span was checked.
        at += vertex * 16 + lane * 2;
        let joint = raw.joint_words[lane];
        let palette = self
            .palette
            .as_ref()
            .ok_or_else(|| refuse(at, PrimaryJointRefusal1800::MissingPalette))?;
        let entry = palette.get(usize::from(joint)).ok_or_else(|| {
            refuse(
                at,
                PrimaryJointRefusal1800::JointOutsidePalette {
                    joint,
                    palette_count: palette.record_count(),
                },
            )
        })?;
        let object_id = entry.object_id;
        let bone_offset = self
            .bones
            .get(&object_id)
            .ok_or_else(|| {
                refuse(
                    at,
                    PrimaryJointRefusal1800::ObjectNotInBoneInventory { object_id },
                )
            })?
            .ok_or_else(|| {
                refuse(
                    at,
                    PrimaryJointRefusal1800::AmbiguousBoneObject { object_id },
                )
            })?;
        let poses = self
            .poses
            .as_ref()
            .ok_or_else(|| refuse(at, PrimaryJointRefusal1800::MissingBindPoses))?;
        let bind_pose = poses.get(object_id as usize).ok_or_else(|| {
            refuse(
                at,
                PrimaryJointRefusal1800::MissingBindPoseRow {
                    object_id,
                    record_count: poses.record_count(),
                },
            )
        })?;
        Ok(PrimaryJointReference1800 {
            version: 1800,
            raw_joint: joint,
            raw_weight: raw.weight_words[lane],
            object_id,
            joint_offset: at,
            weight_offset: at + 8,
            mats_offset: entry.offset,
            bone_object_id_offset: bone_offset,
            bind_pose,
        })
    }
}

fn span<'a>(data: &'a [u8], at: usize, size: usize, what: &'static str) -> Result<&'a [u8], Error> {
    at.checked_add(size)
        .and_then(|end| data.get(at..end))
        .ok_or(Error::Truncated {
            what,
            at,
            need: size,
            have: data.len().saturating_sub(at),
        })
}

impl Mdlx {
    fn joint_chunk(&self, tag: &[u8; 4], duplicate: &'static str) -> Result<Option<&Chunk>, Error> {
        if self.version != 1800 {
            return Err(Error::UnsupportedVersion(self.version));
        }
        let mut chunks = self.chunks.iter().filter(|chunk| &chunk.tag == tag);
        let first = chunks.next();
        if chunks.next().is_some() {
            return Err(Error::UnsupportedVersionedChunk {
                version: self.version,
                chunk: duplicate,
            });
        }
        Ok(first)
    }

    /// Borrow BPOS count/rows without assigning matrix or native pose semantics.
    /// Local v1800 policy: at most one chunk, exactly 4 + count * 48 payload bytes.
    pub fn bind_pose_stream_v1800<'a>(
        &self,
        data: &'a [u8],
    ) -> Result<Option<BindPoseStream1800<'a>>, Error> {
        let Some(chunk) = self.joint_chunk(b"BPOS", "duplicate BPOS")? else {
            return Ok(None);
        };
        let payload = span(data, chunk.payload_offset, chunk.size, "BPOS payload")?;
        let count = read_u32(payload, 0)?;
        let measured = (count as usize)
            .checked_mul(BIND_POSE_SIZE)
            .and_then(|n| n.checked_add(4))
            .ok_or(Error::RecordShape {
                chunk: "BPOS",
                at: chunk.payload_offset,
                size: chunk.size,
                measured: usize::MAX,
            })?;
        if measured != payload.len() {
            return Err(Error::RecordShape {
                chunk: "BPOS",
                at: chunk.payload_offset,
                size: payload.len(),
                measured,
            });
        }
        Ok(Some(BindPoseStream1800 {
            version: self.version,
            offset: chunk.offset,
            payload_offset: chunk.payload_offset,
            count,
            data: payload,
        }))
    }

    fn joint_geoset<'g>(
        &self,
        data: &[u8],
        g: &'g Geoset,
    ) -> Result<(usize, GeosetStreams1800<'g>), Error> {
        let chunk = self.joint_chunk(tag::GEOS, "duplicate GEOS")?.ok_or(
            Error::UnsupportedVersionedChunk {
                version: self.version,
                chunk: "missing GEOS",
            },
        )?;
        let payload = span(data, chunk.payload_offset, chunk.size, "GEOS payload")?;
        let record = span(payload, g.offset, g.size as usize, "GEOS record")?;
        if read_u32(record, 0)? != g.size || record.get(4..) != Some(g.data.as_slice()) {
            return Err(Error::UnsupportedVersionedChunk {
                version: self.version,
                chunk: "GEOS source mismatch",
            });
        }
        let base = chunk.payload_offset + g.offset + 4;
        Ok((base, self.geoset_streams_v1800(g)?))
    }

    fn joint_palette<'a>(
        &self,
        data: &'a [u8],
        base: usize,
        streams: &GeosetStreams1800<'_>,
    ) -> Result<Option<MatrixPalette1800<'a>>, Error> {
        let mut blocks = streams.blocks.iter().filter(|block| &block.tag == b"MATS");
        let Some(block) = blocks.next() else {
            return Ok(None);
        };
        if let Some(second) = blocks.next() {
            return Err(Error::DuplicateGeosetTag {
                at: second.offset,
                tag: *b"MATS",
            });
        }
        let offset = base + block.offset;
        let payload_offset = offset + 8;
        let size = (block.count as usize)
            .checked_mul(4)
            .ok_or(Error::RecordShape {
                chunk: "MATS",
                at: offset,
                size: block.body.len(),
                measured: usize::MAX,
            })?;
        let raw = span(data, payload_offset, size, "MATS words")?;
        Ok(Some(MatrixPalette1800 {
            version: self.version,
            geoset_offset: base - 4,
            offset,
            payload_offset,
            count: block.count,
            data: raw,
        }))
    }

    /// Borrow the single MATS block from the model input, with absolute offsets.
    /// Its object-ID labeling is primary convention, not native-verified.
    pub fn matrix_palette_v1800<'a>(
        &self,
        data: &'a [u8],
        g: &Geoset,
    ) -> Result<Option<MatrixPalette1800<'a>>, Error> {
        let (base, streams) = self.joint_geoset(data, g)?;
        self.joint_palette(data, base, &streams)
    }

    /// Prepare the explicit pinned primary-convention join, not GPU skinning.
    /// The source BONE inventory is parsed once. No direct-ID/default fallback,
    /// weight conversion, lane removal or duplicate merging is performed.
    pub fn primary_joint_join_v1800<'a>(
        &self,
        data: &'a [u8],
        g: &Geoset,
    ) -> Result<PrimaryJointJoin1800<'a>, Error> {
        let (base, streams) = self.joint_geoset(data, g)?;
        let palette = self.joint_palette(data, base, &streams)?;
        let poses = self.bind_pose_stream_v1800(data)?;
        let mut bones = BTreeMap::new();
        if let Some(chunk) = self.joint_chunk(tag::BONE, "duplicate BONE")? {
            let payload = span(data, chunk.payload_offset, chunk.size, "BONE payload")?;
            let mut at = 0;
            while at < payload.len() {
                let node_size = read_u32(payload, at)? as usize;
                let size =
                    node_size
                        .checked_add(BONE_TRAILER)
                        .ok_or(Error::BoneRecordOverruns {
                            at,
                            size: node_size,
                            left: payload.len() - at,
                        })?;
                if size < BONE_FIXED + BONE_TRAILER {
                    return Err(Error::RecordShape {
                        chunk: "BONE",
                        at: chunk.payload_offset + at,
                        size,
                        measured: BONE_FIXED + BONE_TRAILER,
                    });
                }
                // IDs need only the bounded source node header. Track bytes stay
                // opaque; the unchanged classic animation reader is not invoked.
                let record = span(payload, at, size, "BONE record")?;
                let object_id = read_u32(record, 4 + NODE_NAME)?;
                let offset = chunk.payload_offset + at + 4 + NODE_NAME;
                bones
                    .entry(object_id)
                    .and_modify(|offset| *offset = None)
                    .or_insert(Some(offset));
                at += size;
            }
        }
        let skin_offset = streams
            .skin
            .as_ref()
            .map_or(base, |skin| base + skin.offset + 8);
        let skin = streams
            .skin
            .map(|skin| -> Result<_, Error> {
                Ok(SkinStream1800 {
                    offset: skin.offset,
                    word_count: skin.word_count,
                    data: span(data, skin_offset, skin.data.len(), "SKIN words")?,
                })
            })
            .transpose()?;
        Ok(PrimaryJointJoin1800 {
            geoset_offset: base - 4,
            skin_offset,
            skin,
            palette,
            poses,
            bones,
        })
    }
}
