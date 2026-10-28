//! Cell aggregation and lookup for the selected client's reach hierarchy.
//!
//! Callers must supply source-qualified occupancy classes and search inputs.

#[path = "native_spatial_chains.rs"]
pub mod spatial_chains;

/// Entry tests for a cell in the selected client's full-mask reach query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryRules {
    active: u32,
    registered: u32,
    forbidden: u32,
    class_mask: u32,
    object_class: u32,
    skipped_class: u32,
    pathing_mask: u32,
    max_entries: u8,
}

/// One spatial-chain link, in newest-first order. Entry fields must reflect the
/// current object state for every link with the same identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainEntry {
    pub identity: u64,
    pub present: bool,
    pub flags: u32,
    pub state: u32,
    pub class: u32,
}

/// A set-based reach query cannot prove that it sees all native links.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChainBoundError {
    #[error("reach cell {cell:?}: chain bound {bound} reaches visit limit {limit}")]
    Limit {
        cell: [i32; 2],
        bound: usize,
        limit: usize,
    },
    #[error("reach cell {cell:?}: chain bound {bound} is smaller than {entries} live entries")]
    Inconsistent {
        cell: [i32; 2],
        bound: usize,
        entries: usize,
    },
    #[error("reach cell {cell:?}: live entry set is not canonical")]
    NonCanonical { cell: [i32; 2] },
    #[error("reach cell {cell:?}: native link order is not established")]
    UnprovenOrder { cell: [i32; 2] },
}

/// A half-open rectangle in the spatial grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRect {
    pub lower: [usize; 2],
    pub upper: [usize; 2],
}

/// Per-cell upper bounds on links retained since the last known cleanup.
pub struct CellCrossings {
    size: [usize; 2],
    links: Vec<u16>,
}

impl CellCrossings {
    const DIRTY: u16 = 1 << 15;
    const COUNT: u16 = Self::DIRTY - 1;

    /// Allocate the grid once, before any recorded changes.
    pub fn new(size: [usize; 2]) -> Result<Self, String> {
        let count = size[0]
            .checked_mul(size[1])
            .filter(|_| size[0] != 0 && size[1] != 0)
            .ok_or("invalid reach history grid dimensions")?;
        let mut links = Vec::new();
        links.try_reserve_exact(count).map_err(|e| e.to_string())?;
        links.resize(count, 0);
        Ok(Self { size, links })
    }

    fn validate(&self, rect: CellRect) -> Result<(), &'static str> {
        if (0..2)
            .any(|axis| rect.lower[axis] > rect.upper[axis] || rect.upper[axis] > self.size[axis])
        {
            Err("reach history rectangle outside grid")
        } else {
            Ok(())
        }
    }

    /// Record only the cells entering or leaving a changed rectangle.
    pub fn moved(&mut self, old: CellRect, new: CellRect) -> Result<(), &'static str> {
        self.validate(old)?;
        self.validate(new)?;
        if old == new {
            return Ok(());
        }
        for (from, to, increment, dirty) in [(old, new, 2, true), (new, old, 1, false)] {
            for y in from.lower[1]..from.upper[1] {
                for x in from.lower[0]..from.upper[0] {
                    if x < to.lower[0] || x >= to.upper[0] || y < to.lower[1] || y >= to.upper[1] {
                        let counter = &mut self.links[y * self.size[0] + x];
                        Self::record(counter, increment, dirty);
                    }
                }
            }
        }
        Ok(())
    }

    fn record(counter: &mut u16, increment: u16, dirty: bool) {
        let count = (*counter & Self::COUNT)
            .saturating_add(increment)
            .min(Self::COUNT);
        *counter = count | (*counter & Self::DIRTY) | if dirty { Self::DIRTY } else { 0 };
    }

    /// Bound links added by a region registration or removal.
    pub fn region_change(&mut self, rect: CellRect, removed: bool) -> Result<(), &'static str> {
        self.validate(rect)?;
        for y in rect.lower[1]..rect.upper[1] {
            for x in rect.lower[0]..rect.upper[0] {
                let counter = &mut self.links[y * self.size[0] + x];
                Self::record(counter, if removed { 2 } else { 1 }, removed);
            }
        }
        Ok(())
    }

    /// Add the live-set count to the retained-link upper bound.
    pub fn bound(&self, cell: [usize; 2], live: usize) -> Option<usize> {
        if cell[0] >= self.size[0] || cell[1] >= self.size[1] {
            return None;
        }
        Some(live.saturating_add(usize::from(
            self.links[cell[1] * self.size[0] + cell[0]] & Self::COUNT,
        )))
    }

    /// Native cleanup compacts only cells marked dirty by a removal.
    pub fn cleaned(&mut self) {
        for counter in &mut self.links {
            if *counter & Self::DIRTY != 0 {
                *counter = 0;
            }
        }
    }
}

