//! The selected range enumerator's normalized centre-distance test.
use crate::{neutral_rule_input::Pe, script_random::real};

#[derive(Clone, Debug)]
pub(crate) struct Rules {
    minimum: u32,
    adjustment: u32,
    inclusive: bool,
    minimum_radius: f32,
}

impl Rules {
    pub(super) fn exponent(&self) -> i32 {
        ((self.adjustment as i32).unsigned_abs() / (1 << 23)) as i32
    }
    pub(super) fn read(pe: &Pe<'_>, budget: usize) -> Result<Self, String> {
        // Opcode/operand widths of the bounded instruction grammar below.
        const INPUT_BYTES: usize = 7 + 4 + 3 * (6 + 5) + 2 * 5 + 6;
        if budget < INPUT_BYTES {
            return Err("group range source budget exceeded".into());
        }
        Self::decode(|va, len| {
            if pe.zero_filled(va, len as u32) {
                return Ok(vec![0; len]);
            }
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("group range truncated at {va:#x}"))
        })
    }

    fn decode(read: impl Fn(u32, usize) -> Result<Vec<u8>, String>) -> Result<Self, String> {
        let operand = |va, opcode: &[u8]| {
            let code = read(va, opcode.len() + 4)?;
            if !code.starts_with(opcode) || code.len() != opcode.len() + 4 {
                return Err(format!("group range instruction changed at {va:#x}"));
            }
            Ok(u32::from_le_bytes(code[opcode.len()..].try_into().unwrap()))
        };
        let minimum_radius_va = operand(0x807af7, &[0x0f, 0x2f, 0x05])?;
        let minimum_radius = f32::from_bits(u32::from_le_bytes(
            read(minimum_radius_va, 4)?
                .try_into()
                .map_err(|_| "group range radius threshold truncated")?,
        ));
        if !minimum_radius.is_finite() {
            return Err("group range radius threshold unsupported".into());
        }
        let minimum = operand(0x807b2e, &[0x8d, 0x88])?;
        let adjustment = operand(0x807b36, &[0x05])?;
        for (min_va, scale_va) in [(0x807b58, 0x807b60), (0x807b87, 0x807b8f)] {
            if operand(min_va, &[0x8d, 0x88])? != minimum
                || operand(scale_va, &[0x05])? != adjustment
            {
                return Err("group range coordinate/radius scales differ".into());
            }
        }
        for (va, target) in [(0x807d3c_u32, 0x99ffe0), (0x807d71, 0x9a1110)] {
            let displacement = operand(va, &[0xe8])?;
            if va.wrapping_add(5).wrapping_add(displacement) != target {
                return Err("group range numeric call changed".into());
            }
        }
        let edge = read(0x807d7e, 6)?;
        let inclusive = match edge.as_slice() {
            [0x0f, 0x2f, 0x45, 0xec, 0x77, _] => true,
            [0x0f, 0x2f, 0x45, 0xec, 0x73, _] => false,
            _ => return Err("group range distance comparison changed".into()),
        };
        if minimum as i32 >= 0
            || adjustment as i32 >= 0
            || adjustment % (1 << 23) != 0
            || minimum % (1 << 23) != 0
        {
            return Err("group range normalization exponent unsupported".into());
        }
        Ok(Self {
            minimum,
            adjustment,
            inclusive,
            minimum_radius,
        })
    }

    fn normalized(&self, value: u32) -> Result<u32, real::MathError> {
        let mask = !(((value.wrapping_add(self.minimum) ^ value) as i32 >> 31) as u32);
        let bits = mask & value.wrapping_add(self.adjustment);
        real::from_f32(f32::from_bits(bits))
    }

    pub(crate) fn prepare(
        &self,
        center: [f32; 2],
        radius: f32,
        origin: [f32; 2],
    ) -> Result<Query<'_>, String> {
        let prepare = || {
            if radius < self.minimum_radius {
                return Err(real::MathError::Unsupported);
            }
            let radius = self.normalized(real::from_f32(radius)?)?;
            let mut point = [0; 2];
            for axis in 0..2 {
                point[axis] = self.normalized(real::sub(
                    real::from_f32(center[axis])?,
                    real::from_f32(origin[axis])?,
                )?)?;
            }
            Ok(Query {
                rules: self,
                center: point,
                squared: real::mul(radius, radius)?,
                origin,
            })
        };
        prepare().map_err(|e| format!("group range geometry: {e}"))
    }
}

pub(crate) struct Query<'a> {
    rules: &'a Rules,
    center: [u32; 2],
    squared: u32,
    origin: [f32; 2],
}

