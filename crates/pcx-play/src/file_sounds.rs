//! Exact caller-supplied filename bindings; no synthetic table rows or lookup priority.
use crate::label_sounds::{PcmResource, ResourceFailure, WaveError};
use crate::scripts::{AssetIdentity, ScriptAsset};
pub struct SoundInput<'a> {
    /// Exact script filename/member spelling, without case or extension fallback.
    pub member: &'a str,
    pub asset: ScriptAsset<'a>,
    pub origin: AssetIdentity,
}
#[derive(Clone, Copy)]
pub struct Budget {
    pub resources: usize,
    pub resource_bytes: usize,
    pub total_bytes: usize,
    /// Aggregate member names, source labels and identity fields.
    pub metadata_bytes: usize,
}
#[derive(Debug, thiserror::Error)]
#[error("sound file {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input count, byte or platform layout budget exceeded")]
    Budget,
    #[error("empty identity/member/source label or embedded NUL")]
    Identity,
    #[error("duplicate exact member binding")]
    Duplicate,
    #[error("{0}")]
    Wave(#[source] WaveError),
}
#[derive(Debug)]
pub struct Assets {
    resources: Vec<PcmResource>,
    path_limit: usize,
}
impl Assets {
    /// Whole-set budgets/identities precede all waveform parsing and source copies.
    /// These logical input limits do not promise a total heap or decoder-work bound.
    pub fn read(inputs: &[SoundInput<'_>], budget: Budget) -> Result<Self, Error> {
        let fail = |source: &str, problem| Error {
            source_context: source.into(),
            problem,
        };
        std::alloc::Layout::array::<PcmResource>(budget.resources)
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.resource_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.total_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.metadata_bytes))
            .map_err(|_| fail("selected file sounds", Problem::Budget))?;
        if inputs.len() > budget.resources {
            return Err(fail("selected file sounds", Problem::Budget));
        }
        let (mut total, mut metadata) = (0usize, 0usize);
        for (i, input) in inputs.iter().enumerate() {
            let fields = [
                input.member,
                input.asset.label,
                &input.origin.source,
                &input.origin.edition,
                &input.origin.version,
            ];
            if fields
                .iter()
                .any(|s| s.trim().is_empty() || s.contains('\0'))
            {
                return Err(fail(input.asset.label, Problem::Identity));
            }
            if inputs[..i].iter().any(|old| old.member == input.member) {
                return Err(fail(input.asset.label, Problem::Duplicate));
            }
            for field in fields {
                metadata = metadata
                    .checked_add(field.len())
                    .filter(|&n| n <= budget.metadata_bytes)
                    .ok_or_else(|| fail(input.asset.label, Problem::Budget))?;
            }
            total = total
                .checked_add(input.asset.bytes.len())
                .filter(|&n| n <= budget.total_bytes)
                .ok_or_else(|| fail(input.asset.label, Problem::Budget))?;
            if input.asset.bytes.len() > budget.resource_bytes {
                return Err(fail(input.asset.label, Problem::Budget));
            }
        }
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(inputs.len())
            .map_err(|_| fail("selected file sounds", Problem::Budget))?;
        for input in inputs {
            resources.push(
                PcmResource::read(input.member, input.asset, &input.origin).map_err(|e| {
                    fail(
                        input.asset.label,
                        match e {
                            ResourceFailure::Allocation => Problem::Budget,
                            ResourceFailure::Decode(e) => Problem::Wave(e),
                        },
                    )
                })?,
            );
        }
        Ok(Self {
            resources,
            path_limit: budget.metadata_bytes,
        })
    }
    pub fn resources(&self) -> &[PcmResource] {
        &self.resources
    }
    pub(crate) fn path_limit(&self) -> usize {
        self.path_limit
    }
}
#[cfg(test)]
#[path = "file_sound_tests.rs"]
mod tests;
