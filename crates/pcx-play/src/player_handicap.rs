//! Selected XP- and health-handicap limits and null-player results, decoded from source bytes.
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

#[derive(Clone, Debug)]
pub struct HealthBinding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub setter: SourceWindow,
    pub getter: SourceWindow,
    pub maximum_initializer: SourceWindow,
    pub null_initializer: SourceWindow,
    pub reset: SourceWindow,
    pub rounder: SourceWindow,
    pub floor: SourceWindow,
}

#[derive(Debug, thiserror::Error)]
#[error("player health handicap {0}")]
pub struct HealthError(pub String);

/// Nearest-integer policy from the unit-pool callback's own rounder: add the
/// selected bias then floor through the selected routine.
#[derive(Clone, Copy, Debug)]
pub struct LifeRounding {
    bias: f32,
}

impl LifeRounding {
    pub fn round(self, value: f32) -> f32 {
        (value + self.bias).floor()
    }
    pub fn bias(self) -> f32 {
        self.bias
    }
}

#[derive(Debug)]
pub struct HealthRules {
    binding: HealthBinding,
    threshold: f32,
    minimum: f32,
    maximum: f32,
    null: f32,
    initial: f32,
    rounding: LifeRounding,
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
fn shape_with_gaps(
    bytes: &[u8],
    len: usize,
    parts: &[(usize, &[u8])],
    gaps: &[(usize, usize)],
) -> bool {
    bytes.len() == len
        && parts.iter().all(|(at, part)| {
            part.iter().enumerate().all(|(i, expected)| {
                let position = at + i;
                gaps.iter().any(|(start, length)| {
                    position >= *start && position < start.saturating_add(*length)
                }) || bytes.get(position) == Some(expected)
            })
        })
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
            || windows.iter().map(|w| u64::from(w.len)).sum::<u64>() + 8
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

impl HealthRules {
    pub fn read(
        image: &[u8],
        binding: HealthBinding,
        numeric: &NumericPolicy,
        budget: RuleInputBudget,
    ) -> Result<Self, HealthError> {
        let fail = |text: &str| HealthError(format!("{}: {text}", binding.origin.source));
        let windows = [
            &binding.setter,
            &binding.getter,
            &binding.maximum_initializer,
            &binding.null_initializer,
            &binding.reset,
            &binding.rounder,
            &binding.floor,
        ];
        if image.len() > budget.image_bytes
            || windows.iter().map(|w| u64::from(w.len)).sum::<u64>() + 8
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
        let reset = code(&binding.reset)?;
        let rounder = code(&binding.rounder)?;
        let setter_shape = shape_with_gaps(
            setter,
            192,
            &[
                (
                    0,
                    &[
                        0x55, 0x8b, 0xec, 0x83, 0xec, 0x0c, 0x56, 0xff, 0x75, 0x08, 0xe8, 0xc1,
                        0x91, 0xfd, 0xff,
                    ],
                ),
                (
                    15,
                    &[
                        0x8b, 0xf0, 0x83, 0xc4, 0x04, 0x85, 0xf6, 0x0f, 0x84, 0x9f, 0x00, 0x00,
                        0x00, 0x8b, 0x8e, 0x5c, 0x03, 0x00, 0x00, 0x8d, 0x45, 0xfc, 0x57, 0x8b,
                        0x7d, 0x0c, 0x57, 0x50, 0x8d, 0x45, 0xf4, 0x89, 0x4d, 0xfc, 0x50, 0xe8,
                        0xd9, 0x6a, 0x4f, 0x00, 0xf3, 0x0f, 0x10, 0x05, 0x54, 0xb0, 0x0d, 0x01,
                        0x83, 0xc4, 0x0c,
                    ],
                ),
                (
                    66,
                    &[
                        0x8b, 0x00, 0x25, 0xff, 0xff, 0xff, 0x7f, 0x89, 0x45, 0xfc, 0x0f, 0x2f,
                        0x45, 0xfc, 0x77, 0x68,
                    ],
                ),
                (
                    82,
                    &[
                        0x8b, 0x07, 0x6a, 0x1f, 0x6a, 0x06, 0x89, 0x45, 0xf8, 0x0f, 0xb6, 0x46,
                        0x30, 0x6a, 0x04, 0x50, 0x6a, 0x18, 0x8d, 0x45, 0xf8, 0x50, 0x68, 0x70,
                        0x9f, 0x48, 0x00, 0xe8, 0xee, 0x53, 0x1e, 0x00, 0x8b, 0x07, 0x83, 0xc4,
                        0x1c,
                    ],
                ),
                (
                    119,
                    &[
                        0xf3, 0x0f, 0x10, 0x05, 0x58, 0xb0, 0x0d, 0x01, 0x89, 0x45, 0xfc, 0xf3,
                        0x0f, 0x10, 0x4d, 0xfc, 0x0f, 0x2f, 0xc1, 0x76, 0x07, 0xb8, 0x58, 0xb0,
                        0x0d, 0x01, 0xeb, 0x12,
                    ],
                ),
                (147, &[0x0f, 0x2f, 0x0d]),
                (154, &[0xb8]),
                (159, &[0x8d, 0x4d, 0xfc, 0x0f, 0x46, 0xc1]),
                (
                    165,
                    &[
                        0x8b, 0x00, 0x8d, 0x8e, 0x58, 0x03, 0x00, 0x00, 0x89, 0x45, 0xfc, 0x8d,
                        0x55, 0xfc, 0x8b, 0x01, 0x6a, 0x01, 0x52, 0xff, 0x10, 0x5f, 0x5e, 0x8b,
                        0xe5, 0x5d, 0xc3,
                    ],
                ),
            ],
            &[(11, 4), (59, 4), (141, 4), (155, 4)],
        );
        let getter_shape = shape_with_gaps(
            getter,
            33,
            &[
                (
                    0,
                    &[
                        0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0xe8, 0xd5, 0xe1, 0xfe, 0xff, 0x83,
                        0xc4, 0x04, 0x85, 0xc0,
                    ],
                ),
                (16, &[0x75, 0x07, 0xa1]),
                (23, &[0x5d, 0xc3, 0x8b, 0x80]),
                (27, &[0x5c, 0x03, 0x00, 0x00, 0x5d]),
                (32, &[0xc3]),
            ],
            &[(7, 4), (19, 4)],
        );
        let reset_shape = shape_with_gaps(
            reset,
            67,
            &[
                (0, &[0x8d, 0x8e]),
                (56, &[0x8b, 0x01, 0x6a, 0x01, 0x68]),
                (65, &[0xff, 0x10]),
            ],
            &[(2, 4), (61, 4)],
        );
        let rounder_shape = shape_with_gaps(
            rounder,
            40,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x51, 0x68]),
                (9, &[0xff, 0x75, 0x0c, 0x8d, 0x45, 0xfc, 0x50, 0xe8]),
                (21, &[0x50, 0xff, 0x75, 0x08, 0xe8]),
                (
                    30,
                    &[0x8b, 0x45, 0x08, 0x83, 0xc4, 0x14, 0x8b, 0xe5, 0x5d, 0xc3],
                ),
            ],
            &[(5, 4), (17, 4), (26, 4)],
        );
        if !setter_shape
            || !getter_shape
            || !reset_shape
            || !rounder_shape
            || word(setter, 30) != word(getter, 27)
            || word(setter, 169).and_then(|v| v.checked_add(4)) != word(getter, 27)
            || word(reset, 2) != word(setter, 169)
            || word(reset, 61) != word(getter, 19)
            || call(rounder, binding.rounder.va, 25) != Some(binding.floor.va)
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
        if Some(max_address) != word(setter, 155)
            || Some(null_address) != word(getter, 19)
            || Some(null_address) != word(reset, 61)
        {
            return Err(fail(
                "initializer and reset destinations do not match wrapper",
            ));
        }
        let threshold_address =
            word(setter, 59).ok_or_else(|| fail("truncated change threshold"))?;
        let minimum_address = word(setter, 141).ok_or_else(|| fail("truncated minimum operand"))?;
        let threshold = f32::from_bits(
            word(
                pe.read_from(threshold_address, false)
                    .map_err(|e| fail(&e.to_string()))?,
                0,
            )
            .ok_or_else(|| fail("truncated change threshold"))?,
        );
        let minimum = f32::from_bits(
            word(
                pe.read_from(minimum_address, false)
                    .map_err(|e| fail(&e.to_string()))?,
                0,
            )
            .ok_or_else(|| fail("truncated minimum"))?,
        );
        let bias_address = word(rounder, 5).ok_or_else(|| fail("truncated rounding bias"))?;
        let rounding = LifeRounding {
            bias: f32::from_bits(
                word(
                    pe.read_from(bias_address, false)
                        .map_err(|e| fail(&e.to_string()))?,
                    0,
                )
                .ok_or_else(|| fail("truncated rounding bias"))?,
            ),
        };
        if !threshold.is_finite()
            || threshold <= 0.0
            || !minimum.is_finite()
            || !maximum.is_finite()
            || minimum > maximum
            || !null.is_finite()
            || !rounding.bias.is_finite()
            || rounding.bias < 0.0
        {
            return Err(fail("invalid source limits"));
        }
        Ok(Self {
            binding,
            threshold,
            minimum,
            maximum,
            null,
            initial: null,
            rounding,
        })
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.binding.origin
    }
    pub fn image_sha256(&self) -> &Sha256Digest {
        &self.binding.image_sha256
    }
    pub fn clamp(&self, value: f32) -> f32 {
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
    /// Factor installed by the property reset path, not inferred from the null-player return.
    pub fn initial_result(&self) -> f32 {
        self.initial
    }
    pub fn rounding(&self) -> LifeRounding {
        self.rounding
    }
    pub fn changes(&self, stored: f32, requested: f32) -> bool {
        let difference = (requested - stored).abs();
        difference.is_nan() || difference >= self.threshold
    }
}

#[cfg(test)]
#[path = "player_handicap_tests.rs"]
pub(crate) mod tests;
