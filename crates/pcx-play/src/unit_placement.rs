//! Bounded creation placement. The host owns positions; this grid is a collision cache.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::script_random::real;
use crate::scripts::AssetIdentity;
use pcx_map::Pathing;
use pcx_script::HandleId;
use pcx_sim::world::Source;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub normalization: SourceWindow,
    pub initial_position: SourceWindow,
    pub small_radius: SourceWindow,
    pub large_radius: SourceWindow,
    pub one_initializer: SourceWindow,
    pub two_initializer: SourceWindow,
    pub ground_mask: SourceWindow,
}

#[derive(Debug, thiserror::Error)]
#[error("unit placement: {0}")]
pub struct Error(pub String);

fn fail(message: impl Into<String>) -> Error {
    Error(message.into())
}

#[derive(Clone, Debug)]
pub struct Rules {
    binding: Binding,
    exponent: i32,
    initial: u32,
    thresholds: [f32; 3],
    one: f32,
    two: f32,
    mask: u8,
}

impl Rules {
    /// The caller selects reviewed instruction windows, never measured operands.
    pub fn read(image: &[u8], binding: Binding, budget: RuleInputBudget) -> Result<Self, Error> {
        let source = binding.origin.source.clone();
        Self::read_selected(image, binding, budget)
            .map_err(|error| fail(format!("{source}: {}", error.0)))
    }

    fn read_selected(
        image: &[u8],
        binding: Binding,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        let windows = [
            binding.normalization,
            binding.initial_position,
            binding.small_radius,
            binding.large_radius,
            binding.one_initializer,
            binding.two_initializer,
            binding.ground_mask,
        ];
        if image.len() > budget.image_bytes
            || windows.iter().map(|w| u64::from(w.len)).sum::<u64>() + 12
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
        {
            return Err(fail("incomplete identity or image fingerprint mismatch"));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(e.to_string()))?;
        let code = |window: &SourceWindow| pe.window(window, true).map_err(|e| fail(e.to_string()));
        let scale = code(&binding.normalization)?;
        if scale.len() != 5 || scale[0] != 0x05 {
            return Err(fail("normalization instruction"));
        }
        let adjustment = i32::from_le_bytes(scale[1..].try_into().unwrap());
        if adjustment >= 0 || adjustment % (1 << 23) != 0 {
            return Err(fail("normalization exponent"));
        }
        let exponent = -adjustment / (1 << 23);
        if !(1..=23).contains(&exponent) {
            return Err(fail("unsupported cell exponent"));
        }
        let data = |address: u32| -> Result<u32, Error> {
            let bytes = pe
                .read_from(address, false)
                .map_err(|e| fail(e.to_string()))?;
            Ok(u32::from_le_bytes(
                bytes
                    .get(..4)
                    .ok_or_else(|| fail("truncated real"))?
                    .try_into()
                    .unwrap(),
            ))
        };
        let initial = code(&binding.initial_position)?;
        if initial.len() != 5 || initial[0] != 0xa1 {
            return Err(fail("constructor load"));
        }
        let initial = data(u32::from_le_bytes(initial[1..].try_into().unwrap()))?;
        real::from_f32(f32::from_bits(initial)).map_err(|e| fail(e.to_string()))?;
        let threshold = |window: &SourceWindow| -> Result<f32, Error> {
            let bytes = code(window)?;
            if bytes.len() != 8 || bytes[..4] != [0xf3, 0x0f, 0x10, 0x05] {
                return Err(fail("radius threshold load"));
            }
            Ok(f32::from_bits(data(u32::from_le_bytes(
                bytes[4..].try_into().unwrap(),
            ))?))
        };
        let integer = |window: &SourceWindow| -> Result<f32, Error> {
            let bytes = code(window)?;
            let (value, at) = match bytes {
                [0x6a, value, ..] => (i32::from(*value as i8), 2),
                [0x68, a, b, c, d, ..] => (i32::from_le_bytes([*a, *b, *c, *d]), 5),
                _ => return Err(fail("integer initializer")),
            };
            if bytes.len() != at + 14
                || bytes[at] != 0x68
                || bytes[at + 5] != 0xe8
                || bytes[at + 10..] != [0x83, 0xc4, 8, 0xc3]
            {
                return Err(fail("real initializer shape"));
            }
            Ok(f32::from_bits(
                real::from_i32(value).map_err(|e| fail(e.to_string()))?,
            ))
        };
        let one = integer(&binding.one_initializer)?;
        let two = integer(&binding.two_initializer)?;
        let thresholds = [
            threshold(&binding.small_radius)?,
            one,
            threshold(&binding.large_radius)?,
        ];
        if !two.is_finite()
            || two <= one
            || thresholds.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || !thresholds.windows(2).all(|p| p[0] < p[1])
        {
            return Err(fail("unordered radius geometry"));
        }
        let mask = code(&binding.ground_mask)?;
        if mask.len() != 5 || mask[0] != 0xb9 {
            return Err(fail("ground mask instruction"));
        }
        let mask = u8::try_from(u32::from_le_bytes(mask[1..].try_into().unwrap()))
            .map_err(|_| fail("ground mask range"))?;
        if mask == 0 {
            return Err(fail("empty ground mask"));
        }
        Ok(Self {
            binding,
            exponent,
            initial,
            thresholds,
            one,
            two,
            mask,
        })
    }

