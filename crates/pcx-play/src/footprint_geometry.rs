//! Footprint placement geometry decoded from the reviewed executable: the position snap
//! (`0x4e00f0`), the `Misc/BuildingAngle` quarter (`0x46dea0`, `0x687ca0`), the facing
//! quarter of a created unit's registered footprint (`0x4c6ab0`) and the texel traversal
//! (`0x4dfb40`). Evidence and open items: `docs/re-notes/chapter-startup-refusals.md`.
//!
//! Texture dimensions are the native pair `[+0x8, +0xc]`: the TGA height, then the TGA width
//! (`0x4deb67` stores the texture transposed). The first native axis is world x.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
    script_random::real,
    unit_panel_layout::Reader,
};

/// Position alignment of a footprint, from the operands of `0x4e00f0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Snap {
    period: i32,
    half: i32,
    odd: f32,
    edge: f32,
}

impl Snap {
    pub(crate) fn from_operands(
        remainder_mask: u32,
        shift: u8,
        odd: f32,
        edge: f32,
    ) -> Result<Self, String> {
        let period = i32::try_from(remainder_mask.checked_add(1).ok_or("snap period")?)
            .map_err(|_| "snap period")?;
        if shift >= 30 || period != 1_i32 << (shift + 1) || odd <= 0. || edge <= 0. {
            return Err("inconsistent snap operands".into());
        }
        Ok(Self {
            period,
            half: 1 << shift,
            odd,
            edge,
        })
    }

    pub(crate) fn read(
        r: &mut Reader<impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>>,
    ) -> Result<Self, String> {
        let byte = |r: &mut Reader<_>, va, prefix: &[u8]| -> Result<u8, String> {
            let b = r.bytes(va, prefix.len() + 1, true)?;
            if !b.starts_with(prefix) {
                return Err(format!("instruction at {va:#x}"));
            }
            Ok(b[prefix.len()])
        };
        let remainder_mask = r.word(0x4e01a1, &[0x81, 0xe1])? & 0x7fff_ffff;
        let shift = byte(r, 0x4e01cf, &[0xc1, 0xe0])?;
        if byte(r, 0x4e01e6, &[0xc1, 0xe6])? != shift {
            return Err("snap alignment shifts differ".into());
        }
        let odd = f32::from(byte(r, 0x403b40, &[0x6a])?);
        let edge = f32::from(byte(r, 0x4015b0, &[0x6a])?);
        Self::from_operands(remainder_mask, shift, odd, edge)
    }

    /// The offset added toward the coordinate's sign for an odd texture dimension.
    pub(crate) fn odd(self) -> f32 {
        self.odd
    }

    /// Snap a point: clamp to `bounds` (the upper edge less the margin), truncate to the
    /// period, add the half period when bit 1 of the axis dimension is set and the odd
    /// offset when it is odd. Both offsets take the coordinate's sign; zero is positive.
    pub(crate) fn apply(
        self,
        point: [f32; 2],
        bounds: [[f32; 2]; 2],
        dimensions: [u32; 2],
        quarter: u8,
    ) -> Result<[f32; 2], String> {
        if quarter > 3
            || dimensions.contains(&0)
            || point
                .iter()
                .chain(bounds.iter().flatten())
                .any(|v| real::from_f32(*v).is_err())
        {
            return Err("invalid footprint snap inputs".into());
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
                    real::from_f32(self.edge).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
            );
            if upper < bounds[0][axis] || !upper.is_finite() {
                return Err("invalid footprint snap map bounds".into());
            }
            let value = point[axis].clamp(bounds[0][axis], upper);
            if value <= i32::MIN as f32 || value >= i32::MAX as f32 {
                return Err("footprint snap coordinate exceeds integer range".into());
            }
            let integer = value as i32;
            let base = integer - integer % self.period;
            let direction = if value < 0. { -1 } else { 1 };
            let parity = ((dimensions[axis] >> 1) & 1) as i32;
            let aligned = base
                .checked_add(direction * parity * self.half)
                .ok_or("footprint snap overflow")?;
            let offset = if !dimensions[axis].is_multiple_of(2) {
                direction as f32 * self.odd
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
}

/// Quarter of a footprint from an angle in radians, as `0x4c6ab0` classifies the facing
/// stored on the created unit: within `tolerance` of 0, a quarter turn or a half turn
/// selects 0, 1 or 2; every other angle selects 3.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FacingClassifier {
    degrees_to_radians: f32,
    quarter_turn: f32,
    half_turn: f32,
    tolerance: f32,
}

/// Decoded footprint geometry for the selected client and Misc data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootprintGeometry {
    snap: Snap,
    building_quarter: u8,
    facing: FacingClassifier,
}

