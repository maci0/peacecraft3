//! Explicit script-name bindings to qualified local lighting-model bytes.
use crate::neutral_player::{Sha256Digest, sha256};
use crate::scripts::{AssetIdentity, ScriptAsset};
use pcx_mdl::{Light, Mdlx};

pub struct ModelInput<'a> {
    /// Exact script spelling; no extension, case, namespace or archive fallback.
    pub script_path: &'a str,
    pub asset: ScriptAsset<'a>,
    pub origin: AssetIdentity,
}
/// Logical input bounds, not native game limits or a total parser-heap promise.
#[derive(Clone, Copy)]
pub struct Budget {
    pub models: usize,
    pub model_bytes: usize,
    pub total_bytes: usize,
    /// Aggregate script paths, source labels and identity fields, in UTF-8 bytes.
    pub path_bytes: usize,
}
#[derive(Debug, thiserror::Error)]
#[error("day/night input {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input count, byte or platform-layout budget exceeded")]
    Budget,
    #[error("empty source identity, script path or source label")]
    Identity,
    #[error("duplicate script-path binding")]
    Duplicate,
    #[error("MDX lighting decode: {0}")]
    Decode(#[source] pcx_mdl::Error),
    #[error("model has no light records")]
    MissingLights,
    #[error("lighting profile does not support MDX version {0}")]
    Version(u32),
    #[error("repeated model chunk {0:?} is ambiguous")]
    DuplicateChunk([u8; 4]),
}
#[derive(Debug)]
pub struct BoundModel {
    script_path: String,
    source_label: String,
    origin: AssetIdentity,
    sha256: Sha256Digest,
    bytes: Vec<u8>,
    lights: Vec<Light>,
}
impl BoundModel {
    pub fn script_path(&self) -> &str {
        &self.script_path
    }
    pub fn source_label(&self) -> &str {
        &self.source_label
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn sha256(&self) -> Sha256Digest {
        self.sha256
    }
    /// Complete source model, retaining sequence/global clocks, pivots and all chunks.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Authored values and animation tracks; no channel, clock or light-unit conversion.
    pub fn lights(&self) -> &[Light] {
        &self.lights
    }
}
#[derive(Debug)]
pub struct Assets {
    models: Vec<BoundModel>,
    path_limit: usize,
}
impl Assets {
    /// Checks the whole input set before any model decoding or source-byte copies.
    pub fn read(inputs: &[ModelInput<'_>], budget: Budget) -> Result<Self, Error> {
        let fail = |source: &str, problem| Error {
            source_context: source.into(),
            problem,
        };
        std::alloc::Layout::array::<BoundModel>(budget.models)
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.total_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.model_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.path_bytes))
            .map_err(|_| fail("selected model set", Problem::Budget))?;
        if inputs.len() > budget.models {
            return Err(fail("selected model set", Problem::Budget));
        }
        let mut total = 0usize;
        let mut paths = 0usize;
        for (i, input) in inputs.iter().enumerate() {
            let fields = [
                input.script_path,
                input.asset.label,
                &input.origin.source,
                &input.origin.edition,
                &input.origin.version,
            ];
            if fields.iter().any(|s| s.trim().is_empty()) {
                return Err(fail(input.asset.label, Problem::Identity));
            }
            if inputs[..i]
                .iter()
                .any(|old| old.script_path == input.script_path)
            {
                return Err(fail(input.asset.label, Problem::Duplicate));
            }
            for field in fields {
                paths = paths
                    .checked_add(field.len())
                    .filter(|&n| n <= budget.path_bytes)
                    .ok_or_else(|| fail(input.asset.label, Problem::Budget))?;
            }
            total = total
                .checked_add(input.asset.bytes.len())
                .filter(|&n| n <= budget.total_bytes)
                .ok_or_else(|| fail(input.asset.label, Problem::Budget))?;
            if input.asset.bytes.len() > budget.model_bytes {
                return Err(fail(input.asset.label, Problem::Budget));
            }
        }
        let mut models = Vec::new();
        models
            .try_reserve_exact(inputs.len())
            .map_err(|_| fail("selected model set", Problem::Budget))?;
        for input in inputs {
            let decode = |e| fail(input.asset.label, Problem::Decode(e));
            let model = Mdlx::parse(input.asset.bytes).map_err(decode)?;
            if model.version != 800 {
                return Err(fail(input.asset.label, Problem::Version(model.version)));
            }
            let mut tags = std::collections::BTreeSet::new();
            for chunk in &model.chunks {
                if !tags.insert(chunk.tag) {
                    return Err(fail(input.asset.label, Problem::DuplicateChunk(chunk.tag)));
                }
            }
            let lights = model.lights(input.asset.bytes).map_err(decode)?;
            if lights.is_empty() {
                return Err(fail(input.asset.label, Problem::MissingLights));
            }
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(input.asset.bytes.len())
                .map_err(|_| fail(input.asset.label, Problem::Budget))?;
            bytes.extend_from_slice(input.asset.bytes);
            models.push(BoundModel {
                script_path: input.script_path.into(),
                source_label: input.asset.label.into(),
                origin: input.origin.clone(),
                sha256: sha256(&bytes),
                bytes,
                lights,
            });
        }
        Ok(Self {
            models,
            path_limit: budget.path_bytes,
        })
    }
    pub fn models(&self) -> &[BoundModel] {
        &self.models
    }
    pub(crate) fn path_limit(&self) -> usize {
        self.path_limit
    }
}

