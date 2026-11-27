//! Selected rectangle center and translation arithmetic, without world clamping.
use super::{Error, NumericPolicy, Problem, at, call, real, word};
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct Binding {
    pub image_sha256: Sha256Digest,
    pub movement: SourceWindow,
    pub movement_calls: [SourceWindow; 2],
    pub center_getters: [SourceWindow; 2],
    pub center_factor: SourceWindow,
}

#[derive(Debug)]
pub(crate) struct Rules {
    factor: u32,
}

impl Rules {
    fn read(
        image: &[u8],
        b: &Binding,
        n: &NumericPolicy,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: n.origin().source.clone(),
            problem,
        };
        let windows = [&b.movement, &b.center_factor]
            .into_iter()
            .chain(b.movement_calls.iter())
            .chain(b.center_getters.iter());
        if image.len() > budget.image_bytes
            || windows.map(|w| u64::from(w.len)).sum::<u64>() > budget.profile_bytes as u64
        {
            return Err(fail(Problem::Budget));
        }
        if b.image_sha256 != *n.image_sha256() || sha256(image) != b.image_sha256 {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let open = |w: &SourceWindow, code| pe.window(w, code).map_err(|e| fail(Problem::Image(e)));
        let core = open(&b.movement, true)?;
        let factor_bytes = open(&b.center_factor, false)?;
        let factor = word(factor_bytes, 0).ok_or_else(|| fail(Problem::Shape))?;
        let value = f32::from_bits(factor);
        if factor_bytes.len() != 4
            || !value.is_finite()
            || value <= 0.0
            || real::from_f32(value).is_err()
            || core.len() != 0xea
            || word(core, 0xc) != Some(b.center_factor.va)
            || word(core, 0x37) != Some(b.center_factor.va)
        {
            return Err(fail(Problem::Shape));
        }
        for (offset, bytes) in [
            (0, &[0x55, 0x8b, 0xec, 0x83, 0xec, 0x2c, 0x56, 0x57][..]),
            (8, &[0x8d, 0x79, 0x24]),
            (0xb, &[0x68]),
            (0x11, &[0x8d, 0x47, 8]),
            (0x2e, &[0x8d, 0x4f, 4]),
            (0x36, &[0x68]),
            (0x3c, &[0x8d, 0x4f, 0x0c]),
            (0x9c, &[0x8b, 0, 0x89, 7]),
            (0xae, &[0x8b, 0, 0x89, 6]),
            (0xc6, &[0x8b, 0, 0x89, 6]),
            (0xde, &[0x8b, 0, 0x5f, 0x89, 6]),
            (0xe7, &[0xc2, 8, 0]),
        ] {
            if !at(core, offset, bytes) {
                return Err(fail(Problem::Shape));
            }
        }
        for (offset, target) in [
            (0x19, n.binding.math_add.va),
            (0x26, n.binding.math_mul.va),
            (0x41, n.binding.math_add.va),
            (0x4e, n.binding.math_mul.va),
            (0x69, n.binding.math_sub.va),
            (0x7c, n.binding.math_sub.va),
            (0x94, n.binding.math_add.va),
            (0xa9, n.binding.math_add.va),
            (0xbe, n.binding.math_add.va),
            (0xd6, n.binding.math_add.va),
        ] {
            if call(core, b.movement.va, offset) != Some(target) {
                return Err(fail(Problem::Shape));
            }
        }
        for w in &b.movement_calls {
            let bytes = open(w, true)?;
            if bytes.len() != 5 || call(bytes, w.va, 0) != Some(b.movement.va) {
                return Err(fail(Problem::Shape));
            }
        }
        for (w, len, adds, multiplies, factors) in [
            (
                &b.center_getters[0],
                0x72,
                [0x30, 0x56],
                [0x3d, 0x63],
                [0x23, 0x49],
            ),
            (
                &b.center_getters[1],
                0x71,
                [0x2c, 0x54],
                [0x39, 0x61],
                [0x1f, 0x4a],
            ),
        ] {
            let bytes = open(w, true)?;
            if bytes.len() != len
                || adds
                    .iter()
                    .any(|&i| call(bytes, w.va, i) != Some(n.binding.math_add.va))
                || multiplies
                    .iter()
                    .any(|&i| call(bytes, w.va, i) != Some(n.binding.math_mul.va))
                || factors
                    .iter()
                    .any(|&i| word(bytes, i) != Some(b.center_factor.va))
            {
                return Err(fail(Problem::Shape));
            }
        }
        Ok(Self { factor })
    }