impl EntryRules {
    /// Decode the tests from the same verified image used by placement.
    pub fn reviewed_legacy(
        image: &[u8],
        rules: &crate::ai_placement::Rules,
    ) -> Result<Self, String> {
        use crate::neutral_player::{RuleInputBudget, sha256};
        use crate::neutral_rule_input::Pe;

        if rules.image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || sha256(image) != rules.image_sha256
        {
            return Err("reach image identity mismatch".into());
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
        let read = |address, count| -> Result<&[u8], String> {
            pe.read_from(address, true)
                .map_err(|e| e.to_string())?
                .get(..count)
                .ok_or_else(|| format!("truncated reach entry test at {address:#x}"))
        };
        let class = read(0x8be4ac, 6)?;
        let skipped = read(0x8be4b8, 6)?;
        let limit = read(0x8be499, 3)?;
        let active = read(0x8be4ce, 5)?;
        let registered = read(0x8be4d8, 5)?;
        let object = read(0x8be4ea, 6)?;
        let forbidden = read(0x8be4f2, 7)?;
        let pathing = read(0x8be504, 6)?;
        let cleanup_skipped = read(0x8c2c79, 6)?;
        let cleanup_object = read(0x8c2c89, 6)?;
        if class[..2] != [0x81, 0xe3]
            || skipped[..2] != [0x81, 0xfb]
            || limit[..2] != [0x83, 0xff]
            || active[..2] != [0xc1, 0xe8]
            || active[3..] != [0xa8, 0x01]
            || registered[..2] != [0xc1, 0xe8]
            || registered[3..] != [0xa8, 0x01]
            || object[..2] != [0x81, 0xfb]
            || forbidden[..3] != [0xf7, 0x42, 0x44]
            || pathing[..2] != [0xf7, 0xc1]
            || cleanup_skipped[..2] != [0x81, 0xf9]
            || cleanup_skipped[2..] != skipped[2..]
            || cleanup_object[..2] != [0x81, 0xf9]
            || cleanup_object[2..] != object[2..]
        {
            return Err("unrecognized reach entry test instructions".into());
        }
        let word = |bytes: &[u8]| u32::from_le_bytes(bytes.try_into().expect("checked width"));
        Ok(Self {
            active: 1_u32
                .checked_shl(u32::from(active[2]))
                .ok_or("active bit")?,
            registered: 1_u32
                .checked_shl(u32::from(registered[2]))
                .ok_or("registration bit")?,
            forbidden: word(&forbidden[3..]),
            class_mask: word(&class[2..]),
            object_class: word(&object[2..]),
            skipped_class: word(&skipped[2..]),
            pathing_mask: word(&pathing[2..]),
            max_entries: limit[2].checked_sub(1).ok_or("reach chain limit")?,
        })
    }

    /// Test one live, not-yet-visited entry in its cell chain.
    /// The caller still applies the native chain traversal limit and static cell mask.
    pub fn blocks(
        self,
        present: bool,
        entry_flags: u32,
        entry_state: u32,
        chain_class: u32,
        selected_mask: u32,
    ) -> bool {
        present
            && entry_flags & self.active != 0
            && entry_state & self.registered != 0
            && chain_class & self.class_mask == self.object_class
            && entry_state & self.forbidden == 0
            && entry_flags & selected_mask & self.pathing_mask != 0
    }

    /// Apply the full-mask query to an in-bounds cell's newest-first chain.
    pub fn cell_blocked(
        self,
        cell_word: u32,
        selected_mask: u32,
        entries: impl IntoIterator<Item = ChainEntry>,
    ) -> bool {
        if cell_word & selected_mask & self.class_mask != 0 {
            return true;
        }
        let mut visited = [0_u64; u8::MAX as usize];
        let mut visited_count = 0;
        for entry in entries.into_iter().take(usize::from(self.max_entries)) {
            if entry.class & self.class_mask == self.skipped_class
                || !entry.present
                || entry.flags & self.active == 0
                || entry.state & self.registered == 0
                || visited[..visited_count].contains(&entry.identity)
            {
                continue;
            }
            visited[visited_count] = entry.identity;
            visited_count += 1;
            if self.blocks(true, entry.flags, entry.state, entry.class, selected_mask) {
                return true;
            }
        }
        false
    }

    /// Evaluate the current object set when `bound` covers every native link
    /// in the cell, including stale, duplicate, and non-object links, and the
    /// caller proves no newer non-object link suppresses a live object link.
    pub fn bounded_cell_blocked(
        self,
        cell: [i32; 2],
        cell_word: u32,
        selected_mask: u32,
        entries: &[ChainEntry],
        bound: usize,
        order_proven: bool,
    ) -> Result<bool, ChainBoundError> {
        if cell_word & selected_mask & self.class_mask != 0 {
            return Ok(true);
        }
        let limit = usize::from(self.max_entries);
        if bound >= limit {
            return Err(ChainBoundError::Limit { cell, bound, limit });
        }
        if !order_proven {
            return Err(ChainBoundError::UnprovenOrder { cell });
        }
        if entries.len() > bound {
            return Err(ChainBoundError::Inconsistent {
                cell,
                bound,
                entries: entries.len(),
            });
        }
        for (index, entry) in entries.iter().enumerate() {
            if !entry.present
                || entry.class & self.class_mask != self.object_class
                || entries[..index]
                    .iter()
                    .any(|other| other.identity == entry.identity)
            {
                return Err(ChainBoundError::NonCanonical { cell });
            }
        }
        Ok(entries.iter().any(|entry| {
            self.blocks(
                entry.present,
                entry.flags,
                entry.state,
                entry.class,
                selected_mask,
            )
        }))
    }

    /// Compact one newest-first chain when the spatial grid runs its cleanup.
    /// `seen` is reusable caller-owned scratch and is empty on return.
    pub fn compact_chain(
        self,
        entries: &mut Vec<ChainEntry>,
        seen: &mut Vec<u64>,
    ) -> Result<(), std::collections::TryReserveError> {
        seen.clear();
        seen.try_reserve(entries.len())?;
        entries.retain(|entry| {
            let class = entry.class & self.class_mask;
            if class == self.skipped_class || !entry.present || seen.contains(&entry.identity) {
                return false;
            }
            seen.push(entry.identity);
            class == self.object_class
        });
        seen.clear();
        Ok(())
    }
}

/// Reach operands from the same verified image used by the placement diagnostic.
#[cfg(feature = "test-fixtures")]
pub fn diagnostic_operands(
    image: &[u8],
    rules: &crate::ai_placement::Rules,
) -> Result<(u8, u32, u32, usize, i32, u32), String> {
    use crate::neutral_player::{RuleInputBudget, sha256};
    use crate::neutral_rule_input::Pe;

    if rules.image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
        || sha256(image) != rules.image_sha256
    {
        return Err("reach image identity mismatch".into());
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
    let read = |address, count, code| -> Result<&[u8], String> {
        pe.read_from(address, code)
            .map_err(|e| e.to_string())?
            .get(..count)
            .ok_or_else(|| format!("truncated reach operand at {address:#x}"))
    };
    let mask = u32::from_le_bytes(read(0x10e92a8, 4, false)?.try_into().unwrap());
    let work = read(0xad444c, 2, true)?;
    let width = read(0xad444e, 2, true)?;
    let scale = read(0x8a3d33, 15, true)?;
    let result = read(0x8a317b, 6, true)?;
    let levels = read(0x8b7bca, 5, true)?;
    if work[0] != 0x6a
        || width[0] != 0x6a
        || scale[..3] != [0x8d, 0x34, 0x40]
        || scale[12..14] != [0xc1, 0xe6]
        || result != [0x2b, 0xfa, 0x03, 0xff, 0x0f, 0xaf]
        || levels[0] != 0xbb
        || mask >> 24 != mask & 0xff
    {
        return Err("unrecognized reach operand instructions".into());
    }
    let priority = 3_u32
        .checked_shl(u32::from(scale[14]))
        .ok_or("reach scale")?;
    let movement = 1_i32
        .checked_shl(u32::from(width[1]))
        .ok_or("reach width")?;
    let parents = u32::from_le_bytes(levels[1..].try_into().unwrap());
    Ok((
        mask as u8,
        priority,
        1 + 1,
        usize::from(work[1]),
        movement,
        parents + 1,
    ))
}

/// The classification of a cell for one selected occupancy mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Open { link: bool },
    Blocked,
    Mixed,
}

/// Classify four underlying spatial samples; clearing leaves the auxiliary link intact.
pub fn base(samples_blocked: [bool; 4], link: bool, cleared: bool) -> Cell {
    if cleared {
        return Cell::Open { link };
    }
    match samples_blocked
        .into_iter()
        .filter(|blocked| *blocked)
        .count()
    {
        0 => Cell::Open { link },
        4 => Cell::Blocked,
        _ => Cell::Mixed,
    }
}

/// An absent child is outside the map and counts as blocked.
pub fn parent(children: [Option<Cell>; 4]) -> Cell {
    let mut blocked = 0;
    for child in children {
        match child {
            None | Some(Cell::Blocked) => blocked += 1,
            Some(Cell::Mixed | Cell::Open { link: true }) => return Cell::Mixed,
            Some(Cell::Open { link: false }) => {}
        }
    }
    match blocked {
        0 => Cell::Open { link: false },
        4 => Cell::Blocked,
        _ => Cell::Mixed,
    }
}

/// The caller must cache the first base coordinate for each resolved cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup {
    Blocked,
    Mixed,
    Open { level: u32, position: [i32; 2] },
}

/// Resolve a cell and climb through open parents. Missing parent data refuses.
pub fn lookup(
    mut level: u32,
    levels: u32,
    position: [i32; 2],
    mut cell: impl FnMut(u32, [i32; 2]) -> Option<Cell>,
) -> Option<Lookup> {
    if level >= levels || levels > i32::BITS {
        return None;
    }
    let coordinates = |level| [position[0] >> level, position[1] >> level];
    match cell(level, coordinates(level)) {
        None | Some(Cell::Blocked) => return Some(Lookup::Blocked),
        Some(Cell::Mixed) if level == 0 => return Some(Lookup::Blocked),
        Some(Cell::Mixed) => return Some(Lookup::Mixed),
        Some(Cell::Open { .. }) => {}
    }
    while level + 1 < levels {
        match cell(level + 1, coordinates(level + 1)) {
            None => return None,
            Some(Cell::Blocked | Cell::Mixed) => break,
            Some(Cell::Open { .. }) => level += 1,
        }
    }
    Some(Lookup::Open { level, position })
}

/// Resolve one corner, descending through mixed cells at the same coordinate.
pub fn corner_lookup(
    mut level: u32,
    levels: u32,
    position: [i32; 2],
    mut cell: impl FnMut(u32, [i32; 2]) -> Option<Cell>,
) -> Option<Lookup> {
    loop {
        match lookup(level, levels, position, &mut cell)? {
            Lookup::Mixed => level = level.checked_sub(1)?,
            result => return Some(result),
        }
    }
}

/// One hierarchy cell's first base-grid coordinate during a search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub level: u32,
    pub position: [i32; 2],
}

/// A node cannot be created when the search's bound or memory is exhausted.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("invalid hierarchy coordinate or level")]
    Invalid,
    #[error("reach node limit exceeded")]
    Limit,
    #[error("reach node allocation failed: {0}")]
    Allocation(#[from] std::collections::TryReserveError),
}

