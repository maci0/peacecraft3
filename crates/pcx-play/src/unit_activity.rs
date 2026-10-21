//! Source-bound activity identifiers; execution lives on the existing unit arena.
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    order_registry::Registry,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub hold: SourceWindow,
    pub stop: SourceWindow,
    pub movement: SourceWindow,
    pub patrol: SourceWindow,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub(crate) binding: Binding,
    pub(crate) hold: i32,
    pub(crate) stop: i32,
    pub(crate) movement: i32,
    pub(crate) patrol: i32,
}

impl Rules {
    pub fn read(
        image: &[u8],
        binding: Binding,
        registry: &Registry,
        budget: RuleInputBudget,
    ) -> Result<Self, String> {
        let fail = |reason: &str| format!("unit activity {}: {reason}", binding.origin.source);
        if image.len() > budget.image_bytes
            || [binding.hold, binding.stop, binding.movement, binding.patrol]
                .iter()
                .map(|w| u64::from(w.len))
                .sum::<u64>()
                > budget.profile_bytes as u64
        {
            return Err(fail("input budget"));
        }
        if [
            &binding.origin.source,
            &binding.origin.edition,
            &binding.origin.version,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
            || registry.binding().image_sha256 != binding.image_sha256
        {
            return Err(fail("client identity mismatch"));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(&e.to_string()))?;
        let compare = |window: &SourceWindow, prefix: &[u8]| -> Result<i32, String> {
            let b = pe.window(window, true).map_err(|e| fail(&e.to_string()))?;
            if b.len() != prefix.len() + 4 || !b.starts_with(prefix) {
                return Err(fail("unsupported order comparison"));
            }
            let id = i32::from_le_bytes(b[prefix.len()..].try_into().unwrap());
            if registry.name(id).is_none() {
                return Err(fail("order missing from selected registry"));
            }
            Ok(id)
        };
        let hold = compare(&binding.hold, &[0x3d])?;
        let stop = compare(&binding.stop, &[0x81, 0xfb])?;
        let movement = compare(&binding.movement, &[0x3d])?;
        let patrol = compare(&binding.patrol, &[0x3d])?;
        if hold == stop
            || stop == movement
            || hold == movement
            || [hold, stop, movement].contains(&patrol)
        {
            return Err(fail("ambiguous activity identifiers"));
        }
        Ok(Self {
            binding,
            hold,
            stop,
            movement,
            patrol,
        })
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
}