#[cfg(test)]
#[path = "day_night_tests.rs"]
mod tests;

/// One sampled directional sun: travel direction in game space (z up), with
/// raw colours and intensities for the client's light submission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    pub direction: [f32; 3],
    pub colour: [f32; 3],
    pub intensity: f32,
    pub ambient_colour: [f32; 3],
    pub ambient_intensity: f32,
}

impl BoundModel {
    /// The model's single directional light at `day_fraction` (0..1 of the day)
    /// along its first sequence. Colour (`KLAC`) and ambient (`KLBC`) tracks are
    /// sampled at that frame; the rotation must be one fixed `KGRT` key. The
    /// light travels along its node's local -Z, the export convention the
    /// authored key turns downward (an inference, not a native trace).
    pub fn sun(&self, day_fraction: f32) -> Result<Sun, String> {
        let fail = |reason: &str| format!("{}: day/night sun: {reason}", self.script_path);
        let [light] = self.lights.as_slice() else {
            return Err(fail("expected exactly one light"));
        };
        if light.kind != pcx_mdl::LightKind::Directional || light.parent_id != -1 {
            return Err(fail("light is not an unparented directional light"));
        }
        if !day_fraction.is_finite() || !(0.0..=1.0).contains(&day_fraction) {
            return Err(fail("day fraction outside 0..1"));
        }
        let model = Mdlx::parse(&self.bytes).map_err(|e| fail(&e.to_string()))?;
        let sequence = model
            .sequences(&self.bytes)
            .map_err(|e| fail(&e.to_string()))?
            .into_iter()
            .next()
            .ok_or_else(|| fail("no day sequence"))?;
        let span = sequence
            .interval_end
            .saturating_sub(sequence.interval_start);
        let frame = sequence.interval_start + (f64::from(span) * f64::from(day_fraction)) as u32;
        let mut diffuse = light.colour;
        let mut ambient = light.ambient_colour;
        let mut rotation = None;
        for track in light.node_tracks.iter().chain(&light.tracks) {
            let anim = pcx_mdl::AnimTrack {
                magic: track.magic,
                count: track.count,
                interpolation: track.interpolation,
                global_seq_id: track.global_seq_id,
                raw: track.raw.clone(),
            };
            match &track.magic {
                b"KLAC" => diffuse = pcx_mdl::sample_colour(&anim, frame),
                b"KLBC" => ambient = pcx_mdl::sample_colour(&anim, frame),
                b"KGRT" if track.count == 1 && track.raw.len() >= 20 => {
                    let (q, _) = track.raw[4..20].as_chunks::<4>();
                    rotation = Some(std::array::from_fn(|i| f32::from_le_bytes(q[i])));
                }
                _ => return Err(fail("unsupported light track")),
            }
        }
        let [x, y, z, w] = rotation.ok_or_else(|| fail("missing fixed rotation key"))?;
        // Rotate (0, 0, -1) by the unit quaternion.
        let direction = [
            -2.0 * (x * z + w * y),
            -2.0 * (y * z - w * x),
            -(1.0 - 2.0 * (x * x + y * y)),
        ];
        let sun = Sun {
            direction,
            colour: diffuse,
            intensity: light.intensity,
            ambient_colour: ambient,
            ambient_intensity: light.ambient_intensity,
        };
        if sun
            .direction
            .iter()
            .chain(&sun.colour)
            .chain(&sun.ambient_colour)
            .chain([&sun.intensity, &sun.ambient_intensity])
            .any(|v| !v.is_finite())
        {
            return Err(fail("nonfinite sampled light"));
        }
        Ok(sun)
    }
}