/// Search-local node identities, preserving the first coordinate used for each cell.
pub struct Nodes {
    nodes: Vec<Node>,
    limit: usize,
}

impl Nodes {
    pub fn new(limit: usize) -> Self {
        Self {
            nodes: Vec::new(),
            limit,
        }
    }

    pub fn get(&self, id: u32) -> Option<Node> {
        self.nodes.get(id as usize).copied()
    }

    pub fn resolve(&mut self, node: Node) -> Result<u32, NodeError> {
        if node.level >= i32::BITS || node.position.iter().any(|value| *value < 0) {
            return Err(NodeError::Invalid);
        }
        let cell = node.position.map(|value| value >> node.level);
        if let Some(index) = self.nodes.iter().position(|existing| {
            existing.level == node.level
                && existing.position.map(|value| value >> existing.level) == cell
        }) {
            return u32::try_from(index).map_err(|_| NodeError::Limit);
        }
        if self.nodes.len() >= self.limit {
            return Err(NodeError::Limit);
        }
        let index = u32::try_from(self.nodes.len()).map_err(|_| NodeError::Limit)?;
        self.nodes.try_reserve(1)?;
        self.nodes.push(node);
        Ok(index)
    }
}

/// Integer distance with the caller's source-qualified coordinate scale.
pub fn distance(from: [i32; 2], to: [i32; 2], scale: u32) -> Option<u32> {
    let delta = |axis: usize| {
        i64::from(from[axis])
            .abs_diff(i64::from(to[axis]))
            .checked_mul(u64::from(scale))
    };
    let x = delta(0)?;
    let y = delta(1)?;
    let square = x.checked_mul(x)?.checked_add(y.checked_mul(y)?)?;
    // The native Newton iteration stops at the floor of the integer square root.
    Some(u32::try_from(square).ok()?.isqrt())
}

/// Ordered coordinates used when a mixed edge descends one level.
pub fn split_edge(level: u32, coordinate: i32, movement_width: i32) -> Option<[i32; 2]> {
    if level == 0 || level >= i32::BITS || coordinate < 0 || movement_width <= 0 {
        return None;
    }
    let child = level - 1;
    let midpoint = (i64::from(coordinate) >> level << level) + (1_i64 << child);
    if midpoint <= i64::from(coordinate) {
        let width = if child == 0 { 1 } else { movement_width };
        Some([i32::try_from(midpoint - i64::from(width)).ok()?, coordinate])
    } else {
        Some([coordinate, i32::try_from(midpoint).ok()?])
    }
}

/// Class-2 clearance on the negative-y edge, or its transposed negative-x edge.
pub fn negative_edge_clearance(
    position: [i32; 2],
    at_end: bool,
    transpose: bool,
    mut open: impl FnMut([i32; 2]) -> bool,
) -> bool {
    let [x, y] = if transpose {
        [position[1], position[0]]
    } else {
        position
    };
    let mut check = |dx: i32, dy: i32| {
        let (Some(x), Some(y)) = (x.checked_add(dx), y.checked_add(dy)) else {
            return false;
        };
        open(if transpose { [y, x] } else { [x, y] })
    };
    check(1, 0) && (!at_end || (check(1, 1) && (check(1, 2) || check(-1, 0))))
}

/// Class-2 clearance on the positive-x edge, or its transposed positive-y edge.
pub fn positive_edge_clearance(
    position: [i32; 2],
    at_end: bool,
    transpose: bool,
    mut open: impl FnMut([i32; 2]) -> bool,
) -> bool {
    let [x, y] = if transpose {
        [position[1], position[0]]
    } else {
        position
    };
    let mut check = |dx: i32, dy: i32| {
        let (Some(x), Some(y)) = (x.checked_add(dx), y.checked_add(dy)) else {
            return false;
        };
        open(if transpose { [y, x] } else { [x, y] })
    };
    check(1, 0)
        && check(0, 1)
        && check(1, 1)
        && (!at_end
            || (check(0, -1) && check(1, -1))
            || (check(-2, 1) && check(-1, 1))
            || (check(-1, 1) && check(0, -1)))
}

/// Direction of a coarse-cell edge, in the order used by the native search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    NegativeY,
    PositiveX,
    PositiveY,
    NegativeX,
}

/// A bounded edge expansion could not be completed exactly.
#[derive(Debug, thiserror::Error)]
pub enum EdgeError {
    #[error("invalid reach edge geometry")]
    Invalid,
    #[error("reach edge classification unavailable")]
    Unavailable,
    #[error("reach edge work limit exceeded")]
    Limit,
    #[error("reach edge allocation failed: {0}")]
    Allocation(#[from] std::collections::TryReserveError),
}

/// Nodes visited along an edge and whether its two ends admit a corner.
#[derive(Debug, PartialEq, Eq)]
pub struct EdgeResult {
    pub nodes: Vec<Node>,
    pub ends: [bool; 2],
}

/// Source-qualified widths and the independent diagnostic work bound for one edge.
#[derive(Clone, Copy, Debug)]
pub struct EdgeInput {
    pub level: u32,
    pub point: [i32; 2],
    pub movement_width: i32,
    pub clearance_width: i32,
    pub edge: Edge,
    pub max_visits: usize,
}

/// Expand a mixed edge; the work limit bounds this diagnostic, not native queue removals.
pub fn expand_edge(
    input: EdgeInput,
    mut resolve: impl FnMut(u32, [i32; 2]) -> Option<Lookup>,
    mut open: impl FnMut([i32; 2]) -> Option<bool>,
) -> Result<EdgeResult, EdgeError> {
    struct Walker<'a, R, O> {
        width: i32,
        clearance_width: i32,
        edge: Edge,
        remaining: usize,
        nodes: Vec<Node>,
        resolve: &'a mut R,
        open: &'a mut O,
    }

    impl<R, O> Walker<'_, R, O>
    where
        R: FnMut(u32, [i32; 2]) -> Option<Lookup>,
        O: FnMut([i32; 2]) -> Option<bool>,
    {
        fn visit(&mut self, level: u32, point: [i32; 2]) -> Result<[bool; 2], EdgeError> {
            self.remaining = self.remaining.checked_sub(1).ok_or(EdgeError::Limit)?;
            let axis = usize::from(matches!(self.edge, Edge::PositiveX | Edge::NegativeX));
            match (self.resolve)(level, point).ok_or(EdgeError::Unavailable)? {
                Lookup::Blocked => Ok([false; 2]),
                Lookup::Mixed => {
                    let positions =
                        split_edge(level, point[axis], self.width).ok_or(EdgeError::Invalid)?;
                    let mut first = point;
                    first[axis] = positions[0];
                    let mut last = point;
                    last[axis] = positions[1];
                    let first = self.visit(level - 1, first)?;
                    let last = self.visit(level - 1, last)?;
                    Ok([first[0], last[1]])
                }
                Lookup::Open {
                    level: resolved,
                    position,
                } => {
                    if self.width == self.clearance_width {
                        let end =
                            ((i64::from(point[axis]) >> level) << level) + (1_i64 << level) - 1;
                        let at_end = i64::from(point[axis]) >= end;
                        let mut unavailable = false;
                        let mut sample = |cell| match (self.open)(cell) {
                            Some(value) => value,
                            None => {
                                unavailable = true;
                                false
                            }
                        };
                        let clear = match self.edge {
                            Edge::NegativeY => {
                                negative_edge_clearance(point, at_end, false, &mut sample)
                            }
                            Edge::PositiveX => {
                                positive_edge_clearance(point, at_end, false, &mut sample)
                            }
                            Edge::PositiveY => {
                                positive_edge_clearance(point, at_end, true, &mut sample)
                            }
                            Edge::NegativeX => {
                                negative_edge_clearance(point, at_end, true, &mut sample)
                            }
                        };
                        if unavailable {
                            return Err(EdgeError::Unavailable);
                        }
                        if !clear {
                            return Ok([false; 2]);
                        }
                    }
                    self.nodes.try_reserve(1)?;
                    self.nodes.push(Node {
                        level: resolved,
                        position,
                    });
                    Ok([level != 0; 2])
                }
            }
        }
    }

