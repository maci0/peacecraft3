//! W3E blight and source-clock growth. Terrain presentation is separate.
use crate::scripts::AssetIdentity;
use pcx_map::Terrain;
use pcx_script::HandleId;

/// Growth operands from the selected executable.
#[derive(Clone, Debug)]
pub struct GrowthRules {
    pub(crate) ability_class: [u8; 4],
    pub(crate) spacing: f32,
    pub(crate) fine_spacing: f32,
    pub(crate) interval_cap: f32,
    pub(crate) tolerance: f32,
}

impl GrowthRules {
    pub fn reviewed_legacy(image: &[u8]) -> Result<Self, String> {
        use crate::neutral_player::{RuleInputBudget, sha256};
        use crate::neutral_rule_input::Pe;
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("blight growth: unsupported client image".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|error| error.to_string())?;
        let read = |address, len, code| -> Result<&[u8], String> {
            pe.read_from(address, code)
                .map_err(|error| error.to_string())?
                .get(..len)
                .ok_or_else(|| format!("truncated blight operand at {address:#x}"))
        };
        let class = read(0xb8c3a0, 6, true)?;
        let grid = read(0x40a860, 10, true)?;
        let fine_grid = read(0x4015b0, 7, true)?;
        let timer = read(0x6daf14, 8, true)?;
        let tolerance = read(0x6da192, 8, true)?;
        if class[0] != 0xb8
            || class[5] != 0xc3
            || grid[0] != 0x68
            || grid[5] != 0x68
            || fine_grid[0] != 0x6a
            || fine_grid[2] != 0x68
            || timer[..4] != [0xf3, 0x0f, 0x10, 0x05]
            || tolerance[..4] != [0xf3, 0x0f, 0x10, 0x05]
        {
            return Err("blight growth instructions differ".into());
        }
        let word = |bytes: &[u8]| u32::from_le_bytes(bytes.try_into().expect("four-byte operand"));
        if word(&grid[6..10]) != 0x114008c || word(&fine_grid[3..7]) != 0x112d7f4 {
            return Err("blight grid initializer differs".into());
        }
        let number =
            |address| -> Result<f32, String> { Ok(f32::from_bits(word(read(address, 4, false)?))) };
        let rules = Self {
            ability_class: word(&class[1..5]).to_be_bytes(),
            spacing: word(&grid[1..5]) as f32,
            fine_spacing: f32::from(fine_grid[1] as i8),
            interval_cap: number(word(&timer[4..8]))?,
            tolerance: number(word(&tolerance[4..8]))?,
        };
        if [
            rules.spacing,
            rules.fine_spacing,
            rules.interval_cap,
            rules.tolerance,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err("invalid blight growth operands".into());
        }
        Ok(rules)
    }
}

/// A source's discrete blight growth, using caller-selected ability and grid inputs.
#[derive(Clone, Debug)]
pub struct Growth {
    center: [f64; 2],
    grid_origin: [f64; 2],
    spacing: f64,
    fine_spacing: f64,
    start: f64,
    interval: f64,
    increment: f64,
    target: f64,
    tolerance: f64,
    fixed: bool,
}

/// Growth parameters from a selected ability and executable.
#[derive(Clone, Copy, Debug)]
pub struct GrowthSpec {
    pub spacing: f32,
    pub fine_spacing: f32,
    pub interval: f32,
    pub increment: f32,
    pub target: f32,
    pub tolerance: f32,
}

