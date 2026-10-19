//! Selected XP-handicap clamp and null-player result, decoded from source bytes.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::script_numeric::NumericPolicy;
use crate::scripts::AssetIdentity;

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub setter: SourceWindow,
    pub getter: SourceWindow,
    pub maximum_initializer: SourceWindow,
    pub null_initializer: SourceWindow,
}

#[derive(Debug, thiserror::Error)]
#[error("player experience handicap {0}")]
pub struct Error(pub String);

#[derive(Debug)]
pub struct Rules {
    binding: Binding,
    minimum: f32,
    maximum: f32,
    null: f32,
}
fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn call(bytes: &[u8], base: u32, at: usize) -> Option<u32> {
    (bytes.get(at) == Some(&0xe8)).then_some(())?;
    Some(
        base.wrapping_add(at as u32)
            .wrapping_add(5)
            .wrapping_add(word(bytes, at + 1)?),
    )
}
fn shape(bytes: &[u8], len: usize, parts: &[(usize, &[u8])]) -> bool {
    bytes.len() == len
        && parts
            .iter()
            .all(|(at, part)| bytes.get(*at..at + part.len()) == Some(*part))
}
impl Rules {
    pub fn read(
        image: &[u8],
        binding: Binding,
        numeric: &NumericPolicy,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        let fail = |text: &str| Error(format!("{}: {text}", binding.origin.source));
        let windows = [
            &binding.setter,
            &binding.getter,
            &binding.maximum_initializer,
            &binding.null_initializer,
        ];
        if image.len() > budget.image_bytes
            || windows.iter().map(|w| u64::from(w.len)).sum::<u64>() + 4
                > budget.profile_bytes as u64
        {
            return Err(fail("input budget exceeded"));
        }
        if [
            &binding.origin.edition,
            &binding.origin.version,
            &binding.origin.source,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
            || numeric.image_sha256() != &binding.image_sha256
        {
            return Err(fail("incomplete or mismatched source identity"));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(&e.to_string()))?;
        let code = |w| pe.window(w, true).map_err(|e| fail(&e.to_string()));
        let setter = code(&binding.setter)?;
        let getter = code(&binding.getter)?;
        if !shape(
            setter,
            97,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x51, 0xff, 0x75, 8, 0xe8]),
                (
                    12,
                    &[
                        0x8b, 0xd0, 0x83, 0xc4, 4, 0x85, 0xd2, 0x74, 0x48, 0x8b, 0x4d, 12, 0xf3,
                        0x0f, 0x10, 5,
                    ],
                ),
                (
                    32,
                    &[
                        0x8b, 9, 0x89, 0x4d, 0xfc, 0xf3, 0x0f, 0x10, 0x4d, 0xfc, 0x0f, 0x2f, 0xc1,
                        0x76, 7, 0xb8,
                    ],
                ),
                (52, &[0xeb, 0x12, 0x0f, 0x2f, 0x0d]),
                (61, &[0xb8]),
                (
                    66,
                    &[0x8d, 0x4d, 0xfc, 0x0f, 0x46, 0xc1, 0x8b, 0, 0x8d, 0x8a],
                ),
                (
                    80,
                    &[
                        0x89, 0x45, 0xfc, 0x8d, 0x55, 0xfc, 0x8b, 1, 0x6a, 1, 0x52, 0xff, 0x10,
                        0x8b, 0xe5, 0x5d, 0xc3,
                    ],
                ),
            ],
        ) || !shape(
            getter,
            33,
            &[
                (0, &[0x55, 0x8b, 0xec, 0xff, 0x75, 8, 0xe8]),
                (11, &[0x83, 0xc4, 4, 0x85, 0xc0, 0x75, 7, 0xa1]),
                (23, &[0x5d, 0xc3, 0x8b, 0x80]),
                (31, &[0x5d, 0xc3]),
            ],
        ) || call(setter, binding.setter.va, 7) != call(getter, binding.getter.va, 6)
            || word(setter, 28) != word(setter, 48)
            || word(setter, 57) != word(setter, 62)
            || word(setter, 76).and_then(|v| v.checked_add(4)) != word(getter, 27)
        {
            return Err(fail("unsupported wrapper shape or relationship"));
        }
        let (max_address, maximum) = numeric
            .integer_initializer(
                code(&binding.maximum_initializer)?,
                binding.maximum_initializer.va,
            )
            .ok_or_else(|| fail("maximum real initializer"))?;
        let (null_address, null) = numeric
            .integer_initializer(
                code(&binding.null_initializer)?,
                binding.null_initializer.va,
            )
            .ok_or_else(|| fail("null real initializer"))?;
        if Some(max_address) != word(setter, 57) || Some(null_address) != word(getter, 19) {
            return Err(fail("initializer destination does not match wrapper"));
        }
        let data = pe
            .read_from(word(setter, 28).expect("checked shape"), false)
            .map_err(|e| fail(&e.to_string()))?;
        let minimum = f32::from_bits(word(data, 0).ok_or_else(|| fail("truncated minimum"))?);
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum || !null.is_finite() {
            return Err(fail("invalid source limits"));
        }
        Ok(Self {
            binding,
            minimum,
            maximum,
            null,
        })
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.binding.origin
    }
    pub fn image_sha256(&self) -> &Sha256Digest {
        &self.binding.image_sha256
    }
    pub fn clamp(&self, value: f32) -> f32 {
        // Ordered comparisons retain NaN payloads, as the selected COMISS/CMOVBE path does.
        if value < self.minimum {
            self.minimum
        } else if value > self.maximum {
            self.maximum
        } else {
            value
        }
    }
    pub fn null_result(&self) -> f32 {
        self.null
    }
}

#[cfg(test)]
#[path = "player_handicap_tests.rs"]
pub(crate) mod tests;