    if input.level >= i32::BITS || input.movement_width <= 0 || input.clearance_width <= 0 {
        return Err(EdgeError::Invalid);
    }
    let mut walker = Walker {
        width: input.movement_width,
        clearance_width: input.clearance_width,
        edge: input.edge,
        remaining: input.max_visits,
        nodes: Vec::new(),
        resolve: &mut resolve,
        open: &mut open,
    };
    let ends = walker.visit(input.level, input.point)?;
    Ok(EdgeResult {
        nodes: walker.nodes,
        ends,
    })
}

/// Corner direction, in the order used by coarse-node expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    PositiveXNegativeY,
    PositiveXPositiveY,
    NegativeXPositiveY,
    NegativeXNegativeY,
}

/// Class-2 corner clearance after both adjoining edges have been admitted.
pub fn corner_clearance(
    position: [i32; 2],
    corner: Corner,
    mut open: impl FnMut([i32; 2]) -> bool,
) -> bool {
    let mut check = |dx: i32, dy: i32| {
        let (Some(x), Some(y)) = (position[0].checked_add(dx), position[1].checked_add(dy)) else {
            return false;
        };
        open([x, y])
    };
    match corner {
        Corner::PositiveXNegativeY => check(1, 0),
        Corner::PositiveXPositiveY => check(1, 0) && check(0, 1) && check(1, 1),
        Corner::NegativeXPositiveY => check(0, 1),
        Corner::NegativeXNegativeY => true,
    }
}

/// Source-qualified widths and the independent diagnostic work bound for a coarse node.
#[derive(Clone, Copy, Debug)]
pub struct CoarseInput {
    pub node: Node,
    pub movement_width: i32,
    pub clearance_width: i32,
    pub max_visits: usize,
}

/// Expand the four edges and eligible corners in their decoded order.
pub fn expand_coarse(
    input: CoarseInput,
    mut resolve: impl FnMut(u32, [i32; 2]) -> Option<Lookup>,
    mut open: impl FnMut([i32; 2]) -> Option<bool>,
) -> Result<Vec<Node>, EdgeError> {
    let Node { level, position } = input.node;
    if level == 0
        || level >= i32::BITS
        || position.iter().any(|value| *value < 0)
        || input.movement_width <= 0
        || input.clearance_width <= 0
    {
        return Err(EdgeError::Invalid);
    }
    let span = 1_i64 << level;
    let low = position.map(|value| (i64::from(value) >> level) << level);
    let high = [low[0] + span, low[1] + span];
    let coordinate = |value: i64| i32::try_from(value).map_err(|_| EdgeError::Invalid);
    let adjusted = if input.movement_width == input.clearance_width {
        [
            i64::from(position[0]).min(high[0] - i64::from(input.clearance_width)),
            i64::from(position[1]).min(high[1] - i64::from(input.clearance_width)),
        ]
    } else {
        position.map(i64::from)
    };
    let points = [
        [coordinate(adjusted[0])?, coordinate(low[1] - 1)?],
        [coordinate(high[0])?, coordinate(adjusted[1])?],
        [coordinate(adjusted[0])?, coordinate(high[1])?],
        [coordinate(low[0] - 1)?, coordinate(adjusted[1])?],
    ];
    let edges = [
        Edge::NegativeY,
        Edge::PositiveX,
        Edge::PositiveY,
        Edge::NegativeX,
    ];
    let mut nodes = Vec::new();
    let mut ends = [[false; 2]; 4];
    for i in 0..edges.len() {
        let result = expand_edge(
            EdgeInput {
                level,
                point: points[i],
                movement_width: input.movement_width,
                clearance_width: input.clearance_width,
                edge: edges[i],
                max_visits: input.max_visits,
            },
            &mut resolve,
            &mut open,
        )?;
        ends[i] = result.ends;
        nodes.try_reserve(result.nodes.len())?;
        nodes.extend(result.nodes);
    }
    let corners = [
        (
            Corner::PositiveXNegativeY,
            [coordinate(high[0])?, coordinate(low[1] - 1)?],
            ends[0][1] && ends[1][0],
        ),
        (
            Corner::PositiveXPositiveY,
            [coordinate(high[0])?, coordinate(high[1])?],
            ends[1][1] && ends[2][1],
        ),
        (
            Corner::NegativeXPositiveY,
            [coordinate(low[0] - 1)?, coordinate(high[1])?],
            ends[2][0] && ends[3][1],
        ),
        (
            Corner::NegativeXNegativeY,
            [coordinate(low[0] - 1)?, coordinate(low[1] - 1)?],
            ends[3][0] && ends[0][0],
        ),
    ];
    for (corner, point, admitted) in corners {
        if !admitted {
            continue;
        }
        let mut corner_level = level;
        loop {
            match resolve(corner_level, point).ok_or(EdgeError::Unavailable)? {
                Lookup::Blocked => break,
                Lookup::Mixed => {
                    corner_level = corner_level.checked_sub(1).ok_or(EdgeError::Invalid)?
                }
                Lookup::Open { level, position } => {
                    if input.movement_width == input.clearance_width {
                        let mut unavailable = false;
                        let clear = corner_clearance(point, corner, |cell| match open(cell) {
                            Some(value) => value,
                            None => {
                                unavailable = true;
                                false
                            }
                        });
                        if unavailable {
                            return Err(EdgeError::Unavailable);
                        }
                        if !clear {
                            break;
                        }
                    }
                    nodes.try_reserve(1)?;
                    nodes.push(Node { level, position });
                    break;
                }
            }
        }
    }
    Ok(nodes)
}

/// A source-qualified optional auxiliary transition from a base cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseLink {
    Absent,
    Active([i32; 2]),
    Unknown,
}

/// One base-level neighbor; auxiliary transitions have a different native cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseNeighbor {
    pub node: Node,
    pub via_link: bool,
}

