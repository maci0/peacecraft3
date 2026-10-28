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

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("spatial chain grid dimensions are invalid")]
    Dimensions,
    #[error("spatial chain rectangle is outside the grid")]
    Rectangle,
    #[error("spatial chain capacity {capacity} exceeds native index limit {limit}")]
    Capacity { capacity: usize, limit: usize },
    #[error("spatial chain pool exhausted: need {needed} links, {available} available")]
    PoolExhausted { needed: usize, available: usize },
    #[error("spatial chain allocation failed")]
    Allocation,
}

#[derive(Clone, Copy)]
struct Node {
    identity: u64,
    class: u32,
    next: u32,
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
    /// Read the cleanup interval using the selected client's real arithmetic.
    pub fn reviewed_cleanup_interval(
        image: &[u8],
        rules: &crate::ai_placement::Rules,
        numeric: &crate::script_numeric::NumericPolicy,
    ) -> Result<f32, String> {
        use crate::neutral_player::{RuleInputBudget, sha256};
        use crate::neutral_rule_input::Pe;
        use crate::script_random::real;

        if rules.image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || sha256(image) != rules.image_sha256
            || numeric.image_sha256() != &rules.image_sha256
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
        let divisor = real::from_i32(divisor).map_err(|e| e.to_string())?;
        let dividend = real::from_i32(dividend).map_err(|e| e.to_string())?;
        let interval = f32::from_bits(
            numeric
                .division(dividend, divisor)
                .map_err(|e| e.to_string())?,
        );
        let minimum =
            f32::from_le_bytes(read(address, 4, false)?.try_into().expect("checked width"));
        if !interval.is_finite() || !minimum.is_finite() || interval <= 0.0 || minimum <= 0.0 {
            return Err("invalid spatial cleanup interval".into());
        }
        Ok(interval.max(minimum))
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