/// Degrees in a full turn and in a quarter turn, as `0x46dea0` reduces `BuildingAngle`.
struct Turns {
    full: i32,
    quarter: i32,
}

impl Turns {
    fn read(
        r: &mut Reader<impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>>,
    ) -> Result<Self, String> {
        let full = r.word(0x46dec3, &[0xb9])?;
        let quarter = r.bytes(0x401740, 2, true)?;
        if quarter[0] != 0x6a {
            return Err("quarter-turn initializer".into());
        }
        let (full, quarter) = (
            i32::try_from(full).map_err(|_| "full-turn operand")?,
            i32::from(quarter[1]),
        );
        if quarter <= 0 || full != 4 * quarter {
            return Err("inconsistent turn operands".into());
        }
        Ok(Self { full, quarter })
    }

    /// `0x46dea0` then `0x687ca0`: truncate to an integer, reduce modulo a full turn, round
    /// toward zero to a multiple of a quarter turn, add a full turn when negative.
    fn building_quarter(&self, angle: f32) -> Result<u8, String> {
        if !angle.is_finite() || angle <= i32::MIN as f32 || angle >= i32::MAX as f32 {
            return Err("BuildingAngle is not a representable angle".into());
        }
        let reduced = (angle as i32) % self.full;
        let mut rounded = reduced / self.quarter * self.quarter;
        if rounded < 0 {
            rounded += self.full;
        }
        u8::try_from(rounded / self.quarter).map_err(|_| "BuildingAngle quarter".to_string())
    }
}

