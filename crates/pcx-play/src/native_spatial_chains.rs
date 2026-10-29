//! Bounded storage for the native spatial grid's ordered cell links.

use super::{CellRect, ChainEntry, EntryRules};

const NONE: u32 = u32::MAX;

fn cleanup_operands(
    initializer: &[u8],
    loaded: &[u8],
    minimum_check: &[u8],
) -> Result<(i32, i32, u32), String> {
    if initializer.len() < 32
        || loaded.len() < 13
        || minimum_check.len() < 7
        || initializer[0] != 0x6a
        || initializer[15] != 0x6a
        || initializer[27] != 0x68
        || loaded[0] != 0xa1
        || loaded[5..9] != [0xf3, 0x0f, 0x10, 0x05]
        || minimum_check[..3] != [0x0f, 0x47, 0x05]
        || initializer[28..32] != loaded[1..5]
        || loaded[9..13] != minimum_check[3..7]
    {
        return Err("unrecognized spatial cleanup interval instructions".into());
    }
    Ok((
        i32::from(initializer[16] as i8),
        i32::from(initializer[1] as i8),
        u32::from_le_bytes(loaded[9..13].try_into().expect("checked width")),
    ))
}

fn region_class_operand(call: &[u8], select: &[u8]) -> Result<u32, String> {
    if call.len() < 8
        || call[..4] != [0x6a, 1, 0x50, 0xe8]
        || select.len() < 12
        || select[..8] != [0x8b, 0x45, 0x0c, 0xf7, 0xd8, 0x1b, 0xc0, 0x25]
    {
        return Err("unrecognized region link class instructions".into());
    }
    let displacement = i32::from_le_bytes(call[4..8].try_into().expect("checked width"));
    if i64::from(0x4a4f1e) + i64::from(displacement) != 0x801a50 {
        return Err("region addition calls a different target".into());
    }
    let class = u32::from_le_bytes(select[8..12].try_into().expect("checked width"));
    if class == 0 {
        return Err("region addition class is empty".into());
    }
    Ok(class)
}

/// Selected movement-entry state test. Velocity is in the native spatial units.
#[derive(Clone, Copy, Debug)]
pub struct MovementStateRule {
    threshold: f32,
    moving_bit: u32,
}

impl MovementStateRule {
    #[cfg(test)]
    pub(crate) fn synthetic(threshold: f32, moving_bit: u32) -> Self {
        Self {
            threshold,
            moving_bit,
        }
    }

