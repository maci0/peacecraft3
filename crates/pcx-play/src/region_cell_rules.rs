//! Selected region rectangle and point conversion into world-relative cells.
use super::{Error, NumericPolicy, Problem, at, call, real, word};
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct Binding {
    pub image_sha256: Sha256Digest,
    pub point: SourceWindow,
    pub rectangle: SourceWindow,
    pub rounding: SourceWindow,
    pub floor: SourceWindow,
    pub truncate: SourceWindow,
    pub cell_coordinates: [SourceWindow; 2],
    pub cell_edit: SourceWindow,
    pub cell_rounding: SourceWindow,
    pub cell_calls: [SourceWindow; 4],
}

#[derive(Debug)]
pub(crate) struct Rules {
    guard: u32,
    scale: u32,
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
        if image.len() > budget.image_bytes
            || [
                &b.point,
                &b.rectangle,
                &b.rounding,
                &b.floor,
                &b.truncate,
                &b.cell_edit,
                &b.cell_rounding,
            ]
            .into_iter()
            .chain(b.cell_coordinates.iter())
            .chain(b.cell_calls.iter())
            .map(|w| u64::from(w.len))
            .sum::<u64>()
                > budget.profile_bytes as u64
        {
            return Err(fail(Problem::Budget));
        }
        if b.image_sha256 != *n.image_sha256() || sha256(image) != b.image_sha256 {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let open = |w| pe.window(w, true).map_err(|e| fail(Problem::Image(e)));
        let point = open(&b.point)?;
        let rectangle = open(&b.rectangle)?;
        let rounding = open(&b.rounding)?;
        let floor_body = open(&b.floor)?;
        let truncate_body = open(&b.truncate)?;
        if floor_body.len() != 152
            || truncate_body.len() != 80
            || !at(
                floor_body,
                0,
                &[0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x8b, 0x10],
            )
            || !at(floor_body, 0x11, &[0x83, 0xee, 0x7f])
            || !at(floor_body, 0x35, &[0x83, 0xfe, 0x17])
            || !at(
                truncate_body,
                0,
                &[0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0x56, 0x8b, 0x30],
            )
            || !at(truncate_body, 0x11, &[0x83, 0xf8, 0x7f])
        {
            return Err(fail(Problem::Shape));
        }

        if point.len() != 0xc8 || rectangle.len() != 0xe5 || rounding.len() != 0x7f {
            return Err(fail(Problem::Shape));
        }
        // Both paths subtract the same world's X/Y origin before exponent scaling.
        if !at(point, 0, &[0xa1])
            || !at(point, 5, &[0x83, 0xc0, 0x70])
            || !at(point, 0x2b, &[0x83, 0xc0, 0x74])
            || word(point, 1) != word(point, 0x27)
            || word(point, 1) != word(rectangle, 1)
            || [0x32, 0x62, 0x92]
                .into_iter()
                .any(|at| word(rectangle, at) != word(point, 1))
            || !at(rectangle, 9, &[0x83, 0xc0, 0x70])
            || !at(rectangle, 0x36, &[0x83, 0xc0, 0x74])
            || !at(rectangle, 0x66, &[0x83, 0xc0, 0x70])
            || !at(rectangle, 0x96, &[0x83, 0xc0, 0x74])
        {
            return Err(fail(Problem::Shape));
        }
        for offset in [0x10, 0x40] {
            if call(point, b.point.va, offset) != Some(n.binding.math_sub.va) {
                return Err(fail(Problem::Shape));
            }
        }
        for offset in [0x15, 0x45, 0x75, 0xa2] {
            if call(rectangle, b.rectangle.va, offset) != Some(n.binding.math_sub.va) {
                return Err(fail(Problem::Shape));
            }
        }
        let guard = word(point, 0x1a).ok_or_else(|| fail(Problem::Shape))?;
        let scale = word(point, 0x22).ok_or_else(|| fail(Problem::Shape))?;
        let exponent = scale.wrapping_neg() >> 23;
        if exponent == 0
            || exponent > 23
            || scale.wrapping_neg() != exponent << 23
            || guard.wrapping_neg() != (exponent + 1) << 23
        {
            return Err(fail(Problem::Shape));
        }
        for offset in [0x18, 0x4e] {
            if !at(point, offset, &[0x8d, 0x88])
                || word(point, offset + 2) != Some(guard)
                || !at(point, offset + 6, &[0x33, 0xc8, 0x8d, 0x90])
                || word(point, offset + 10) != Some(scale)
            {
                return Err(fail(Problem::Shape));
            }
        }
        if !at(point, 0x2e, &[0xc1, 0xf9, 0x1f])
            || !at(point, 0x35, &[0xf7, 0xd1])
            || !at(point, 0x3a, &[0x23, 0xca])
            || !at(point, 0x5c, &[0xc1, 0xf9, 0x1f, 0xf7, 0xd1, 0x23, 0xca])
        {
            return Err(fail(Problem::Shape));
        }
        for offset in [0x1d, 0x4d, 0x7d, 0xad] {
            if !at(rectangle, offset, &[0x8d, 0x88])
                || word(rectangle, offset + 2) != Some(guard)
                || !at(rectangle, offset + 6, &[0x33, 0xc8, 0x05])
                || word(rectangle, offset + 9) != Some(scale)
                || !at(
                    rectangle,
                    offset + 13,
                    &[0xc1, 0xf9, 0x1f, 0xf7, 0xd1, 0x23, 0xc8],
                )
            {
                return Err(fail(Problem::Shape));
            }
        }
        let floor = call(rounding, b.rounding.va, 0x14).ok_or_else(|| fail(Problem::Shape))?;
        if floor != b.floor.va {
            return Err(fail(Problem::Shape));
        }
        let integer = call(rounding, b.rounding.va, 0x1a).ok_or_else(|| fail(Problem::Shape))?;
        if integer != b.truncate.va {
            return Err(fail(Problem::Shape));
        }
        for (f, i) in [(0x2a, 0x30), (0x3f, 0x45), (0x51, 0x57)] {
            if call(rounding, b.rounding.va, f) != Some(floor)
                || call(rounding, b.rounding.va, i) != Some(integer)
            {
                return Err(fail(Problem::Shape));
            }
        }
        if call(point, b.point.va, 0xc3) != Some(floor)
            || call(rectangle, b.rectangle.va, 0xe0) != Some(b.rounding.va)
            || !at(rounding, 0x5f, &[0x8d, 0x4f, 1])
            || !at(rounding, 0x6f, &[0x41])
        {
            return Err(fail(Problem::Shape));
        }
        for (axis, w) in b.cell_coordinates.iter().enumerate() {
            let bytes = open(w)?;
            if bytes.len() != 0x3a
                || !at(bytes, 0, &[0x55, 0x8b, 0xec, 0xa1])
                || word(bytes, 4) != word(point, 1)
                || !at(bytes, 0xc, &[0x83, 0xc0, 0x70 + axis as u8 * 4])
                || call(bytes, w.va, 0x14) != Some(n.binding.math_sub.va)
                || !at(bytes, 0x20, &[0x8d, 0x91])
                || word(bytes, 0x22) != Some(guard)
                || !at(bytes, 0x26, &[0x33, 0xd1, 0x81, 0xc1])
                || word(bytes, 0x2a) != Some(scale)
                || !at(
                    bytes,
                    0x2e,
                    &[
                        0xc1, 0xfa, 0x1f, 0xf7, 0xd2, 0x23, 0xd1, 0x89, 0x16, 0x5e, 0x5d, 0xc3,
                    ],
                )
            {
                return Err(fail(Problem::Shape));
            }
        }
        let edit = open(&b.cell_edit)?;
        let single_rounding = open(&b.cell_rounding)?;
        if edit.len() != 0x90
            || single_rounding.len() != 0x43
            || !at(edit, 7, &[0x8b, 0x71, 0x20])
            || !at(edit, 0x20, &[0x8b, 0x7e, 0x2c])
            || call(edit, b.cell_edit.va, 0x40) != Some(b.cell_coordinates[0].va)
            || call(edit, b.cell_edit.va, 0x4c) != Some(b.cell_coordinates[1].va)
            || call(edit, b.cell_edit.va, 0x6a) != Some(b.cell_rounding.va)
            || !at(edit, 0x6f, &[0x8b, 0x45, 0x10])
            || !at(edit, 0x74, &[0xf7, 0xd8, 0x1b, 0xc0, 0x25])
            || !word(edit, 0x79).is_some_and(u32::is_power_of_two)
            || !at(single_rounding, 9, &[0x8d, 0x46, 4])
            || !at(single_rounding, 0x34, &[0x89, 0x79, 4])
            || !at(single_rounding, 0x38, &[0x89, 1])
        {
            return Err(fail(Problem::Shape));
        }
        for (f, i) in [(0x11, 0x17), (0x23, 0x29)] {
            if call(single_rounding, b.cell_rounding.va, f) != Some(floor)
                || call(single_rounding, b.cell_rounding.va, i) != Some(integer)
            {
                return Err(fail(Problem::Shape));
            }
        }
        for (index, w) in b.cell_calls.iter().enumerate() {
            let bytes = open(w)?;
            let location = index % 2 != 0;
            let params: &[u8] = if location {
                &[0x8d, 0x4d, 0xfc, 0x51, 0x8d, 0x4d, 0xf8, 0x51, 0x8b, 0xc8]
            } else {
                &[0xff, 0x75, 0x10, 0x8b, 0xc8, 0xff, 0x75, 0x0c]
            };
            if bytes.len() != params.len() + 7
                || !at(bytes, 0, &[0x6a, u8::from(index < 2)])
                || !at(bytes, 2, params)
                || call(bytes, w.va, params.len() + 2) != Some(b.cell_edit.va)
            {
                return Err(fail(Problem::Shape));
            }
        }
        Ok(Self { guard, scale })
    }