impl FootprintGeometry {
    /// Read the operands from the reviewed image and `Misc/BuildingAngle` from the profiles
    /// (stock first, map last). Both must be present.
    pub fn read(
        image: &[u8],
        profiles: &[(&str, &[u8])],
        profile_bytes: usize,
    ) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != REVIEWED_IMAGE {
            return Err("footprint geometry: unsupported client identity".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        let mut r = Reader(|va, len, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated operand at {va:#x}"))
        });
        let turns = Turns::read(&mut r)?;
        let mut real_at = |va| -> Result<f32, String> {
            let value = f32::from_le_bytes(r.bytes(va, 4, false)?.try_into().unwrap());
            if value.is_finite() && value > 0. {
                Ok(value)
            } else {
                Err(format!("facing constant at {va:#x}"))
            }
        };
        let facing = FacingClassifier {
            degrees_to_radians: real_at(0x10db0f8)?,
            quarter_turn: real_at(0x10db10c)?,
            half_turn: real_at(0x10db110)?,
            tolerance: real_at(0x10db054)?,
        };
        let snap = Snap::read(&mut r)?;
        let ([angle], _) =
            pcx_objects::editor::misc::misc_scalars(profiles, ["BuildingAngle"], profile_bytes)?;
        let angle = angle.ok_or("missing Misc/BuildingAngle")?;
        Ok(Self {
            snap,
            building_quarter: turns.building_quarter(angle)?,
            facing,
        })
    }

    /// The quarter `0x687ca0` takes from `Misc/BuildingAngle` for the creation snap.
    pub fn building_quarter(&self) -> u8 {
        self.building_quarter
    }

    pub(crate) fn snap(
        &self,
        point: [f32; 2],
        bounds: [[f32; 2]; 2],
        dimensions: [u32; 2],
        quarter: u8,
    ) -> Result<[f32; 2], String> {
        self.snap.apply(point, bounds, dimensions, quarter)
    }

    /// The quarter of a created unit's registered footprint from its facing in degrees.
    /// The facing is stored unnormalized as `degrees * (pi / 180)` (`0x48ee00`); how the
    /// pose normalizes an angle outside one turn is not decoded, so such a facing is refused.
    pub fn facing_quarter(&self, degrees: f32) -> Result<u8, String> {
        if !(0.0..360.0).contains(&degrees) {
            return Err("facing outside one turn: pose normalization is not decoded".into());
        }
        let FacingClassifier {
            degrees_to_radians,
            quarter_turn,
            half_turn,
            tolerance,
        } = self.facing;
        let radians = real::mul(
            real::from_f32(degrees).map_err(|e| e.to_string())?,
            real::from_f32(degrees_to_radians).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let near = |target: f32| -> Result<bool, String> {
            let gap = real::sub(radians, real::from_f32(target).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            Ok(tolerance > f32::from_bits(real::abs(gap)))
        };
        Ok(if near(0.)? {
            0
        } else if near(quarter_turn)? {
            1
        } else if near(half_turn)? {
            2
        } else {
            3
        })
    }
}

/// Cell offset of one texel from the footprint centre after `quarter` quarter turns.
///
/// `cell` is the unrotated offset `[row - dimensions[0] / 2, column - dimensions[1] / 2]` in
/// native axes (integer division). The texel centre lies `(index - (dimension - 1) / 2)` cells
/// from the centre: a whole cell for an odd dimension, a half cell for an even one. Rotation
/// acts on those centres, `0x4dfb40`'s four step tables, and the cell is the floor of the
/// rotated centre. The centre is at a cell corner for an even snap axis and at a cell centre
/// for an odd one, which `0x4e00f0` establishes.
pub(crate) fn rotate_offset(cell: [i32; 2], dimensions: [u32; 2], quarter: u8) -> [i32; 2] {
    let doubled = |offset: i32, dimension: u32| {
        2 * i64::from(offset) + i64::from(dimension.is_multiple_of(2))
    };
    let (a, b) = (
        doubled(cell[0], dimensions[0]),
        doubled(cell[1], dimensions[1]),
    );
    let (x, y) = match quarter {
        0 => (a, b),
        1 => (-b, a),
        2 => (-a, -b),
        _ => (b, -a),
    };
    [x, y].map(|v| v.div_euclid(2) as i32)
}

/// The grid cell a stationary footprint is anchored on. An even axis rounds the position to the
/// nearest cell corner. An odd axis is centred on a cell: the position is first snapped by
/// `0x4e00f0` (`0x6bd2a0` snaps every widget, destructables included, before it walks the
/// texture), and the anchor is the cell containing the snapped coordinate.
pub(crate) fn stationary_anchor(
    geometry: Option<&FootprintGeometry>,
    bounds: [[f32; 2]; 2],
    size: f32,
    point: [f32; 2],
    dimensions: [u32; 2],
    quarter: u8,
) -> Result<[i64; 2], String> {
    let axis_dimensions = if quarter.is_multiple_of(2) {
        dimensions
    } else {
        [dimensions[1], dimensions[0]]
    };
    let odd = axis_dimensions.map(|dimension| !dimension.is_multiple_of(2));
    let mut point = point;
    if odd.contains(&true) {
        let snapped = geometry
            .ok_or("odd-dimension footprint requires the decoded snap")?
            .snap(point, bounds, dimensions, quarter)?;
        for axis in 0..2 {
            if odd[axis] {
                point[axis] = snapped[axis];
            }
        }
    }
    let mut anchor = [0; 2];
    for axis in 0..2 {
        let cell = (f64::from(point[axis]) - f64::from(bounds[0][axis])) / f64::from(size);
        let cell = if odd[axis] {
            cell.floor()
        } else {
            cell.round_ties_even()
        };
        if cell.abs() > f64::from(i32::MAX) {
            return Err("stationary cell range".into());
        }
        anchor[axis] = cell as i64;
    }
    Ok(anchor)
}

/// Clamp a texel cell into a `width` by `height` grid: `0x804be0` clamps the texel position
/// to the map rectangle (`0x7f4a60`) before converting it to a cell, so a texel outside the
/// map is linked to the nearest border cell.
pub(crate) fn clamp_cell(cell: [i64; 2], width: u32, height: u32) -> [i64; 2] {
    [
        cell[0].clamp(0, i64::from(width.saturating_sub(1))),
        cell[1].clamp(0, i64::from(height.saturating_sub(1))),
    ]
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn geometry() -> FootprintGeometry {
        with_building_quarter(3)
    }

    /// The reviewed client's operands with a chosen `BuildingAngle` quarter.
    pub(crate) fn with_building_quarter(building_quarter: u8) -> FootprintGeometry {
        FootprintGeometry {
            snap: Snap::from_operands(63, 5, 16., 32.).unwrap(),
            building_quarter,
            facing: FacingClassifier {
                degrees_to_radians: 0.017453292,
                quarter_turn: std::f32::consts::FRAC_PI_2,
                half_turn: std::f32::consts::PI,
                tolerance: 0.001,
            },
        }
    }

    const BOUNDS: [[f32; 2]; 2] = [[-1024., -1024.], [1024., 1024.]];

    #[test]
    fn building_angle_reduces_like_the_native_routine() {
        let turns = Turns {
            full: 360,
            quarter: 90,
        };
        let quarter = |angle| turns.building_quarter(angle).unwrap();
        assert_eq!(quarter(270.), 3);
        assert_eq!(quarter(0.), 0);
        assert_eq!(quarter(90.), 1);
        assert_eq!(quarter(180.), 2);
        assert_eq!(quarter(-90.), 3);
        assert_eq!(quarter(450.), 1);
        assert_eq!(quarter(359.9), 3);
        assert_eq!(quarter(100.7), 1);
        assert_eq!(quarter(-45.), 0);
        assert!(turns.building_quarter(f32::NAN).is_err());
        assert!(turns.building_quarter(1e12).is_err());
    }

    #[test]
    fn turn_operands_must_agree() {
        let mut r = Reader(|va, len, _| {
            Ok(match va {
                0x46dec3 => vec![0xb9, 0x68, 0x01, 0, 0],
                _ => vec![0x6a, 0x5b][..len].to_vec(),
            })
        });
        assert!(Turns::read(&mut r).is_err());
        let mut r = Reader(|va, len, _| {
            Ok(match va {
                0x46dec3 => vec![0xb9, 0x68, 0x01, 0, 0],
                _ => vec![0x6a, 0x5a][..len].to_vec(),
            })
        });
        let turns = Turns::read(&mut r).unwrap();
        assert_eq!((turns.full, turns.quarter), (360, 90));
    }

    #[test]
    fn facing_quarter_is_exact_or_three() {
        let g = geometry();
        assert_eq!(g.facing_quarter(0.).unwrap(), 0);
        assert_eq!(g.facing_quarter(90.).unwrap(), 1);
        assert_eq!(g.facing_quarter(180.).unwrap(), 2);
        assert_eq!(g.facing_quarter(270.).unwrap(), 3);
        assert_eq!(g.facing_quarter(45.).unwrap(), 3);
        assert_eq!(g.facing_quarter(92.).unwrap(), 3);
        assert_eq!(g.facing_quarter(0.04).unwrap(), 0);
        assert_eq!(g.facing_quarter(359.9).unwrap(), 3);
        for outside in [-90., 360., 450., f32::NAN, f32::INFINITY] {
            assert!(g.facing_quarter(outside).is_err(), "{outside}");
        }
    }

    #[test]
    fn snap_aligns_even_and_odd_dimensions() {
        let g = geometry();
        let at = |p, dims, q| g.snap(p, BOUNDS, dims, q).unwrap();
        // Dimension bit 1 clear: multiples of 64. Bit 1 set: half a period further.
        assert_eq!(at([100., 100.], [4, 4], 0), [64., 64.]);
        assert_eq!(at([100., 100.], [2, 2], 0), [96., 96.]);
        // Odd dimensions add 16 toward the sign; zero is positive.
        assert_eq!(at([100., 100.], [5, 5], 0), [80., 80.]);
        assert_eq!(at([-100., -100.], [5, 5], 0), [-80., -80.]);
        assert_eq!(at([0., 0.], [1, 1], 0), [16., 16.]);
        assert_eq!(at([-0.5, 0.], [1, 1], 0), [-16., 16.]);
        // 3: bit 1 set and odd.
        assert_eq!(at([70., 70.], [3, 3], 0), [112., 112.]);
        // The first axis takes the first dimension for even quarters, the second for odd.
        assert_eq!(at([100., 100.], [2, 5], 0), [96., 80.]);
        assert_eq!(at([100., 100.], [2, 5], 1), [80., 96.]);
        assert_eq!(at([100., 100.], [2, 5], 2), [96., 80.]);
        assert_eq!(at([100., 100.], [2, 5], 3), [80., 96.]);
    }

    #[test]
    fn snap_clamps_before_aligning() {
        let g = geometry();
        // The upper bound is the map edge less the margin 32.
        assert_eq!(
            g.snap([5000., -5000.], BOUNDS, [4, 4], 0).unwrap(),
            [960., -1024.]
        );
        assert!(g.snap([0., 0.], BOUNDS, [0, 2], 0).is_err());
        assert!(g.snap([0., 0.], BOUNDS, [2, 2], 4).is_err());
        assert!(g.snap([f32::NAN, 0.], BOUNDS, [2, 2], 0).is_err());
        assert!(g.snap([0., 0.], [[0., 0.], [16., 16.]], [2, 2], 0).is_err());
    }

    #[test]
    fn odd_dimension_rotation_keeps_whole_cell_offsets() {
        // Odd by odd (3 x 3): offsets are whole cells and rotate exactly.
        let d = [3, 3];
        assert_eq!(rotate_offset([-1, 0], d, 0), [-1, 0]);
        assert_eq!(rotate_offset([-1, 0], d, 1), [0, -1]);
        assert_eq!(rotate_offset([-1, 0], d, 2), [1, 0]);
        assert_eq!(rotate_offset([-1, 0], d, 3), [0, 1]);
        assert_eq!(rotate_offset([1, 1], d, 1), [-1, 1]);
    }

    #[test]
    fn even_dimension_rotation_matches_the_corner_formulas() {
        let d = [4, 4];
        for x in -2..2 {
            for y in -2..2 {
                assert_eq!(rotate_offset([x, y], d, 0), [x, y]);
                assert_eq!(rotate_offset([x, y], d, 1), [-y - 1, x]);
                assert_eq!(rotate_offset([x, y], d, 2), [-x - 1, -y - 1]);
                assert_eq!(rotate_offset([x, y], d, 3), [y, -x - 1]);
            }
        }
    }

    #[test]
    fn mixed_parity_rotation_floors_per_axis() {
        // 2 x 3: first axis even (half-cell centres), second odd (whole-cell centres).
        let d = [2, 3];
        // Centres of the first axis are -0.5 and +0.5 cells; of the second -1, 0, 1.
        assert_eq!(rotate_offset([-1, -1], d, 0), [-1, -1]);
        assert_eq!(rotate_offset([0, 1], d, 0), [0, 1]);
        // Quarter 1 maps (a, b) to (-b, a): (-0.5, -1) -> (1, -0.5) -> cells [1, -1].
        assert_eq!(rotate_offset([-1, -1], d, 1), [1, -1]);
        // (+0.5, +1) -> (-1, +0.5) -> cells [-1, 0].
        assert_eq!(rotate_offset([0, 1], d, 1), [-1, 0]);
        // Quarter 2: (-0.5, -1) -> (0.5, 1) -> cells [0, 1].
        assert_eq!(rotate_offset([-1, -1], d, 2), [0, 1]);
        // Quarter 3: (a, b) -> (b, -a): (-0.5, -1) -> (-1, 0.5) -> cells [-1, 0].
        assert_eq!(rotate_offset([-1, -1], d, 3), [-1, 0]);
    }

    #[test]
    fn texels_outside_the_map_link_to_the_border_cell() {
        assert_eq!(clamp_cell([-3, 5], 8, 8), [0, 5]);
        assert_eq!(clamp_cell([4, 99], 8, 8), [4, 7]);
        assert_eq!(clamp_cell([8, -1], 8, 8), [7, 0]);
        assert_eq!(clamp_cell([3, 3], 8, 8), [3, 3]);
    }

    #[test]
    fn stationary_anchor_rounds_even_axes_and_floors_odd_snapped_axes() {
        let g = geometry();
        let anchor = |p, dims, q| stationary_anchor(Some(&g), BOUNDS, 32., p, dims, q);
        // Even by even: nearest corner, ties to even, origin at -1024.
        assert_eq!(anchor([-960., -992.], [4, 4], 0).unwrap(), [2, 1]);
        assert_eq!(anchor([-976., -1008.], [4, 4], 0).unwrap(), [2, 0]);
        // Even by odd (first axis even): the odd second axis sits on a cell centre.
        assert_eq!(anchor([-960., -976.], [4, 5], 0).unwrap(), [2, 1]);
        assert_eq!(anchor([-960., -912.], [4, 5], 0).unwrap(), [2, 3]);
        // The odd dimension moves to the first axis for an odd quarter.
        assert_eq!(anchor([-976., -960.], [4, 5], 1).unwrap(), [1, 2]);
        // An odd axis snaps first: -970 becomes -976, cell 1. Without the snap it is refused.
        assert_eq!(anchor([-960., -970.], [4, 5], 0).unwrap(), [2, 1]);
        assert!(stationary_anchor(None, BOUNDS, 32., [-960., -976.], [4, 5], 0).is_err());
        assert_eq!(
            stationary_anchor(None, BOUNDS, 32., [-960., -992.], [4, 4], 0).unwrap(),
            [2, 1]
        );
    }

    #[test]
    fn snap_operands_are_validated() {
        assert!(Snap::from_operands(63, 5, 16., 32.).is_ok());
        assert!(Snap::from_operands(63, 4, 16., 32.).is_err());
        assert!(Snap::from_operands(63, 5, 0., 32.).is_err());
        assert!(Snap::from_operands(63, 5, 16., 0.).is_err());
        assert!(Snap::from_operands(u32::MAX, 5, 16., 32.).is_err());
    }
}
