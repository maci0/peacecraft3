//! Selected-client fog lattice, authored occluder heights and integer sight spans.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    grid_shift: u8,
    height_shift: u8,
    vertex_bias: i32,
    rounding_bias: f32,
    recurrence: [i32; 8],
    excluded_owner: i32,
    player_mask: u32,
    unblocked_movement: String,
}

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 64 << 10,
            sections: 96,
        };
        if image.len() > budget.image_bytes || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
        {
            return Err(format!(
                "native sight {}: unsupported client",
                origin.source
            ));
        }
        let pe = Pe::parse(image, budget).map_err(|e| e.to_string())?;
        Self::decode(origin, sha256(image), |va, n, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("native sight truncated at {va:#x}"))
        })
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        read: impl Fn(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let operand = |va, op: &[u8], n| -> Result<Vec<u8>, String> {
            let bytes = read(va, op.len() + n, true)?;
            if !bytes.starts_with(op) || bytes.len() != op.len() + n {
                return Err(format!("native sight changed instruction at {va:#x}"));
            }
            Ok(bytes[op.len()..].to_vec())
        };
        let word = |va, op: &[u8]| -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                operand(va, op, 4)?
                    .try_into()
                    .map_err(|_| "native sight word")?,
            ))
        };
        let grid_shift = operand(0x4c632a, &[0xc1, 0xf8], 1)?[0];
        if operand(0x4c6336, &[0xc1, 0xf8], 1)?[0] != grid_shift {
            return Err("native sight coordinate shifts differ".into());
        }
        let height_shift = operand(0x4c6448, &[0xc1, 0xe0], 1)?[0];
        let vertex_bias = i32::from(operand(0x7411cc, &[0x83, 0xc0], 1)?[0]);
        if i32::from(operand(0x7411e3, &[0x8d, 0x41], 1)?[0]) != vertex_bias {
            return Err("native sight vertex axes differ".into());
        }
        let address = word(0x9a1d74, &[0x68])?;
        let rounding_bias = f32::from_le_bytes(
            read(address, 4, false)?
                .try_into()
                .map_err(|_| "native sight rounding bias")?,
        );
        let double_sib = operand(0x4c4e63, &[0x8d, 0x04], 1)?[0];
        let quad = operand(0x4c4e7d, &[0x8d, 0x04], 5)?;
        if double_sib & 0x3f != 0x1b || quad[0] & 0x3f != 0x1d || quad[1..] != [0; 4] {
            return Err("native sight recurrence addressing".into());
        }
        let recurrence = [
            word(0x4c4e66, &[0xc7, 0x45, 0x0c])? as i32,
            word(0x4c4e6d, &[0xba])? as i32,
            word(0x4c4e72, &[0xc7, 0x45, 0xfc])? as i32,
            word(0x4c4e97, &[0xc7, 0x45, 0xf8])? as i32,
            word(0x4c4eb5, &[0xc7, 0x45, 0xf8])? as i32,
            i32::from(operand(0x4c4ec3, &[0x83, 0x45, 0xfc], 1)?[0]),
            1 + (1 << (double_sib >> 6)),
            1 << (quad[0] >> 6),
        ];
        let excluded_owner = i32::from(operand(0x4c62d6, &[0x83, 0xf8], 1)?[0]);
        let player_mask = word(0x4c62c4, &[0xf7, 0xc7])?;
        let movement = operand(0x66f055, &[0x83, 0xf8], 1)?[0];
        let table = word(0x691a9b, &[0xb8])?;
        let mut unblocked_movement = None;
        for index in 0..64 {
            let address = table
                .checked_add(index * 8)
                .ok_or("sight movement table overflow")?;
            let row: [u8; 8] = read(address, 8, false)?
                .try_into()
                .map_err(|_| "sight movement row size")?;
            let address =
                u32::from_le_bytes(row[..4].try_into().map_err(|_| "sight movement row")?);
            if address == 0 {
                break;
            }
            let value =
                u32::from_le_bytes(row[4..].try_into().map_err(|_| "sight movement value")?);
            if value == u32::from(movement) {
                if unblocked_movement.is_some() {
                    return Err("ambiguous unblocked sight movement".into());
                }
                let mut name = Vec::new();
                for offset in 0..32 {
                    let at = address
                        .checked_add(offset)
                        .ok_or("sight movement name overflow")?;
                    let byte = read(at, 1, false)?
                        .first()
                        .copied()
                        .ok_or("sight movement name truncated")?;
                    if byte == 0 {
                        break;
                    }
                    name.push(byte);
                }
                if name.is_empty() || name.len() == 32 {
                    return Err("invalid sight movement name".into());
                }
                unblocked_movement =
                    Some(String::from_utf8(name).map_err(|_| "sight movement encoding")?);
            }
        }
        let unblocked_movement = unblocked_movement.ok_or("unblocked sight movement absent")?;

        if grid_shift == 0
            || grid_shift >= 16
            || height_shift >= 15
            || vertex_bias < 0
            || vertex_bias >= 1 << grid_shift
            || !rounding_bias.is_finite()
            || !(0. ..1.).contains(&rounding_bias)
            || recurrence.iter().any(|v| *v < 0 || *v > 64)
            || excluded_owner <= 0
            || player_mask == 0
        {
            return Err("native sight invalid operands".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            grid_shift,
            height_shift,
            vertex_bias,
            rounding_bias,
            recurrence,
            excluded_owner,
            player_mask,
            unblocked_movement,
        })
    }

    pub fn cell_world(&self, pathing_cell: f32) -> Result<f32, String> {
        let value = pathing_cell * (1u32 << self.grid_shift) as f32;
        if !value.is_finite() || value <= 0. {
            return Err("native sight invalid lattice pitch".into());
        }
        Ok(value)
    }

    pub fn ignores_occlusion(&self, movement: &str) -> bool {
        movement == self.unblocked_movement
    }

    pub fn unblocked_cells(
        &self,
        grid: &HeightGrid,
        position: [f32; 2],
        radius: f32,
        work: &mut Work,
    ) -> Result<Vec<usize>, String> {
        let radius = (radius * grid.cell.recip() + self.rounding_bias).floor();
        if !radius.is_finite() || radius < 0. || radius > i32::MAX as f32 {
            return Err("native sight invalid unblocked radius".into());
        }
        // The unit path includes both end rows; modifier circles omit the last.
        let mut cells = self.circle_cells(grid, position, radius * grid.cell, work)?;
        if radius > 2. {
            let radius = radius as i32;
            let centre = grid.raw_point(position)?;
            let spans = self.spans(radius as usize, work)?;
            let [left, right] = spans[spans.len() - 1];
            for x in left..=right {
                if let Some([x, y]) = centre[0]
                    .checked_add(x)
                    .zip(centre[1].checked_add(radius))
                    .map(|(x, y)| [x, y])
                    && let Some(at) = grid.index([x, y])
                {
                    cells.push(at);
                }
            }
        }
        Ok(cells)
    }

    pub fn eligible(&self, owner: i32, viewer: i32) -> bool {
        owner < self.excluded_owner
            || u32::try_from(viewer)
                .ok()
                .and_then(|v| 1u32.checked_shl(v))
                .is_some_and(|mask| mask & self.player_mask != 0)
    }

    pub fn spans(&self, radius: usize, work: &mut Work) -> Result<Vec<[i32; 2]>, String> {
        let side = radius
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or("native sight span overflow")?;
        if side > 4 << 20 {
            return Err("native sight span allocation bound".into());
        }
        work.charge(side)?;
        let radius = i32::try_from(radius).map_err(|_| "native sight radius overflow")?;
        let [
            outer_bias,
            error_bias,
            inner_bias,
            outer_step,
            inner_step,
            increment,
            double,
            quad,
        ] = self.recurrence;
        let mut y = radius;
        let mut x = 0;
        let mut error = i64::from(error_bias) - i64::from(radius) * i64::from(double);
        let mut outer = i64::from(outer_bias) - i64::from(radius) * i64::from(quad);
        let mut inner = i64::from(inner_bias);
        let mut arc = Vec::new();
        while x <= y {
            work.charge(1)?;
            let (contribution, step) = if error < 0 {
                (inner, inner_step)
            } else {
                arc.push(x);
                y -= 1;
                (outer, outer_step)
            };
            error += contribution;
            x += 1;
            outer += i64::from(step);
            inner += i64::from(increment);
        }
        let mut rows = vec![[0; 2]; side];
        let centre = radius as usize;
        rows[centre] = [-radius, radius];
        let mut minor = 1usize;
        let mut extent = radius;
        for (row, value) in arc.into_iter().enumerate() {
            let value = usize::try_from(value).map_err(|_| "native sight negative arc")?;
            if row >= side || value > centre {
                return Err("native sight invalid arc extent".into());
            }
            rows[row] = [-(value as i32), value as i32];
            rows[side - 1 - row] = rows[row];
            while minor <= value {
                rows[centre - minor] = [-extent, extent];
                rows[centre + minor] = rows[centre - minor];
                minor += 1;
            }
            extent -= 1;
        }
        Ok(rows)
    }

    pub fn visible_cells(
        &self,
        grid: &HeightGrid,
        position: [f32; 2],
        radius: f32,
        work: &mut Work,
    ) -> Result<Vec<usize>, String> {
        if !radius.is_finite() || radius < 0. {
            return Err("native sight invalid source radius".into());
        }
        let radius = (radius * grid.cell.recip() + self.rounding_bias).floor();
        if radius > i32::MAX as f32 {
            return Err("native sight radius outside integer range".into());
        }
        let radius = radius as i32;
        let Some(centre) = grid.point(position) else {
            return Err("native sight source outside map".into());
        };
        if radius == 0 {
            return Ok(Vec::new());
        }
        let side =
            usize::try_from(i64::from(radius) * 2 + 1).map_err(|_| "native sight side overflow")?;
        let count = side.checked_mul(side).ok_or("native sight node overflow")?;
        if count > 4 << 20 {
            return Err("native sight node allocation bound".into());
        }
        work.charge(count)?;
        let mut blocked = vec![true; count];
        let spans = self.spans(radius as usize, work)?;
        let eye = grid.eye(position, self)?;
        let mut visible = Vec::new();
        for ring in 0..=radius {
            for y in -ring..=ring {
                let top_bottom = y.abs() == ring;
                let xs = (-ring..=ring)
                    .take(if top_bottom {
                        (ring as usize) * 2 + 1
                    } else {
                        1
                    })
                    .chain((!top_bottom).then_some(ring));
                for x in xs {
                    work.charge(1)?;
                    if radius == 2 && x.abs() + y.abs() > 2 {
                        continue;
                    }
                    if radius > 2 {
                        if x < spans[(y + radius) as usize][0]
                            || x > spans[(y + radius) as usize][1]
                        {
                            continue;
                        }
                        let horizontal = x.abs() > y.abs()
                            && x * y < 0
                            && y.abs() == spans[(radius + x.abs()) as usize][1];
                        let vertical = y.abs() > x.abs()
                            && x * y > 0
                            && x.abs() == spans[(radius + y.abs()) as usize][1];
                        if horizontal || vertical {
                            continue;
                        }
                    }
                    let at = (y + radius) as usize * side + (x + radius) as usize;
                    let col = centre[0] + x;
                    let row = centre[1] + y;
                    let Some(target) = grid.index([col, row]) else {
                        continue;
                    };
                    let inherited = if ring <= 1 || radius <= 2 {
                        false
                    } else {
                        let sx = x.signum();
                        let sy = y.signum();
                        let parents = if x.abs() > y.abs() {
                            [[x - sx, y], [x - sx, y - sy]]
                        } else if x.abs() < y.abs() {
                            [[x, y - sy], [x - sx, y - sy]]
                        } else {
                            [[x - sx, y - sy]; 2]
                        };
                        parents.iter().any(|[px, py]| {
                            blocked[(py + radius) as usize * side + (px + radius) as usize]
                        })
                    };
                    if !inherited {
                        visible.push(target);
                    }
                    blocked[at] = inherited || grid.heights[target] > eye;
                }
            }
        }
        Ok(visible)
    }
}

