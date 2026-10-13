//! Version-800 material layers, framed by native readers 0x68fe90/0x68fdb0.
use super::{AnimTrack, Error, Interpolation, Material, TexLayer, read_f32, read_i32, read_u32};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureFrameTrack {
    pub interpolation: Interpolation,
    pub global_seq_id: u32,
    /// Signed key times and integer texture references; KMTF has no tangents.
    pub keys: Vec<(i32, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialLayer {
    /// Inclusive layer size word's offset in Material.data.
    pub offset: usize,
    pub size: u32,
    pub properties: TexLayer,
    pub alpha_tracks: Vec<AnimTrack>,
    pub texture_tracks: Vec<TextureFrameTrack>,
}

impl Material {
    /// Decode every layer, requiring all group counts, records and tracks to tile.
    pub fn checked_layers(&self) -> Result<Vec<MaterialLayer>, Error> {
        let p = &self.data;
        read_i32(p, 0)?; // priority plane
        read_u32(p, 4)?; // material flags
        let mut out = Vec::new();
        let mut at = 8;
        while at < p.len() {
            let magic: [u8; 4] = p
                .get(at..at + 4)
                .ok_or(Error::TrackOverruns {
                    chunk: "MTLS",
                    at,
                    size: 4,
                    left: p.len() - at,
                })?
                .try_into()
                .unwrap();
            if &magic != b"LAYS" {
                return Err(Error::UnknownTrackMagic {
                    chunk: "MTLS",
                    at,
                    tag: magic,
                });
            }
            let count = read_u32(p, at + 4)?;
            at += 8;
            for _ in 0..count {
                let size = read_u32(p, at)? as usize;
                let end = at.checked_add(size).ok_or(Error::RecordShape {
                    chunk: "LAYS",
                    at,
                    size,
                    measured: 28,
                })?;
                if size < 28 || end > p.len() {
                    return Err(Error::RecordShape {
                        chunk: "LAYS",
                        at,
                        size,
                        measured: 28,
                    });
                }
                let rec = &p[at..end];
                let properties = TexLayer {
                    index: out.len() as u32,
                    shader_id: read_u32(rec, 4)?,
                    blend_mode: read_u32(rec, 8)?,
                    filter_mode: read_u32(rec, 4)?,
                    shading_flags: read_u32(rec, 8)?,
                    texture_id: read_u32(rec, 12)?,
                    texture_anim_id: read_u32(rec, 16)?,
                    coord_id: read_u32(rec, 20)?,
                    static_alpha: read_f32(rec, 24)?,
                };
                let mut layer = MaterialLayer {
                    offset: at,
                    size: size as u32,
                    properties,
                    alpha_tracks: Vec::new(),
                    texture_tracks: Vec::new(),
                };
                let mut cur = 28;
                while cur < rec.len() {
                    let magic: [u8; 4] = rec
                        .get(cur..cur + 4)
                        .ok_or(Error::TrackOverruns {
                            chunk: "LAYS",
                            at: at + cur,
                            size: 4,
                            left: rec.len() - cur,
                        })?
                        .try_into()
                        .unwrap();
                    if &magic != b"KMTA" && &magic != b"KMTF" {
                        return Err(Error::UnknownTrackMagic {
                            chunk: "LAYS",
                            at: at + cur,
                            tag: magic,
                        });
                    }
                    let count = read_u32(rec, cur + 4)?;
                    let kind = read_u32(rec, cur + 8)?;
                    let interpolation =
                        Interpolation::from_u32(kind).ok_or(Error::UnknownTrackInterpolation {
                            chunk: "LAYS",
                            at: at + cur,
                            kind,
                        })?;
                    let global_seq_id = read_u32(rec, cur + 12)?;
                    let stride = if &magic == b"KMTA" && interpolation.has_tangents() {
                        16
                    } else {
                        8
                    };
                    let next = (count as usize)
                        .checked_mul(stride)
                        .and_then(|n| n.checked_add(16))
                        .and_then(|n| cur.checked_add(n))
                        .filter(|n| *n <= rec.len())
                        .ok_or(Error::TrackOverruns {
                            chunk: "LAYS",
                            at: at + cur,
                            size: count as usize,
                            left: rec.len() - cur,
                        })?;
                    let raw = &rec[cur + 16..next];
                    if &magic == b"KMTA" {
                        layer.alpha_tracks.push(AnimTrack {
                            magic,
                            count,
                            interpolation,
                            global_seq_id,
                            raw: raw.to_vec(),
                        });
                    } else {
                        let keys = raw
                            .chunks_exact(8)
                            .map(|key| {
                                (
                                    i32::from_le_bytes(key[..4].try_into().unwrap()),
                                    u32::from_le_bytes(key[4..].try_into().unwrap()),
                                )
                            })
                            .collect();
                        layer.texture_tracks.push(TextureFrameTrack {
                            interpolation,
                            global_seq_id,
                            keys,
                        });
                    }
                    cur = next;
                }
                out.push(layer);
                at = end;
            }
        }
        Ok(out)
    }
}

impl Material {
    pub fn priority_plane(&self) -> Result<i32, Error> {
        read_i32(&self.data, 0)
    }
    pub fn flags(&self) -> Result<u32, Error> {
        read_u32(&self.data, 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(alpha: f32, coord_id: u32, tracks: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        for word in [28 + tracks.len() as u32, 4, 16, 3, u32::MAX, coord_id] {
            data.extend(word.to_le_bytes());
        }
        data.extend(alpha.to_le_bytes());
        data.extend(tracks);
        data
    }
    fn material(records: &[Vec<u8>]) -> Material {
        let mut data = Vec::new();
        data.extend((-3i32).to_le_bytes());
        data.extend(5u32.to_le_bytes());
        data.extend(b"LAYS");
        data.extend((records.len() as u32).to_le_bytes());
        for record in records {
            data.extend(record);
        }
        Material {
            offset: 0,
            len: data.len() as u32 + 4,
            data,
        }
    }

    #[test]
    fn every_layer_is_decoded_and_alpha_is_not_the_coordinate_id() {
        let material = material(&[layer(0.25, 7, &[]), layer(0.75, 11, &[])]);
        let layers = material.checked_layers().unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].properties.coord_id, 7);
        assert_eq!(layers[0].properties.static_alpha, 0.25);
        assert_eq!(layers[1].properties.index, 1);
        assert_eq!(layers[1].properties.coord_id, 11);
        assert_eq!(layers[1].properties.static_alpha, 0.75);
        assert_eq!(material.priority_plane().unwrap(), -3);
        assert_eq!(material.flags().unwrap(), 5);
    }

    #[test]
    fn texture_frame_keys_are_integers_and_never_have_tangents() {
        let mut tracks = b"KMTF".to_vec();
        for word in [2u32, 3, 6] {
            tracks.extend(word.to_le_bytes());
        }
        for (frame, texture) in [(-12i32, 4u32), (32, 8)] {
            tracks.extend(frame.to_le_bytes());
            tracks.extend(texture.to_le_bytes());
        }
        tracks.extend(b"KMTA");
        for word in [1u32, 2, u32::MAX, 123] {
            tracks.extend(word.to_le_bytes());
        }
        for value in [0.5f32, 0.75, 0.25] {
            tracks.extend(value.to_le_bytes());
        }
        let material = material(&[layer(0.875, 2, &tracks)]);
        let layers = material.checked_layers().unwrap();
        assert_eq!(layers[0].texture_tracks[0].keys, [(-12, 4), (32, 8)]);
        assert_eq!(layers[0].texture_tracks[0].global_seq_id, 6);
        assert_eq!(layers[0].alpha_tracks[0].value(0), 0.5);
        assert_eq!(layers[0].alpha_tracks[0].in_tangent(0), 0.75);
        assert_eq!(layers[0].alpha_tracks[0].out_tangent(0), 0.25);
    }

    #[test]
    fn invalid_layer_counts_and_lengths_are_errors() {
        let mut material = material(&[layer(0.5, 0, &[])]);
        material.data[12..16].copy_from_slice(&2u32.to_le_bytes());
        assert!(material.checked_layers().is_err());
        material.data[12..16].copy_from_slice(&1u32.to_le_bytes());
        material.data[16..20].copy_from_slice(&24u32.to_le_bytes());
        assert!(matches!(
            material.checked_layers(),
            Err(Error::RecordShape { .. })
        ));
    }
}
