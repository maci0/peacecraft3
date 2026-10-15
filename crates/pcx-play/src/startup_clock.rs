//! Opt-in startup snapshot, not a running simulation clock.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{NeutralError, RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;

/// Caller-reviewed integer-to-real static initializer in one complete local PE.
/// The fingerprint establishes identity, not the semantic role of the routine.
#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub initializer: SourceWindow,
}
#[derive(Debug, thiserror::Error)]
#[error("startup clock {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input/selection budget exceeded")]
    Budget,
    #[error("missing identity or incorrect complete-image fingerprint")]
    Identity,
    #[error("invalid initialized code window: {0}")]
    Image(#[source] NeutralError),
    #[error("unsupported selected initializer shape")]
    Initializer,
}
#[derive(Debug)]
pub struct StartupClock {
    binding: Binding,
    hour: f32,
}
impl StartupClock {
    /// Supports only the reviewed x86 `push imm8; push address; call rel32;
    /// add esp,8; ret` integer-to-real initializer shape. Runtime/BSS bytes are
    /// never read as file contents. No saved-game or active-clock state is inferred.
    pub fn read(image: &[u8], binding: Binding, budget: RuleInputBudget) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: binding.origin.source.clone(),
            problem,
        };
        if image.len() > budget.image_bytes
            || binding.initializer.len as usize > budget.profile_bytes
        {
            return Err(fail(Problem::Budget));
        }
        if [
            &binding.origin.edition,
            &binding.origin.version,
            &binding.origin.source,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
        {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let b = pe
            .window(&binding.initializer, true)
            .map_err(|e| fail(Problem::Image(e)))?;
        if b.len() != 16
            || b[0] != 0x6a
            || b[2] != 0x68
            || b[7] != 0xe8
            || b[12..] != [0x83, 0xc4, 8, 0xc3]
        {
            return Err(fail(Problem::Initializer));
        }
        let hour = i8::from_le_bytes([b[1]]) as f32;
        Ok(Self { binding, hour })
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn hour(&self) -> f32 {
        self.hour
    }
}

#[cfg(test)]
#[path = "startup_clock_tests.rs"]
mod tests;