pub struct Work {
    remaining: usize,
    spent: usize,
}
impl Work {
    pub fn new(limit: usize) -> Self {
        Self {
            remaining: limit,
            spent: 0,
        }
    }
    pub fn charge(&mut self, amount: usize) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or("native sight work budget")?;
        self.spent += amount;
        Ok(())
    }
    pub fn spent(&self) -> usize {
        self.spent
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeightGrid {
    heights: Vec<i16>,
    layers: Vec<u8>,
    width: usize,
    height: usize,
    origin: [f32; 2],
    cell: f32,
    pathing_cell: f32,
}
impl HeightGrid {
    pub fn heights(&self) -> &[i16] {
        &self.heights
    }
    pub fn dimensions(&self) -> [usize; 2] {
        [self.width, self.height]
    }
    pub fn origin(&self) -> [f32; 2] {
        self.origin
    }
    pub fn new(
        terrain: &pcx_map::Terrain,
        pathing_cell: f32,
        rules: &Rules,
        work: &mut Work,
    ) -> Result<Self, String> {
        let (width, height) = (terrain.width() as usize, terrain.height() as usize);
        let count = width
            .checked_mul(height)
            .ok_or("native sight grid overflow")?;
        if count == 0 || count > 4 << 20 {
            return Err("native sight grid allocation bound".into());
        }
        work.charge(count)?;
        let mut heights = Vec::with_capacity(count);
        for y in 0..height {
            for x in 0..width {
                let layer = [[x, y], [x + 1, y], [x, y + 1], [x + 1, y + 1]]
                    .into_iter()
                    .try_fold(0, |height, [x, y]| {
                        terrain
                            .corner(x as u32, y as u32)
                            .map(|c| height.max(c.layer_height()))
                            .ok_or("native sight terrain corner")
                    })?;
                heights.push(
                    i16::try_from(u32::from(layer) << rules.height_shift)
                        .map_err(|_| "native sight terrain height overflow")?,
                );
            }
        }
        let (x, y) = terrain.origin();
        if !x.is_finite() || !y.is_finite() {
            return Err("native sight invalid terrain origin".into());
        }
        Ok(Self {
            heights,
            layers: terrain.corners().iter().map(|c| c.layer_height()).collect(),
            width,
            height,
            origin: [x, y],
            cell: rules.cell_world(pathing_cell)?,
            pathing_cell,
        })
    }
    pub fn point(&self, position: [f32; 2]) -> Option<[i32; 2]> {
        if position.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let point =
            std::array::from_fn(|i| ((position[i] - self.origin[i]) / self.cell).floor() as i32);
        self.index(point).map(|_| point)
    }
    fn index(&self, [x, y]: [i32; 2]) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height)
            .then(|| y as usize * self.width + x as usize)
    }
    fn eye(&self, position: [f32; 2], rules: &Rules) -> Result<i16, String> {
        let vertex: [i64; 2] = std::array::from_fn(|i| {
            (((position[i] - self.origin[i]) / self.pathing_cell).floor() as i64
                + i64::from(rules.vertex_bias))
                >> rules.grid_shift
        });
        if vertex[0] < 0
            || vertex[1] < 0
            || vertex[0] as usize > self.width
            || vertex[1] as usize > self.height
        {
            return Err("native sight eye vertex outside map".into());
        }
        i16::try_from(
            u32::from(self.layers[vertex[1] as usize * (self.width + 1) + vertex[0] as usize])
                << rules.height_shift,
        )
        .map_err(|_| "native sight eye height overflow".into())
    }
    pub fn occlude(
        &mut self,
        position: [f32; 2],
        height: f32,
        rules: &Rules,
        work: &mut Work,
    ) -> Result<(), String> {
        work.charge(1)?;
        if !height.is_finite() || height < 0. || height > f32::from(i16::MAX) {
            return Err("native sight unsupported occluder height".into());
        }
        if height == 0. {
            return Ok(());
        }
        let at = self
            .point(position)
            .and_then(|p| self.index(p))
            .ok_or("native sight occluder outside map")?;
        let value = i32::from(self.eye(position, rules)?) + height as i32;
        self.heights[at] = self.heights[at]
            .max(i16::try_from(value).map_err(|_| "native sight occluder height overflow")?);
        Ok(())
    }
}

