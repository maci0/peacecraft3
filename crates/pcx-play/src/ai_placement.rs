//! Source-qualified pieces of the campaign AI's structure-site search.
//!
//! The caller must supply terrain sampling, texture dimensions, object enumeration,
//! reach check and shared building validator. This module alone does
//! not admit an AI construction order.
use std::collections::VecDeque;

use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
    unit_panel_layout::Reader,
};

/// Operands from the reviewed Legacy client, with their input identity.
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    width: usize,
    center: usize,
    step: f32,
    margin: f32,
    good: i8,
    cliff: i8,
    pathing: i8,
    occupied: i8,
    classes: [i8; 4],
    other_radius: i32,
    neighbors: [[i32; 2]; 8],
    half: f32,
    snap_period: i32,
    snap_half: i32,
    snap_odd: f32,
    snap_edge: f32,
}

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |s: String| format!("AI placement {}: {s}", origin.source);
        if image.len() > 32 << 20
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
            || sha256(image) != REVIEWED_IMAGE
        {
            return Err(fail("unsupported client identity".into()));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(e.to_string()))?;
        Self::decode(origin.clone(), |va, len, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated operand at {va:#x}"))
        })
        .map_err(fail)
    }

    fn decode(
        origin: AssetIdentity,
        read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut r = Reader(read);
        let width = r.word(0xad3de9, &[0xb9])? as usize;
        let center = r.word(0xad3f95, &[0xc7, 0x83, 0xb0, 0x55, 0, 0])? as usize;
        if r.word(0xad3f9f, &[0xc7, 0x83, 0x18, 0xf5, 0, 0])? as usize != center {
            return Err("different grid center operands".into());
        }
        let step_bytes = r.bytes(0x40a910, 2, true)?;
        if step_bytes[0] != 0x6a {
            return Err("grid step instruction".into());
        }
        let step = f32::from(step_bytes[1]);
        let margin = r.word(0x4161a0, &[0x68])? as f32;
        let byte = |r: &mut Reader<_>, va, prefix: &[u8]| -> Result<u8, String> {
            let b = r.bytes(va, prefix.len() + 1, true)?;
            if !b.starts_with(prefix) {
                return Err(format!("instruction at {va:#x}"));
            }
            Ok(b[prefix.len()])
        };
        let cliff = byte(&mut r, 0xad40d6, &[0xc6, 4, 0x33])? as i8;
        let mask = byte(&mut r, 0xad40f4, &[0x24])?;
        let good = byte(&mut r, 0xad40f6, &[0x04])?;
        let pathing = good.wrapping_add(mask) as i8;
        let occupied = byte(&mut r, 0xad4aea, &[0xc6, 0x44, 0x18, 0x2c])? as i8;
        let mut classes = [0; 4];
        for (slot, va) in [0xad4c4a, 0xad4c7a, 0xad4c86, 0xad4c5a]
            .into_iter()
            .enumerate()
        {
            classes[slot] =
                i8::try_from(r.word(va, &[0xc7, 0x46, 4])?).map_err(|_| "class operand")?;
        }
        let other_radius =
            i32::try_from(r.word(0xad4c92, &[0xb8])?).map_err(|_| "radius operand")?;
        let west = byte(&mut r, 0xad458a, &[0x8d, 0x4b])? as i8 as i32;
        let south = byte(&mut r, 0xad45e3, &[0x8d, 0x46])? as i8 as i32;
        let north = byte(&mut r, 0xad4643, &[0x8d, 0x46])? as i8 as i32;
        let east = byte(&mut r, 0xad46a2, &[0x8d, 0x4b])? as i8 as i32;
        let half_bytes = r.bytes(0x10db0a8, 4, false)?;
        let half = f32::from_le_bytes(half_bytes.try_into().unwrap());
        let remainder_mask = r.word(0x4e01a1, &[0x81, 0xe1])? & 0x7fff_ffff;
        let shift = byte(&mut r, 0x4e01cf, &[0xc1, 0xe0])?;
        if byte(&mut r, 0x4e01e6, &[0xc1, 0xe6])? != shift || shift >= 30 {
            return Err("snap alignment shifts differ".into());
        }
        let snap_period = i32::try_from(remainder_mask.checked_add(1).ok_or("snap period")?)
            .map_err(|_| "snap period")?;
        let snap_half = 1_i32 << shift;
        let snap_odd = f32::from(byte(&mut r, 0x403b40, &[0x6a])?);
        let snap_edge = f32::from(byte(&mut r, 0x4015b0, &[0x6a])?);
        if width == 0
            || width > 4096
            || center >= width
            || step <= 0.
            || !half.is_finite()
            || west >= 0
            || south >= 0
            || east <= 0
            || north <= 0
            || margin != center as f32 * step
            || width != center * 2 + 1
            || classes.iter().any(|v| *v < 0 || *v >= good as i8)
            || snap_period != snap_half * 2
            || snap_odd <= 0.
            || snap_edge <= 0.
        {
            return Err("inconsistent search operands".into());
        }
        Ok(Self {
            origin,
            image_sha256: REVIEWED_IMAGE,
            width,
            center,
            step,
            margin,
            good: good as i8,
            cliff,
            pathing,
            occupied,
            classes,
            other_radius,
            neighbors: [
                [west, 0],
                [west, south],
                [west, north],
                [east, 0],
                [east, south],
                [east, north],
                [0, south],
                [0, north],
            ],
            half,
            snap_period,
            snap_half,
            snap_odd,
            snap_edge,
        })
    }

    /// Snap a point using the selected texture dimensions and map bounds.
    pub fn snap(
        &self,
        point: [f32; 2],
        bounds: [[f32; 2]; 2],
        dimensions: [u32; 2],
        quarter: u8,
    ) -> Result<[f32; 2], String> {
        use crate::script_random::real;
        if quarter > 3
            || dimensions.contains(&0)
            || point
                .iter()
                .chain(bounds.iter().flatten())
                .any(|v| real::from_f32(*v).is_err())
        {
            return Err("invalid AI placement snap inputs".into());
        }
        let dimensions = if quarter.is_multiple_of(2) {
            dimensions
        } else {
            [dimensions[1], dimensions[0]]
        };
        let mut result = [0.; 2];
        for axis in 0..2 {
            let upper = f32::from_bits(
                real::sub(
                    real::from_f32(bounds[1][axis]).map_err(|e| e.to_string())?,
                    real::from_f32(self.snap_edge).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
            );
            if upper < bounds[0][axis] || !upper.is_finite() {
                return Err("invalid AI placement map bounds".into());
            }
            let value = point[axis].clamp(bounds[0][axis], upper);
            if value <= i32::MIN as f32 || value >= i32::MAX as f32 {
                return Err("AI placement snap coordinate exceeds integer range".into());
            }
            let integer = value as i32;
            let base = integer - integer % self.snap_period;
            let direction = if value < 0. { -1 } else { 1 };
            let parity = ((dimensions[axis] >> 1) & 1) as i32;
            let aligned = base
                .checked_add(direction * parity * self.snap_half)
                .ok_or("AI placement snap overflow")?;
            let offset = if !dimensions[axis].is_multiple_of(2) {
                direction as f32 * self.snap_odd
            } else {
                0.
            };
            result[axis] = f32::from_bits(
                real::add(
                    real::from_i32(aligned).map_err(|e| e.to_string())?,
                    real::from_f32(offset).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        Ok(result)
    }

    /// Refresh the coordinate stored on a town record.
    pub fn town_center(
        &self,
        previous: [f32; 2],
        mine: Option<[f32; 2]>,
        hall: Option<[f32; 2]>,
    ) -> Result<[f32; 2], String> {
        let center = match (mine, hall) {
            (Some(a), Some(b)) => [(a[0] + b[0]) * self.half, (a[1] + b[1]) * self.half],
            (Some(p), None) | (None, Some(p)) => p,
            (None, None) => previous,
        };
        if center.iter().all(|v| v.is_finite()) {
            Ok(center)
        } else {
            Err("AI town coordinate is not finite".into())
        }
    }
}

/// Category of a nearby object's selected `buffType`.
#[derive(Clone, Copy, Debug)]
pub enum BuffType {
    TownHall,
    Resource,
    Factory,
    Buffer,
    Other,
}

/// The sampled score raster. Points passed to callbacks are world coordinates.
pub struct Grid<'a> {
    rules: &'a Rules,
    origin: [f32; 2],
    scores: Vec<i8>,
}

impl<'a> Grid<'a> {
    /// Sampling must implement the client's cliff and walkability lookup, including edges.
    pub fn sample(
        rules: &'a Rules,
        center: [f32; 2],
        mut sample: impl FnMut([f32; 2]) -> Result<(i32, bool), String>,
    ) -> Result<Self, String> {
        if center.iter().any(|v| !v.is_finite()) {
            return Err("AI placement center is not finite".into());
        }
        let base = sample(center)?.0;
        let origin = [center[0] - rules.margin, center[1] - rules.margin];
        let mut scores = Vec::with_capacity(rules.width * rules.width);
        for y in 0..rules.width {
            for x in 0..rules.width {
                let p = [
                    origin[0] + x as f32 * rules.step,
                    origin[1] + y as f32 * rules.step,
                ];
                let (cliff, blocked) = sample(p)?;
                scores.push(if cliff != base {
                    rules.cliff
                } else if blocked {
                    rules.pathing
                } else {
                    rules.good
                });
            }
        }
        Ok(Self {
            rules,
            origin,
            scores,
        })
    }

    /// Apply a single object's complete pathing-texture rectangle, expressed in grid indices.
    /// For the special buffer override, the caller must first establish the native count branch.
    pub fn score_rectangle(
        &mut self,
        cells: impl IntoIterator<Item = [i32; 2]>,
        kind: BuffType,
        radius: i32,
    ) -> Result<(), String> {
        let (cap, radius) = match kind {
            BuffType::TownHall => (self.rules.classes[0], radius),
            BuffType::Resource => (self.rules.classes[1], radius),
            BuffType::Factory => (self.rules.classes[2], radius),
            BuffType::Buffer => (self.rules.classes[3], radius),
            BuffType::Other => (self.rules.classes[3], self.rules.other_radius),
        };
        if radius < 0 {
            return Err("negative AI placement buffer radius".into());
        }
        let width = self.rules.width as i32;
        for [x, y] in cells {
            if (0..width).contains(&x) && (0..width).contains(&y) {
                let i = y as usize * self.rules.width + x as usize;
                self.scores[i] = self.scores[i].min(self.rules.occupied);
            }
            let left = x.saturating_sub(radius).max(0);
            let right = x.saturating_add(radius).min(width - 1);
            let bottom = y.saturating_sub(radius).max(0);
            let top = y.saturating_add(radius).min(width - 1);
            for yy in bottom..=top {
                for xx in left..=right {
                    let i = yy as usize * self.rules.width + xx as usize;
                    self.scores[i] = self.scores[i].min(cap);
                }
            }
        }
        Ok(())
    }

    /// Search twice only when the additional check rejected a candidate in a failed first pass.
    /// Snapping must return the snapped world point and its corresponding grid indices.
    pub fn search(
        &self,
        mut snap: impl FnMut([f32; 2]) -> Result<([f32; 2], [i32; 2]), String>,
        mut reach: impl FnMut([f32; 2]) -> Result<bool, String>,
        mut valid: impl FnMut([f32; 2]) -> Result<bool, String>,
    ) -> Result<Option<[f32; 2]>, String> {
        let mut retry = false;
        for use_reach in [true, false] {
            if !use_reach && !retry {
                break;
            }
            let mut visited = vec![false; self.scores.len()];
            let mut checked = vec![false; self.scores.len()];
            let mut queue = VecDeque::from([[self.rules.center as i32; 2]]);
            let mut retained: [Option<[f32; 2]>; 4] = [None; 4];
            while let Some([x, y]) = queue.pop_front() {
                let point = [
                    self.origin[0] + x as f32 * self.rules.step,
                    self.origin[1] + y as f32 * self.rules.step,
                ];
                let (point, [sx, sy]) = snap(point)?;
                let width = self.rules.width as i32;
                if (0..width).contains(&sx) && (0..width).contains(&sy) {
                    let i = sy as usize * self.rules.width + sx as usize;
                    let score = self.scores[i];
                    let class = self.rules.classes.iter().position(|v| *v == score);
                    if !checked[i]
                        && (score == self.rules.good
                            || class.is_some_and(|c| retained[c].is_none()))
                    {
                        if use_reach && !reach(point)? {
                            retry = true;
                        } else if valid(point)? {
                            if score == self.rules.good {
                                return Ok(Some(point));
                            }
                            if let Some(c) = class {
                                retained[c] = Some(point);
                            }
                        }
                    }
                }
                for [dx, dy] in self.rules.neighbors {
                    let (nx, ny) = (x + dx, y + dy);
                    if (0..width).contains(&nx) && (0..width).contains(&ny) {
                        let i = ny as usize * self.rules.width + nx as usize;
                        if !visited[i] && !checked[i] {
                            visited[i] = true;
                            queue.push_back([nx, ny]);
                        }
                    }
                }
                if (0..width).contains(&sx) && (0..width).contains(&sy) {
                    checked[sy as usize * self.rules.width + sx as usize] = true;
                }
            }
            if let Some(point) = retained.into_iter().rev().flatten().next() {
                return Ok(Some(point));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules {
            origin: AssetIdentity {
                source: "fixture".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            image_sha256: REVIEWED_IMAGE,
            width: 5,
            center: 2,
            step: 7.,
            margin: 14.,
            good: 8,
            cliff: -30,
            pathing: -40,
            occupied: -50,
            classes: [0, 1, 2, 3],
            other_radius: 2,
            neighbors: [
                [-1, 0],
                [-1, -1],
                [-1, 1],
                [1, 0],
                [1, -1],
                [1, 1],
                [0, -1],
                [0, 1],
            ],
            half: 0.5,
            snap_period: 16,
            snap_half: 8,
            snap_odd: 4.,
            snap_edge: 8.,
        }
    }

    fn snap(p: [f32; 2]) -> Result<([f32; 2], [i32; 2]), String> {
        Ok((p, [(p[0] / 7.) as i32, (p[1] / 7.) as i32]))
    }

    #[test]
    fn town_refresh_retains_or_averages_resolved_positions() {
        let r = rules();
        assert_eq!(r.town_center([1., 2.], None, None).unwrap(), [1., 2.]);
        assert_eq!(
            r.town_center([1., 2.], Some([4., 6.]), None).unwrap(),
            [4., 6.]
        );
        assert_eq!(
            r.town_center([1., 2.], Some([4., 6.]), Some([10., 16.]))
                .unwrap(),
            [7., 11.]
        );
        assert!(r.town_center([f32::NAN, 0.], None, None).is_err());
    }

    #[test]
    fn snapping_uses_signed_alignment_dimensions_rotation_and_bounds() {
        let r = rules();
        let bounds = [[-100., -100.], [100., 100.]];
        assert_eq!(r.snap([19., -19.], bounds, [4, 2], 0).unwrap(), [16., -24.]);
        assert_eq!(r.snap([19., -19.], bounds, [4, 2], 1).unwrap(), [24., -16.]);
        assert_eq!(r.snap([19., -19.], bounds, [3, 1], 0).unwrap(), [28., -20.]);
        assert_eq!(
            r.snap([200., -200.], bounds, [4, 4], 0).unwrap(),
            [80., -96.]
        );
        assert!(r.snap([0., 0.], bounds, [0, 2], 0).is_err());
        assert!(r.snap([0., 0.], bounds, [2, 2], 4).is_err());
        let mut r = r;
        r.snap_odd = 3.;
        assert_eq!(
            r.snap(
                [16_777_216., 0.],
                [[0., 0.], [33_554_432., 100.]],
                [1, 1],
                0
            )
            .unwrap()[0],
            16_777_216.
        );
    }

    #[test]
    fn grid_samples_cliffs_and_pathing_and_clips_object_buffers() {
        let r = rules();
        let mut grid = Grid::sample(&r, [14., 14.], |p| {
            Ok((i32::from(p == [0., 0.]), p == [7., 0.]))
        })
        .unwrap();
        assert_eq!(&grid.scores[..3], &[-30, -40, 8]);
        grid.score_rectangle([[1, 1], [2, 1]], BuffType::Resource, 1)
            .unwrap();
        assert_eq!(grid.scores[6], -50);
        assert_eq!(grid.scores[7], -50);
        assert_eq!(grid.scores[2], 1);
        assert_eq!(grid.scores[0], -30);
        grid.score_rectangle([[-1, 4]], BuffType::Factory, 1)
            .unwrap();
        assert_eq!(grid.scores[20], 2);
        assert!(
            grid.score_rectangle([[0, 0]], BuffType::Buffer, -1)
                .is_err()
        );
    }

    #[test]
    fn traversal_and_retained_selection_follow_queue_order() {
        let r = rules();
        let mut grid = Grid::sample(&r, [14., 14.], |_| Ok((0, false))).unwrap();
        let mut seen = Vec::new();
        let result = grid
            .search(
                snap,
                |_| Ok(true),
                |p| {
                    seen.push(p);
                    Ok(p == [7., 7.])
                },
            )
            .unwrap();
        assert_eq!(result, Some([7., 7.]));
        assert_eq!(seen, [[14., 14.], [7., 14.], [7., 7.]]);
        grid.scores.fill(-50);
        grid.scores[2 * 5 + 1] = 1;
        grid.scores[6] = 3;
        grid.scores[3 * 5 + 1] = 3;
        assert_eq!(
            grid.search(snap, |_| Ok(true), |_| Ok(true)).unwrap(),
            Some([7., 7.])
        );
    }

    #[test]
    fn retry_occurs_only_after_reach_rejection_and_failure() {
        let r = rules();
        let mut grid = Grid::sample(&r, [14., 14.], |_| Ok((0, false))).unwrap();
        grid.scores.fill(-50);
        grid.scores[12] = r.good;
        let mut reach_calls = 0;
        let found = grid
            .search(
                snap,
                |_| {
                    reach_calls += 1;
                    Ok(false)
                },
                |_| Ok(true),
            )
            .unwrap();
        assert_eq!(found, Some([14., 14.]));
        assert_eq!(reach_calls, 1);
        let mut valid_calls = 0;
        let result = grid
            .search(
                snap,
                |_| Ok(true),
                |_| {
                    valid_calls += 1;
                    Ok(false)
                },
            )
            .unwrap();
        assert_eq!(result, None);
        assert_eq!(valid_calls, 1);
        assert!(
            grid.search(snap, |_| Err("unknown reach".into()), |_| Ok(true))
                .unwrap_err()
                .contains("unknown reach")
        );
    }

    #[test]
    fn snapped_cells_are_checked_only_once() {
        let r = rules();
        let grid = Grid::sample(&r, [14., 14.], |_| Ok((0, false))).unwrap();
        let mut calls = 0;
        let found = grid
            .search(
                |_| Ok(([7., 7.], [1, 1])),
                |_| Ok(true),
                |_| {
                    calls += 1;
                    Ok(false)
                },
            )
            .unwrap();
        assert_eq!(found, None);
        assert_eq!(calls, 1);
    }

    #[test]
    fn unknown_client_is_rejected() {
        assert!(Rules::reviewed_legacy(&[], rules().origin).is_err());
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE from the reviewed local install"]
    fn reviewed_executable_operands_are_admitted() {
        let path = std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE");
        let image = std::fs::read(path).unwrap();
        Rules::reviewed_legacy(&image, rules().origin).unwrap();
    }
}