    /// Read the movement threshold and state mask from the reviewed image.
    pub fn reviewed_legacy(
        image: &[u8],
        rules: &crate::ai_placement::Rules,
    ) -> Result<Self, String> {
        use crate::neutral_player::{RuleInputBudget, sha256};
        use crate::neutral_rule_input::Pe;

        if rules.image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || sha256(image) != rules.image_sha256
        {
            return Err("spatial movement image identity mismatch".into());
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
        let read = |address, size, code| -> Result<&[u8], String> {
            pe.read_from(address, code)
                .map_err(|e| format!("spatial movement source at {address:#x}: {e}"))?
                .get(..size)
                .ok_or_else(|| format!("truncated spatial movement source at {address:#x}"))
        };
        let threshold_load = read(0x8bcae7, 8, true)?;
        let zero_load = read(0x8bcad5, 5, true)?;
        let set = read(0x8bcb6d, 7, true)?;
        let clear = read(0x8bcb15, 7, true)?;
        if threshold_load[..4] != [0xf3, 0x0f, 0x10, 0x05]
            || zero_load[0] != 0x68
            || set[..3] != [0x81, 0x48, 0x44]
            || clear[..3] != [0x81, 0x60, 0x44]
        {
            return Err("unrecognized spatial movement test instructions".into());
        }
        let address = |bytes: &[u8]| u32::from_le_bytes(bytes.try_into().expect("checked width"));
        let threshold = f32::from_le_bytes(
            read(address(&threshold_load[4..]), 4, false)?
                .try_into()
                .expect("checked width"),
        );
        let zero_address = address(&zero_load[1..]);
        let moving_bit = address(&set[3..]);
        if !threshold.is_normal()
            || threshold <= 0.0
            || !pe.zero_filled(zero_address, 4)
            || moving_bit.count_ones() != 1
            || address(&clear[3..]) != !moving_bit
        {
            return Err("unsupported spatial movement threshold or state mask".into());
        }
        Ok(Self {
            threshold,
            moving_bit,
        })
    }

    /// Apply the decoded state test to a qualified sole movement contribution.
    pub fn apply(self, state: u32, velocity: [f32; 2], limit: f32) -> Result<u32, Error> {
        use crate::script_random::real;

        if !limit.is_finite() || limit < 0.0 {
            return Err(Error::MovementState);
        }
        let [x, y] = velocity.map(real::from_f32);
        let (x, y) = (
            x.map_err(|_| Error::MovementState)?,
            y.map_err(|_| Error::MovementState)?,
        );
        let magnitude = real::add(
            real::mul(x, x).map_err(|_| Error::MovementState)?,
            real::mul(y, y).map_err(|_| Error::MovementState)?,
        )
        .map_err(|_| Error::MovementState)?;
        let moving = f32::from_bits(magnitude) >= self.threshold && limit > 0.0;
        Ok(if moving {
            state | self.moving_bit
        } else {
            state & !self.moving_bit
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("spatial chain grid dimensions are invalid")]
    Dimensions,
    #[error("spatial chain rectangle is outside the grid")]
    Rectangle,
    #[error("spatial chain region coordinates cannot be converted")]
    Coordinates,
    #[error("spatial chain capacity {capacity} exceeds native index limit {limit}")]
    Capacity { capacity: usize, limit: usize },
    #[error("spatial chain pool exhausted: need {needed} links, {available} available")]
    PoolExhausted { needed: usize, available: usize },
    #[error("spatial chain allocation failed")]
    Allocation,
    #[error("spatial cleanup clock is invalid or cannot advance")]
    Clock,
    #[error("spatial entry {0} is missing")]
    MissingEntry(u64),
    #[error("spatial entry {0} is already registered")]
    DuplicateEntry(u64),
    #[error("spatial entry {0} has no movement rectangle")]
    MissingMovement(u64),
    #[error("spatial entry {0} exclusion count is unbalanced")]
    Exclusion(u64),
    #[error("spatial entry capacity {0} is exhausted")]
    EntryCapacity(usize),
    #[error("spatial registration sequence overflow")]
    Sequence,
    #[error("spatial movement state has unsupported velocity or limit")]
    MovementState,
    #[error("spatial entry {identity} has an unqualified movement contributor: {contributor}")]
    MovementContributor {
        identity: u64,
        contributor: &'static str,
    },
}

/// Convert a world-space region to the native grid's clipped, transposed cells.
pub fn region_cells(
    bounds: [f32; 4],
    origin: [f32; 2],
    cell_size: f32,
    size: [usize; 2],
) -> Result<CellRect, Error> {
    use crate::script_random::real;

    if bounds
        .iter()
        .chain(origin.iter())
        .any(|value| !value.is_finite())
        || bounds[0] > bounds[2]
        || bounds[1] > bounds[3]
        || !cell_size.is_normal()
        || cell_size <= 0.0
        || cell_size.to_bits() & ((1 << 23) - 1) != 0
        || size.contains(&0)
        || size.iter().any(|&axis| axis > i32::MAX as usize)
    {
        return Err(Error::Coordinates);
    }
    let scale = real::from_f32(1.0 / cell_size).map_err(|_| Error::Coordinates)?;
    let mut lower = [0; 2];
    let mut upper = [0; 2];
    for native_axis in 0..2 {
        let world_axis = 1 - native_axis;
        let offset = real::from_f32(origin[world_axis]).map_err(|_| Error::Coordinates)?;
        let coordinate = |value: f32| -> Result<i64, Error> {
            let value = real::from_f32(value).map_err(|_| Error::Coordinates)?;
            let delta = real::sub(value, offset).map_err(|_| Error::Coordinates)?;
            let cell = f64::from(f32::from_bits(
                real::mul(delta, scale).map_err(|_| Error::Coordinates)?,
            ))
            .floor();
            if cell < f64::from(i32::MIN) || cell >= f64::from(i32::MAX) {
                return Err(Error::Coordinates);
            }
            Ok(cell as i64)
        };
        let limit = size[native_axis] as i64;
        lower[native_axis] = coordinate(bounds[world_axis])?.clamp(0, limit) as usize;
        upper[native_axis] = (coordinate(bounds[world_axis + 2])? + 1).clamp(0, limit) as usize;
    }
    Ok(CellRect { lower, upper })
}

/// The cleanup interval's operands as read from the verified image.
#[derive(Clone, Debug)]
pub struct CleanupOperands {
    image_sha256: crate::neutral_player::Sha256Digest,
    dividend: i32,
    divisor: i32,
    minimum: f32,
}

impl CleanupOperands {
    #[cfg(test)]
    pub(crate) fn synthetic() -> Self {
        Self {
            image_sha256: [0; 32],
            dividend: 1,
            divisor: 10,
            minimum: 0.1,
        }
    }

    /// Divide with the session's real arithmetic and apply the image's minimum.
    pub fn interval(
        &self,
        numeric: &crate::script_numeric::NumericPolicy,
    ) -> Result<f32, String> {
        use crate::script_random::real;

        if numeric.image_sha256() != &self.image_sha256 {
            return Err("spatial cleanup image identity mismatch".into());
        }
        let divisor = real::from_i32(self.divisor).map_err(|e| e.to_string())?;
        let dividend = real::from_i32(self.dividend).map_err(|e| e.to_string())?;
        let interval = f32::from_bits(
            numeric
                .division(dividend, divisor)
                .map_err(|e| e.to_string())?,
        );
        if !interval.is_finite()
            || !self.minimum.is_finite()
            || interval <= 0.0
            || self.minimum <= 0.0
        {
            return Err("invalid spatial cleanup interval".into());
        }
        Ok(interval.max(self.minimum))
    }
}

/// A recurring cleanup deadline in the selected client's real arithmetic.
#[derive(Clone, Copy, Debug)]
pub struct CleanupClock {
    interval: u32,
    next: u32,
}

impl CleanupClock {
    pub fn new(start: f32, interval: f32) -> Result<Self, Error> {
        use crate::script_random::real;

        if !start.is_finite() || start < 0.0 || !interval.is_finite() || interval <= 0.0 {
            return Err(Error::Clock);
        }
        let start = real::from_f32(start).map_err(|_| Error::Clock)?;
        let interval = real::from_f32(interval).map_err(|_| Error::Clock)?;
        let next = real::add(start, interval).map_err(|_| Error::Clock)?;
        if f32::from_bits(next) <= f32::from_bits(start) {
            return Err(Error::Clock);
        }
        Ok(Self { interval, next })
    }

    pub fn next(&self) -> f32 {
        f32::from_bits(self.next)
    }

    /// Pop a due deadline; callers deliver one cleanup before popping another.
    pub fn pop_due(&mut self, now: f32) -> Result<Option<f32>, Error> {
        use crate::script_random::real;

        if !now.is_finite() || now < 0.0 {
            return Err(Error::Clock);
        }
        let current = self.next();
        if now < current {
            return Ok(None);
        }
        let next = real::add(self.next, self.interval).map_err(|_| Error::Clock)?;
        if f32::from_bits(next) <= current {
            return Err(Error::Clock);
        }
        self.next = next;
        Ok(Some(current))
    }
}

#[derive(Clone, Copy)]
struct Node {
    identity: u64,
    class: u32,
    next: u32,
}

/// One link in a cell chain: the entry it refers to and the link class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainLink {
    pub identity: u64,
    pub class: u32,
}

/// Pool capacity is supplied by the session's resource budget and may not
/// exceed the index limit read from the selected executable.
pub struct Chains {
    size: [usize; 2],
    heads: Vec<u32>,
    dirty: Vec<u32>,
    nodes: Vec<Node>,
    free: u32,
    used: usize,
    capacity: usize,
    removal_class: u32,
    seen: Vec<u64>,
}

impl Chains {
    /// Read the link class used for an added region rectangle.
    pub fn reviewed_region_class(
        image: &[u8],
        rules: &crate::ai_placement::Rules,
    ) -> Result<u32, String> {
        use crate::neutral_player::{RuleInputBudget, sha256};
        use crate::neutral_rule_input::Pe;

        if rules.image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || sha256(image) != rules.image_sha256
        {
            return Err("spatial chain image identity mismatch".into());
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
        let call = pe.read_from(0x4a4f16, true).map_err(|e| e.to_string())?;
        let select = pe.read_from(0x801b6e, true).map_err(|e| e.to_string())?;
        region_class_operand(call, select)
    }

    /// Read the cleanup interval using the selected client's real arithmetic.
    pub fn reviewed_cleanup_interval(
        image: &[u8],
        rules: &crate::ai_placement::Rules,
        numeric: &crate::script_numeric::NumericPolicy,
    ) -> Result<f32, String> {
        Self::reviewed_cleanup_operands(image, rules)?.interval(numeric)
    }

    /// Read the interval operands from the verified image; the session's numeric
    /// policy completes the division later.
    pub fn reviewed_cleanup_operands(
        image: &[u8],
        rules: &crate::ai_placement::Rules,
    ) -> Result<CleanupOperands, String> {
        use crate::neutral_player::{RuleInputBudget, sha256};
        use crate::neutral_rule_input::Pe;

        if rules.image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || sha256(image) != rules.image_sha256
        {
            return Err("spatial cleanup image identity mismatch".into());
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
        let read = |address, size, code| -> Result<&[u8], String> {
            pe.read_from(address, code)
                .map_err(|e| e.to_string())?
                .get(..size)
                .ok_or_else(|| format!("truncated spatial cleanup source at {address:#x}"))
        };
        let initializer = read(0x411909, 0x25, true)?;
        let loaded = read(0x8c149c, 13, true)?;
        let minimum_check = read(0x8c14b6, 7, true)?;
        let (dividend, divisor, address) = cleanup_operands(initializer, loaded, minimum_check)?;
        let minimum =
            f32::from_le_bytes(read(address, 4, false)?.try_into().expect("checked width"));
        Ok(CleanupOperands {
            image_sha256: rules.image_sha256,
            dividend,
            divisor,
            minimum,
        })
    }

    /// Read the link-index sentinel from the verified client image.
    pub fn reviewed_limit(image: &[u8], rules: &crate::ai_placement::Rules) -> Result<u32, String> {
        use crate::neutral_player::{RuleInputBudget, sha256};
        use crate::neutral_rule_input::Pe;

        if rules.image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || sha256(image) != rules.image_sha256
        {
            return Err("spatial chain image identity mismatch".into());
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
        let instruction = pe
            .read_from(0x8c24b8, true)
            .map_err(|e| e.to_string())?
            .get(..6)
            .ok_or("truncated spatial chain index limit")?;
        if instruction[..2] != [0x81, 0xf9] {
            return Err("unrecognized spatial chain index limit".into());
        }
        let limit = u32::from_le_bytes(instruction[2..].try_into().expect("checked width"));
        if limit == 0 || limit & limit.wrapping_add(1) != 0 {
            return Err("invalid spatial chain index limit".into());
        }
        Ok(limit)
    }

    /// Read the link classes used by movement from the verified client image.
    pub fn reviewed_classes(
        image: &[u8],
        rules: &crate::ai_placement::Rules,
        entries: EntryRules,
    ) -> Result<(u32, u32), String> {
        use crate::neutral_player::{RuleInputBudget, sha256};
        use crate::neutral_rule_input::Pe;
        if rules.image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || sha256(image) != rules.image_sha256
        {
            return Err("spatial chain image identity mismatch".into());
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
        let removed = pe
            .read_from(0x8c3170, true)
            .map_err(|e| e.to_string())?
            .get(..2)
            .ok_or("truncated removal link class")?;
        let inserted = pe
            .read_from(0x8c3225, true)
            .map_err(|e| e.to_string())?
            .get(..5)
            .ok_or("truncated object link class")?;
        let dirty = pe
            .read_from(0x8c2488, true)
            .map_err(|e| e.to_string())?
            .get(..4)
            .ok_or("truncated dirty-link test")?;
        if removed[0] != 0x6a || inserted[0] != 0x68 || dirty != [0x85, 0xdb, 0x75, 0x1a] {
            return Err("unrecognized spatial link classes".into());
        }
        let removal_class = i32::from(removed[1] as i8) as u32;
        let object_class = u32::from_le_bytes(inserted[1..].try_into().expect("checked width"));
        if removal_class != 0 || object_class & entries.class_mask != entries.object_class {
            return Err("inconsistent spatial link classes".into());
        }
        Ok((removal_class, object_class))
    }

    pub fn new(
        size: [usize; 2],
        capacity: usize,
        native_limit: u32,
        removal_class: u32,
    ) -> Result<Self, Error> {
        let cells = size[0]
            .checked_mul(size[1])
            .filter(|_| size[0] != 0 && size[1] != 0)
            .ok_or(Error::Dimensions)?;
        let limit = usize::try_from(native_limit).map_err(|_| Error::Dimensions)?;
        if capacity > limit {
            return Err(Error::Capacity { capacity, limit });
        }
        let mut heads = Vec::new();
        heads
            .try_reserve_exact(cells)
            .map_err(|_| Error::Allocation)?;
        heads.resize(cells, NONE);
        let mut dirty = Vec::new();
        let dirty_words = cells.div_ceil(u32::BITS as usize);
        dirty
            .try_reserve_exact(dirty_words)
            .map_err(|_| Error::Allocation)?;
        dirty.resize(dirty_words, 0);
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(capacity)
            .map_err(|_| Error::Allocation)?;
        let mut seen = Vec::new();
        seen.try_reserve_exact(capacity)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            size,
            heads,
            dirty,
            nodes,
            free: NONE,
            used: 0,
            capacity,
            removal_class,
            seen,
        })
    }

    fn validate(&self, rect: CellRect) -> Result<(), Error> {
        if (0..2).any(|i| rect.lower[i] > rect.upper[i] || rect.upper[i] > self.size[i]) {
            Err(Error::Rectangle)
        } else {
            Ok(())
        }
    }

    fn reserve_links(&self, needed: usize) -> Result<(), Error> {
        let available = self.capacity - self.used;
        if needed > available {
            Err(Error::PoolExhausted { needed, available })
        } else {
            Ok(())
        }
    }

    fn insert(&mut self, cell: usize, identity: u64, class: u32) {
        let node = Node {
            identity,
            class,
            next: self.heads[cell],
        };
        let index = if self.free == NONE {
            let index = u32::try_from(self.nodes.len()).expect("validated node capacity");
            self.nodes.push(node);
            index
        } else {
            let index = self.free;
            self.free = self.nodes[index as usize].next;
            self.nodes[index as usize] = node;
            index
        };
        self.heads[cell] = index;
        self.used += 1;
        if class == self.removal_class {
            self.dirty[cell / u32::BITS as usize] |= 1 << (cell % u32::BITS as usize);
        }
    }

    /// Insert one class of link across a region rectangle already clipped to the grid.
    pub fn region(&mut self, rect: CellRect, identity: u64, class: u32) -> Result<(), Error> {
        self.validate(rect)?;
        let count = (rect.upper[0] - rect.lower[0])
            .checked_mul(rect.upper[1] - rect.lower[1])
            .ok_or(Error::Dimensions)?;
        self.reserve_links(count)?;
        for y in rect.lower[1]..rect.upper[1] {
            for x in rect.lower[0]..rect.upper[0] {
                self.insert(y * self.size[0] + x, identity, class);
            }
        }
        Ok(())
    }

    /// Update a moving spatial entry, preserving links in the overlap.
    pub fn moved(
        &mut self,
        old: CellRect,
        new: CellRect,
        identity: u64,
        object_class: u32,
    ) -> Result<(), Error> {
        self.validate(old)?;
        self.validate(new)?;
        let changed = |a: CellRect, b: CellRect| {
            (a.lower[1]..a.upper[1]).flat_map(move |y| {
                (a.lower[0]..a.upper[0]).filter_map(move |x| {
                    (x < b.lower[0] || x >= b.upper[0] || y < b.lower[1] || y >= b.upper[1])
                        .then_some((x, y))
                })
            })
        };
        let needed = changed(old, new)
            .count()
            .checked_add(changed(new, old).count())
            .ok_or(Error::Dimensions)?;
        self.reserve_links(needed)?;
        for (from, to, class) in [(old, new, self.removal_class), (new, old, object_class)] {
            for (x, y) in changed(from, to) {
                self.insert(y * self.size[0] + x, identity, class);
            }
        }
        Ok(())
    }

    /// The cell's links, newest first, without applying any query rule.
    pub fn links(
        &self,
        cell: [usize; 2],
    ) -> Result<impl Iterator<Item = ChainLink> + '_, Error> {
        if cell[0] >= self.size[0] || cell[1] >= self.size[1] {
            return Err(Error::Rectangle);
        }
        let mut next = self.heads[cell[1] * self.size[0] + cell[0]];
        Ok(std::iter::from_fn(move || {
            if next == NONE {
                return None;
            }
            let node = self.nodes[next as usize];
            next = node.next;
            Some(ChainLink {
                identity: node.identity,
                class: node.class,
            })
        }))
    }

    /// Query the newest-first chain using current object state.
    pub fn blocked(
        &self,
        cell: [usize; 2],
        cell_word: u32,
        mask: u32,
        rules: EntryRules,
        mut resolve: impl FnMut(u64) -> Option<(u32, u32)>,
    ) -> Result<bool, Error> {
        if cell[0] >= self.size[0] || cell[1] >= self.size[1] {
            return Err(Error::Rectangle);
        }
        let mut next = self.heads[cell[1] * self.size[0] + cell[0]];
        let entries = std::iter::from_fn(|| {
            if next == NONE {
                return None;
            }
            let node = self.nodes[next as usize];
            next = node.next;
            let state = resolve(node.identity);
            let (flags, state_word) = state.unwrap_or_default();
            Some(ChainEntry {
                identity: node.identity,
                present: state.is_some(),
                flags,
                state: state_word,
                class: node.class,
            })
        });
        Ok(rules.cell_blocked(cell_word, mask, entries))
    }

    /// Compact dirty cells, recycling discarded links into the pool.
    pub fn cleanup(&mut self, rules: EntryRules, mut present: impl FnMut(u64) -> bool) {
        for word in 0..self.dirty.len() {
            let mut bits = std::mem::take(&mut self.dirty[word]);
            while bits != 0 {
                let cell = word * u32::BITS as usize + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                self.cleanup_cell(cell, rules, &mut present);
            }
        }
    }

    /// Deliver one due cleanup; the caller orders it with other game events.
    pub fn cleanup_due(
        &mut self,
        clock: &mut CleanupClock,
        now: f32,
        rules: EntryRules,
        present: impl FnMut(u64) -> bool,
    ) -> Result<bool, Error> {
        if clock.pop_due(now)?.is_none() {
            return Ok(false);
        }
        self.cleanup(rules, present);
        Ok(true)
    }

    fn cleanup_cell(
        &mut self,
        cell: usize,
        rules: EntryRules,
        present: &mut impl FnMut(u64) -> bool,
    ) {
        self.seen.clear();
        let mut previous = NONE;
        let mut current = self.heads[cell];
        while current != NONE {
            let node = self.nodes[current as usize];
            let class = node.class & rules.class_mask;
            let known = class != rules.skipped_class
                && present(node.identity)
                && !self.seen.contains(&node.identity);
            if known {
                self.seen.push(node.identity);
            }
            if known && class == rules.object_class {
                previous = current;
            } else {
                if previous == NONE {
                    self.heads[cell] = node.next;
                } else {
                    self.nodes[previous as usize].next = node.next;
                }
                self.nodes[current as usize].next = self.free;
                self.free = current;
                self.used -= 1;
            }
            current = node.next;
        }
    }
}

#[derive(Clone, Copy)]
struct LiveEntry {
    identity: u64,
    flags: u32,
    state: u32,
    exclusions: u32,
    movement: Option<CellRect>,
}

/// Ordered cell links and the current state shared by all links to an entry.
pub struct LiveChains {
    chains: Chains,
    entries: Vec<LiveEntry>,
    clock: CleanupClock,
    rules: EntryRules,
    sequence: u64,
    entry_capacity: usize,
}

impl LiveChains {
    /// Allocate the pool and entry table before recording any registrations.
    pub fn new(
        chains: Chains,
        clock: CleanupClock,
        rules: EntryRules,
        entry_capacity: usize,
    ) -> Result<Self, Error> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(entry_capacity)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            chains,
            entries,
            clock,
            rules,
            sequence: 0,
            entry_capacity,
        })
    }