    fn coordinate(&self, value: f32, origin: f32) -> Result<i32, real::MathError> {
        let bits = real::sub(real::from_f32(value)?, real::from_f32(origin)?)?;
        let mask = !(((bits.wrapping_add(self.guard) ^ bits) as i32) >> 31) as u32;
        let scaled = f32::from_bits(mask & bits.wrapping_add(self.scale));
        let floored = f64::from(scaled).floor();
        if !floored.is_finite() || floored < f64::from(i32::MIN) || floored >= f64::from(i32::MAX) {
            return Err(real::MathError::Unsupported);
        }
        Ok(floored as i32)
    }

    pub(crate) fn point(
        &self,
        point: [f32; 2],
        origin: [f32; 2],
    ) -> Result<[i32; 2], real::MathError> {
        Ok([
            self.coordinate(point[0], origin[0])?,
            self.coordinate(point[1], origin[1])?,
        ])
    }

    pub(crate) fn cover(
        &self,
        bounds: [f32; 4],
        origin: [f32; 2],
    ) -> Result<[i32; 4], real::MathError> {
        let min = self.point([bounds[0], bounds[1]], origin)?;
        let max = self.point([bounds[2], bounds[3]], origin)?;
        Ok([min[0], min[1], max[0], max[1]])
    }
}

