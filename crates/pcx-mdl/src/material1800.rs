//! v1800 stored material framing; see REFORGED_MTLS.md for primary/byte evidence.
use super::{Error, Interpolation, Material, Mdlx, TexLayer, read_f32, read_u32, tag};

/// Raw-backed track framing, without evaluating shader or interpolation semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialTrack1800 {
    /// Header offset within Material.data.
    pub offset: usize,
    pub magic: [u8; 4],
    pub count: u32,
    pub interpolation: Interpolation,
    pub global_seq_id: u32,
    pub stride: usize,
    /// Exact keys, including authored times, values and any tangents.
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubTexture1800 {
    /// Texture word offset within Material.data; ordinal is its vector position.
    pub offset: usize,
    pub texture_id: u32,
    /// Authored second word, preserved without replacing it with an ordinal.
    pub stored_semantic: u32,
    pub texture_track: Option<MaterialTrack1800>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialLayer1800 {
    /// Inclusive layer size word's offset within Material.data.
    pub offset: usize,
    pub size: u32,
    /// Stored classic prefix. Its texture_id is not the subtexture binding.
    pub prefix: TexLayer,
    pub emissive_gain: f32,
    pub fresnel_color: [f32; 3],
    pub fresnel_opacity: f32,
    pub fresnel_team_color: f32,
    /// Authored shader identifier, without selecting a render implementation.
    pub shader: u32,
    pub subtextures: Vec<SubTexture1800>,
    /// Layer-level tracks in original order, with source offsets and exact keys.
    pub tracks: Vec<MaterialTrack1800>,
}

fn magic(p: &[u8], at: usize, base: usize) -> Result<[u8; 4], Error> {
    p.get(at..at + 4)
        .ok_or(Error::TrackOverruns {
            chunk: "LAYS",
            at: base + at,
            size: 4,
            left: p.len().saturating_sub(at),
        })
        .map(|bytes| bytes.try_into().unwrap())
}

fn track(p: &[u8], at: usize, base: usize) -> Result<(MaterialTrack1800, usize), Error> {
    let magic = magic(p, at, base)?;
    let components = match &magic {
        b"KMTA" | b"KMTE" | b"KFCA" | b"KFTC" | b"KMTF" => 1,
        b"KFC3" => 3,
        _ => {
            return Err(Error::UnknownTrackMagic {
                chunk: "LAYS",
                at: base + at,
                tag: magic,
            });
        }
    };
    let head = p.get(at..at + 16).ok_or(Error::TrackOverruns {
        chunk: "LAYS",
        at: base + at,
        size: 16,
        left: p.len().saturating_sub(at),
    })?;
    let count = read_u32(head, 4)?;
    let kind = read_u32(head, 8)?;
    let interpolation = Interpolation::from_u32(kind).ok_or(Error::UnknownTrackInterpolation {
        chunk: "LAYS",
        at: base + at,
        kind,
    })?;
    // The primary integer reader consumes time/value pairs regardless of kind;
    // retain the authored header instead of applying its interpolation rewrite.
    let values = if &magic != b"KMTF" && interpolation.has_tangents() {
        3
    } else {
        1
    };
    let stride = 4 + components * 4 * values;
    let end = (count as usize)
        .checked_mul(stride)
        .and_then(|width| at.checked_add(16)?.checked_add(width))
        .filter(|end| *end <= p.len())
        .ok_or(Error::TrackOverruns {
            chunk: "LAYS",
            at: base + at,
            size: count as usize,
            left: p.len().saturating_sub(at),
        })?;
    Ok((
        MaterialTrack1800 {
            offset: base + at,
            magic,
            count,
            interpolation,
            global_seq_id: read_u32(head, 12)?,
            stride,
            raw: p[at + 16..end].to_vec(),
        },
        end,
    ))
}

impl Material {
    /// Decode the established v1800 layout; the classic accessors stay v800.
    /// All offsets refer to this Material.data, which retains the entire record.
    pub fn checked_layers_v1800(&self) -> Result<Vec<MaterialLayer1800>, Error> {
        self.priority_plane()?;
        self.flags()?;
        let p = &self.data;
        let tag = magic(p, 8, 0)?;
        if &tag != b"LAYS" {
            return Err(Error::UnknownTrackMagic {
                chunk: "MTLS",
                at: 8,
                tag,
            });
        }
        let count = read_u32(p, 12)?;
        let mut at = 16;
        let mut layers = Vec::new();
        for _ in 0..count {
            let size = read_u32(p, at)? as usize;
            let end = at
                .checked_add(size)
                .filter(|end| size >= 60 && *end <= p.len())
                .ok_or(Error::RecordShape {
                    chunk: "LAYS",
                    at,
                    size,
                    measured: 60,
                })?;
            let rec = &p[at..end];
            let mut layer = MaterialLayer1800 {
                offset: at,
                size: size as u32,
                prefix: TexLayer {
                    index: layers.len() as u32,
                    shader_id: read_u32(rec, 4)?,
                    blend_mode: read_u32(rec, 8)?,
                    filter_mode: read_u32(rec, 4)?,
                    shading_flags: read_u32(rec, 8)?,
                    texture_id: read_u32(rec, 12)?,
                    texture_anim_id: read_u32(rec, 16)?,
                    coord_id: read_u32(rec, 20)?,
                    static_alpha: read_f32(rec, 24)?,
                },
                emissive_gain: read_f32(rec, 28)?,
                fresnel_color: [read_f32(rec, 32)?, read_f32(rec, 36)?, read_f32(rec, 40)?],
                fresnel_opacity: read_f32(rec, 44)?,
                fresnel_team_color: read_f32(rec, 48)?,
                shader: read_u32(rec, 52)?,
                subtextures: Vec::new(),
                tracks: Vec::new(),
            };
            let texture_count = read_u32(rec, 56)?;
            let mut cur = 60;
            for _ in 0..texture_count {
                let fields = rec.get(cur..cur + 8).ok_or(Error::TrackOverruns {
                    chunk: "LAYS",
                    at: at + cur,
                    size: 8,
                    left: rec.len().saturating_sub(cur),
                })?;
                let mut texture = SubTexture1800 {
                    offset: at + cur,
                    texture_id: read_u32(fields, 0)?,
                    stored_semantic: read_u32(fields, 4)?,
                    texture_track: None,
                };
                cur += 8;
                if rec.get(cur..cur + 4) == Some(b"KMTF") {
                    let (parsed, end) = track(rec, cur, at)?;
                    texture.texture_track = Some(parsed);
                    cur = end;
                }
                layer.subtextures.push(texture);
            }
            while cur < rec.len() {
                let (parsed, end) = track(rec, cur, at)?;
                layer.tracks.push(parsed);
                cur = end;
            }
            layers.push(layer);
            at = end;
        }
        if at != p.len() {
            return Err(Error::RecordShape {
                chunk: "MTLS",
                at: 0,
                size: p.len(),
                measured: at,
            });
        }
        Ok(layers)
    }
}

impl Mdlx {
    /// Read v1800 materials without projecting them into the v800 layer API.
    /// Use Material::checked_layers_v1800 on the returned raw-backed records.
    pub fn materials_v1800(&self, data: &[u8]) -> Result<Vec<Material>, Error> {
        if self.version != 1800 {
            return Err(Error::UnsupportedVersionedChunk {
                version: self.version,
                chunk: "MTLS",
            });
        }
        let Some(p) = self.payload(data, tag::MTLS) else {
            return Ok(Vec::new());
        };
        let mut at = 0;
        let mut materials = Vec::new();
        while at < p.len() {
            let size = read_u32(p, at)? as usize;
            let end = at
                .checked_add(size)
                .filter(|end| size >= 4 && *end <= p.len())
                .ok_or(Error::RaggedRecords {
                    chunk: "MTLS",
                    size: p.len(),
                    stride: size,
                })?;
            let material = Material {
                offset: at,
                len: size as u32,
                data: p[at + 4..end].to_vec(),
            };
            material.checked_layers_v1800()?;
            materials.push(material);
            at = end;
        }
        Ok(materials)
    }
}