    pub fn cell_size(&self) -> f32 {
        (1u32 << self.exponent) as f32
    }

    pub fn ground_mask(&self) -> u8 {
        self.mask
    }

    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    fn shift(&self, value: u32, exponent: i32) -> Result<u32, Error> {
        if value & 0x7fff_ffff == 0 {
            return Ok(value);
        }
        let encoded = ((value >> 23) & 255) as i32 + exponent;
        if !(1..255).contains(&encoded) {
            return Err(fail("coordinate exponent outside normal range"));
        }
        let result = i64::from(value) + (i64::from(exponent) << 23);
        let result = u32::try_from(result).map_err(|_| fail("coordinate exponent overflow"))?;
        real::from_f32(f32::from_bits(result)).map_err(|e| fail(e.to_string()))?;
        Ok(result)
    }

    fn normalize(&self, point: [f32; 2], origin: [f32; 2]) -> Result<[f32; 2], Error> {
        let mut result = [0.0; 2];
        for axis in 0..2 {
            let word = real::sub(
                real::from_f32(point[axis]).map_err(|e| fail(e.to_string()))?,
                real::from_f32(origin[axis]).map_err(|e| fail(e.to_string()))?,
            )
            .map_err(|e| fail(e.to_string()))?;
            let cell = self.shift(word, -self.exponent)?;
            let cell = real::add(
                self.initial,
                real::sub(cell, self.initial).map_err(|e| fail(e.to_string()))?,
            )
            .map_err(|e| fail(e.to_string()))?;
            result[axis] = f32::from_bits(
                real::add(self.shift(cell, self.exponent)?, origin[axis].to_bits())
                    .map_err(|e| fail(e.to_string()))?,
            );
        }
        Ok(result)
    }

    fn extents(&self, radius: f32) -> [i32; 2] {
        let band = self.thresholds.iter().take_while(|v| radius >= **v).count();
        match band {
            0 => [0, 0],
            1 => [self.one as i32, 0],
            2 => [self.one as i32, self.one as i32],
            _ => [self.two as i32, self.one as i32],
        }
    }
}

/// An explicitly selected, derived static collision raster, including stationary
/// footprints. It is a cache, not another object/handle arena. Its owner must
/// rebuild it when stationary obstacles change; mutation is not admitted here.
pub struct Grid {
    pub source: Source,
    pub origin: [f32; 2],
    pub pathing: Pathing,
    /// Terrain-domain labels in pathing-cell order, derived from the selected W3E.
    pub domains: Vec<u8>,
}

/// Already decoded pathing texture. Coordinates are cells relative to its centre.
/// Keeping the decode outside this module reuses the selected texture loader.
#[derive(Clone)]
pub struct Footprint {
    pub origin: AssetIdentity,
    pub cells: Vec<[i32; 2]>,
}

