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