/// Expand the native cardinal and diagonal order from a base cell.
pub fn expand_base(
    position: [i32; 2],
    movement_width: i32,
    clearance_width: i32,
    link: BaseLink,
    mut resolve: impl FnMut(u32, [i32; 2]) -> Option<Lookup>,
    mut open: impl FnMut([i32; 2]) -> Option<bool>,
) -> Result<Vec<BaseNeighbor>, EdgeError> {
    if position.iter().any(|value| *value < 0) || movement_width <= 0 || clearance_width <= 0 {
        return Err(EdgeError::Invalid);
    }
    struct Walker<'a, R, O> {
        width: i32,
        checked_width: i32,
        resolve: &'a mut R,
        open: &'a mut O,
        nodes: Vec<BaseNeighbor>,
    }
    impl<R, O> Walker<'_, R, O>
    where
        R: FnMut(u32, [i32; 2]) -> Option<Lookup>,
        O: FnMut([i32; 2]) -> Option<bool>,
    {
        fn sample(&mut self, p: [i32; 2], dx: i32, dy: i32) -> Result<bool, EdgeError> {
            let (Some(x), Some(y)) = (p[0].checked_add(dx), p[1].checked_add(dy)) else {
                return Ok(false);
            };
            (self.open)([x, y]).ok_or(EdgeError::Unavailable)
        }
        fn node(&mut self, p: [i32; 2]) -> Result<Option<Node>, EdgeError> {
            match (self.resolve)(0, p).ok_or(EdgeError::Unavailable)? {
                Lookup::Blocked => Ok(None),
                Lookup::Mixed => Err(EdgeError::Invalid),
                Lookup::Open { level, position } => Ok(Some(Node { level, position })),
            }
        }
        fn push(&mut self, node: Node, via_link: bool) -> Result<(), EdgeError> {
            self.nodes.try_reserve(1)?;
            self.nodes.push(BaseNeighbor { node, via_link });
            Ok(())
        }
        fn cardinal(&mut self, p: [i32; 2], direction: Edge) -> Result<bool, EdgeError> {
            let Some(node) = self.node(p)? else {
                return Ok(false);
            };
            if self.width == self.checked_width {
                let wide = match direction {
                    Edge::NegativeY => {
                        self.sample(p, 1, 0)?
                            && (p[0] & 1 == 0
                                || node.level == 0
                                || self.sample(p, 1, -self.checked_width)?
                                || self.sample(p, -1, 0)?)
                    }
                    Edge::PositiveX => self.sample(p, 1, 0)? && self.sample(p, 1, 1)?,
                    Edge::PositiveY => self.sample(p, 0, 1)? && self.sample(p, 1, 1)?,
                    Edge::NegativeX => {
                        self.sample(p, 0, 1)?
                            && (p[1] & 1 == 0
                                || node.level == 0
                                || self.sample(p, -self.checked_width, 1)?
                                || self.sample(p, 0, -1)?)
                    }
                };
                if !wide {
                    return Ok(false);
                }
            }
            self.push(node, false)?;
            Ok(true)
        }
        fn diagonal(&mut self, p: [i32; 2], corner: Corner) -> Result<(), EdgeError> {
            let Some(node) = self.node(p)? else {
                return Ok(());
            };
            if self.width == self.checked_width {
                let clear = match corner {
                    Corner::PositiveXNegativeY => self.sample(p, 1, 0)?,
                    Corner::PositiveXPositiveY => self.sample(p, 1, 1)?,
                    Corner::NegativeXPositiveY => self.sample(p, 0, 1)?,
                    Corner::NegativeXNegativeY => true,
                };
                if !clear {
                    return Ok(());
                }
            }
            self.push(node, false)
        }
    }
    let offset =
        |dx: i32, dy: i32| Some([position[0].checked_add(dx)?, position[1].checked_add(dy)?]);
    let point = |dx, dy| offset(dx, dy).ok_or(EdgeError::Invalid);
    let mut walker = Walker {
        width: movement_width,
        checked_width: clearance_width,
        resolve: &mut resolve,
        open: &mut open,
        nodes: Vec::new(),
    };
    let n = walker.cardinal(point(0, -1)?, Edge::NegativeY)?;
    let e = walker.cardinal(point(1, 0)?, Edge::PositiveX)?;
    let s = walker.cardinal(point(0, 1)?, Edge::PositiveY)?;
    let w = walker.cardinal(point(-1, 0)?, Edge::NegativeX)?;
    if n && e {
        walker.diagonal(point(1, -1)?, Corner::PositiveXNegativeY)?;
    }
    if e && s {
        walker.diagonal(point(1, 1)?, Corner::PositiveXPositiveY)?;
    }
    if s && w {
        walker.diagonal(point(-1, 1)?, Corner::NegativeXPositiveY)?;
    }
    if w && n {
        walker.diagonal(point(-1, -1)?, Corner::NegativeXNegativeY)?;
    }
    match link {
        BaseLink::Absent => {}
        BaseLink::Unknown => return Err(EdgeError::Unavailable),
        BaseLink::Active(target) if target != position => {
            if let Some(node) = walker.node(target)? {
                walker.push(node, true)?;
            }
        }
        BaseLink::Active(_) => {}
    }
    Ok(walker.nodes)
}

/// Source-qualified search costs and independent allocation bounds.
#[derive(Clone, Copy, Debug)]
pub struct SearchInput {
    pub start: Node,
    pub goal: Node,
    pub priority_scale: u32,
    pub result_scale: u32,
    pub link_priority_cost: u32,
    pub link_result_cost: u32,
    pub removal_limit: usize,
    pub node_limit: usize,
}

