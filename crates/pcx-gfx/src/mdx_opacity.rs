//! Authored first-layer/geoset opacity and source-profile geoset colour.
use crate::mdx_runtime::RuntimePlan;
use pcx_mdl::{AnimTrack, NO_ID};

#[derive(Debug, thiserror::Error)]
#[error("geoset {geoset}: unsupported or malformed opacity or colour: {reason}")]
pub struct Error {
    pub geoset: usize,
    pub reason: String,
}

#[derive(Debug)]
struct Curve {
    track: AnimTrack,
    global_duration: Option<u32>,
}

impl Curve {
    fn checked(
        tracks: &[AnimTrack],
        tag: &[u8; 4],
        interval: [u32; 2],
        globals: &[u32],
    ) -> Result<Option<Self>, String> {
        let label = if tag == b"KGAC" { "colour" } else { "alpha" };
        let mut matches = tracks.iter().filter(|track| &track.magic == tag);
        let Some(source) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Err(format!("multiple {tag:?} tracks; precedence unverified"));
        }
        let stride = source.key_len();
        if (source.count as usize).checked_mul(stride) != Some(source.raw.len()) {
            return Err(format!("{label} track size mismatch"));
        }
        let mut previous = None;
        for key in source.raw.chunks_exact(stride) {
            let frame = u32::from_le_bytes(key[..4].try_into().unwrap());
            if previous.is_some_and(|previous| frame < previous) {
                return Err(format!("unordered {label} keys"));
            }
            if previous == Some(frame) {
                return Err(format!(
                    "duplicate {tag:?} {label} keys at frame {frame}; precedence unverified"
                ));
            }
            previous = Some(frame);
            if key[4..]
                .as_chunks::<4>()
                .0
                .iter()
                .any(|bytes| !f32::from_le_bytes(*bytes).is_finite())
            {
                return Err(format!("nonfinite {label} key or tangent"));
            }
        }
        let global_duration = if source.global_seq_id == NO_ID {
            None
        } else {
            let duration = globals
                .get(source.global_seq_id as usize)
                .copied()
                .ok_or_else(|| format!("missing {label} global sequence"))?;
            if duration == 0 {
                return Err(format!("zero-duration {label} global sequence"));
            }
            Some(duration)
        };
        let mut track = source.clone();
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
        Ok((track.count != 0).then_some(Self {
            track,
            global_duration,
        }))
    }

    fn frame(&self, frame: u32, elapsed_ms: u64) -> u32 {
        self.global_duration
            .map_or(frame, |duration| (elapsed_ms % u64::from(duration)) as u32)
    }
}

#[derive(Debug)]
struct Channel {
    base: f32,
    curve: Option<Curve>,
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
        Ok(Self {
            base,
            curve: Curve::checked(tracks, tag, interval, globals)?,
        })
    }
    fn sample(&self, frame: u32, elapsed_ms: u64) -> f32 {
        self.curve.as_ref().map_or(self.base, |curve| {
            pcx_mdl::sample_scalar(&curve.track, curve.frame(frame, elapsed_ms))
        })
    }
}

#[derive(Debug)]
struct ColourChannel {
    base: [f32; 3],
    curve: Option<Curve>,
}
impl ColourChannel {
    fn checked(
        base: [f32; 3],
        tracks: &[AnimTrack],
        interval: [u32; 2],
        globals: &[u32],
    ) -> Result<Self, String> {
        if base.iter().any(|value| !value.is_finite()) {
            return Err("nonfinite static colour".into());
        }
        Ok(Self {
            base,
            curve: Curve::checked(tracks, b"KGAC", interval, globals)?,
        })
    }
    fn sample_material(&self, frame: u32, elapsed_ms: u64) -> [f32; 3] {
        let Some(curve) = &self.curve else {
            return self.base;
        };
        let raw = pcx_mdl::sample_colour(&curve.track, curve.frame(frame, elapsed_ms));
        // Warsmash f9e0aeed: static reverse cancels shader BGRA; KGAC needs that swizzle.
        [raw[2], raw[1], raw[0]]
    }
}

fn geoset_colour_channel(
    animation: Option<&pcx_mdl::GeosetAnim>,
    interval: [u32; 2],
    globals: &[u32],
) -> Result<ColourChannel, String> {
    match animation {
        None => ColourChannel::checked([1.0; 3], &[], interval, globals),
        Some(animation) => ColourChannel::checked(
            animation.static_colour,
            &animation.tracks,
            interval,
            globals,
        ),
    }
}

/// One entry per authored GEOS ordinal; GEOA record order is never used as an ID.
#[derive(Debug)]
pub struct OpacityPlan {
    channels: Vec<(Channel, Channel, ColourChannel)>,
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
                let animation = match binding.animation_records.as_slice() {
                    [] => None,
                    [index] => Some(
                        plan.geoset_animations
                            .get(*index)
                            .ok_or("missing GEOA record")?,
                    ),
                    _ => {
                        return Err(
                            "multiple GEOA records for one geoset; composition unverified".into(),
                        );
                    }
                };
                if let Some(animation) = animation
                    && animation.tracks.iter().any(|track| track.magic == *b"KGAA")
                {
                    return Err(
                        "KGAA semantics unverified; KGAO is the established alpha track".into(),
                    );
                }
                let geoset = match animation {
                    None => Channel::checked(1.0, &[], b"KGAO", interval, &plan.global_sequences)?,
                    Some(animation) => Channel::checked(
                        animation.static_alpha,
                        &animation.tracks,
                        b"KGAO",
                        interval,
                        &plan.global_sequences,
                    )?,
                };
                let colour = geoset_colour_channel(animation, interval, &plan.global_sequences)?;
                Ok((layer, geoset, colour))
            };
            channels.push(build().map_err(|reason| Error { geoset, reason })?);
        }
        Ok(Self { channels })
    }

    pub fn sample(&self, geoset: usize, frame: u32, elapsed_ms: u64) -> Option<f32> {
        self.channels.get(geoset).map(|(layer, geoset, _)| {
            layer.sample(frame, elapsed_ms) * geoset.sample(frame, elapsed_ms)
        })
    }

    /// Material RGB in the pinned Warsmash profile; refuses nonfinite interpolation results.
    pub fn sample_material_colour(
        &self,
        geoset: usize,
        frame: u32,
        elapsed_ms: u64,
    ) -> Result<[f32; 3], Error> {
        let (_, _, colour) = self.channels.get(geoset).ok_or_else(|| Error {
            geoset,
            reason: "missing colour binding".into(),
        })?;
        let rgb = colour.sample_material(frame, elapsed_ms);
        if !rgb.iter().all(|value| value.is_finite()) {
            return Err(Error {
                geoset,
                reason: "nonfinite interpolated colour".into(),
            });
        }
        Ok(rgb)
    }
}