    pub(crate) fn centers(&self, bounds: [f32; 4]) -> Result<[f32; 2], real::MathError> {
        let mut centers = [0.; 2];
        for i in 0..2 {
            centers[i] = f32::from_bits(real::mul(
                real::add(real::from_f32(bounds[i + 2])?, real::from_f32(bounds[i])?)?,
                self.factor,
            )?);
        }
        Ok(centers)
    }

    pub(crate) fn translate(
        &self,
        bounds: [f32; 4],
        target: [f32; 2],
    ) -> Result<[f32; 4], real::MathError> {
        let center = self.centers(bounds)?;
        let delta = [
            real::sub(real::from_f32(target[0])?, real::from_f32(center[0])?)?,
            real::sub(real::from_f32(target[1])?, real::from_f32(center[1])?)?,
        ];
        let mut moved = [0.; 4];
        for (i, coordinate) in bounds.into_iter().enumerate() {
            moved[i] = f32::from_bits(real::add(real::from_f32(coordinate)?, delta[i % 2])?);
        }
        Ok(moved)
    }
}

impl NumericPolicy {
    pub fn with_rectangle_motion(
        mut self,
        image: &[u8],
        binding: &Binding,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        self.rectangle_motion = Some(Rules::read(image, binding, &self, budget)?);
        Ok(self)
    }
    pub(crate) fn rectangle_motion(&self) -> Option<&Rules> {
        self.rectangle_motion.as_ref()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn fixture(factor: f32) -> (Vec<u8>, super::super::Binding, Binding, RuleInputBudget) {
        let (mut image, mut n, budget) = super::super::tests::fixture();
        let mut put = |offset: usize, bytes: Vec<u8>| {
            image[0x200 + offset..0x200 + offset + bytes.len()].copy_from_slice(&bytes);
            SourceWindow {
                va: 0x301000 + offset as u32,
                len: bytes.len() as u32,
                digest: sha256(&bytes),
            }
        };
        let patch = |bytes: &mut [u8], base: u32, at: usize, target: u32| {
            bytes[at] = 0xe8;
            bytes[at + 1..at + 5]
                .copy_from_slice(&target.wrapping_sub(base + at as u32 + 5).to_le_bytes());
        };
        let factor_va = 0x301c00u32;
        let mut core = vec![0x90; 0xea];
        for (i, bytes) in [
            (0, &[0x55, 0x8b, 0xec, 0x83, 0xec, 0x2c, 0x56, 0x57][..]),
            (8, &[0x8d, 0x79, 0x24]),
            (0xb, &[0x68]),
            (0x11, &[0x8d, 0x47, 8]),
            (0x2e, &[0x8d, 0x4f, 4]),
            (0x36, &[0x68]),
            (0x3c, &[0x8d, 0x4f, 0x0c]),
            (0x9c, &[0x8b, 0, 0x89, 7]),
            (0xae, &[0x8b, 0, 0x89, 6]),
            (0xc6, &[0x8b, 0, 0x89, 6]),
            (0xde, &[0x8b, 0, 0x5f, 0x89, 6]),
            (0xe7, &[0xc2, 8, 0]),
        ] {
            core[i..i + bytes.len()].copy_from_slice(bytes);
        }
        for i in [0xc, 0x37] {
            core[i..i + 4].copy_from_slice(&factor_va.to_le_bytes());
        }
        for (i, target) in [
            (0x19, n.math_add.va),
            (0x26, n.math_mul.va),
            (0x41, n.math_add.va),
            (0x4e, n.math_mul.va),
            (0x69, n.math_sub.va),
            (0x7c, n.math_sub.va),
            (0x94, n.math_add.va),
            (0xa9, n.math_add.va),
            (0xbe, n.math_add.va),
            (0xd6, n.math_add.va),
        ] {
            patch(&mut core, 0x301900, i, target);
        }
        let movement = put(0x900, core);
        let movement_calls = std::array::from_fn(|i| {
            let offset = 0xa00 + i * 0x20;
            let mut bytes = vec![0; 5];
            patch(&mut bytes, 0x301000 + offset as u32, 0, movement.va);
            put(offset, bytes)
        });
        let center_getters = std::array::from_fn(|axis| {
            let (offset, len, adds, muls, factors) = if axis == 0 {
                (0xa40, 0x72, [0x30, 0x56], [0x3d, 0x63], [0x23, 0x49])
            } else {
                (0xb00, 0x71, [0x2c, 0x54], [0x39, 0x61], [0x1f, 0x4a])
            };
            let mut bytes = vec![0x90; len];
            for i in adds {
                patch(&mut bytes, 0x301000 + offset as u32, i, n.math_add.va);
            }
            for i in muls {
                patch(&mut bytes, 0x301000 + offset as u32, i, n.math_mul.va);
            }
            for i in factors {
                bytes[i..i + 4].copy_from_slice(&factor_va.to_le_bytes());
            }
            put(offset, bytes)
        });
        let center_factor = put(0xc00, factor.to_le_bytes().to_vec());
        n.image_sha256 = sha256(&image);
        let b = Binding {
            image_sha256: n.image_sha256,
            movement,
            movement_calls,
            center_getters,
            center_factor,
        };
        (image, n, b, budget)
    }

    pub(crate) fn policy(factor: f32) -> NumericPolicy {
        let (image, n, b, budget) = fixture(factor);
        NumericPolicy::read(&image, n, budget, 64)
            .unwrap()
            .with_rectangle_motion(&image, &b, budget)
            .unwrap()
    }

    #[test]
    fn data_factor_and_selected_add_subtract_order_determine_motion() {
        let p = policy(0.25);
        let r = p.rectangle_motion().unwrap();
        assert_eq!(r.centers([2., 4., 10., 12.]).unwrap(), [3., 4.]);
        assert_eq!(
            r.translate([2., 4., 10., 12.], [7., 11.]).unwrap(),
            [6., 11., 14., 19.]
        );
        let p = policy(0.5);
        let r = p.rectangle_motion().unwrap();
        let large = r.translate([-4., 2., 8., 10.], [10000000.; 2]).unwrap();
        assert_eq!(
            r.translate(large, [1.25, 2.75]).unwrap(),
            [-6., -1., 6., 7.]
        );
        assert_eq!(r.centers([-6., -1., 6., 7.]).unwrap(), [0., 3.]);
    }

    #[test]
    fn changed_operand_or_arithmetic_calls_refuse_even_with_updated_window_hashes() {
        let (image, n, b, budget) = fixture(0.5);
        for offset in [0x900 + 0xc, 0x900 + 0x6a, 0xa00 + 1, 0xa40 + 0x31] {
            let mut image = image.clone();
            image[0x200 + offset] ^= 1;
            let mut n = n.clone();
            n.image_sha256 = sha256(&image);
            let mut b = b.clone();
            b.image_sha256 = n.image_sha256;
            for w in [&mut b.movement, &mut b.center_factor]
                .into_iter()
                .chain(b.movement_calls.iter_mut())
                .chain(b.center_getters.iter_mut())
            {
                let at = 0x200 + (w.va - 0x301000) as usize;
                w.digest = sha256(&image[at..at + w.len as usize]);
            }
            let p = NumericPolicy::read(&image, n, budget, 64).unwrap();
            assert!(matches!(
                p.with_rectangle_motion(&image, &b, budget)
                    .unwrap_err()
                    .problem,
                Problem::Shape
            ));
        }
        for invalid in [0., -0.5, f32::NAN, f32::INFINITY] {
            let (image, n, b, budget) = fixture(invalid);
            let p = NumericPolicy::read(&image, n, budget, 64).unwrap();
            assert!(matches!(
                p.with_rectangle_motion(&image, &b, budget)
                    .unwrap_err()
                    .problem,
                Problem::Shape
            ));
        }
        let p = NumericPolicy::read(&image, n.clone(), budget, 64).unwrap();
        assert!(matches!(
            p.with_rectangle_motion(
                &image,
                &b,
                RuleInputBudget {
                    profile_bytes: 1,
                    ..budget
                }
            )
            .unwrap_err()
            .problem,
            Problem::Budget
        ));
        let p = NumericPolicy::read(&image, n, budget, 64).unwrap();
        let mut changed = image;
        changed[0x1900] ^= 1;
        assert!(matches!(
            p.with_rectangle_motion(&changed, &b, budget)
                .unwrap_err()
                .problem,
            Problem::Identity
        ));
    }
}