impl Query<'_> {
    /// Software-real bits in the selected image's normalized coordinate space.
    pub(crate) fn distance_squared(&self, point: [f32; 2]) -> Result<u32, String> {
        let distance = || {
            let mut squared = [0; 2];
            for axis in 0..2 {
                let point = self.rules.normalized(real::sub(
                    real::from_f32(point[axis])?,
                    real::from_f32(self.origin[axis])?,
                )?)?;
                let delta = real::sub(self.center[axis], point)?;
                squared[axis] = real::mul(delta, delta)?;
            }
            real::add(squared[0], squared[1])
        };
        distance().map_err(|e| format!("group range distance: {e}"))
    }

    pub(crate) fn contains(&self, point: [f32; 2]) -> Result<bool, String> {
        let distance = self.distance_squared(point)?;
        Ok(if self.rules.inclusive {
            real::below_or_equal(distance, self.squared)
        } else {
            f32::from_bits(distance) < f32::from_bits(self.squared)
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn rules(exponent: u32) -> Rules {
        decode_fixture(exponent, true, 0.).unwrap()
    }
    fn decode_fixture(
        exponent: u32,
        inclusive: bool,
        minimum_radius: f32,
    ) -> Result<Rules, String> {
        Rules::decode(|va, _| {
            let (opcode, value): (&[u8], u32) = match va {
                0x807af7 => (&[0x0f, 0x2f, 0x05], 0x300200),
                0x300200 => return Ok(minimum_radius.to_le_bytes().to_vec()),
                0x807b2e | 0x807b58 | 0x807b87 => {
                    (&[0x8d, 0x88], 0u32.wrapping_sub((exponent + 1) << 23))
                }
                0x807b36 | 0x807b60 | 0x807b8f => (&[0x05], 0u32.wrapping_sub(exponent << 23)),
                0x807d3c => (&[0xe8], 0x99ffe0u32.wrapping_sub(va + 5)),
                0x807d71 => (&[0xe8], 0x9a1110u32.wrapping_sub(va + 5)),
                0x807d7e => {
                    return Ok(vec![
                        0x0f,
                        0x2f,
                        0x45,
                        0xec,
                        if inclusive { 0x77 } else { 0x73 },
                        0x1e,
                    ]);
                }
                _ => return Err("unprovided fixture instruction".into()),
            };
            Ok([opcode, &value.to_le_bytes()].concat())
        })
    }
    #[test]
    fn nonstock_range_operands_preserve_edges_origin_and_numeric_refusals() {
        for exponent in [2, 4, 7] {
            for inclusive in [false, true] {
                let rules = decode_fixture(exponent, inclusive, 0.).unwrap();
                let query = rules.prepare([35., -12.], 13., [19., -28.]).unwrap();
                assert_eq!(query.contains([40., 0.]).unwrap(), inclusive);
                assert!(query.contains([35., -12.]).unwrap());
                assert!(!query.contains([48.25, -12.]).unwrap());
                for radius in [-1., f32::NAN, f32::INFINITY] {
                    assert!(rules.prepare([0.; 2], radius, [0.; 2]).is_err());
                }
            }
        }
        assert!(decode_fixture(0, true, 0.).is_err());
        let threshold = decode_fixture(3, true, 7.).unwrap();
        assert!(threshold.prepare([0.; 2], 6., [0.; 2]).is_err());
        assert!(threshold.prepare([0.; 2], 8., [0.; 2]).is_ok());
        assert!(Rules::decode(|_, n| Ok(vec![0; n])).is_err());
        assert!(Rules::decode(|_, _| Ok(Vec::new())).is_err());
    }

    #[test]
    fn normalized_distance_matches_membership_with_nonstock_exponents_and_origins() {
        for exponent in [2, 4, 7] {
            let rules = decode_fixture(exponent, true, 0.).unwrap();
            let center = [35., -12.];
            let query = rules.prepare(center, 13., [19., -28.]).unwrap();
            let edge = query.distance_squared([40., 0.]).unwrap();
            assert_eq!(edge, query.squared);
            assert_eq!(query.distance_squared(center).unwrap(), 0);
            assert!(query.distance_squared([40., -1.]).unwrap() < edge);
            assert!(query.distance_squared([40., 1.]).unwrap() > edge);
            for point in [[40., -1.], [40., 0.], [40., 1.], center] {
                assert_eq!(
                    query.contains(point).unwrap(),
                    real::below_or_equal(query.distance_squared(point).unwrap(), query.squared)
                );
            }
            for point in [[f32::NAN, 0.], [0., f32::INFINITY]] {
                assert!(query.distance_squared(point).is_err());
                assert!(query.contains(point).is_err());
            }
            let zero = rules.prepare([-0., 0.], 0., [0.; 2]).unwrap();
            assert_eq!(zero.distance_squared([0., -0.]).unwrap(), 0);
        }
    }

    #[test]
    #[ignore = "requires PC3_RANGE_IMAGE pointing to the selected local Legacy executable"]
    fn supplied_group_range_image_decodes_the_selected_inclusive_center_test() {
        use std::io::Read;
        const IMAGE_BUDGET: usize = 32 << 20;
        let path = std::env::var("PC3_RANGE_IMAGE").expect("PC3_RANGE_IMAGE required");
        let mut image = Vec::new();
        std::fs::File::open(path)
            .unwrap()
            .take(IMAGE_BUDGET as u64 + 1)
            .read_to_end(&mut image)
            .unwrap();
        assert!(
            image.len() <= IMAGE_BUDGET,
            "selected image exceeds test budget"
        );
        assert_eq!(
            crate::neutral_player::sha256(&image),
            crate::menu_lighting::REVIEWED_IMAGE
        );
        let pe = Pe::parse(
            &image,
            crate::neutral_player::RuleInputBudget {
                image_bytes: IMAGE_BUDGET,
                profile_bytes: 4096,
                sections: 96,
            },
        )
        .unwrap();
        assert!(Rules::read(&pe, 0).is_err());
        let rules = Rules::read(&pe, 4096).unwrap();
        let query = rules.prepare([16., 16.], 32., [0., 0.]).unwrap();
        assert!(query.contains([48., 16.]).unwrap());
        assert!(!query.contains([48.25, 16.]).unwrap());
    }
}