    fn entry(&self, identity: u64) -> Result<usize, Error> {
        self.entries
            .binary_search_by_key(&identity, |entry| entry.identity)
            .map_err(|_| Error::MissingEntry(identity))
    }

    fn next_sequence(&self) -> Result<u64, Error> {
        self.sequence.checked_add(1).ok_or(Error::Sequence)
    }

    /// Register an entry and its first rectangle in the simulation's call order.
    pub fn register(
        &mut self,
        identity: u64,
        flags: u32,
        state: u32,
        rectangle: CellRect,
        class: u32,
        moving: bool,
    ) -> Result<u64, Error> {
        let sequence = self.next_sequence()?;
        let index = self
            .entries
            .binary_search_by_key(&identity, |entry| entry.identity)
            .map_or_else(Ok, |_| Err(Error::DuplicateEntry(identity)))?;
        if self.entries.len() == self.entry_capacity {
            return Err(Error::EntryCapacity(self.entry_capacity));
        }
        self.chains.region(rectangle, identity, class)?;
        self.entries.insert(
            index,
            LiveEntry {
                identity,
                flags,
                state,
                exclusions: 0,
                movement: moving.then_some(rectangle),
            },
        );
        self.sequence = sequence;
        Ok(sequence)
    }

    /// Add another rectangle for an existing entry, such as a region addition.
    pub fn add_rectangle(
        &mut self,
        identity: u64,
        rectangle: CellRect,
        class: u32,
    ) -> Result<u64, Error> {
        self.entry(identity)?;
        let sequence = self.next_sequence()?;
        self.chains.region(rectangle, identity, class)?;
        self.sequence = sequence;
        Ok(sequence)
    }

