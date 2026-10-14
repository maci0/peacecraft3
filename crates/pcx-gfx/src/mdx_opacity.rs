//! Authored first-layer/geoset opacity. Blend modes and native clocks are separate.
use crate::mdx_runtime::RuntimePlan;
use pcx_mdl::{AnimTrack, NO_ID};

#[derive(Debug, thiserror::Error)]
#[error("geoset {geoset}: unsupported or malformed opacity: {reason}")]
pub struct Error {
    pub geoset: usize,
    pub reason: String,
}

#[derive(Debug)]
struct Channel {
    base: f32,
    track: Option<AnimTrack>,
    global_duration: Option<u32>,
}

impl Channel {
    fn checked(
        base: f32,
        tracks: &[AnimTrack],
        tag: &[u8; 4],
        interval: [u32; 2],
        globals: &[u32],
    ) -> Result<Self, String> {
        if !base.is_finite() {
            return Err("nonfinite static alpha".into());
        }
        let matches: Vec<_> = tracks.iter().filter(|track| &track.magic == tag).collect();
        if matches.len() > 1 {
            return Err(format!("multiple {tag:?} tracks; precedence unverified"));
        }
        let Some(source) = matches.first() else {
            return Ok(Self {
                base,
                track: None,
                global_duration: None,
            });
        };
        let stride = source.key_len();
        if (source.count as usize).checked_mul(stride) != Some(source.raw.len()) {
            return Err("alpha track size mismatch".into());
        }
        let mut previous = None;
        for key in source.raw.chunks_exact(stride) {
            let frame = u32::from_le_bytes(key[..4].try_into().unwrap());
            if previous.is_some_and(|previous| frame < previous) {
                return Err("unordered alpha keys".into());
            }
            if previous == Some(frame) {
                return Err(format!(
                    "duplicate {tag:?} alpha keys at frame {frame}; precedence unverified"
                ));
            }
            previous = Some(frame);
            if key[4..]
                .as_chunks::<4>()
                .0
                .iter()
                .any(|bytes| !f32::from_le_bytes(*bytes).is_finite())
            {
                return Err("nonfinite alpha key or tangent".into());
            }
        }
        let global_duration = if source.global_seq_id == NO_ID {
            None
        } else {
            let duration = globals
                .get(source.global_seq_id as usize)
                .copied()
                .ok_or("missing alpha global sequence")?;
            if duration == 0 {
                return Err("zero-duration alpha global sequence".into());
            }
            Some(duration)
        };
        let mut track = (**source).clone();
        if global_duration.is_none() {
            track.raw = source
                .raw
                .chunks_exact(stride)
                .filter(|key| {
                    let frame = u32::from_le_bytes(key[..4].try_into().unwrap());
                    frame >= interval[0] && frame <= interval[1]
                })
                .flatten()
                .copied()
                .collect();
            track.count = (track.raw.len() / stride) as u32;
        }
        Ok(Self {
            base,
            track: (track.count != 0).then_some(track),
            global_duration,
        })
    }

    fn sample(&self, frame: u32, elapsed_ms: u64) -> f32 {
        self.track.as_ref().map_or(self.base, |track| {
            let frame = self
                .global_duration
                .map_or(frame, |duration| (elapsed_ms % u64::from(duration)) as u32);
            pcx_mdl::sample_scalar(track, frame)
        })
    }
}

/// One entry per authored GEOS ordinal; GEOA record order is never used as an ID.
#[derive(Debug)]
pub struct OpacityPlan {
    channels: Vec<(Channel, Channel)>,
}

impl OpacityPlan {
    pub fn checked(plan: &RuntimePlan, interval: [u32; 2]) -> Result<Self, Error> {
        let mut channels = Vec::with_capacity(plan.geosets.len());
        for binding in &plan.geosets {
            let geoset = binding.geoset_id;
            let build = || -> Result<_, String> {
                if interval[1] < interval[0] {
                    return Err("reversed sequence interval".into());
                }
                let layer = plan
                    .materials
                    .get(binding.material_id)
                    .ok_or("missing material")?
                    .layers
                    .first()
                    .ok_or("missing first material layer")?;
                let layer = Channel::checked(
                    layer.properties.static_alpha,
                    &layer.alpha_tracks,
                    b"KMTA",
                    interval,
                    &plan.global_sequences,
                )?;
                let geoset = match binding.animation_records.as_slice() {
                    [] => Channel::checked(1.0, &[], b"KGAO", interval, &plan.global_sequences)?,
                    [index] => {
                        let animation = plan
                            .geoset_animations
                            .get(*index)
                            .ok_or("missing GEOA record")?;
                        if animation.tracks.iter().any(|track| track.magic == *b"KGAA") {
                            return Err(
                                "KGAA semantics unverified; KGAO is the established alpha track"
                                    .into(),
                            );
                        }
                        Channel::checked(
                            animation.static_alpha,
                            &animation.tracks,
                            b"KGAO",
                            interval,
                            &plan.global_sequences,
                        )?
                    }
                    _ => {
                        return Err(
                            "multiple GEOA records for one geoset; composition unverified".into(),
                        );
                    }
                };
                Ok((layer, geoset))
            };
            channels.push(build().map_err(|reason| Error { geoset, reason })?);
        }
        Ok(Self { channels })
    }

    pub fn sample(&self, geoset: usize, frame: u32, elapsed_ms: u64) -> Option<f32> {
        self.channels.get(geoset).map(|(layer, geoset)| {
            layer.sample(frame, elapsed_ms) * geoset.sample(frame, elapsed_ms)
        })
    }
}
