//! Finite rectangle corner clamping from the selected executable's shared helper.
use super::{Error, NumericPolicy, Problem, at, call, real, word};
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct Binding {
    pub image_sha256: Sha256Digest,
    pub point_clamp: SourceWindow,
    pub inset_initializer: SourceWindow,
    /// Two shared-helper calls in Rect, RectFromLoc, SetRect and SetRectFromLoc.
    pub corner_calls: [SourceWindow; 4],
}

#[derive(Debug)]
pub(crate) struct Rules {
    inset: f32,
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
        let windows = [&b.point_clamp, &b.inset_initializer]
            .into_iter()
            .chain(b.corner_calls.iter());
        if image.len() > budget.image_bytes
            || windows.clone().map(|w| u64::from(w.len)).sum::<u64>() > budget.profile_bytes as u64
        {
            return Err(fail(Problem::Budget));
        }
        if b.image_sha256 != *n.image_sha256() || sha256(image) != b.image_sha256 {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let open = |w: &SourceWindow| pe.window(w, true).map_err(|e| fail(Problem::Image(e)));
        let point = open(&b.point_clamp)?;
        let (inset_va, inset) = n
            .integer_initializer(open(&b.inset_initializer)?, b.inset_initializer.va)
            .ok_or_else(|| fail(Problem::Shape))?;
        if !inset.is_finite()
            || inset <= 0.0
            || point.len() != 0x91
            || word(point, 7) != Some(inset_va)
            || call(point, b.point_clamp.va, 0x22) != Some(n.binding.math_sub.va)
            || call(point, b.point_clamp.va, 0x58) != Some(n.binding.math_sub.va)
        {
            return Err(fail(Problem::Shape));
        }
        for (offset, bytes) in [
            (0, &[0x55, 0x8b, 0xec, 0x83, 0xec, 8][..]),
            (6, &[0xa1]),
            (0xc, &[0x8b, 0x35]),
            (0x1a, &[0x8d, 0x46]),
            (0x2c, &[0xf3, 0x0f, 0x10, 0x46]),
            (0x37, &[0xf3, 0x0f, 0x10, 0x0f]),
            (0x3b, &[0x0f, 0x2f, 0xc1]),
            (0x3e, &[0x77, 8]),
            (0x40, &[0x0f, 0x2f, 9]),
            (0x43, &[0x8b, 0xc7]),
            (0x45, &[0x0f, 0x47, 0xc1]),
            (0x48, &[0x8b, 0]),
            (0x4a, &[0x89, 7]),
            (0x50, &[0x8d, 0x46]),
            (0x5d, &[0xf3, 0x0f, 0x10, 0x4f, 4]),
            (0x65, &[0xf3, 0x0f, 0x10, 0x46]),
            (0x6c, &[0x0f, 0x2f, 0xc1]),
            (0x6f, &[0x76, 0x0c]),
            (0x7d, &[0x0f, 0x2f, 0x0a]),
            (0x83, &[0x0f, 0x47, 0xc2]),
            (0x88, &[0x89, 0x47, 4]),
            (0x90, &[0xc3]),
        ] {
            if !at(point, offset, bytes) {
                return Err(fail(Problem::Shape));
            }
        }
        let min_x = point[0x30];
        if min_x.checked_add(4) != Some(point[0x69])
            || min_x.checked_add(8) != Some(point[0x1c])
            || min_x.checked_add(12) != Some(point[0x52])
        {
            return Err(fail(Problem::Shape));
        }
        for (window, offsets) in b
            .corner_calls
            .iter()
            .zip([[4, 13], [7, 16], [9, 18], [7, 16]])
        {
            let bytes = open(window)?;
            if bytes.len() != offsets[1] + 5
                || offsets
                    .iter()
                    .any(|&offset| call(bytes, window.va, offset) != Some(b.point_clamp.va))
            {
                return Err(fail(Problem::Shape));
            }
        }
        Ok(Self { inset })
    }

    pub(crate) fn limits(&self, world: [f32; 4]) -> Result<[f32; 4], real::MathError> {
        let inset = real::from_f32(self.inset)?;
        Ok([
            world[0],
            world[1],
            f32::from_bits(real::sub(real::from_f32(world[2])?, inset)?),
            f32::from_bits(real::sub(real::from_f32(world[3])?, inset)?),
        ])
    }
}

impl NumericPolicy {
    pub fn with_rectangle_clamp(
        mut self,
        image: &[u8],
        binding: &Binding,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        self.rectangle_clamp = Some(Rules::read(image, binding, &self, budget)?);
        Ok(self)
    }

