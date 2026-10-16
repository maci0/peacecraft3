//! Exact caller-supplied filename bindings; no synthetic table rows or lookup priority.
use crate::label_sounds::{PcmResource, ResourceFailure, WaveError};
use crate::mp3_frame::{self, DecodeError, FrameReport};
use crate::neutral_player::Sha256Digest;
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
    #[error("{0}")]
    Mp3(#[source] DecodeError),
}
#[derive(Debug)]
pub struct Assets {
    resources: Vec<PcmResource>,
    mp3: Vec<Mp3Resource>,
    path_limit: usize,
}
#[derive(Clone, Copy)]
pub struct Mp3Budget {
    pub max_input_allocation_bytes: usize,
    pub max_decoder_steps: u32,
    pub max_packets: u32,
    pub max_frames: u64,
    pub max_samples: u64,
}
#[derive(Debug)]
pub struct Mp3Resource {
    member: String,
    source_label: String,
    origin: AssetIdentity,
    bytes: Vec<u8>,
    frames: FrameReport,
}
impl Mp3Resource {
    pub fn member(&self) -> &str {
        &self.member
    }
    pub fn source_label(&self) -> &str {
        &self.source_label
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn sha256(&self) -> Sha256Digest {
        self.frames.encoded_sha256
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn frames(&self) -> &FrameReport {
        &self.frames
    }
}

impl Assets {
    /// Whole-set budgets/identities precede all waveform parsing and source copies.
    /// These logical input limits do not promise a total heap or decoder-work bound.
    pub fn read(inputs: &[SoundInput<'_>], budget: Budget) -> Result<Self, Error> {
        Self::read_inner(inputs, budget, None)
    }
    /// Accept integer PCM or bounded MP3, retaining each member's original bytes.
    pub fn read_encoded(
        inputs: &[SoundInput<'_>],
        budget: Budget,
        mp3: Mp3Budget,
    ) -> Result<Self, Error> {
        Self::read_inner(inputs, budget, Some(mp3))
    }
    fn read_inner(
        inputs: &[SoundInput<'_>],
        budget: Budget,
        mp3_budget: Option<Mp3Budget>,
    ) -> Result<Self, Error> {
        let fail = |source: &str, problem| Error {
            source_context: source.into(),
            problem,
        };
        std::alloc::Layout::array::<PcmResource>(budget.resources)
            .and_then(|_| std::alloc::Layout::array::<Mp3Resource>(budget.resources))
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
        let mut mp3 = Vec::new();
        for input in inputs {
            if let Some(limits) = mp3_budget
                && !input.asset.bytes.starts_with(b"RIFF")
            {
                let frames = mp3_frame::decode(
                    input.asset.bytes,
                    &mp3_frame::Limits {
                        max_encoded_bytes: budget.resource_bytes,
                        max_input_allocation_bytes: limits.max_input_allocation_bytes,
                        max_decoder_steps: limits.max_decoder_steps,
                        max_packets: limits.max_packets,
                        max_frames: limits.max_frames,
                        max_samples: limits.max_samples,
                    },
                )
                .map_err(|error| fail(input.asset.label, Problem::Mp3(error)))?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(input.asset.bytes.len())
                    .map_err(|_| fail(input.asset.label, Problem::Budget))?;
                bytes.extend_from_slice(input.asset.bytes);
                mp3.try_reserve(1)
                    .map_err(|_| fail(input.asset.label, Problem::Budget))?;
                mp3.push(Mp3Resource {
                    member: input.member.into(),
                    source_label: input.asset.label.into(),
                    origin: input.origin.clone(),
                    bytes,
                    frames,
                });
            } else {
                resources
                    .try_reserve(1)
                    .map_err(|_| fail(input.asset.label, Problem::Budget))?;
                resources.push(
                    PcmResource::read(input.member, input.asset, &input.origin).map_err(
                        |error| {
                            fail(
                                input.asset.label,
                                match error {
                                    ResourceFailure::Allocation => Problem::Budget,
                                    ResourceFailure::Decode(error) => Problem::Wave(error),
                                },
                            )
                        },
                    )?,
                );
            }
        }
        Ok(Self {
            resources,
            mp3,
            path_limit: budget.metadata_bytes,
        })
    }
    pub fn resources(&self) -> &[PcmResource] {
        &self.resources
    }
    pub fn mp3_resources(&self) -> &[Mp3Resource] {
        &self.mp3
    }
    pub(crate) fn path_limit(&self) -> usize {
        self.path_limit
    }
}
#[cfg(test)]
#[path = "file_sound_tests.rs"]
mod tests;