impl Growth {
    /// Values are supplied by the selected ability, game clock and terrain grid.
    pub fn new(
        center: [f32; 2],
        grid_origin: [f32; 2],
        start: f64,
        spec: GrowthSpec,
    ) -> Result<Self, String> {
        if center.iter().chain(&grid_origin).any(|v| !v.is_finite())
            || !start.is_finite()
            || [
                spec.spacing,
                spec.fine_spacing,
                spec.interval,
                spec.increment,
                spec.target,
                spec.tolerance,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
        {
            return Err("blight growth needs finite positive source parameters".into());
        }
        let spacing = f64::from(spec.spacing);
        Ok(Self {
            center: center.map(|v| (f64::from(v) / spacing).round() * spacing),
            grid_origin: grid_origin.map(f64::from),
            spacing,
            fine_spacing: f64::from(spec.fine_spacing),
            start,
            interval: f64::from(spec.interval),
            increment: f64::from(spec.increment),
            target: ((f64::from(spec.target) + spacing / 2.0) / spacing).floor() * spacing,
            tolerance: f64::from(spec.tolerance),
            fixed: false,
        })
    }

    /// A script paints a fixed radius immediately, without a growth timer.
    pub fn fixed(
        center: [f32; 2],
        grid_origin: [f32; 2],
        spacing: f32,
        fine_spacing: f32,
        start: f64,
        radius: f32,
    ) -> Result<Self, String> {
        if center
            .iter()
            .chain(&grid_origin)
            .any(|value| !value.is_finite())
            || !spacing.is_finite()
            || spacing <= 0.0
            || !fine_spacing.is_finite()
            || fine_spacing <= 0.0
            || !start.is_finite()
            || !radius.is_finite()
            || radius < 0.0
        {
            return Err("script blight needs a finite nonnegative radius and grid".into());
        }
        let spacing = f64::from(spacing);
        Ok(Self {
            center: center.map(|value| (f64::from(value) / spacing).round() * spacing),
            grid_origin: grid_origin.map(f64::from),
            spacing,
            fine_spacing: f64::from(fine_spacing),
            start,
            interval: 0.0,
            increment: f64::from(radius),
            target: f64::from(radius),
            tolerance: 0.0,
            fixed: true,
        })
    }

    /// The spatial pathing flag at a point. Owner-mask cells are a separate layer.
    pub fn is_blighted_at(&self, point: [f32; 2], now: f64) -> Result<bool, String> {
        self.mask_at(point, now, false)
    }

    fn mask_at(&self, point: [f32; 2], now: f64, owner_grid: bool) -> Result<bool, String> {
        if !now.is_finite() || point.iter().any(|v| !v.is_finite()) {
            return Err("blight growth needs a finite clock and point".into());
        }
        if now < self.start {
            return Ok(false);
        }
        let radius = if self.fixed {
            self.increment
        } else {
            let steps = ((now - self.start) / self.interval).floor() + 1.0;
            let stop = ((self.target - self.tolerance) / self.increment)
                .ceil()
                .max(1.0);
            let stop_radius = stop * self.increment;
            let stop = if (stop_radius - self.target).abs() >= self.tolerance
                && stop_radius <= self.target
            {
                stop + 1.0
            } else {
                stop
            };
            self.increment * steps.min(stop)
        };
        let n = ((radius + self.spacing / 2.0) / self.spacing).floor();
        if n > f64::from(u16::MAX / 2) {
            return Err("blight growth radius exceeds native circle budget".into());
        }
        let n = n as i32;
        let cell = |value: f64, axis: usize| {
            ((value - self.grid_origin[axis]) / self.spacing).trunc() as i64
        };
        let x = cell(f64::from(point[0]), 0) - cell(self.center[0], 0);
        let y = cell(f64::from(point[1]), 1) - cell(self.center[1], 1);
        if !owner_grid && n <= 2 {
            let fine_cell = |value: f64, axis: usize| {
                ((value - self.grid_origin[axis]) / self.fine_spacing)
                    .trunc()
                    .max(0.0)
            };
            let inside = |bounds: [i32; 4]| {
                (0..2).all(|axis| {
                    let coordinate = fine_cell(f64::from(point[axis]), axis);
                    let lower = fine_cell(
                        self.center[axis] + f64::from(bounds[axis]) * self.spacing,
                        axis,
                    );
                    let upper = fine_cell(
                        self.center[axis] + f64::from(bounds[axis + 2]) * self.spacing,
                        axis,
                    );
                    coordinate >= lower && coordinate < upper
                })
            };
            return Ok(if n < 2 {
                inside([-1, -1, 1, 1])
            } else {
                inside([-3, -1, 2, 1])
                    || inside([-2, -2, 2, -1])
                    || inside([-2, 1, 2, 2])
                    || inside([-1, -3, 1, -2])
                    || inside([-1, 2, 1, 3])
            });
        }
        if n < 2 {
            return Ok((-1..=0).contains(&x) && (-1..=0).contains(&y));
        }
        if n == 2 {
            let max_x = if owner_grid { 2 } else { 1 };
            return Ok(((-1..=0).contains(&y) && (-3..=max_x).contains(&x))
                || (matches!(y, -2 | 1) && (-2..=1).contains(&x))
                || (matches!(y, -3 | 2) && (-1..=0).contains(&x)));
        }
        if y < i64::from(-n - 1) || y > i64::from(n) || x < i64::from(-n - 1) || x > i64::from(n) {
            return Ok(false);
        }
        let requested = [y + i64::from(n), y + 1 + i64::from(n)];
        let mut extent = if requested.contains(&i64::from(n)) {
            n
        } else {
            0
        };
        let (mut decision, mut a, mut b) = (3 - 2 * n, 6, 10 - 4 * n);
        let (mut x0, mut y0, mut index) = (0, n, 0_usize);
        let (mut lower, mut upper, mut inner) = (n - 1, n + 1, 1);
        while x0 <= y0 {
            let (step, add) = if decision < 0 {
                (a, 4)
            } else {
                if requested.contains(&(index as i64))
                    || requested.contains(&(i64::from(2 * n) - index as i64))
                {
                    extent = extent.max(x0);
                }
                while inner <= x0 {
                    if requested.contains(&i64::from(lower))
                        || requested.contains(&i64::from(upper))
                    {
                        extent = extent.max(y0);
                    }
                    lower -= 1;
                    upper += 1;
                    inner += 1;
                }
                index += 1;
                y0 -= 1;
                (b, 8)
            };
            decision += step;
            x0 += 1;
            b += add;
            a += 4;
        }
        Ok(x >= -i64::from(extent) - 1 && x <= i64::from(extent))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceState {
    Active,
    Removed,
    UnqualifiedRemoval,
}

/// Map-authored corner flags and live growth on their source-derived world grid.
/// shortcut: initial flags use nearest corners; replace with the native initial mask grid.
#[derive(Debug)]
pub struct InitialBlight {
    pub origin: AssetIdentity,
    world_origin: [f64; 2],
    spacing: f64,
    size: [usize; 2],
    corners: Vec<u8>,
    sources: Vec<(Option<HandleId>, HandleId, Growth, SourceState)>,
    pub(crate) growth_rules: Option<GrowthRules>,
}

impl InitialBlight {
    /// `spacing` is derived by the caller from admitted pathing normalization and
    /// the map's terrain/pathing dimension ratio; no measured tile size is copied.
    pub fn read(
        terrain: &Terrain,
        spacing: f32,
        origin: AssetIdentity,
        max_corners: usize,
    ) -> Result<Self, String> {
        if terrain.corner_count() > max_corners {
            return Err("initial blight corner budget exceeded".into());
        }
        let (x, y) = terrain.origin();
        if !spacing.is_finite() || spacing <= 0.0 || !x.is_finite() || !y.is_finite() {
            return Err("initial blight needs finite origin and positive spacing".into());
        }
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err("initial blight needs explicit source identity".into());
        }
        let size = [terrain.width(), terrain.height()].map(|n| n as usize + 1);
        if size[0].checked_mul(size[1]) != Some(terrain.corner_count()) {
            return Err("initial blight corner dimensions disagree".into());
        }
        let mut corners = Vec::new();
        let byte_count = terrain.corner_count().div_ceil(8);
        corners
            .try_reserve_exact(byte_count)
            .map_err(|_| "initial blight corner storage")?;
        corners.resize(byte_count, 0);
        for (index, corner) in terrain.corners().iter().enumerate() {
            if corner.is_blighted() {
                corners[index / 8] |= 1 << (index % 8);
            }
        }
        Ok(Self {
            origin,
            world_origin: [f64::from(x), f64::from(y)],
            spacing: f64::from(spacing),
            size,
            corners,
            sources: Vec::new(),
            growth_rules: None,
        })
    }

    pub fn with_growth_rules(mut self, rules: GrowthRules) -> Self {
        self.growth_rules = Some(rules);
        self
    }

    pub(crate) fn growth_origin(&self) -> [f32; 2] {
        self.world_origin.map(|value| value as f32)
    }

    pub(crate) fn has_growth(&self) -> bool {
        self.sources
            .iter()
            .any(|(_, _, _, state)| *state != SourceState::UnqualifiedRemoval)
    }

    /// Retain the area of an unqualified removal without aborting construction.
    pub(crate) fn add_unqualified_removal(
        &mut self,
        unit: HandleId,
        owner: HandleId,
        effect: Growth,
    ) -> Result<(), String> {
        if effect.grid_origin != self.world_origin || effect.spacing != self.spacing {
            return Err("blight removal grid differs from terrain".into());
        }
        self.sources
            .try_reserve(1)
            .map_err(|_| "blight removal allocation")?;
        self.sources
            .push((Some(unit), owner, effect, SourceState::UnqualifiedRemoval));
        Ok(())
    }

    /// Refuse placement only when its cell intersects a pending removal.
    pub(crate) fn is_blighted_for_placement(
        &self,
        position: [f32; 2],
        now: f64,
    ) -> Result<bool, String> {
        for (unit, _, effect, state) in &self.sources {
            if *state == SourceState::UnqualifiedRemoval && effect.is_blighted_at(position, now)? {
                return Err(format!("blight removal effect on {unit:?} is not admitted"));
            }
        }
        self.is_blighted_at(position, now)
    }

    fn grid(&self, p: [f32; 2]) -> Result<[f64; 2], String> {
        if p.iter().any(|v| !v.is_finite()) {
            return Err("initial blight needs finite position".into());
        }
        Ok(std::array::from_fn(|i| {
            (f64::from(p[i]) - self.world_origin[i]) / self.spacing + 0.5
        }))
    }

    /// Outside the corner footprints there is no map-authored blight.
    pub fn is_blighted(&self, position: [f32; 2]) -> Result<bool, String> {
        if self.has_growth() {
            return Err("dynamic blight query requires a game clock".into());
        }
        Ok(self.at_grid(self.grid(position)?))
    }

    /// Register a completed structure's growth effect.
    pub fn add_growth(
        &mut self,
        unit: HandleId,
        owner: HandleId,
        growth: Growth,
    ) -> Result<(), String> {
        if growth.grid_origin != self.world_origin || growth.spacing != self.spacing {
            return Err("blight source grid differs from terrain".into());
        }
        if self.sources.iter().any(|(id, _, _, _)| *id == Some(unit)) {
            return Err(format!("blight source {unit:?} already registered"));
        }
        self.sources
            .try_reserve(1)
            .map_err(|_| "blight source allocation")?;
        self.sources
            .push((Some(unit), owner, growth, SourceState::Active));
        Ok(())
    }

    /// Register a script-owned fixed patch.
    pub fn add_script_patch(&mut self, owner: HandleId, growth: Growth) -> Result<(), String> {
        if growth.grid_origin != self.world_origin
            || growth.spacing != self.spacing
            || !growth.fixed
        {
            return Err("script blight grid or effect differs".into());
        }
        if self
            .sources
            .iter()
            .any(|(unit, player, existing, removed)| {
                unit.is_none()
                    && *player == owner
                    && *removed == SourceState::Active
                    && existing.fixed
                    && existing.center == growth.center
                    && existing.increment == growth.increment
                    && existing.start <= growth.start
            })
        {
            return Ok(());
        }
        self.sources
            .try_reserve(1)
            .map_err(|_| "script blight allocation")?;
        self.sources
            .push((None, owner, growth, SourceState::Active));
        Ok(())
    }

    /// Mark a source removed; queries of its affected cells refuse until removal is qualified.
    pub fn remove_growth(&mut self, unit: HandleId) {
        if let Some((_, _, _, removed)) = self
            .sources
            .iter_mut()
            .find(|(id, _, _, state)| *id == Some(unit) && *state == SourceState::Active)
        {
            *removed = SourceState::Removed;
        }
    }

    /// Query the current spatial flag, refusing overlapping or removed sources.
    pub fn is_blighted_at(&self, position: [f32; 2], now: f64) -> Result<bool, String> {
        self.mask_at(position, now, false, None)
    }

    /// Query the coarse owner layer, optionally restricted to one owner.
    pub fn owner_mask_at(
        &self,
        position: [f32; 2],
        now: f64,
        owner: Option<HandleId>,
    ) -> Result<bool, String> {
        self.mask_at(position, now, true, owner)
    }

    fn mask_at(
        &self,
        position: [f32; 2],
        now: f64,
        owner_grid: bool,
        owner: Option<HandleId>,
    ) -> Result<bool, String> {
        let mut found: Option<Option<HandleId>> = None;
        let mut selected = false;
        for (unit, source_owner, growth, removed) in &self.sources {
            // shortcut: placement alone checks pending dispels until their raster is decoded.
            if *removed == SourceState::UnqualifiedRemoval {
                continue;
            }
            if growth.mask_at(position, now, owner_grid)? {
                if *removed == SourceState::Removed {
                    return Err(format!("blight source {unit:?} was removed"));
                }
                if let Some(other) = found {
                    return Err(format!("blight sources {other:?} and {unit:?} overlap"));
                }
                found = Some(*unit);
                selected = owner.is_none_or(|owner| owner == *source_owner);
            }
        }
        Ok(selected || self.at_grid(self.grid(position)?))
    }

    fn at_grid(&self, p: [f64; 2]) -> bool {
        if (0..2).any(|i| p[i] < 0.0 || p[i] >= self.size[i] as f64) {
            return false;
        }
        let index = p[1].floor() as usize * self.size[0] + p[0].floor() as usize;
        self.corners[index / 8] & (1 << (index % 8)) != 0
    }

    /// Integrate a straight movement span against source-clock changes and grid edges.
    pub(crate) fn seconds_at(
        &self,
        from: [f32; 2],
        to: [f32; 2],
        start: f64,
        end: f64,
        work: &mut usize,
    ) -> Result<f64, String> {
        if !start.is_finite() || !end.is_finite() || end < start {
            return Err("blight integration needs a monotonic clock".into());
        }
        if !self.has_growth() {
            return self.seconds(from, to, end - start, work);
        }
        self.grid(from)?;
        self.grid(to)?;
        if end == start {
            return Ok(0.0);
        }
        let duration = end - start;
        let a = from.map(f64::from);
        let delta: [f64; 2] = std::array::from_fn(|i| f64::from(to[i]) - a[i]);
        let mut cursor = 0.0_f64;
        let mut total = 0.0;
        while cursor < 1.0 {
            *work = work.checked_sub(1).ok_or("blight work exhausted")?;
            let mut next = 1.0_f64;
            for axis in 0..2 {
                if delta[axis] == 0.0 {
                    continue;
                }
                let mut next_grid_edge = |spacing: f64, shift: f64| {
                    let grid = (a[axis] + delta[axis] * cursor - self.world_origin[axis]) / spacing
                        + shift;
                    let boundary = if delta[axis] > 0.0 {
                        grid.floor() + 1.0
                    } else {
                        grid.ceil() - 1.0
                    };
                    let edge = ((boundary - shift) * spacing + self.world_origin[axis] - a[axis])
                        / delta[axis];
                    if edge > cursor {
                        next = next.min(edge);
                    }
                };
                next_grid_edge(self.spacing, 0.0);
                next_grid_edge(self.spacing, 0.5);
                for (_, _, growth, state) in &self.sources {
                    if *state != SourceState::UnqualifiedRemoval {
                        next_grid_edge(growth.fine_spacing, 0.0);
                    }
                }
            }
            let time = start + cursor * duration;
            for (_, _, growth, state) in &self.sources {
                if *state == SourceState::UnqualifiedRemoval {
                    continue;
                }
                let boundary = if time < growth.start {
                    Some(growth.start)
                } else if growth.fixed {
                    None
                } else {
                    let steps = ((time - growth.start) / growth.interval).floor() + 1.0;
                    (growth.increment * steps < growth.target)
                        .then_some(growth.start + steps * growth.interval)
                };
                if let Some(boundary) = boundary {
                    let fraction = (boundary - start) / duration;
                    if fraction > cursor {
                        next = next.min(fraction);
                    }
                }
            }
            if next <= cursor {
                return Err("blight integration did not advance".into());
            }
            let middle = (cursor + next) * 0.5;
            let point = std::array::from_fn(|i| (a[i] + delta[i] * middle) as f32);
            if self.is_blighted_at(point, start + middle * duration)? {
                total += (next - cursor) * duration;
            }
            cursor = next;
        }
        Ok(total)
    }

    /// Exact time fraction inside blighted footprints on one straight segment.
    /// Every visited corner consumes caller work; no step-size sampling is used.
    pub(crate) fn seconds(
        &self,
        from: [f32; 2],
        to: [f32; 2],
        duration: f64,
        work: &mut usize,
    ) -> Result<f64, String> {
        if self.has_growth() {
            return Err("dynamic blight regeneration requires source-clock integration".into());
        }
        if !duration.is_finite() || duration < 0.0 {
            return Err("initial blight needs finite nonnegative duration".into());
        }
        let a = self.grid(from)?;
        let b = self.grid(to)?;
        let d: [f64; 2] = std::array::from_fn(|i| b[i] - a[i]);
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for i in 0..2 {
            if d[i] == 0.0 {
                if a[i] < 0.0 || a[i] >= self.size[i] as f64 {
                    return Ok(0.0);
                }
            } else {
                let t0 = -a[i] / d[i];
                let t1 = (self.size[i] as f64 - a[i]) / d[i];
                enter = enter.max(t0.min(t1));
                leave = leave.min(t0.max(t1));
            }
        }
        if enter >= leave || duration == 0.0 {
            return Ok(0.0);
        }
        let mut edge = [f64::INFINITY; 2];
        let mut stride = [f64::INFINITY; 2];
        for i in 0..2 {
            let p = a[i] + d[i] * enter;
            if d[i] > 0.0 {
                edge[i] = (p.floor() + 1.0 - a[i]) / d[i];
            } else if d[i] < 0.0 {
                edge[i] = (p.ceil() - 1.0 - a[i]) / d[i];
            }
            if d[i] != 0.0 {
                stride[i] = 1.0 / d[i].abs();
            }
        }
        let mut total = 0.0;
        while enter < leave {
            *work = work.checked_sub(1).ok_or("initial blight work exhausted")?;
            let end = leave.min(edge[0]).min(edge[1]);
            if end <= enter {
                return Err("initial blight crossing did not advance".into());
            }
            let middle = (enter + end) * 0.5;
            if self.at_grid(std::array::from_fn(|i| a[i] + d[i] * middle)) {
                total += (end - enter) * duration;
            }
            for i in 0..2 {
                if edge[i] <= end {
                    edge[i] += stride[i];
                }
            }
            enter = end;
        }
        Ok(total)
    }
}

#[cfg(test)]
#[path = "terrain_blight_tests.rs"]
pub(crate) mod tests;