/// Search failure is distinct from invalid or unavailable source data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub distance: Option<u32>,
    pub removals: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error(transparent)]
    Node(#[from] NodeError),
    #[error(transparent)]
    Edge(#[from] EdgeError),
    #[error("reach arithmetic or path state is outside supported bounds")]
    Invalid,
    #[error("reach self-edge update has not been qualified")]
    SelfEdge,
    #[error("reach allocation failed: {0}")]
    Allocation(#[from] std::collections::TryReserveError),
}

/// Search a caller-supplied, source-qualified hierarchy using native queue accounting.
pub fn search(
    input: SearchInput,
    expand: impl FnMut(Node) -> Result<Vec<BaseNeighbor>, EdgeError>,
) -> Result<SearchResult, SearchError> {
    search_observed(input, expand, |_, _, _| {})
}

/// Search while reporting each removal before stale-entry filtering.
pub fn search_observed(
    input: SearchInput,
    mut expand: impl FnMut(Node) -> Result<Vec<BaseNeighbor>, EdgeError>,
    mut observe: impl FnMut(Node, crate::native_path_queue::Entry, u32),
) -> Result<SearchResult, SearchError> {
    use crate::native_path_queue::{Entry, Queue};

    #[derive(Clone, Copy, Default)]
    struct State {
        cost: Option<u32>,
        generation: u32,
        parent: Option<u32>,
        via_link: bool,
    }
    fn ensure(states: &mut Vec<State>, id: u32) -> Result<(), SearchError> {
        let target = usize::try_from(id)
            .map_err(|_| SearchError::Invalid)?
            .checked_add(1)
            .ok_or(SearchError::Invalid)?;
        if target > states.len() {
            states.try_reserve(target - states.len())?;
            states.resize(target, State::default());
        }
        Ok(())
    }
    let mut nodes = Nodes::new(input.node_limit);
    let start = nodes.resolve(input.start)?;
    let goal = nodes.resolve(input.goal)?;
    if start == goal {
        return Ok(SearchResult {
            distance: Some(
                distance(
                    input.start.position,
                    input.goal.position,
                    input.result_scale,
                )
                .ok_or(SearchError::Invalid)?,
            ),
            removals: 0,
        });
    }
    let mut states = Vec::new();
    ensure(&mut states, goal)?;
    states[start as usize].cost = Some(0);
    states[start as usize].generation = 1;
    let mut queue = Queue::default();
    queue.push(Entry {
        priority: distance(
            input.start.position,
            input.goal.position,
            input.priority_scale,
        )
        .ok_or(SearchError::Invalid)?,
        node: start,
        generation: 1,
    })?;
    let mut remaining = input.removal_limit;
    while let Some(entry) = queue.pop(&mut remaining) {
        let current = usize::try_from(entry.node).map_err(|_| SearchError::Invalid)?;
        observe(
            nodes.get(entry.node).ok_or(SearchError::Invalid)?,
            entry,
            states[current].generation,
        );
        if states[current].generation != entry.generation {
            continue;
        }
        states[current].generation = states[current]
            .generation
            .checked_add(1)
            .ok_or(SearchError::Invalid)?;
        if entry.node == goal {
            let mut id = goal;
            let mut total = 0_u32;
            let mut path_bound = states.len();
            while let Some(parent) = states[id as usize].parent {
                path_bound = path_bound.checked_sub(1).ok_or(SearchError::Invalid)?;
                // The native path-length walk tests the parent's link marker.
                let cost = if states[parent as usize].via_link {
                    input.link_result_cost
                } else {
                    distance(
                        nodes.get(id).ok_or(SearchError::Invalid)?.position,
                        nodes.get(parent).ok_or(SearchError::Invalid)?.position,
                        input.result_scale,
                    )
                    .ok_or(SearchError::Invalid)?
                };
                total = total.checked_add(cost).ok_or(SearchError::Invalid)?;
                id = parent;
            }
            if id != start {
                return Err(SearchError::Invalid);
            }
            return Ok(SearchResult {
                distance: Some(total),
                removals: input.removal_limit - remaining,
            });
        }
        let current_node = nodes.get(entry.node).ok_or(SearchError::Invalid)?;
        let current_cost = states[current].cost.ok_or(SearchError::Invalid)?;
        for neighbor in expand(current_node)? {
            let id = nodes.resolve(neighbor.node)?;
            if id == entry.node {
                return Err(SearchError::SelfEdge);
            }
            ensure(&mut states, id)?;
            let node = nodes.get(id).ok_or(SearchError::Invalid)?;
            let edge_cost = if neighbor.via_link {
                input.link_priority_cost
            } else {
                distance(current_node.position, node.position, input.priority_scale)
                    .ok_or(SearchError::Invalid)?
            };
            let cost = current_cost
                .checked_add(edge_cost)
                .ok_or(SearchError::Invalid)?;
            let state = &mut states[id as usize];
            if state.cost.is_some_and(|old| old <= cost) {
                continue;
            }
            let increment = if state.cost.is_some() { 2 } else { 1 };
            state.generation = state
                .generation
                .checked_add(increment)
                .ok_or(SearchError::Invalid)?;
            state.cost = Some(cost);
            state.parent = Some(entry.node);
            state.via_link = neighbor.via_link;
            let priority = cost
                .checked_add(
                    distance(node.position, input.goal.position, input.priority_scale)
                        .ok_or(SearchError::Invalid)?,
                )
                .ok_or(SearchError::Invalid)?;
            queue.push(Entry {
                priority,
                node: id,
                generation: state.generation,
            })?;
        }
        states[current].generation = states[current]
            .generation
            .checked_add(1)
            .ok_or(SearchError::Invalid)?;
    }
    Ok(SearchResult {
        distance: None,
        removals: input.removal_limit - remaining,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_tests_keep_object_flags_and_chain_class_distinct() {
        let rules = EntryRules {
            active: 0x100,
            registered: 0x200,
            forbidden: 0x40,
            class_mask: 0xf000,
            object_class: 0x3000,
            skipped_class: 0x4000,
            pathing_mask: 0xff,
            max_entries: 3,
        };
        let check =
            |present, flags, state, class, mask| rules.blocks(present, flags, state, class, mask);
        assert!(check(true, 0x104, 0x200, 0x3001, 4));
        assert!(!check(false, 0x104, 0x200, 0x3001, 4));
        assert!(!check(true, 4, 0x200, 0x3001, 4));
        assert!(!check(true, 0x104, 0, 0x3001, 4));
        assert!(!check(true, 0x104, 0x240, 0x3001, 4));
        assert!(!check(true, 0x104, 0x200, 0x4001, 4));
        assert!(!check(true, 0x104, 0x200, 0x3001, 2));
        assert!(!check(true, 0, 0x200, 0x3001, 4));
    }

    #[test]
    fn cell_chain_counts_skipped_entries_and_deduplicates_before_class_check() {
        let rules = EntryRules {
            active: 0x100,
            registered: 0x200,
            forbidden: 0x40,
            class_mask: 0xf000,
            object_class: 0x3000,
            skipped_class: 0x4000,
            pathing_mask: 0xff,
            max_entries: 3,
        };
        let blocked = ChainEntry {
            identity: 7,
            present: true,
            flags: 0x104,
            state: 0x200,
            class: 0x3000,
        };
        let inactive = ChainEntry {
            flags: 0,
            ..blocked
        };
        let tombstone = ChainEntry {
            class: 0,
            ..blocked
        };
        let skipped = ChainEntry {
            class: 0x4000,
            ..blocked
        };
        assert!(rules.cell_blocked(0x1000, 0x1004, []));
        assert!(rules.cell_blocked(0, 4, [inactive, inactive, blocked]));
        assert!(!rules.cell_blocked(0, 4, [inactive, inactive, inactive, blocked]));
        assert!(!rules.cell_blocked(0, 4, [tombstone, blocked]));
        assert!(rules.cell_blocked(0, 4, [skipped, blocked]));
    }

    #[test]
    fn cell_cleanup_keeps_newest_live_object_and_removes_tombstones() {
        let rules = EntryRules {
            active: 0x100,
            registered: 0x200,
            forbidden: 0x40,
            class_mask: 0xf000,
            object_class: 0x3000,
            skipped_class: 0x4000,
            pathing_mask: 0xff,
            max_entries: 3,
        };
        let object = ChainEntry {
            identity: 1,
            present: true,
            flags: 0x104,
            state: 0x200,
            class: 0x3000,
        };
        let removed = ChainEntry {
            identity: 2,
            ..object
        };
        let other = ChainEntry {
            identity: 3,
            flags: 0,
            ..object
        };
        let mut entries = vec![
            ChainEntry {
                class: 0x4000,
                ..object
            },
            ChainEntry {
                class: 0,
                ..removed
            },
            other,
            object,
            removed,
            object,
            ChainEntry {
                identity: 4,
                present: false,
                ..object
            },
        ];
        let mut seen = Vec::new();
        assert!(!rules.cell_blocked(0, 4, entries.iter().copied()));
        rules.compact_chain(&mut entries, &mut seen).unwrap();
        assert_eq!(entries, [other, object]);
        assert!(seen.is_empty());
        assert!(rules.cell_blocked(0, 4, entries));
    }

    #[test]
    fn bounded_set_query_refuses_unproved_or_inconsistent_chains() {
        let rules = EntryRules {
            active: 0x100,
            registered: 0x200,
            forbidden: 0x40,
            class_mask: 0xf000,
            object_class: 0x3000,
            skipped_class: 0x4000,
            pathing_mask: 0xff,
            max_entries: 3,
        };
        let entry = ChainEntry {
            identity: 7,
            present: true,
            flags: 0x104,
            state: 0x200,
            class: 0x3000,
        };
        let cell = [12, 34];
        assert_eq!(
            rules.bounded_cell_blocked(cell, 0, 4, &[entry], 2, true),
            Ok(true)
        );
        assert_eq!(
            rules.bounded_cell_blocked(cell, 0, 2, &[entry], 2, true),
            Ok(false)
        );
        assert_eq!(
            rules.bounded_cell_blocked(cell, 0, 4, &[entry], 3, true),
            Err(ChainBoundError::Limit {
                cell,
                bound: 3,
                limit: 3
            })
        );
        assert_eq!(
            rules.bounded_cell_blocked(cell, 0, 4, &[entry], 0, true),
            Err(ChainBoundError::Inconsistent {
                cell,
                bound: 0,
                entries: 1
            })
        );
        assert_eq!(
            rules.bounded_cell_blocked(cell, 0, 4, &[entry, entry], 2, true),
            Err(ChainBoundError::NonCanonical { cell })
        );
        assert_eq!(
            rules.bounded_cell_blocked(cell, 0x1000, 0x1004, &[], 4, false),
            Ok(true)
        );
        let removal = ChainEntry { class: 0, ..entry };
        assert!(!rules.cell_blocked(0, 4, [removal, entry]));
        assert_eq!(
            rules.bounded_cell_blocked(cell, 0, 4, &[entry], 2, false),
            Err(ChainBoundError::UnprovenOrder { cell })
        );
    }

    #[test]
    fn crossing_bounds_include_the_old_link_and_reset_only_at_cleanup() {
        let mut history = CellCrossings::new([5, 4]).unwrap();
        let old = CellRect {
            lower: [1, 1],
            upper: [3, 3],
        };
        let new = CellRect {
            lower: [2, 1],
            upper: [4, 3],
        };
        history.moved(old, old).unwrap();
        assert_eq!(history.bound([1, 1], 1), Some(1));
        history.moved(old, new).unwrap();
        assert_eq!(history.bound([1, 1], 0), Some(2));
        assert_eq!(history.bound([2, 1], 1), Some(1));
        assert_eq!(history.bound([3, 1], 1), Some(2));
        history
            .region_change(
                CellRect {
                    lower: [1, 1],
                    upper: [2, 2],
                },
                true,
            )
            .unwrap();
        assert_eq!(history.bound([1, 1], 0), Some(4));
        let invalid = CellRect {
            lower: [4, 0],
            upper: [6, 1],
        };
        assert!(history.moved(new, invalid).is_err());
        assert_eq!(history.bound([1, 1], 0), Some(4));
        history.cleaned();
        assert_eq!(history.bound([1, 1], 0), Some(0));
        assert_eq!(history.bound([3, 1], 1), Some(2));
        assert_eq!(history.bound([5, 1], 0), None);
    }

    #[test]
    fn crossing_counter_saturates_without_wrapping() {
        let mut history = CellCrossings::new([1, 1]).unwrap();
        let rect = CellRect {
            lower: [0, 0],
            upper: [1, 1],
        };
        for _ in 0..usize::from(u16::MAX) {
            history.region_change(rect, false).unwrap();
        }
        history.cleaned();
        assert_eq!(
            history.bound([0, 0], 0),
            Some(usize::from(CellCrossings::COUNT))
        );
        history.region_change(rect, true).unwrap();
        history.cleaned();
        assert_eq!(history.bound([0, 0], 0), Some(0));
    }

    #[test]
    fn parent_counts_outside_and_blocked_children_and_preserves_links() {
        let open = Some(Cell::Open { link: false });
        let blocked = Some(Cell::Blocked);
        assert_eq!(parent([open; 4]), Cell::Open { link: false });
        assert_eq!(parent([blocked, None, blocked, None]), Cell::Blocked);
        assert_eq!(parent([open, open, None, open]), Cell::Mixed);
        assert_eq!(parent([open, Some(Cell::Mixed), open, open]), Cell::Mixed);
        assert_eq!(
            parent([open, Some(Cell::Open { link: true }), open, open]),
            Cell::Mixed
        );
    }

    #[test]
    fn base_uses_all_four_samples_and_clearing_preserves_the_link() {
        assert_eq!(base([false; 4], true, false), Cell::Open { link: true });
        assert_eq!(base([true; 4], false, false), Cell::Blocked);
        assert_eq!(base([false, true, false, false], false, false), Cell::Mixed);
        assert_eq!(base([true; 4], true, true), Cell::Open { link: true });
    }

    #[test]
    fn lookup_climbs_only_open_parents_and_returns_the_query_position() {
        let classify = |level, point: [i32; 2]| match (level, point) {
            (0, [7, 3]) | (1, [3, 1]) => Some(Cell::Open { link: false }),
            (2, [1, 0]) => Some(Cell::Mixed),
            _ => None,
        };
        assert_eq!(
            lookup(0, 4, [7, 3], classify),
            Some(Lookup::Open {
                level: 1,
                position: [7, 3]
            })
        );
        assert_eq!(lookup(2, 4, [7, 3], classify), Some(Lookup::Mixed));
        assert_eq!(lookup(0, 4, [-1, 3], classify), Some(Lookup::Blocked));
        assert_eq!(lookup(4, 4, [7, 3], classify), None);
        assert_eq!(
            lookup(0, 2, [7, 3], classify),
            Some(Lookup::Open {
                level: 1,
                position: [7, 3]
            })
        );
        assert_eq!(
            lookup(1, 4, [7, 3], |level, _| (level == 1)
                .then_some(Cell::Open { link: false })),
            None
        );
        assert_eq!(
            lookup(0, 1, [7, 3], |_, _| Some(Cell::Mixed)),
            Some(Lookup::Blocked)
        );
    }

    #[test]
    fn negative_edges_require_the_interior_and_one_end_alternative() {
        let present = [[11, 20], [11, 21], [9, 20]];
        assert!(negative_edge_clearance([10, 20], false, false, |p| present.contains(&p)));
        assert!(negative_edge_clearance([10, 20], true, false, |p| present.contains(&p)));
        assert!(negative_edge_clearance([10, 20], true, false, |p| {
            [[11, 20], [11, 21], [11, 22]].contains(&p)
        }));
        assert!(!negative_edge_clearance([10, 20], true, false, |p| p == [11, 20]));
        assert!(negative_edge_clearance([20, 10], true, true, |p| present.contains(&[p[1], p[0]])));
        assert!(!negative_edge_clearance(
            [i32::MAX, 0],
            false,
            false,
            |_| true
        ));
    }

    #[test]
    fn nodes_preserve_the_first_coordinate_and_enforce_the_bound() {
        let mut nodes = Nodes::new(2);
        let first = Node {
            level: 2,
            position: [9, 7],
        };
        assert_eq!(nodes.resolve(first).unwrap(), 0);
        assert_eq!(
            nodes
                .resolve(Node {
                    level: 2,
                    position: [11, 4]
                })
                .unwrap(),
            0
        );
        assert_eq!(nodes.get(0), Some(first));
        assert_eq!(
            nodes
                .resolve(Node {
                    level: 1,
                    position: [9, 7]
                })
                .unwrap(),
            1
        );
        assert!(matches!(
            nodes.resolve(Node {
                level: 0,
                position: [9, 7]
            }),
            Err(NodeError::Limit)
        ));
        assert!(matches!(
            nodes.resolve(Node {
                level: 32,
                position: [9, 7]
            }),
            Err(NodeError::Invalid)
        ));
        assert!(matches!(
            nodes.resolve(Node {
                level: 0,
                position: [-1, 7]
            }),
            Err(NodeError::Invalid)
        ));
    }

    #[test]
    fn positive_edges_apply_all_end_alternatives_and_reject_missing_base_cells() {
        let square = [[11, 20], [10, 21], [11, 21]];
        let alternatives = [
            [[10, 19], [11, 19]],
            [[8, 21], [9, 21]],
            [[9, 21], [10, 19]],
        ];
        assert!(positive_edge_clearance([10, 20], false, false, |p| square.contains(&p)));
        assert!(!positive_edge_clearance([10, 20], true, false, |p| square.contains(&p)));
        for extra in alternatives {
            assert!(positive_edge_clearance([10, 20], true, false, |p| square
                .contains(&p)
                || extra.contains(&p)));
            assert!(positive_edge_clearance([20, 10], true, true, |p| square
                .contains(&[p[1], p[0]])
                || extra.contains(&[p[1], p[0]])));
        }
        assert!(!positive_edge_clearance([10, 20], false, false, |p| p == [11, 20]));
    }

    #[test]
    fn scaled_distance_rounds_down_and_refuses_overflow() {
        assert_eq!(distance([3, 5], [6, 9], 7), Some(35));
        assert_eq!(distance([3, 5], [4, 6], 3), Some(4));
        assert_eq!(distance([3, 5], [3, 5], 7), Some(0));
        assert_eq!(distance([i32::MIN, 0], [i32::MAX, 0], 2), None);
    }

    #[test]
    fn split_edges_visit_the_lower_half_first_and_account_for_width() {
        assert_eq!(split_edge(2, 5, 2), Some([5, 6]));
        assert_eq!(split_edge(2, 7, 2), Some([4, 7]));
        assert_eq!(split_edge(1, 7, 2), Some([6, 7]));
        assert_eq!(split_edge(0, 7, 2), None);
        assert_eq!(split_edge(2, -1, 2), None);
        assert_eq!(split_edge(2, 7, 0), None);
    }

    #[test]
    fn corner_clearance_uses_the_native_directional_cells() {
        let all = [[11, 20], [10, 21], [11, 21]];
        let check = |corner| corner_clearance([10, 20], corner, |p| all.contains(&p));
        assert!(check(Corner::PositiveXNegativeY));
        assert!(check(Corner::PositiveXPositiveY));
        assert!(check(Corner::NegativeXPositiveY));
        assert!(corner_clearance(
            [10, 20],
            Corner::NegativeXNegativeY,
            |_| false
        ));
        assert!(!corner_clearance(
            [10, 20],
            Corner::PositiveXPositiveY,
            |p| p == [11, 20]
        ));
        assert!(!corner_clearance(
            [i32::MAX, 20],
            Corner::PositiveXNegativeY,
            |_| true
        ));
    }

    #[test]
    fn corner_lookup_descends_mixed_cells_until_open_or_blocked() {
        let classify = |level, _: [i32; 2]| match level {
            0 => Some(Cell::Open { link: false }),
            1 | 2 => Some(Cell::Mixed),
            _ => None,
        };
        assert_eq!(
            corner_lookup(2, 3, [5, 7], classify),
            Some(Lookup::Open {
                level: 0,
                position: [5, 7]
            })
        );
        assert_eq!(
            corner_lookup(2, 3, [5, 7], |level, _| Some(if level == 0 {
                Cell::Blocked
            } else {
                Cell::Mixed
            })),
            Some(Lookup::Blocked)
        );
    }

    #[test]
    fn mixed_edge_expands_in_order_and_preserves_end_flags() {
        let input = EdgeInput {
            level: 2,
            point: [5, 3],
            movement_width: 2,
            clearance_width: 2,
            edge: Edge::NegativeY,
            max_visits: 3,
        };
        let resolve = |level, position| {
            Some(if level == 2 {
                Lookup::Mixed
            } else {
                Lookup::Open { level, position }
            })
        };
        let result = expand_edge(input, resolve, |_| Some(true)).unwrap();
        assert_eq!(
            result.nodes,
            [
                Node {
                    level: 1,
                    position: [5, 3]
                },
                Node {
                    level: 1,
                    position: [6, 3]
                }
            ]
        );
        assert_eq!(result.ends, [true, true]);
        let blocked = |level, position: [i32; 2]| {
            Some(if level == 2 {
                Lookup::Mixed
            } else if position[0] == 6 {
                Lookup::Blocked
            } else {
                Lookup::Open { level, position }
            })
        };
        assert_eq!(
            expand_edge(input, blocked, |_| Some(true)).unwrap().ends,
            [true, false]
        );
        assert!(matches!(
            expand_edge(
                EdgeInput {
                    max_visits: 1,
                    ..input
                },
                resolve,
                |_| Some(true)
            ),
            Err(EdgeError::Limit)
        ));
        assert!(matches!(
            expand_edge(
                EdgeInput {
                    level: 1,
                    max_visits: 1,
                    ..input
                },
                resolve,
                |_| None
            ),
            Err(EdgeError::Unavailable)
        ));
    }

    #[test]
    fn coarse_expansion_visits_edges_before_corners_and_requires_both_edges() {
        let input = CoarseInput {
            node: Node {
                level: 1,
                position: [5, 3],
            },
            movement_width: 1,
            clearance_width: 2,
            max_visits: 8,
        };
        let open_node = |level, position| Some(Lookup::Open { level, position });
        let nodes = expand_coarse(input, open_node, |_| Some(true)).unwrap();
        assert_eq!(
            nodes.iter().map(|node| node.position).collect::<Vec<_>>(),
            [
                [5, 1],
                [6, 3],
                [5, 4],
                [3, 3],
                [6, 1],
                [6, 4],
                [3, 4],
                [3, 1]
            ]
        );
        let blocked = |level, position| {
            Some(if position == [6, 3] {
                Lookup::Blocked
            } else {
                Lookup::Open { level, position }
            })
        };
        assert_eq!(
            expand_coarse(input, blocked, |_| Some(true)).unwrap().len(),
            5
        );
    }

    #[test]
    fn base_expansion_orders_neighbors_and_handles_qualified_links() {
        let resolve = |level, position| Some(Lookup::Open { level, position });
        let nodes = expand_base([5, 3], 1, 2, BaseLink::Active([9, 8]), resolve, |_| {
            Some(true)
        })
        .unwrap();
        assert_eq!(
            nodes
                .iter()
                .map(|entry| entry.node.position)
                .collect::<Vec<_>>(),
            [
                [5, 2],
                [6, 3],
                [5, 4],
                [4, 3],
                [6, 2],
                [6, 4],
                [4, 4],
                [4, 2],
                [9, 8]
            ]
        );
        assert!(nodes.last().unwrap().via_link);
        assert!(nodes[..8].iter().all(|entry| !entry.via_link));
        let blocked = |level, position| {
            Some(if position == [6, 3] {
                Lookup::Blocked
            } else {
                Lookup::Open { level, position }
            })
        };
        assert_eq!(
            expand_base([5, 3], 1, 2, BaseLink::Absent, blocked, |_| Some(true))
                .unwrap()
                .len(),
            5
        );
        assert!(matches!(
            expand_base([5, 3], 1, 2, BaseLink::Unknown, resolve, |_| Some(true)),
            Err(EdgeError::Unavailable)
        ));
        assert!(matches!(
            expand_base([5, 3], 2, 2, BaseLink::Absent, resolve, |_| None),
            Err(EdgeError::Unavailable)
        ));
        let promoted = |_, position| Some(Lookup::Open { level: 1, position });
        let parity = expand_base([5, 3], 2, 2, BaseLink::Absent, promoted, |p| {
            Some(p != [6, 0] && p != [4, 2])
        })
        .unwrap();
        assert!(parity.iter().all(|entry| entry.node.position != [5, 2]));
    }

    #[test]
    fn search_reports_integer_path_length_and_removal_limit() {
        let start = Node {
            level: 0,
            position: [1, 1],
        };
        let middle = Node {
            level: 0,
            position: [4, 1],
        };
        let goal = Node {
            level: 0,
            position: [4, 5],
        };
        let input = SearchInput {
            start,
            goal,
            priority_scale: 3,
            result_scale: 7,
            link_priority_cost: 1,
            link_result_cost: 7,
            removal_limit: 3,
            node_limit: 5,
        };
        let graph = |node| {
            Ok(if node == start {
                vec![BaseNeighbor {
                    node: middle,
                    via_link: false,
                }]
            } else if node == middle {
                vec![BaseNeighbor {
                    node: goal,
                    via_link: false,
                }]
            } else {
                Vec::new()
            })
        };
        assert_eq!(
            search(input, graph).unwrap(),
            SearchResult {
                distance: Some(49),
                removals: 3
            }
        );
        assert_eq!(
            search(
                SearchInput {
                    removal_limit: 2,
                    ..input
                },
                graph
            )
            .unwrap(),
            SearchResult {
                distance: None,
                removals: 2
            }
        );
        assert!(matches!(
            search(input, |node| Ok(vec![BaseNeighbor {
                node,
                via_link: true
            }])),
            Err(SearchError::SelfEdge)
        ));
    }

    #[test]
    fn search_counts_stale_removals_without_expanding_them() {
        let start = Node {
            level: 0,
            position: [0, 0],
        };
        let a = Node {
            level: 0,
            position: [8, 0],
        };
        let b = Node {
            level: 0,
            position: [1, 0],
        };
        let input = SearchInput {
            start,
            goal: Node {
                level: 0,
                position: [10, 0],
            },
            priority_scale: 1,
            result_scale: 1,
            link_priority_cost: 1,
            link_result_cost: 1,
            removal_limit: 4,
            node_limit: 5,
        };
        let mut expanded = Vec::new();
        let result = search(input, |node| {
            expanded.push(node);
            Ok(if node == start {
                vec![
                    BaseNeighbor {
                        node: a,
                        via_link: false,
                    },
                    BaseNeighbor {
                        node: b,
                        via_link: true,
                    },
                ]
            } else if node == b {
                vec![BaseNeighbor {
                    node: a,
                    via_link: true,
                }]
            } else {
                Vec::new()
            })
        })
        .unwrap();
        assert_eq!(
            result,
            SearchResult {
                distance: None,
                removals: 4
            }
        );
        assert_eq!(expanded, [start, b, a]);
    }

    #[test]
    fn search_path_length_checks_the_parent_link_marker() {
        let start = Node {
            level: 0,
            position: [0, 0],
        };
        let middle = Node {
            level: 0,
            position: [8, 0],
        };
        let goal = Node {
            level: 0,
            position: [10, 0],
        };
        let input = SearchInput {
            start,
            goal,
            priority_scale: 1,
            result_scale: 1,
            link_priority_cost: 1,
            link_result_cost: 1,
            removal_limit: 3,
            node_limit: 5,
        };
        let result = search(input, |node| {
            Ok(if node == start {
                vec![BaseNeighbor {
                    node: middle,
                    via_link: true,
                }]
            } else if node == middle {
                vec![BaseNeighbor {
                    node: goal,
                    via_link: false,
                }]
            } else {
                Vec::new()
            })
        })
        .unwrap();
        assert_eq!(
            result,
            SearchResult {
                distance: Some(9),
                removals: 3
            }
        );
    }

    #[test]
    fn search_uses_base_expansion_on_a_synthetic_grid() {
        let input = SearchInput {
            start: Node {
                level: 0,
                position: [1, 1],
            },
            goal: Node {
                level: 0,
                position: [3, 3],
            },
            priority_scale: 3,
            result_scale: 7,
            link_priority_cost: 1,
            link_result_cost: 7,
            removal_limit: 8,
            node_limit: 16,
        };
        let result = search(input, |node| {
            expand_base(
                node.position,
                1,
                2,
                BaseLink::Absent,
                |level, position| {
                    Some(if position.iter().all(|value| (0..4).contains(value)) {
                        Lookup::Open { level, position }
                    } else {
                        Lookup::Blocked
                    })
                },
                |_| Some(true),
            )
        })
        .unwrap();
        assert_eq!(
            result,
            SearchResult {
                distance: Some(18),
                removals: 3
            }
        );
    }
}