    pub(crate) fn rectangle_clamp(&self) -> Option<&Rules> {
        self.rectangle_clamp.as_ref()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn fixture(inset: i8) -> (Vec<u8>, super::super::Binding, Binding, RuleInputBudget) {
        let (mut image, mut numeric, budget) = super::super::tests::fixture();
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
        let global = 0x302d00u32;
        let mut point = vec![0x90; 0x91];
        for (offset, bytes) in [
            (0, &[0x55, 0x8b, 0xec, 0x83, 0xec, 8][..]),
            (6, &[0xa1]),
            (0xc, &[0x8b, 0x35]),
            (0x1a, &[0x8d, 0x46, 0x18]),
            (0x2c, &[0xf3, 0x0f, 0x10, 0x46, 0x10]),
            (0x37, &[0xf3, 0x0f, 0x10, 0x0f]),
            (0x3b, &[0x0f, 0x2f, 0xc1]),
            (0x3e, &[0x77, 8]),
            (0x40, &[0x0f, 0x2f, 9]),
            (0x43, &[0x8b, 0xc7]),
            (0x45, &[0x0f, 0x47, 0xc1]),
            (0x48, &[0x8b, 0]),
            (0x4a, &[0x89, 7]),
            (0x50, &[0x8d, 0x46, 0x1c]),
            (0x5d, &[0xf3, 0x0f, 0x10, 0x4f, 4]),
            (0x65, &[0xf3, 0x0f, 0x10, 0x46, 0x14]),
            (0x6c, &[0x0f, 0x2f, 0xc1]),
            (0x6f, &[0x76, 0x0c]),
            (0x7d, &[0x0f, 0x2f, 0x0a]),
            (0x83, &[0x0f, 0x47, 0xc2]),
            (0x88, &[0x89, 0x47, 4]),
            (0x90, &[0xc3]),
        ] {
            point[offset..offset + bytes.len()].copy_from_slice(bytes);
        }
        point[7..11].copy_from_slice(&global.to_le_bytes());
        patch(&mut point, 0x301900, 0x22, numeric.math_sub.va);
        patch(&mut point, 0x301900, 0x58, numeric.math_sub.va);
        let point_clamp = put(0x900, point);
        let mut init = vec![0x6a, inset as u8, 0x68];
        init.extend(global.to_le_bytes());
        init.extend([0; 5]);
        init.extend([0x83, 0xc4, 8, 0xc3]);
        patch(&mut init, 0x301a00, 7, numeric.math_i32_to_real.va);
        let inset_initializer = put(0xa00, init);
        let corner_calls = std::array::from_fn(|i| {
            let offsets = [[4, 13], [7, 16], [9, 18], [7, 16]][i];
            let offset = 0xa40 + i * 0x40;
            let mut bytes = vec![0x90; offsets[1] + 5];
            for at in offsets {
                patch(&mut bytes, 0x301000 + offset as u32, at, point_clamp.va);
            }
            put(offset, bytes)
        });
        numeric.image_sha256 = sha256(&image);
        let binding = Binding {
            image_sha256: numeric.image_sha256,
            point_clamp,
            inset_initializer,
            corner_calls,
        };
        (image, numeric, binding, budget)
    }

    pub(crate) fn policy(inset: i8) -> NumericPolicy {
        let (image, numeric, binding, budget) = fixture(inset);
        NumericPolicy::read(&image, numeric, budget, 64)
            .unwrap()
            .with_rectangle_clamp(&image, &binding, budget)
            .unwrap()
    }

    #[test]
    fn initializer_values_determine_clamp_limits_without_changing_world_extent() {
        for inset in [3, 7, 12] {
            let policy = policy(inset);
            assert_eq!(
                policy
                    .rectangle_clamp()
                    .unwrap()
                    .limits([-20., -30., 100., 80.])
                    .unwrap(),
                [-20., -30., 100. - f32::from(inset), 80. - f32::from(inset)]
            );
        }
        for invalid in [0, -1] {
            let (image, numeric, binding, budget) = fixture(invalid);
            let n = NumericPolicy::read(&image, numeric, budget, 64).unwrap();
            assert!(n.with_rectangle_clamp(&image, &binding, budget).is_err());
        }
    }

    #[test]
    fn image_budgets_fingerprints_and_call_or_operand_mutations_refuse() {
        let (image, numeric, binding, budget) = fixture(5);
        for offset in [0x900 + 7, 0x900 + 0x23, 0x900 + 0x69, 0xa00 + 8, 0xa40 + 5] {
            let mut changed = image.clone();
            changed[0x200 + offset] ^= 1;
            let mut n = numeric.clone();
            n.image_sha256 = sha256(&changed);
            let mut b = binding.clone();
            b.image_sha256 = n.image_sha256;
            for window in [&mut b.point_clamp, &mut b.inset_initializer]
                .into_iter()
                .chain(b.corner_calls.iter_mut())
            {
                let at = 0x200 + (window.va - 0x301000) as usize;
                window.digest = sha256(&changed[at..at + window.len as usize]);
            }
            let n = NumericPolicy::read(&changed, n, budget, 64).unwrap();
            assert!(matches!(
                n.with_rectangle_clamp(&changed, &b, budget)
                    .unwrap_err()
                    .problem,
                Problem::Shape
            ));
        }
        for small in [
            RuleInputBudget {
                image_bytes: image.len() - 1,
                ..budget
            },
            RuleInputBudget {
                profile_bytes: 1,
                ..budget
            },
        ] {
            let n = NumericPolicy::read(&image, numeric.clone(), budget, 64).unwrap();
            assert!(matches!(
                n.with_rectangle_clamp(&image, &binding, small)
                    .unwrap_err()
                    .problem,
                Problem::Budget
            ));
        }
        let n = NumericPolicy::read(&image, numeric, budget, 64).unwrap();
        let mut other = image;
        other[0x1900] ^= 1;
        assert!(matches!(
            n.with_rectangle_clamp(&other, &binding, budget)
                .unwrap_err()
                .problem,
            Problem::Identity
        ));
    }
}