#[derive(Clone, Copy)]
pub struct Budget {
    pub cells: usize,
    pub footprint_cells: usize,
    /// Deterministic cell/occupancy queries per creation, independent of wall time.
    pub queries: usize,
}

pub struct Placement {
    source: Source,
    grid: Grid,
    rules: Rules,
    footprints: BTreeMap<String, Footprint>,
    footprint_loader: Option<FootprintLoader>,
    occupied: Vec<u32>,
    footprint_occupied: Vec<u32>,
    stamps: Vec<(HandleId, Prepared)>,
    budget: Budget,
}

type FootprintLoader = Box<dyn FnMut(&str) -> Result<Footprint, String> + Send + Sync>;

#[derive(Debug)]
pub(crate) struct Prepared {
    pub position: [f32; 2],
    cells: Vec<usize>,
    footprint: Vec<usize>,
    active: bool,
}

#[cfg(test)]
#[path = "unit_placement_tests.rs"]
pub(crate) mod tests;

impl Placement {
    pub fn source(&self) -> &Source {
        &self.source
    }
    pub fn new(
        grid: Grid,
        rules: Rules,
        footprints: BTreeMap<String, Footprint>,
        budget: Budget,
    ) -> Result<Self, Error> {
        let count = usize::try_from(u64::from(grid.pathing.width) * u64::from(grid.pathing.height))
            .map_err(|_| fail("grid dimensions"))?;
        let footprint_count = footprints
            .values()
            .try_fold(0usize, |n, f| n.checked_add(f.cells.len()))
            .ok_or_else(|| fail("footprint count overflow"))?;
        if count == 0
            || count > budget.cells
            || grid.pathing.width > i32::MAX as u32
            || grid.pathing.height > i32::MAX as u32
            || grid.pathing.cells.len() != count
            || grid.domains.len() != count
            || footprint_count > budget.footprint_cells
            || budget.queries == 0
        {
            return Err(fail("grid/footprint/query budget or dimensions"));
        }
        if grid.origin.iter().any(|v| !v.is_finite())
            || [&grid.source.install, &grid.source.edition, &grid.source.map]
                .iter()
                .any(|v| v.trim().is_empty())
            || footprints.iter().any(|(path, f)| {
                path.is_empty()
                    || [&f.origin.source, &f.origin.edition, &f.origin.version]
                        .iter()
                        .any(|v| v.trim().is_empty())
            })
        {
            return Err(fail("missing map/footprint provenance or finite origin"));
        }
        let mut occupied = Vec::new();
        occupied
            .try_reserve_exact(count)
            .map_err(|_| fail("occupancy allocation"))?;
        occupied.resize(count, 0);
        let mut footprint_occupied = Vec::new();
        footprint_occupied
            .try_reserve_exact(count)
            .map_err(|_| fail("footprint occupancy allocation"))?;
        footprint_occupied.resize(count, 0);
        Ok(Self {
            source: grid.source.clone(),
            grid,
            rules,
            footprints,
            footprint_loader: None,
            occupied,
            footprint_occupied,
            stamps: Vec::new(),
            budget,
        })
    }

    /// Load executed footprint paths through the caller's retained source reader.
    pub fn with_footprint_loader(
        mut self,
        loader: impl FnMut(&str) -> Result<Footprint, String> + Send + Sync + 'static,
    ) -> Self {
        self.footprint_loader = Some(Box::new(loader));
        self
    }

