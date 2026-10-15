//! Checked first-layer TXAN translation, not a general rotation/scale implementation.
use crate::mdx_runtime::RuntimePlan;
use pcx_mdl::{BoneTrack, KeyTrack, NO_ID, TrackKind};

#[derive(Debug, thiserror::Error)]
#[error("{context}: {reason}")]
pub struct Error {
    pub context: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvTranslation {
    pub coord_id: u32,
    pub offset: [f32; 2],
}

impl UvTranslation {
    /// Apply to the authored UV, never to the previous frame's transformed UV.
    pub fn apply(self, uv: [f32; 2]) -> [f32; 2] {
        [uv[0] + self.offset[0], uv[1] + self.offset[1]]
    }
}

#[derive(Debug)]
struct Channel {
    material: usize,
    animation: u32,
    coord_id: u32,
    track: Option<BoneTrack>,
    global_duration: Option<u32>,
}

fn checked_track(
    source: &KeyTrack,
    interval: [u32; 2],
    globals: &[u32],
) -> Result<(Option<BoneTrack>, Option<u32>), String> {
    if source.magic != *b"KTAT" {
        return Err(format!(
            "unsupported TXAN track {:?}; translation only",
            source.magic
        ));
    }
    let stride = 4 + 3
        * 4
        * if source.interpolation.has_tangents() {
            3
        } else {
            1
        };
    if source.width != 3
        || source.stride != stride
        || (source.count as usize).checked_mul(stride) != Some(source.raw.len())
    {
        return Err("malformed KTAT width, stride or count".into());
    }
    let mut previous = None;
    for key in source.raw.chunks_exact(stride) {
        let frame = u32::from_le_bytes(key[..4].try_into().unwrap());
        if frame > i32::MAX as u32 {
            return Err("TXAN time exceeds supported nonnegative i32 domain".into());
        }
        if previous.is_some_and(|previous| previous >= frame) {
            return Err("duplicate or unordered KTAT keys".into());
        }
        previous = Some(frame);
        if key[4..]
            .as_chunks::<4>()
            .0
            .iter()
            .any(|value| !f32::from_le_bytes(*value).is_finite())
        {
            return Err("nonfinite KTAT value or tangent".into());
        }
    }
    let global_duration = if source.global_seq_id == NO_ID {
        None
    } else {
        let duration = globals
            .get(source.global_seq_id as usize)
            .copied()
            .ok_or("missing KTAT global sequence")?;
        if duration == 0 {
            return Err("zero-duration KTAT global sequence".into());
        }
        Some(duration)
    };
    let raw: Vec<_> = source
        .raw
        .chunks_exact(stride)
        .filter(|key| {
            let frame = u32::from_le_bytes(key[..4].try_into().unwrap());
            global_duration.is_some() || (interval[0]..=interval[1]).contains(&frame)
        })
        .flatten()
        .copied()
        .collect();
    let count = u32::try_from(raw.len() / stride).map_err(|_| "KTAT key count exceeds u32")?;
    let track = (count != 0).then_some(BoneTrack {
        kind: TrackKind::Translation,
        count,
        interpolation: source.interpolation,
        global_sequence_id: i32::from_le_bytes(source.global_seq_id.to_le_bytes()),
        offset: 0,
        stride,
        raw,
    });
    Ok((track, global_duration))
}

/// Source-derived UV-set counts must be parallel to authored GEOS ordinals.
/// The current mesh accessor retains the last UVBS; only single-set inputs are
/// admitted until callers can explicitly select another authored UVBS.
#[derive(Debug)]
pub struct UvTranslationPlan {
    source: String,
    channels: Vec<Channel>,
}

impl UvTranslationPlan {
    /// Use counts retained by the source mesh parser, never a caller's guessed
    /// single-set count. Typed-only RuntimeInput has no source metadata.
    pub fn from_runtime(
        plan: &RuntimePlan,
        interval: [u32; 2],
        source: &str,
    ) -> Result<Self, Error> {
        if plan.uv_set_counts.len() != plan.geosets.len() {
            return Err(Error {
                context: source.into(),
                reason: "UV-set metadata/GEOS count mismatch".into(),
            });
        }
        let counts = plan
            .uv_set_counts
            .iter()
            .enumerate()
            .map(|(geoset, count)| {
                count.ok_or_else(|| Error {
                    context: format!("{source} geoset {geoset}"),
                    reason: "UV-set metadata unavailable".into(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::checked(plan, interval, &counts, source)
    }

    pub fn checked(
        plan: &RuntimePlan,
        interval: [u32; 2],
        uv_set_counts: &[u32],
        source: &str,
    ) -> Result<Self, Error> {
        let fail = |reason: &str| Error {
            context: source.into(),
            reason: reason.into(),
        };
        if interval[1] < interval[0] {
            return Err(fail("reversed sequence interval"));
        }
        if uv_set_counts.len() != plan.geosets.len() {
            return Err(fail("UV-set metadata/GEOS count mismatch"));
        }
        let mut channels = Vec::with_capacity(plan.geosets.len());
        for (geoset, binding) in plan.geosets.iter().enumerate() {
            let mut context = format!(
                "{source} geoset {geoset} material {} layer 0",
                binding.material_id
            );
            let build = || -> Result<Channel, String> {
                if binding.geoset_id != geoset {
                    return Err("inconsistent GEOS ordinal".into());
                }
                let layer = plan
                    .materials
                    .get(binding.material_id)
                    .and_then(|material| material.layers.first())
                    .ok_or("missing first material layer")?;
                let properties = layer.properties;
                if properties.coord_id >= uv_set_counts[geoset] {
                    return Err("missing authored UV coordinate set".into());
                }
                if uv_set_counts[geoset] != 1 {
                    return Err("multiple UV sets unsupported by current mesh projection".into());
                }
                let (track, global_duration) = if properties.texture_anim_id == NO_ID {
                    (None, None)
                } else {
                    let animation = plan
                        .texture_animations
                        .get(properties.texture_anim_id as usize)
                        .ok_or("missing TXAN record")?;
                    match animation.tracks.as_slice() {
                        [] => (None, None),
                        [track] => checked_track(track, interval, &plan.global_sequences)?,
                        tracks if tracks.iter().any(|track| track.magic != *b"KTAT") => {
                            return Err("unsupported rotation/scale or unknown TXAN track; translation only".into());
                        }
                        _ => return Err("multiple KTAT tracks; precedence unsupported".into()),
                    }
                };
                Ok(Channel {
                    material: binding.material_id,
                    animation: properties.texture_anim_id,
                    coord_id: properties.coord_id,
                    track,
                    global_duration,
                })
            };
            let result = build();
            if let Some(layer) = plan
                .materials
                .get(binding.material_id)
                .and_then(|material| material.layers.first())
            {
                context.push_str(&format!(
                    " TXAN {} coord {}",
                    layer.properties.texture_anim_id, layer.properties.coord_id
                ));
            }
            channels.push(result.map_err(|reason| Error { context, reason })?);
        }
        Ok(Self {
            source: source.into(),
            channels,
        })
    }

    /// Endpoint clamping and local-key projection match the existing vector
    /// sampler. XY projection is corroborated by two reference renderers;
    /// neither that nor the caller's elapsed origin establishes native behavior.
    pub fn sample(
        &self,
        geoset: usize,
        frame: u32,
        elapsed_ms: u64,
    ) -> Result<Option<UvTranslation>, Error> {
        let Some(channel) = self.channels.get(geoset) else {
            return Ok(None);
        };
        let frame = channel
            .global_duration
            .map_or(frame, |duration| (elapsed_ms % u64::from(duration)) as u32);
        let translation = channel.track.as_ref().map_or([0.0; 3], |track| {
            pcx_mdl::sample_bone_track(track, frame).translation
        });
        if !translation.into_iter().all(f32::is_finite) {
            return Err(Error {
                context: format!(
                    "{} geoset {geoset} material {} layer 0 TXAN {}",
                    self.source, channel.material, channel.animation
                ),
                reason: "nonfinite sampled KTAT translation".into(),
            });
        }
        Ok(Some(UvTranslation {
            coord_id: channel.coord_id,
            offset: [translation[0], translation[1]],
        }))
    }
}