impl NumericPolicy {
    pub fn with_region_cells(
        mut self,
        image: &[u8],
        binding: &Binding,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        self.region_cells = Some(Rules::read(image, binding, &self, budget)?);
        Ok(self)
    }
    pub(crate) fn region_cells(&self) -> Option<&Rules> {
        self.region_cells.as_ref()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn fixture(exponent: u32) -> (Vec<u8>, super::super::Binding, Binding, RuleInputBudget) {
        let (mut image, mut numeric, budget) = super::super::tests::fixture();
        let base = 0x301000u32;
        let mut point = vec![0x90; 0xc8];
        let mut rectangle = vec![0x90; 0xe5];
        let mut rounding = vec![0x90; 0x7f];
        let mut floor = vec![0x90; 152];
        let mut truncate = vec![0x90; 80];
        let put = |bytes: &mut [u8], offset: usize, value: &[u8]| {
            bytes[offset..offset + value.len()].copy_from_slice(value)
        };
        let patch = |bytes: &mut [u8], address: u32, offset: usize, target: u32| {
            bytes[offset] = 0xe8;
            bytes[offset + 1..offset + 5].copy_from_slice(
                &target
                    .wrapping_sub(address + offset as u32 + 5)
                    .to_le_bytes(),
            );
        };
        for bytes in [&mut point, &mut rectangle] {
            put(bytes, 0, &[0xa1, 1, 2, 3, 4]);
        }
        put(&mut point, 5, &[0x83, 0xc0, 0x70]);
        put(&mut point, 0x26, &[0xa1, 1, 2, 3, 4, 0x83, 0xc0, 0x74]);
        for (at, field) in [(9, 0x70), (0x36, 0x74), (0x66, 0x70), (0x96, 0x74)] {
            put(&mut rectangle, at, &[0x83, 0xc0, field]);
        }
        for offset in [0x31, 0x61, 0x91] {
            put(&mut rectangle, offset, &[0xa1, 1, 2, 3, 4]);
        }
        let guard = ((exponent + 1) << 23).wrapping_neg().to_le_bytes();
        let scale = (exponent << 23).wrapping_neg().to_le_bytes();
        for offset in [0x18, 0x4e] {
            put(&mut point, offset, &[0x8d, 0x88]);
            put(&mut point, offset + 2, &guard);
            put(&mut point, offset + 6, &[0x33, 0xc8, 0x8d, 0x90]);
            put(&mut point, offset + 10, &scale);
        }
        put(&mut point, 0x2e, &[0xc1, 0xf9, 0x1f]);
        put(&mut point, 0x35, &[0xf7, 0xd1]);
        put(&mut point, 0x3a, &[0x23, 0xca]);
        put(
            &mut point,
            0x5c,
            &[0xc1, 0xf9, 0x1f, 0xf7, 0xd1, 0x23, 0xca],
        );
        for offset in [0x1d, 0x4d, 0x7d, 0xad] {
            put(&mut rectangle, offset, &[0x8d, 0x88]);
            put(&mut rectangle, offset + 2, &guard);
            put(&mut rectangle, offset + 6, &[0x33, 0xc8, 0x05]);
            put(&mut rectangle, offset + 9, &scale);
            put(
                &mut rectangle,
                offset + 13,
                &[0xc1, 0xf9, 0x1f, 0xf7, 0xd1, 0x23, 0xc8],
            );
        }
        for offset in [0x10, 0x40] {
            patch(&mut point, base + 0x1000, offset, numeric.math_sub.va);
        }
        for offset in [0x15, 0x45, 0x75, 0xa2] {
            patch(&mut rectangle, base + 0x1100, offset, numeric.math_sub.va);
        }
        for (f, i) in [(0x14, 0x1a), (0x2a, 0x30), (0x3f, 0x45), (0x51, 0x57)] {
            patch(&mut rounding, base + 0x1300, f, base + 0x1400);
            patch(&mut rounding, base + 0x1300, i, base + 0x1500);
        }
        patch(&mut point, base + 0x1000, 0xc3, base + 0x1400);
        patch(&mut rectangle, base + 0x1100, 0xe0, base + 0x1300);
        put(&mut rounding, 0x5f, &[0x8d, 0x4f, 1]);
        put(&mut rounding, 0x6f, &[0x41]);
        put(
            &mut floor,
            0,
            &[0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x8b, 0x10],
        );
        put(&mut floor, 0x11, &[0x83, 0xee, 0x7f]);
        put(&mut floor, 0x35, &[0x83, 0xfe, 0x17]);
        put(
            &mut truncate,
            0,
            &[0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0x56, 0x8b, 0x30],
        );
        put(&mut truncate, 0x11, &[0x83, 0xf8, 0x7f]);
        let mut coordinates = [vec![0x90; 0x3a], vec![0x90; 0x3a]];
        for (axis, bytes) in coordinates.iter_mut().enumerate() {
            put(bytes, 0, &[0x55, 0x8b, 0xec, 0xa1, 1, 2, 3, 4]);
            put(bytes, 0xc, &[0x83, 0xc0, 0x70 + axis as u8 * 4]);
            patch(
                bytes,
                base + 0x1600 + axis as u32 * 0x40,
                0x14,
                numeric.math_sub.va,
            );
            put(bytes, 0x20, &[0x8d, 0x91]);
            put(bytes, 0x22, &guard);
            put(bytes, 0x26, &[0x33, 0xd1, 0x81, 0xc1]);
            put(bytes, 0x2a, &scale);
            put(
                bytes,
                0x2e,
                &[
                    0xc1, 0xfa, 0x1f, 0xf7, 0xd2, 0x23, 0xd1, 0x89, 0x16, 0x5e, 0x5d, 0xc3,
                ],
            );
        }
        let mut edit = vec![0x90; 0x90];
        put(&mut edit, 7, &[0x8b, 0x71, 0x20]);
        put(&mut edit, 0x20, &[0x8b, 0x7e, 0x2c]);
        patch(&mut edit, base + 0x1700, 0x40, base + 0x1600);
        patch(&mut edit, base + 0x1700, 0x4c, base + 0x1640);
        patch(&mut edit, base + 0x1700, 0x6a, base + 0x1800);
        put(&mut edit, 0x6f, &[0x8b, 0x45, 0x10]);
        put(&mut edit, 0x74, &[0xf7, 0xd8, 0x1b, 0xc0, 0x25]);
        put(&mut edit, 0x79, &16u32.to_le_bytes());
        let mut single = vec![0x90; 0x43];
        put(&mut single, 9, &[0x8d, 0x46, 4]);
        put(&mut single, 0x34, &[0x89, 0x79, 4]);
        put(&mut single, 0x38, &[0x89, 1]);
        for (f, i) in [(0x11, 0x17), (0x23, 0x29)] {
            patch(&mut single, base + 0x1800, f, base + 0x1400);
            patch(&mut single, base + 0x1800, i, base + 0x1500);
        }
        let mut calls = [
            vec![0x90; 15],
            vec![0x90; 17],
            vec![0x90; 15],
            vec![0x90; 17],
        ];
        for (index, bytes) in calls.iter_mut().enumerate() {
            put(bytes, 0, &[0x6a, u8::from(index < 2)]);
            let params: &[u8] = if index % 2 == 0 {
                &[0xff, 0x75, 0x10, 0x8b, 0xc8, 0xff, 0x75, 0x0c]
            } else {
                &[0x8d, 0x4d, 0xfc, 0x51, 0x8d, 0x4d, 0xf8, 0x51, 0x8b, 0xc8]
            };
            put(bytes, 2, params);
            patch(
                bytes,
                base + 0x1900 + index as u32 * 0x20,
                params.len() + 2,
                base + 0x1700,
            );
        }
        let mut window = |offset: usize, bytes: &[u8]| {
            image[0x200 + offset..0x200 + offset + bytes.len()].copy_from_slice(bytes);
            SourceWindow {
                va: base + offset as u32,
                len: bytes.len() as u32,
                digest: sha256(bytes),
            }
        };
        let mut binding = Binding {
            image_sha256: [0; 32],
            point: window(0x1000, &point),
            rectangle: window(0x1100, &rectangle),
            rounding: window(0x1300, &rounding),
            floor: window(0x1400, &floor),
            truncate: window(0x1500, &truncate),
            cell_coordinates: [
                window(0x1600, &coordinates[0]),
                window(0x1640, &coordinates[1]),
            ],
            cell_edit: window(0x1700, &edit),
            cell_rounding: window(0x1800, &single),
            cell_calls: [
                window(0x1900, &calls[0]),
                window(0x1920, &calls[1]),
                window(0x1940, &calls[2]),
                window(0x1960, &calls[3]),
            ],
        };
        numeric.image_sha256 = sha256(&image);
        binding.image_sha256 = numeric.image_sha256;
        (image, numeric, binding, budget)
    }

    pub(crate) fn policy(exponent: u32) -> NumericPolicy {
        let (image, n, b, budget) = fixture(exponent);
        NumericPolicy::read(&image, n, budget, 4096)
            .unwrap()
            .with_region_cells(&image, &b, budget)
            .unwrap()
    }

    #[test]
    fn derives_pitch_from_both_paths_and_preserves_boundary_floor() {
        for (exponent, expected) in [(3, [7, 8, 9, 9]), (5, [1, 2, 2, 2])] {
            let policy = policy(exponent);
            let rules = policy.region_cells().unwrap();
            assert_eq!(
                rules.cover([-4., 2., 8., 10.], [-64., -64.]).unwrap(),
                expected
            );
            assert_eq!(rules.point([-64.25, -64.], [-64., -64.]).unwrap(), [-1, 0]);
            assert!(rules.point([f32::NAN, 0.], [0., 0.]).is_err());
        }
    }

    #[test]
    fn rejects_mismatched_paths_calls_hashes_and_budget() {
        for offset in [
            0x1000 + 0x18,
            0x1100 + 0x4f,
            0x1300 + 0x15,
            0x1400 + 0x35,
            0x1600 + 0x22,
            0x1640 + 0xd,
            0x1700 + 0x41,
            0x1800 + 0x24,
            0x1900 + 1,
            0x1920 + 3,
            0x1940 + 1,
            0x1960 + 0xd,
        ] {
            let (mut image, mut n, mut b, budget) = fixture(5);
            image[0x200 + offset] ^= 1;
            n.image_sha256 = sha256(&image);
            b.image_sha256 = n.image_sha256;
            for w in [
                &mut b.point,
                &mut b.rectangle,
                &mut b.rounding,
                &mut b.floor,
                &mut b.truncate,
                &mut b.cell_edit,
                &mut b.cell_rounding,
            ]
            .into_iter()
            .chain(b.cell_coordinates.iter_mut())
            .chain(b.cell_calls.iter_mut())
            {
                let start = 0x200 + (w.va - 0x301000) as usize;
                w.digest = sha256(&image[start..start + w.len as usize]);
            }
            let numeric = NumericPolicy::read(&image, n, budget, 4096).unwrap();
            assert!(numeric.with_region_cells(&image, &b, budget).is_err());
        }
        let (image, n, mut b, budget) = fixture(5);
        b.point.digest[0] ^= 1;
        assert!(
            NumericPolicy::read(&image, n.clone(), budget, 4096)
                .unwrap()
                .with_region_cells(&image, &b, budget)
                .is_err()
        );
        let (_, _, b, _) = fixture(5);
        let mut tiny = budget;
        tiny.profile_bytes = 1;
        assert!(
            NumericPolicy::read(&image, n, budget, 4096)
                .unwrap()
                .with_region_cells(&image, &b, tiny)
                .is_err()
        );
    }
}