impl Rules {
    pub fn circle_cells(
        &self,
        grid: &HeightGrid,
        position: [f32; 2],
        radius: f32,
        work: &mut Work,
    ) -> Result<Vec<usize>, String> {
        if !radius.is_finite() || radius < 0. {
            return Err("native sight invalid modifier radius".into());
        }
        let centre = grid.raw_point(position)?;
        let radius = (radius * grid.cell.recip()).floor();
        if radius > i32::MAX as f32 {
            return Err("native sight modifier radius overflow".into());
        }
        let radius = radius as i32;
        if radius == 0 {
            return Ok(Vec::new());
        }
        let side = (radius as usize)
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or("native sight modifier side")?;
        let count = side
            .checked_mul(side)
            .ok_or("native sight modifier work overflow")?;
        if count > 4 << 20 {
            return Err("native sight modifier allocation bound".into());
        }
        work.charge(count)?;
        if centre.into_iter().zip(grid.dimensions()).any(|(c, n)| {
            i64::from(c) + i64::from(radius) < 0 || i64::from(c) - i64::from(radius) >= n as i64
        }) {
            return Ok(Vec::new());
        }
        let spans = self.spans(radius as usize, work)?;
        let mut cells = Vec::new();
        let last = if radius <= 2 { radius } else { radius - 1 };
        for y in -radius..=last {
            let row = if radius <= 2 {
                [-radius.min(1), radius.min(1)]
            } else {
                spans[(y + radius) as usize]
            };
            for x in row[0]..=row[1] {
                if radius == 2 && y.abs() == 2 && x != 0 {
                    continue;
                }
                if let Some(at) = grid.index([centre[0] + x, centre[1] + y]) {
                    cells.push(at);
                }
            }
        }
        if radius == 2 {
            for x in [-radius, radius] {
                if let Some(at) = grid.index([centre[0] + x, centre[1]]) {
                    cells.push(at);
                }
            }
        }
        Ok(cells)
    }
    pub fn rectangle_cells(
        &self,
        grid: &HeightGrid,
        bounds: [f32; 4],
        work: &mut Work,
    ) -> Result<Vec<usize>, String> {
        if bounds.iter().any(|v| !v.is_finite()) || bounds[0] > bounds[2] || bounds[1] > bounds[3] {
            return Err("native sight invalid modifier rectangle".into());
        }
        let lower = grid.raw_point([bounds[0], bounds[1]])?;
        let upper = grid.raw_point([bounds[2], bounds[3]])?;
        let minx = lower[0].max(0);
        let miny = lower[1].max(0);
        let maxx = upper[0].min(grid.width as i32 - 1);
        let maxy = upper[1].min(grid.height as i32 - 1);
        if minx > maxx || miny > maxy {
            return Ok(Vec::new());
        }
        let count = (maxx - minx + 1) as usize * (maxy - miny + 1) as usize;
        work.charge(count)?;
        let mut cells = Vec::with_capacity(count);
        for y in miny..=maxy {
            for x in minx..=maxx {
                cells.push(y as usize * grid.width + x as usize);
            }
        }
        Ok(cells)
    }
}
impl HeightGrid {
    fn raw_point(&self, position: [f32; 2]) -> Result<[i32; 2], String> {
        let values: [f32; 2] =
            std::array::from_fn(|i| ((position[i] - self.origin[i]) / self.cell).floor());
        if values.iter().any(|v| {
            !v.is_finite()
                || f64::from(*v) < f64::from(i32::MIN)
                || f64::from(*v) > f64::from(i32::MAX)
        }) {
            return Err("native sight coordinate outside integer range".into());
        }
        Ok(values.map(|v| v as i32))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn identity() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic sight".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }
    fn fixture() -> BTreeMap<u32, u8> {
        let mut bytes = BTreeMap::new();
        let mut put = |va, data: &[u8]| {
            for (i, b) in data.iter().enumerate() {
                bytes.insert(va + i as u32, *b);
            }
        };
        for va in [0x4c632a, 0x4c6336] {
            put(va, &[0xc1, 0xf8, 1]);
        }
        put(0x4c6448, &[0xc1, 0xe0, 3]);
        put(0x7411cc, &[0x83, 0xc0, 1]);
        put(0x7411e3, &[0x8d, 0x41, 1]);
        put(0x9a1d74, &[0x68, 0, 0x10, 0, 0]);
        put(0x1000, &0.25f32.to_le_bytes());
        put(0x4c4e63, &[0x8d, 0x04, 0x1b]);
        put(0x4c4e7d, &[0x8d, 0x04, 0x9d, 0, 0, 0, 0]);
        for (va, op, value) in [
            (0x4c4e66, &[0xc7, 0x45, 0x0c][..], 10u32),
            (0x4c4e6d, &[0xba][..], 3),
            (0x4c4e72, &[0xc7, 0x45, 0xfc][..], 6),
            (0x4c4e97, &[0xc7, 0x45, 0xf8][..], 8),
            (0x4c4eb5, &[0xc7, 0x45, 0xf8][..], 4),
        ] {
            put(va, &[op, &value.to_le_bytes()].concat());
        }
        put(0x4c4ec3, &[0x83, 0x45, 0xfc, 4]);
        put(0x4c62d6, &[0x83, 0xf8, 5]);
        put(0x4c62c4, &[0xf7, 0xc7, 3, 0, 0, 0]);
        put(0x66f055, &[0x83, 0xf8, 9]);
        put(0x691a9b, &[0xb8, 0, 0x20, 0, 0]);
        put(0x2000, &[0, 0x30, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        put(0x3000, b"synthetic-air\0");

        bytes
    }
    fn decode(bytes: &BTreeMap<u32, u8>) -> Result<Rules, String> {
        Rules::decode(identity(), sha256(b"fixture"), |va, n, _| {
            (0..n)
                .map(|i| {
                    bytes
                        .get(&(va + i as u32))
                        .copied()
                        .ok_or_else(|| "missing fixture byte".into())
                })
                .collect()
        })
    }
    pub(crate) fn rules() -> Rules {
        decode(&fixture()).unwrap()
    }
    pub(crate) fn terrain(width: u32, height: u32, layers: &[u8]) -> pcx_map::Terrain {
        assert_eq!(layers.len(), ((width + 1) * (height + 1)) as usize);
        let mut b = b"W3E!".to_vec();
        b.extend(11u32.to_le_bytes());
        b.push(b'L');
        b.extend(0u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend(b"T001");
        b.extend(0u32.to_le_bytes());
        b.extend((width + 1).to_le_bytes());
        b.extend((height + 1).to_le_bytes());
        b.extend([0; 8]);
        for layer in layers {
            b.extend([0, 0, 0, 0, 0, 0, *layer]);
        }
        pcx_map::Terrain::from_bytes(b).unwrap()
    }
    #[test]
    fn source_operands_drive_lattice_rounding_height_and_players() {
        let r = rules();
        assert_eq!(r.cell_world(3.).unwrap(), 6.);
        assert_eq!(r.height_shift, 3);
        assert!(r.ignores_occlusion("synthetic-air"));
        assert!(!r.ignores_occlusion("fly"));
        assert_eq!(r.rounding_bias, 0.25);
        assert!(r.eligible(4, 7));
        assert!(!r.eligible(5, 7));
        assert!(r.eligible(5, 1));
        let mut bytes = fixture();
        bytes.insert(0x4c4e69, 12);
        assert_eq!(decode(&bytes).unwrap().recurrence[0], 12);
        bytes.insert(0x4c632a, 0);
        assert!(decode(&bytes).is_err());
        assert!(Rules::reviewed_legacy(b"not a client", identity()).is_err());
    }
    #[test]
    fn heights_use_four_corners_and_authored_single_cell_occluders() {
        let r = rules();
        let mut work = Work::new(1000);
        let t = terrain(2, 2, &[1, 1, 1, 1, 3, 1, 1, 1, 1]);
        let mut g = HeightGrid::new(&t, 1., &r, &mut work).unwrap();
        assert_eq!(g.heights, [24; 4]);
        g.occlude([0.1, 0.1], 30., &r, &mut work).unwrap();
        assert_eq!(g.heights, [38, 24, 24, 24]);
        g.occlude([0.1, 0.1], 2., &r, &mut work).unwrap();
        assert_eq!(g.heights[0], 38);
        assert!(g.occlude([f32::NAN, 0.], 1., &r, &mut work).is_err());
        assert!(HeightGrid::new(&t, 1., &r, &mut Work::new(0)).is_err());
    }
    #[test]
    fn graph_blocks_after_the_obstacle_and_keeps_native_boundary_omissions() {
        let r = rules();
        let t = terrain(16, 16, &vec![0; 17 * 17]);
        let mut w = Work::new(100000);
        let mut g = HeightGrid::new(&t, 1., &r, &mut w).unwrap();
        let p = [16., 16.];
        let before = r.visible_cells(&g, p, 8., &mut w).unwrap();
        assert_eq!(before.len(), 57);
        let index = |x: usize, y: usize| y * 16 + x;
        assert!(before.contains(&index(10, 8)));
        assert!(!before.contains(&index(12, 7)));
        g.occlude([18., 16.], 8., &r, &mut w).unwrap();
        let after = r.visible_cells(&g, p, 8., &mut w).unwrap();
        let unblocked = r.unblocked_cells(&g, p, 8., &mut w).unwrap();
        let full_span_count: usize = r
            .spans(4, &mut w)
            .unwrap()
            .iter()
            .map(|[left, right]| (right - left + 1) as usize)
            .sum();
        assert_eq!(unblocked.len(), full_span_count);
        assert!(unblocked.contains(&index(10, 8)));
        assert!(unblocked.contains(&index(12, 7)));
        assert!(after.contains(&index(9, 8)));
        assert!(!after.contains(&index(10, 8)));
        assert!(r.visible_cells(&g, p, 8., &mut Work::new(1)).is_err());
        assert!(r.visible_cells(&g, p, f32::NAN, &mut w).is_err());
    }
    #[test]
    fn modifiers_use_truncated_radii_and_half_open_large_circle_rows() {
        let r = rules();
        let t = terrain(16, 16, &vec![0; 17 * 17]);
        let mut w = Work::new(100000);
        let g = HeightGrid::new(&t, 1., &r, &mut w).unwrap();
        let p = [16., 16.];
        assert!(r.circle_cells(&g, p, 1.99, &mut w).unwrap().is_empty());
        assert_eq!(r.circle_cells(&g, p, 2., &mut w).unwrap().len(), 9);
        assert_eq!(r.circle_cells(&g, p, 4., &mut w).unwrap().len(), 13);
        let cells = r.circle_cells(&g, p, 8., &mut w).unwrap();
        assert!(cells.iter().any(|at| at / 16 == 4));
        assert!(!cells.iter().any(|at| at / 16 == 12));
        assert_eq!(
            r.rectangle_cells(&g, [0., 0., 2., 2.], &mut w).unwrap(),
            [0, 1, 16, 17]
        );
        assert!(
            r.rectangle_cells(&g, [-4., -4., -2., -2.], &mut w)
                .unwrap()
                .is_empty()
        );
        assert!(r.rectangle_cells(&g, [2., 0., 0., 2.], &mut w).is_err());
        assert!(
            r.circle_cells(&g, [4_000_000_000., 0.], 8., &mut w)
                .unwrap()
                .is_empty()
        );
        assert!(r.circle_cells(&g, p, 8., &mut Work::new(1)).is_err());
    }
    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE and PCX_NATIVE_SIGHT_RUN with local native span evidence"]
    fn supplied_native_spans_and_graph_propagation_match() {
        let path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").unwrap();
        let image = std::fs::read(&path).unwrap();
        let r = Rules::reviewed_legacy(
            &image,
            AssetIdentity {
                source: path,
                edition: "explicit local Legacy".into(),
                version: "reviewed sight".into(),
            },
        )
        .unwrap();
        let root = std::path::PathBuf::from(std::env::var("PCX_NATIVE_SIGHT_RUN").unwrap());
        let mut w = Work::new(1 << 20);
        let mut checked = 0;
        for entry in std::fs::read_dir(&root).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let Some(radius) = name
                .strip_prefix("sight-")
                .and_then(|s| s.strip_suffix("-spans.bin"))
                .and_then(|s| s.parse::<usize>().ok())
            else {
                continue;
            };
            let native = std::fs::read(entry.path()).unwrap();
            let rows = r.spans(radius, &mut w).unwrap();
            let bytes: Vec<_> = rows
                .iter()
                .flat_map(|row| row.iter().flat_map(|v| v.to_le_bytes()))
                .collect();
            assert_eq!(bytes, native, "radius {radius}");
            let nodes = std::fs::read(root.join(format!("sight-{radius}-nodes.bin"))).unwrap();
            let header = std::fs::read(root.join(format!("sight-{radius}-header.bin"))).unwrap();
            let initial = u32::from_le_bytes(header[0x20..0x24].try_into().unwrap()) as usize;
            assert_eq!(nodes.len() % 20, 0);
            let side = radius * 2 + 5;
            let t = terrain(side as u32, side as u32, &vec![0; (side + 1).pow(2)]);
            let mut g = HeightGrid::new(&t, 1., &r, &mut w).unwrap();
            for (i, height) in g.heights.iter_mut().enumerate() {
                *height = i16::from(i % 7 == 0);
            }
            let centre = radius + 2;
            let mut blocks = Vec::new();
            let mut expected = Vec::new();
            for (i, node) in nodes.as_chunks::<20>().0.iter().enumerate() {
                let dx = i16::from_le_bytes(node[4..6].try_into().unwrap());
                let dy = i16::from_le_bytes(node[6..8].try_into().unwrap());
                let a = i32::from_le_bytes(node[8..12].try_into().unwrap());
                let b = i32::from_le_bytes(node[12..16].try_into().unwrap());
                let inherited = i >= initial && (blocks[a as usize] || blocks[b as usize]);
                let at = (centre as i32 + i32::from(dy)) as usize * side
                    + (centre as i32 + i32::from(dx)) as usize;
                if !inherited {
                    expected.push(at);
                }
                blocks.push(inherited || g.heights[at] > 0);
            }
            let position = [centre as f32 * g.cell; 2];
            let mut actual = r
                .visible_cells(&g, position, radius as f32 * g.cell, &mut w)
                .unwrap();
            expected.sort_unstable();
            actual.sort_unstable();
            assert_eq!(actual, expected, "native graph propagation radius {radius}");
            checked += 1;
        }
        assert!(checked > 0, "no native span fixtures found");
    }
}