    /// Record the removal link for one rectangle without retiring its entry.
    pub fn remove_rectangle(&mut self, identity: u64, rectangle: CellRect) -> Result<u64, Error> {
        self.entry(identity)?;
        let sequence = self.next_sequence()?;
        self.chains
            .region(rectangle, identity, self.chains.removal_class)?;
        self.sequence = sequence;
        Ok(sequence)
    }

    /// Move one registered entry while preserving the links in its overlap.
    pub fn move_entry(
        &mut self,
        identity: u64,
        rectangle: CellRect,
        class: u32,
    ) -> Result<u64, Error> {
        let index = self.entry(identity)?;
        let old = self.entries[index]
            .movement
            .ok_or(Error::MissingMovement(identity))?;
        let sequence = self.next_sequence()?;
        self.chains.moved(old, rectangle, identity, class)?;
        self.entries[index].movement = Some(rectangle);
        self.sequence = sequence;
        Ok(sequence)
    }

    /// Temporarily increment or restore the entry state without changing links.
    pub fn exclusion(&mut self, identity: u64, exclude: bool) -> Result<(), Error> {
        let index = self.entry(identity)?;
        let entry = &mut self.entries[index];
        if exclude {
            entry.exclusions = entry
                .exclusions
                .checked_add(1)
                .ok_or(Error::Exclusion(identity))?;
            entry.state = entry.state.wrapping_add(1);
        } else {
            entry.exclusions = entry
                .exclusions
                .checked_sub(1)
                .ok_or(Error::Exclusion(identity))?;
            entry.state = entry.state.wrapping_sub(1);
        }
        Ok(())
    }

