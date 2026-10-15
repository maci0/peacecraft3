//! Checked first-layer KMTF selection; returns authored TEXS IDs, not image paths.
use crate::mdx_runtime::RuntimePlan;
use pcx_mdl::{Interpolation, NO_ID};

#[derive(Debug, thiserror::Error)]
#[error("texture selection {context}: {reason}")]
pub struct Error {
    pub context: String,
    pub reason: String,
}

#[derive(Debug)]
struct Channel {
    base: u32,
    keys: Vec<(i32, u32)>,
    global_duration: Option<u32>,
}

impl Channel {
    fn sample(&self, frame: u32, elapsed_ms: u64) -> u32 {
        let frame = self
            .global_duration
            .map_or(frame, |duration| (elapsed_ms % u64::from(duration)) as u32);
        let after = self
            .keys
            .partition_point(|&(time, _)| i64::from(time) <= i64::from(frame));
        // Endpoint convention matches the existing scalar sampler: clamp to the
        // first applicable key before its time, and hold the last afterwards.
        self.keys
            .get(after.saturating_sub(1))
            .map_or(self.base, |&(_, texture)| texture)
    }
}

/// One selection channel per authored geoset ordinal, using only its first layer.
/// Local keys are projected to the selected clip; global keys keep their own clock.
#[derive(Debug)]
pub struct TextureSelectionPlan {
    channels: Vec<Channel>,
}

impl TextureSelectionPlan {
    pub fn checked(plan: &RuntimePlan, interval: [u32; 2]) -> Result<Self, Error> {
        if interval[1] < interval[0] {
            return Err(Error {
                context: "clip".into(),
                reason: "reversed sequence interval".into(),
            });
        }
        let mut channels = Vec::with_capacity(plan.geosets.len());
        for (ordinal, binding) in plan.geosets.iter().enumerate() {
            let build = || -> Result<Channel, String> {
                if binding.geoset_id != ordinal {
                    return Err(format!("GEOS ordinal is {ordinal}"));
                }
                let layer = plan
                    .materials
                    .get(binding.material_id)
                    .ok_or("missing material")?
                    .layers
                    .first()
                    .ok_or("missing first material layer")?;
                let base = layer.properties.texture_id;
                let texture_exists = |texture: u32| -> Result<(), String> {
                    if plan.textures.get(texture as usize).is_none() {
                        return Err(format!("missing TEXS record {texture}"));
                    }
                    Ok(())
                };
                texture_exists(base)?;
                if layer.texture_tracks.len() > 1 {
                    return Err("multiple KMTF tracks; precedence unsupported".into());
                }
                let Some(track) = layer.texture_tracks.first() else {
                    return Ok(Channel {
                        base,
                        keys: Vec::new(),
                        global_duration: None,
                    });
                };
                if track.interpolation != Interpolation::None {
                    return Err(format!(
                        "unsupported KMTF interpolation {:?}; only STEP is implemented",
                        track.interpolation
                    ));
                }
                let mut previous = None;
                for &(time, texture) in &track.keys {
                    if previous == Some(time) {
                        return Err(format!("duplicate KMTF key at frame {time}"));
                    }
                    if previous.is_some_and(|previous| time < previous) {
                        return Err(format!("unordered KMTF key at frame {time}"));
                    }
                    previous = Some(time);
                    texture_exists(texture)?;
                }
                let global_duration = if track.global_seq_id == NO_ID {
                    None
                } else {
                    let duration = plan
                        .global_sequences
                        .get(track.global_seq_id as usize)
                        .copied()
                        .ok_or("missing KMTF global sequence")?;
                    if duration == 0 {
                        return Err("zero-duration KMTF global sequence".into());
                    }
                    Some(duration)
                };
                let keys = track
                    .keys
                    .iter()
                    .copied()
                    .filter(|&(time, _)| {
                        global_duration.is_some()
                            || (i64::from(time) >= i64::from(interval[0])
                                && i64::from(time) <= i64::from(interval[1]))
                    })
                    .collect();
                Ok(Channel {
                    base,
                    keys,
                    global_duration,
                })
            };
            channels.push(build().map_err(|reason| Error {
                context: format!(
                    "geoset {} material {} layer 0",
                    binding.geoset_id, binding.material_id
                ),
                reason,
            })?);
        }
        Ok(Self { channels })
    }

    /// Select at spawn as well as advance. `elapsed_ms` never wraps with the clip.
    /// Missing geosets return None; an applicable channel preserves integer IDs.
    pub fn sample(&self, geoset: usize, frame: u32, elapsed_ms: u64) -> Option<u32> {
        self.channels
            .get(geoset)
            .map(|channel| channel.sample(frame, elapsed_ms))
    }
}