    fn ensure_footprint(&mut self, path: &str) -> Result<(), Error> {
        if matches!(path, "" | "_" | "none") || self.footprints.contains_key(path) {
            return Ok(());
        }
        let loader = self
            .footprint_loader
            .as_mut()
            .ok_or_else(|| fail(format!("missing selected footprint {path}")))?;
        let input = loader(path).map_err(fail)?;
        let count = self
            .footprints
            .values()
            .try_fold(input.cells.len(), |n, f| n.checked_add(f.cells.len()))
            .ok_or_else(|| fail("footprint count overflow"))?;
        if count > self.budget.footprint_cells
            || [
                &input.origin.source,
                &input.origin.edition,
                &input.origin.version,
            ]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err(fail("footprint budget or provenance"));
        }
        self.footprints.insert(path.into(), input);
        Ok(())
    }

    fn cell(&self, point: [f32; 2]) -> Result<[i32; 2], Error> {
        let mut cell = [0; 2];
        for axis in 0..2 {
            let value = real::sub(
                real::from_f32(point[axis]).map_err(|e| fail(e.to_string()))?,
                real::from_f32(self.grid.origin[axis]).map_err(|e| fail(e.to_string()))?,
            )
            .map_err(|e| fail(e.to_string()))?;
            let value = f32::from_bits(self.rules.shift(value, -self.rules.exponent)?);
            if value < 0.0
                || value >= [self.grid.pathing.width, self.grid.pathing.height][axis] as f32
            {
                return Err(fail("out-of-map creation requires qualified clamping"));
            }
            cell[axis] = value as i32;
        }
        Ok(cell)
    }

    fn index(&self, x: i64, y: i64) -> Option<usize> {
        (x >= 0
            && y >= 0
            && x < i64::from(self.grid.pathing.width)
            && y < i64::from(self.grid.pathing.height))
        .then(|| y as usize * self.grid.pathing.width as usize + x as usize)
    }

    fn rectangle(&self, cell: [i32; 2], extents: [i32; 2]) -> Result<Vec<usize>, Error> {
        let mut cells = Vec::new();
        let width = i64::from(extents[0]) + i64::from(extents[1]) + 1;
        let count = width
            .checked_mul(width)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| fail("occupied rectangle size"))?;
        if count > self.budget.footprint_cells {
            return Err(fail("occupied rectangle budget"));
        }
        cells
            .try_reserve_exact(count)
            .map_err(|_| fail("occupied rectangle allocation"))?;
        for y in
            i64::from(cell[1]) - i64::from(extents[0])..=i64::from(cell[1]) + i64::from(extents[1])
        {
            for x in i64::from(cell[0]) - i64::from(extents[0])
                ..=i64::from(cell[0]) + i64::from(extents[1])
            {
                cells.push(
                    self.index(x, y)
                        .ok_or_else(|| fail("occupied rectangle outside map"))?,
                );
            }
        }
        Ok(cells)
    }

    pub(crate) fn prepare(
        &mut self,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
    ) -> Result<Prepared, Error> {
        self.ensure_footprint(path)?;
        self.stamps
            .try_reserve(1)
            .map_err(|_| fail("unit occupancy allocation"))?;
        self.candidate(request, radius, path, face, None, true)?
            .ok_or_else(|| fail("creation placement refused"))
    }

    fn blocked(&self, i: usize, exclude: Option<&Prepared>) -> bool {
        self.grid.pathing.cells[i] & self.rules.mask != 0
            || (exclude.is_none_or(|p| p.active)
                && (self.occupied[i]
                    > u32::from(exclude.is_some_and(|p| p.cells.binary_search(&i).is_ok()))
                    || self.footprint_occupied[i]
                        > u32::from(
                            exclude.is_some_and(|p| p.footprint.binary_search(&i).is_ok()),
                        )))
    }

    fn candidate(
        &self,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
        exclude: Option<&Prepared>,
        creation: bool,
    ) -> Result<Option<Prepared>, Error> {
        if request.iter().any(|v| !v.is_finite()) {
            return Err(fail("nonfinite unit position"));
        }
        let start = self.cell(request)?;
        let wanted = self.grid.domains[self
            .index(i64::from(start[0]), i64::from(start[1]))
            .ok_or_else(|| fail("missing domain"))?];
        let mut position = request;
        let normalized_radius = radius
            .map(|value| {
                if !value.is_finite() || value < 0.0 || value > self.rules.cell_size() {
                    return Err(fail("unsupported creation collision radius"));
                }
                self.rules
                    .shift(
                        real::from_f32(value).map_err(|e| fail(e.to_string()))?,
                        -self.rules.exponent,
                    )
                    .map(f32::from_bits)
            })
            .transpose()?;
        if let Some(radius) = normalized_radius {
            let extents = self.rules.extents(radius);
            let mut work = 0usize;
            let mut good = |cell: [i32; 2]| -> Result<bool, Error> {
                work = work
                    .checked_add(1)
                    .ok_or_else(|| fail("query count overflow"))?;
                if work > self.budget.queries {
                    return Err(fail("creation query budget exceeded"));
                }
                let [x, y] = cell.map(i64::from);
                if self
                    .index(x - i64::from(extents[0]), y - i64::from(extents[0]))
                    .is_none()
                    || self
                        .index(x + i64::from(extents[1]), y + i64::from(extents[1]))
                        .is_none()
                {
                    return Ok(false);
                }
                let cells = self.rectangle(cell, extents)?;
                Ok(cells.iter().all(|&i| !self.blocked(i, exclude))
                    && self
                        .index(i64::from(cell[0]), i64::from(cell[1]))
                        .is_some_and(|i| self.grid.domains[i] == wanted))
            };
            if !good(start)? {
                if !creation {
                    return Ok(None);
                }
                // shortcut: deterministic perimeter search; qualify platform overlays and native boundary failures before admitting them.
                let mut found = None;
                'rings: for ring in
                    1..=i64::from(self.grid.pathing.width.max(self.grid.pathing.height))
                {
                    let [cx, cy] = start.map(i64::from);
                    let (lo_x, lo_y, hi_x, hi_y) = (cx - ring, cy - ring, cx + ring, cy + ring);
                    let edges = [
                        (lo_x + 1, lo_y, 1, 0),
                        (hi_x, lo_y + 1, 0, 1),
                        (hi_x - 1, hi_y, -1, 0),
                        (lo_x, hi_y - 1, 0, -1),
                    ];
                    for (mut x, mut y, dx, dy) in edges {
                        for _ in 0..2 * ring {
                            if let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y))
                                && good([x, y])?
                            {
                                found = Some([x, y]);
                                break 'rings;
                            }
                            x += dx;
                            y += dy;
                        }
                    }
                }
                let found = found.ok_or_else(|| fail("no qualified placement within map"))?;
                for axis in 0..2 {
                    let center = real::add(
                        real::from_i32(found[axis]).map_err(|e| fail(e.to_string()))?,
                        self.rules.thresholds[0].to_bits(),
                    )
                    .map_err(|e| fail(e.to_string()))?;
                    position[axis] = f32::from_bits(
                        real::add(
                            self.rules.shift(center, self.rules.exponent)?,
                            self.grid.origin[axis].to_bits(),
                        )
                        .map_err(|e| fail(e.to_string()))?,
                    );
                }
            }
        }
        let position = if creation {
            self.rules.normalize(position, self.grid.origin)?
        } else {
            position
        };
        let cell = self.cell(position)?;
        let cells = if let Some(radius) = normalized_radius {
            self.rectangle(cell, self.rules.extents(radius))?
        } else {
            Vec::new()
        };
        if cells.iter().any(|&i| self.occupied[i] == u32::MAX) {
            return Err(fail("occupancy count overflow"));
        }
        let mut footprint = Vec::new();
        if !matches!(path, "" | "_" | "none") {
            let input = self
                .footprints
                .get(path)
                .ok_or_else(|| fail(format!("missing selected footprint {path}")))?;
            footprint
                .try_reserve_exact(input.cells.len())
                .map_err(|_| fail("footprint allocation"))?;
            if !face.is_finite() {
                return Err(fail("finite facing required"));
            }
            let turns = f64::from(face) / 90.0;
            if turns.fract() != 0.0 || turns.abs() > 4.0 {
                return Err(fail("only quarter-turn footprint facing is admitted"));
            }
            let quarter = turns.rem_euclid(4.0) as u8;
            // shortcut: quarter-turn footprint orientation; qualify arbitrary facing before supporting it.
            for &[x, y] in &input.cells {
                let (x, y) = match quarter {
                    0 => (i64::from(x), i64::from(y)),
                    1 => (-i64::from(y) - 1, i64::from(x)),
                    2 => (-i64::from(x) - 1, -i64::from(y) - 1),
                    _ => (i64::from(y), -i64::from(x) - 1),
                };
                footprint.push(
                    self.index(i64::from(cell[0]) + x, i64::from(cell[1]) + y)
                        .ok_or_else(|| fail("footprint outside map"))?,
                );
            }
        }
        footprint.sort_unstable();
        footprint.dedup();
        if footprint
            .iter()
            .any(|&i| self.footprint_occupied[i] == u32::MAX)
        {
            return Err(fail("footprint occupancy overflow"));
        }
        Ok(Some(Prepared {
            position,
            cells,
            footprint,
            active: exclude.is_none_or(|p| p.active),
        }))
    }

    pub(crate) fn hide(&mut self, unit: HandleId) -> Result<(), Error> {
        let stamp = self
            .stamps
            .iter_mut()
            .find(|(id, _)| *id == unit)
            .map(|(_, p)| p)
            .ok_or_else(|| fail("unit occupancy is missing"))?;
        if stamp.active {
            for &i in &stamp.cells {
                self.occupied[i] -= 1;
            }
            for &i in &stamp.footprint {
                self.footprint_occupied[i] -= 1;
            }
            stamp.active = false;
        }
        Ok(())
    }

    pub(crate) fn prepare_show(
        &mut self,
        unit: HandleId,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
    ) -> Result<Prepared, Error> {
        if !self.stamps.iter().any(|(id, p)| *id == unit && !p.active) {
            return Err(fail("hidden unit occupancy required"));
        }
        self.ensure_footprint(path)?;
        if let Some(prepared) = self.candidate(request, radius, path, face, None, false)? {
            return Ok(prepared);
        }
        self.candidate(request, radius, path, face, None, true)?
            .ok_or_else(|| fail("no admitted reveal placement"))
    }

    pub(crate) fn prepare_relocation(
        &mut self,
        unit: HandleId,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
    ) -> Result<Prepared, Error> {
        self.ensure_footprint(path)?;
        let stamp = self
            .stamps
            .iter()
            .find(|(id, _)| *id == unit)
            .map(|(_, stamp)| stamp)
            .ok_or_else(|| fail("unit occupancy is missing"))?;
        self.candidate(request, radius, path, face, Some(stamp), true)?
            .ok_or_else(|| fail("no admitted relocation placement"))
    }

    pub(crate) fn prepare_motion(
        &self,
        unit: HandleId,
        point: [f32; 2],
        radius: f32,
    ) -> Result<Option<Prepared>, Error> {
        let stamp = self
            .stamps
            .iter()
            .find(|(id, _)| *id == unit)
            .map(|(_, p)| p)
            .ok_or_else(|| fail("unit occupancy is missing"))?;
        if !stamp.footprint.is_empty() {
            return Err(fail("moving pathing textures are not admitted"));
        }
        self.candidate(point, Some(radius), "_", 0.0, Some(stamp), false)
    }

    pub(crate) fn segment_clear(
        &self,
        unit: HandleId,
        from: [f32; 2],
        to: [f32; 2],
        radius: f32,
        work: &mut usize,
    ) -> Result<bool, Error> {
        Ok(self.clear_fraction(unit, from, to, radius, work)? == 1.0)
    }

    /// Fraction of `from..to` that is clear, sampled every half cell with corner
    /// checks between samples. `1.0` when the whole segment is clear. Otherwise the
    /// fraction of the last clear sample before the first blocked one, `0.0` when
    /// the first sample is blocked. A caller stops the unit at
    /// `from + (to - from) * fraction`, which is the same point the sample used.
    pub(crate) fn clear_fraction(
        &self,
        unit: HandleId,
        from: [f32; 2],
        to: [f32; 2],
        radius: f32,
        work: &mut usize,
    ) -> Result<f64, Error> {
        let delta = [
            f64::from(to[0]) - f64::from(from[0]),
            f64::from(to[1]) - f64::from(from[1]),
        ];
        let steps = (delta[0].hypot(delta[1]) / (f64::from(self.rules.cell_size()) * 0.5))
            .ceil()
            .max(1.);
        if !steps.is_finite() || steps > *work as f64 {
            return Err(fail("movement segment work exhausted"));
        }
        let steps = steps as usize;
        let centre = |p: [i32; 2]| {
            [
                self.grid.origin[0] + (p[0] as f32 + 0.5) * self.rules.cell_size(),
                self.grid.origin[1] + (p[1] as f32 + 0.5) * self.rules.cell_size(),
            ]
        };
        let mut previous = self.cell(from)?;
        for n in 1..=steps {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| fail("movement segment work exhausted"))?;
            let point = [
                (f64::from(from[0]) + delta[0] * n as f64 / steps as f64) as f32,
                (f64::from(from[1]) + delta[1] * n as f64 / steps as f64) as f32,
            ];
            let before = (n - 1) as f64 / steps as f64;
            if self.prepare_motion(unit, point, radius)?.is_none() {
                return Ok(before);
            }
            let cell = self.cell(point)?;
            if cell[0] != previous[0] && cell[1] != previous[1] {
                for p in [[cell[0], previous[1]], [previous[0], cell[1]]] {
                    *work = work
                        .checked_sub(1)
                        .ok_or_else(|| fail("movement segment work exhausted"))?;
                    if self.prepare_motion(unit, centre(p), radius)?.is_none() {
                        return Ok(before);
                    }
                }
            }
            previous = cell;
        }
        Ok(1.0)
    }

    /// Bounded deterministic cell routing on this same live collision cache.
    /// shortcut: A* tie order and cell-centre steering are reconstruction policy;
    /// qualify native path choice, local avoidance and steering before parity claims.
    pub(crate) fn route(
        &self,
        unit: HandleId,
        start: [f32; 2],
        target: [f32; 2],
        radius: f32,
        work: &mut usize,
    ) -> Result<Vec<[f32; 2]>, Error> {
        use std::collections::{BTreeMap, BinaryHeap};
        #[derive(PartialEq)]
        struct Open {
            cost: f64,
            travelled: f64,
            cell: usize,
        }
        impl Eq for Open {}
        impl PartialOrd for Open {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Open {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                other
                    .cost
                    .total_cmp(&self.cost)
                    .then_with(|| other.cell.cmp(&self.cell))
                    .then_with(|| other.travelled.total_cmp(&self.travelled))
            }
        }
        if self.segment_clear(unit, start, target, radius, work)? {
            let mut path = Vec::new();
            path.try_reserve_exact(1)
                .map_err(|_| fail("route storage"))?;
            path.push(target);
            return Ok(path);
        }
        // Exhausted work ends the search; the unit then goes as near as it got.
        let mut charge = || -> bool {
            match work.checked_sub(1) {
                Some(left) => {
                    *work = left;
                    true
                }
                None => false,
            }
        };
        if !charge() {
            return Err(fail("movement route work exhausted"));
        }
        let target_clear = self.prepare_motion(unit, target, radius)?.is_some();
        let start_cell = self.cell(start)?;
        let target_cell = self.cell(target)?;
        let index = |p: [i32; 2]| {
            self.index(i64::from(p[0]), i64::from(p[1]))
                .ok_or_else(|| fail("movement cell outside map"))
        };
        let begin = index(start_cell)?;
        let end = index(target_cell).ok();
        let xy = |i: usize| {
            [
                (i % self.grid.pathing.width as usize) as i32,
                (i / self.grid.pathing.width as usize) as i32,
            ]
        };
        let centre = |p: [i32; 2]| {
            [
                self.grid.origin[0] + (p[0] as f32 + 0.5) * self.rules.cell_size(),
                self.grid.origin[1] + (p[1] as f32 + 0.5) * self.rules.cell_size(),
            ]
        };
        let heuristic = |p: [i32; 2]| {
            let dx = (i64::from(p[0]) - i64::from(target_cell[0])).unsigned_abs() as f64;
            let dy = (i64::from(p[1]) - i64::from(target_cell[1])).unsigned_abs() as f64;
            dx.max(dy) + (std::f64::consts::SQRT_2 - 1.) * dx.min(dy)
        };
        let mut frontier = BinaryHeap::new();
        frontier.try_reserve(1).map_err(|_| fail("route storage"))?;
        frontier.push(Open {
            cost: heuristic(start_cell),
            travelled: 0.,
            cell: begin,
        });
        let mut reached = BTreeMap::new();
        reached.insert(begin, (0.0, None));
        // Nearest explored cell to the target: (remaining estimate, travelled, cell).
        let mut best = (heuristic(start_cell), 0.0, begin);
        let mut arrived = false;
        'search: while let Some(open) = frontier.pop() {
            if !charge() {
                break;
            }
            if open.travelled > reached[&open.cell].0 {
                continue;
            }
            let remaining = heuristic(xy(open.cell));
            if (remaining, open.travelled) < (best.0, best.1) {
                best = (remaining, open.travelled, open.cell);
            }
            if Some(open.cell) == end && target_clear {
                best = (0.0, open.travelled, open.cell);
                arrived = true;
                break;
            }
            let point = xy(open.cell);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let p = [point[0] + dx, point[1] + dy];
                    let Ok(next) = index(p) else {
                        continue;
                    };
                    let distance = if dx != 0 && dy != 0 {
                        std::f64::consts::SQRT_2
                    } else {
                        1.
                    };
                    let travelled = open.travelled + distance;
                    if reached.get(&next).is_some_and(|(old, _)| *old <= travelled) {
                        continue;
                    }
                    if !charge() {
                        break 'search;
                    }
                    if self.prepare_motion(unit, centre(p), radius)?.is_none() {
                        continue;
                    }
                    if dx != 0 && dy != 0 {
                        if !charge() {
                            break 'search;
                        }
                        if self
                            .prepare_motion(unit, centre([p[0], point[1]]), radius)?
                            .is_none()
                        {
                            continue;
                        }
                        if !charge() {
                            break 'search;
                        }
                        if self
                            .prepare_motion(unit, centre([point[0], p[1]]), radius)?
                            .is_none()
                        {
                            continue;
                        }
                    }
                    frontier.try_reserve(1).map_err(|_| fail("route storage"))?;
                    reached.insert(next, (travelled, Some(open.cell)));
                    frontier.push(Open {
                        cost: travelled + heuristic(p),
                        travelled,
                        cell: next,
                    });
                }
            }
        }
        let mut path = Vec::new();
        let mut at = best.2;
        while at != begin {
            path.try_reserve(1).map_err(|_| fail("route storage"))?;
            path.push(centre(xy(at)));
            at = reached[&at].1.ok_or_else(|| fail("broken route"))?;
        }
        path.reverse();
        match path.last_mut() {
            Some(last) if arrived => *last = target,
            Some(_) => {}
            None if arrived => {
                path.try_reserve(1).map_err(|_| fail("route storage"))?;
                path.push(target);
            }
            // Already as near as the unit can get: stay put.
            None => {
                path.try_reserve(1).map_err(|_| fail("route storage"))?;
                path.push(start);
            }
        }
        Ok(path)
    }

    pub(crate) fn commit(&mut self, unit: HandleId, prepared: Prepared) {
        let old = self.stamps.iter().position(|(id, _)| *id == unit);
        if let Some(at) = old
            && self.stamps[at].1.active
        {
            for &i in &self.stamps[at].1.cells {
                self.occupied[i] -= 1;
            }
            for &i in &self.stamps[at].1.footprint {
                self.footprint_occupied[i] -= 1;
            }
        }
        if prepared.active {
            for &i in &prepared.cells {
                self.occupied[i] += 1;
            }
            for &i in &prepared.footprint {
                self.footprint_occupied[i] += 1;
            }
        }
        if let Some(at) = old {
            self.stamps[at].1 = prepared;
        } else {
            self.stamps.push((unit, prepared));
        }
    }
}