    /// Update the shared state when the represented object changes.
    pub fn update_state(&mut self, identity: u64, flags: u32, state: u32) -> Result<(), Error> {
        let index = self.entry(identity)?;
        let entry = &mut self.entries[index];
        if entry.exclusions != 0 {
            return Err(Error::Exclusion(identity));
        }
        entry.flags = flags;
        entry.state = state;
        Ok(())
    }

    /// Apply the walking contribution without changing links or exclusion counts.
    pub fn update_walking(
        &mut self,
        identity: u64,
        rule: MovementStateRule,
        velocity: [f32; 2],
        limit: f32,
        other_contributor: Option<&'static str>,
    ) -> Result<(), Error> {
        let index = self.entry(identity)?;
        if self.entries[index].movement.is_none() {
            return Err(Error::MissingMovement(identity));
        }
        if let Some(contributor) = other_contributor {
            return Err(Error::MovementContributor {
                identity,
                contributor,
            });
        }
        let state = rule.apply(self.entries[index].state, velocity, limit)?;
        self.entries[index].state = state;
        Ok(())
    }

    /// Retire a movement entry, inserting its removal links before removing its state.
    pub fn retire_movement(&mut self, identity: u64) -> Result<u64, Error> {
        let index = self.entry(identity)?;
        let entry = self.entries[index];
        if entry.exclusions != 0 {
            return Err(Error::Exclusion(identity));
        }
        let old = entry.movement.ok_or(Error::MissingMovement(identity))?;
        let sequence = self.next_sequence()?;
        self.chains
            .region(old, identity, self.chains.removal_class)?;
        self.entries.remove(index);
        self.sequence = sequence;
        Ok(sequence)
    }

    /// The cell's links, newest first.
    pub fn chain(&self, cell: [usize; 2]) -> Result<impl Iterator<Item = ChainLink> + '_, Error> {
        self.chains.links(cell)
    }

    /// Current flags and state of a registered entry.
    pub fn entry_state(&self, identity: u64) -> Option<(u32, u32)> {
        let index = self.entry(identity).ok()?;
        let entry = self.entries[index];
        Some((entry.flags, entry.state))
    }

    /// The registration sequence number of the latest operation.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Read the current entry state through the ordered chain.
    pub fn blocked(&self, cell: [usize; 2], word: u32, mask: u32) -> Result<bool, Error> {
        self.chains
            .blocked(cell, word, mask, self.rules, |identity| {
                self.entry(identity).ok().map(|index| {
                    let entry = self.entries[index];
                    (entry.flags, entry.state)
                })
            })
    }

    /// Earliest cleanup deadline to interleave with the game scheduler.
    pub fn next_cleanup(&self) -> f32 {
        self.clock.next()
    }

    /// Apply one due cleanup so the scheduler can interleave other events.
    pub fn cleanup_due(&mut self, now: f32) -> Result<bool, Error> {
        if self.clock.pop_due(now)?.is_none() {
            return Ok(false);
        }
        let entries = &self.entries;
        self.chains.cleanup(self.rules, |identity| {
            entries
                .binary_search_by_key(&identity, |entry| entry.identity)
                .is_ok()
        });
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(limit: u8) -> EntryRules {
        EntryRules {
            active: 0x100,
            registered: 0x200,
            forbidden: 0x40,
            class_mask: 0xf000,
            object_class: 0x3000,
            skipped_class: 0x4000,
            pathing_mask: 0xff,
            region_flags: 0,
            region_state: 0x200,
            max_entries: limit,
        }
    }

    fn rect(left: usize, right: usize) -> CellRect {
        CellRect {
            lower: [left, 0],
            upper: [right, 1],
        }
    }

    #[test]
    fn live_entries_follow_registration_order_and_temporary_exclusions() {
        let mut rules = rules(7);
        rules.forbidden = 0xf;
        let chains = Chains::new([3, 1], 8, 15, 0).unwrap();
        let clock = CleanupClock::new(0.0, 0.25).unwrap();
        let mut live = LiveChains::new(chains, clock, rules, 2).unwrap();
        assert_eq!(
            live.register(9, 0x104, 0x200, rect(0, 2), rules.object_class, true),
            Ok(1)
        );
        assert!(live.blocked([0, 0], 0, 4).unwrap());
        live.exclusion(9, true).unwrap();
        live.exclusion(9, true).unwrap();
        assert!(!live.blocked([0, 0], 0, 4).unwrap());
        assert_eq!(live.retire_movement(9), Err(Error::Exclusion(9)));
        live.exclusion(9, false).unwrap();
        assert!(!live.blocked([0, 0], 0, 4).unwrap());
        live.exclusion(9, false).unwrap();
        assert!(live.blocked([0, 0], 0, 4).unwrap());
        assert_eq!(live.exclusion(9, false), Err(Error::Exclusion(9)));
        assert_eq!(live.move_entry(9, rect(1, 3), rules.object_class), Ok(2));
        assert!(!live.blocked([0, 0], 0, 4).unwrap());
        assert!(live.blocked([1, 0], 0, 4).unwrap());
        assert!(live.blocked([2, 0], 0, 4).unwrap());
        assert_eq!(live.cleanup_due(0.125), Ok(false));
        assert_eq!(live.cleanup_due(0.25), Ok(true));
        assert_eq!(live.cleanup_due(0.25), Ok(false));
        assert_eq!(live.retire_movement(9), Ok(3));
        assert!(!live.blocked([2, 0], 0, 4).unwrap());
        assert_eq!(live.cleanup_due(0.5), Ok(true));
    }

    #[test]
    fn walking_state_uses_threshold_and_preserves_exclusions() {
        let mut rules = rules(7);
        rules.forbidden = 0x4f;
        let chains = Chains::new([1, 1], 2, 4, 0).unwrap();
        let clock = CleanupClock::new(0.0, 0.25).unwrap();
        let mut live = LiveChains::new(chains, clock, rules, 1).unwrap();
        let movement = MovementStateRule {
            threshold: 4.0,
            moving_bit: 0x40,
        };
        live.register(1, 0x104, 0x200, rect(0, 1), rules.object_class, true)
            .unwrap();
        assert!(live.blocked([0, 0], 0, 4).unwrap());
        live.update_walking(1, movement, [1.0, -1.0], 1.0, None)
            .unwrap();
        assert!(live.blocked([0, 0], 0, 4).unwrap());
        live.update_walking(1, movement, [2.0, 0.0], 1.0, None)
            .unwrap();
        assert!(!live.blocked([0, 0], 0, 4).unwrap());
        live.exclusion(1, true).unwrap();
        live.update_walking(1, movement, [0.0, 0.0], 1.0, None)
            .unwrap();
        assert!(!live.blocked([0, 0], 0, 4).unwrap());
        live.exclusion(1, false).unwrap();
        assert!(live.blocked([0, 0], 0, 4).unwrap());
        live.update_walking(1, movement, [4.0, 0.0], 0.0, None)
            .unwrap();
        assert!(live.blocked([0, 0], 0, 4).unwrap());
        assert_eq!(
            live.update_walking(1, movement, [4.0, 0.0], 1.0, Some("knockback")),
            Err(Error::MovementContributor {
                identity: 1,
                contributor: "knockback"
            })
        );
        assert_eq!(
            live.update_walking(1, movement, [f32::NAN, 0.0], 1.0, None),
            Err(Error::MovementState)
        );
        assert!(live.blocked([0, 0], 0, 4).unwrap());
    }

    #[test]
    #[ignore = "requires the selected local Warcraft III executable in PCX_CAMPAIGN_IMAGE"]
    fn selected_movement_state_rule_reads_the_client() {
        let path = std::env::var("PCX_CAMPAIGN_IMAGE").unwrap();
        let image = std::fs::read(&path).unwrap();
        let identity = crate::campaign_inputs::client_identity(&image, &path);
        let placement = crate::ai_placement::Rules::reviewed_legacy(&image, identity).unwrap();
        let rule = MovementStateRule::reviewed_legacy(&image, &placement).unwrap();
        assert_eq!(rule.apply(rule.moving_bit, [0.0, 0.0], 1.0), Ok(0));
        assert_eq!(rule.apply(0, [1.0, 0.0], 0.0), Ok(0));
    }

    #[test]
    fn live_regions_and_capacity_failures_leave_the_order_intact() {
        let rules = rules(7);
        let chains = Chains::new([3, 1], 3, 7, 0).unwrap();
        let clock = CleanupClock::new(0.0, 0.5).unwrap();
        let mut live = LiveChains::new(chains, clock, rules, 1).unwrap();
        assert_eq!(
            live.register(4, 0x104, 0x200, rect(0, 1), rules.object_class, false),
            Ok(1)
        );
        assert_eq!(live.add_rectangle(4, rect(1, 2), rules.object_class), Ok(2));
        assert_eq!(
            live.register(5, 0x104, 0x200, rect(2, 3), rules.object_class, false),
            Err(Error::EntryCapacity(1))
        );
        assert_eq!(live.remove_rectangle(4, rect(0, 1)), Ok(3));
        assert!(!live.blocked([0, 0], 0, 4).unwrap());
        assert!(live.blocked([1, 0], 0, 4).unwrap());
        assert_eq!(
            live.add_rectangle(4, rect(2, 3), rules.object_class),
            Err(Error::PoolExhausted {
                needed: 1,
                available: 0
            })
        );
        assert_eq!(live.cleanup_due(0.5), Ok(true));
        assert_eq!(live.add_rectangle(4, rect(2, 3), rules.object_class), Ok(4));
    }

    #[test]
    fn region_link_class_requires_the_expected_call_and_selection() {
        let mut call = [0x6a, 1, 0x50, 0xe8, 0, 0, 0, 0];
        call[4..].copy_from_slice(&(0x801a50_i32 - 0x4a4f1e_i32).to_le_bytes());
        let mut select = [0x8b, 0x45, 0x0c, 0xf7, 0xd8, 0x1b, 0xc0, 0x25, 0, 0, 0, 0];
        select[8..].copy_from_slice(&0x2500_0000_u32.to_le_bytes());
        assert_eq!(region_class_operand(&call, &select).unwrap(), 0x2500_0000);
        call[4] ^= 1;
        assert!(region_class_operand(&call, &select).is_err());
        assert!(region_class_operand(&[], &select).is_err());
    }

    #[test]
    fn region_cells_transpose_scale_and_clip_the_source_rectangle() {
        assert_eq!(
            region_cells([92., 208., 116., 224.], [100., 200.], 8., [8, 8]),
            Ok(CellRect {
                lower: [1, 0],
                upper: [4, 3]
            })
        );
        assert_eq!(
            region_cells([60., 180., 220., 300.], [100., 200.], 8., [8, 8]),
            Ok(CellRect {
                lower: [0, 0],
                upper: [8, 8]
            })
        );
        assert_eq!(
            region_cells([0., 0., 1., 1.], [0., 0.], 3., [8, 8]),
            Err(Error::Coordinates)
        );
        assert_eq!(
            region_cells([1., 0., 0., 1.], [0., 0.], 8., [8, 8]),
            Err(Error::Coordinates)
        );
    }

    #[test]
    fn cleanup_source_operands_must_share_the_same_globals() {
        let mut init = [0_u8; 32];
        init[0] = 0x6a;
        init[1] = 7;
        init[15] = 0x6a;
        init[16] = 3;
        init[27] = 0x68;
        init[28..32].copy_from_slice(&25_u32.to_le_bytes());
        let mut loaded = [0_u8; 13];
        loaded[0] = 0xa1;
        loaded[1..5].copy_from_slice(&25_u32.to_le_bytes());
        loaded[5..9].copy_from_slice(&[0xf3, 0x0f, 0x10, 0x05]);
        loaded[9..13].copy_from_slice(&99_u32.to_le_bytes());
        let mut check = [0_u8; 7];
        check[..3].copy_from_slice(&[0x0f, 0x47, 0x05]);
        check[3..].copy_from_slice(&99_u32.to_le_bytes());
        assert_eq!(
            cleanup_operands(&init, &loaded, &check).unwrap(),
            (3, 7, 99)
        );
        check[3] ^= 1;
        assert!(cleanup_operands(&init, &loaded, &check).is_err());
        assert!(cleanup_operands(&[], &loaded, &check).is_err());
    }

    #[test]
    fn cleanup_clock_delivers_each_due_deadline_and_rejects_stalled_time() {
        let mut clock = CleanupClock::new(2.0, 0.375).unwrap();
        assert_eq!(clock.pop_due(2.25), Ok(None));
        assert_eq!(clock.pop_due(3.125), Ok(Some(2.375)));
        assert_eq!(clock.pop_due(3.125), Ok(Some(2.75)));
        assert_eq!(clock.pop_due(3.125), Ok(Some(3.125)));
        assert_eq!(clock.pop_due(3.125), Ok(None));
        assert_eq!(clock.pop_due(f32::NAN), Err(Error::Clock));
        assert!(CleanupClock::new(f32::MAX, 0.375).is_err());
    }

    #[test]
    fn cleanup_at_deadline_recycles_removal_links() {
        let rules = rules(7);
        let mut chains = Chains::new([2, 1], 2, 7, 0).unwrap();
        let mut clock = CleanupClock::new(0.0, 0.25).unwrap();
        chains.region(rect(0, 1), 1, rules.object_class).unwrap();
        chains
            .moved(rect(0, 1), rect(0, 0), 1, rules.object_class)
            .unwrap();
        assert!(
            !chains
                .cleanup_due(&mut clock, 0.125, rules, |_| true)
                .unwrap()
        );
        assert!(chains.region(rect(1, 2), 2, rules.object_class).is_err());
        assert!(
            chains
                .cleanup_due(&mut clock, 0.25, rules, |_| true)
                .unwrap()
        );
        assert!(
            !chains
                .cleanup_due(&mut clock, 0.25, rules, |_| true)
                .unwrap()
        );
        chains.region(rect(1, 2), 2, rules.object_class).unwrap();
        assert!(
            chains
                .blocked([1, 0], 0, 4, rules, |_| Some((0x104, 0x200)))
                .unwrap()
        );
    }

    #[test]
    fn removal_suppresses_old_link_and_cleanup_reuses_pool() {
        let rules = rules(7);
        let mut chains = Chains::new([3, 1], 4, 7, 0).unwrap();
        chains.region(rect(0, 2), 1, rules.object_class).unwrap();
        chains
            .moved(rect(0, 2), rect(1, 3), 1, rules.object_class)
            .unwrap();
        let state = |_| Some((0x104, 0x200));
        assert!(!chains.blocked([0, 0], 0, 4, rules, state).unwrap());
        assert!(chains.blocked([1, 0], 0, 4, rules, state).unwrap());
        assert!(chains.blocked([2, 0], 0, 4, rules, state).unwrap());
        assert_eq!(
            chains.region(rect(0, 1), 2, rules.object_class),
            Err(Error::PoolExhausted {
                needed: 1,
                available: 0
            })
        );
        chains.cleanup(rules, |_| true);
        chains.region(rect(0, 1), 2, rules.object_class).unwrap();
        assert!(chains.blocked([0, 0], 0, 4, rules, state).unwrap());
    }

    #[test]
    fn insertion_only_survives_cleanup_and_visit_limit_counts_duplicates() {
        let rules = rules(2);
        let mut chains = Chains::new([1, 1], 8, 15, 0).unwrap();
        chains.region(rect(0, 1), 1, rules.object_class).unwrap();
        chains.region(rect(0, 1), 2, rules.skipped_class).unwrap();
        chains.region(rect(0, 1), 2, rules.skipped_class).unwrap();
        let state = |_| Some((0x104, 0x200));
        assert!(!chains.blocked([0, 0], 0, 4, rules, state).unwrap());
        chains.cleanup(rules, |_| true);
        assert!(!chains.blocked([0, 0], 0, 4, rules, state).unwrap());
        chains.region(rect(0, 1), 2, 0).unwrap();
        chains.cleanup(rules, |_| true);
        assert!(chains.blocked([0, 0], 0, 4, rules, state).unwrap());
    }

    #[test]
    fn exhaustion_and_invalid_rectangles_leave_chains_unchanged() {
        let rules = rules(3);
        let mut chains = Chains::new([3, 1], 2, 3, 0).unwrap();
        chains.region(rect(0, 1), 1, rules.object_class).unwrap();
        assert_eq!(
            chains.moved(rect(0, 1), rect(1, 2), 1, rules.object_class),
            Err(Error::PoolExhausted {
                needed: 2,
                available: 1
            })
        );
        assert_eq!(chains.region(rect(2, 4), 2, 0), Err(Error::Rectangle));
        assert!(
            chains
                .blocked([0, 0], 0, 4, rules, |_| Some((0x104, 0x200)))
                .unwrap()
        );
        assert!(
            !chains
                .blocked([1, 0], 0, 4, rules, |_| Some((0x104, 0x200)))
                .unwrap()
        );
    }

    #[test]
    fn cleanup_handles_dirty_cells_across_bitmap_words() {
        let rules = rules(4);
        let boundary = u32::BITS as usize;
        let mut chains = Chains::new([boundary + 1, 1], 4, 7, 0).unwrap();
        for x in [boundary - 1, boundary] {
            chains
                .region(rect(x, x + 1), 1, rules.object_class)
                .unwrap();
            chains.region(rect(x, x + 1), 1, 0).unwrap();
        }
        chains.cleanup(rules, |_| true);
        assert_eq!(chains.used, 0);
        chains
            .region(rect(boundary, boundary + 1), 2, rules.object_class)
            .unwrap();
        assert!(
            chains
                .blocked([boundary, 0], 0, 4, rules, |_| Some((0x104, 0x200)))
                .unwrap()
        );
    }
}
