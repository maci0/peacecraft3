//! Selection subgroups: units of one type form a subgroup, subgroups are ordered, and one
//! is active. The portrait, unit panel and command card follow the active subgroup.
//! Evidence labels for every rule are in `docs/re-notes/input-selection-rules.md`.
use pcx_script::HandleId;
use std::cmp::Reverse;

/// One selected unit as the subgroup model sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    pub id: HandleId,
    /// Unit type rawcode. Units of equal type share a subgroup.
    pub kind: [u8; 4],
    pub hero: bool,
    /// `UnitData/prio` of the unit type.
    pub priority: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subgroup {
    pub kind: [u8; 4],
    /// Members in selection order. The first one represents the subgroup.
    pub units: Vec<HandleId>,
}

/// Splits `members` (selection order) into subgroups, highest first: heroes, then descending
/// priority, then ascending type rawcode as a big-endian number. Units of one type share a
/// subgroup in selection order, except heroes: each hero is a subgroup of its own.
pub fn group(members: &[Member]) -> Vec<Subgroup> {
    let mut ranked: Vec<(Member, Vec<HandleId>)> = Vec::new();
    for member in members {
        let shared = ranked
            .iter_mut()
            .find(|(first, _)| !member.hero && !first.hero && first.kind == member.kind);
        match shared {
            Some((_, units)) => units.push(member.id),
            None => ranked.push((*member, vec![member.id])),
        }
    }
    ranked.sort_by_key(|(first, _)| {
        (
            Reverse(first.hero),
            Reverse(first.priority),
            u32::from_be_bytes(first.kind),
        )
    });
    ranked
        .into_iter()
        .map(|(first, units)| Subgroup {
            kind: first.kind,
            units,
        })
        .collect()
}

/// The subgroup after (`forward`) or before `active`, wrapping at both ends.
/// `count` is the number of subgroups; with fewer than two there is nothing to cycle.
pub fn cycle(count: usize, active: usize, forward: bool) -> usize {
    match (count, forward) {
        (0 | 1, _) => 0,
        (count, true) => (active + 1) % count,
        (count, false) => (active + count - 1) % count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(n: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(n + 1).unwrap())
    }

    fn member(n: u64, kind: &[u8; 4], hero: bool, priority: i32) -> Member {
        Member {
            id: handle(n),
            kind: *kind,
            hero,
            priority,
        }
    }

    #[test]
    fn units_of_one_type_share_a_subgroup_in_selection_order() {
        let groups = group(&[
            member(0, b"hfoo", false, 6),
            member(1, b"hpea", false, 1),
            member(2, b"hfoo", false, 6),
        ]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].kind, *b"hfoo");
        assert_eq!(groups[0].units, [handle(0), handle(2)]);
        assert_eq!(groups[1].units, [handle(1)]);
    }

    #[test]
    fn heroes_come_first_then_priority_descending() {
        let groups = group(&[
            member(0, b"hpea", false, 1),
            member(1, b"hkni", false, 3),
            member(2, b"Hpal", true, 9),
            member(3, b"hfoo", false, 6),
        ]);
        let kinds: Vec<_> = groups.iter().map(|g| g.kind).collect();
        assert_eq!(kinds, [*b"Hpal", *b"hfoo", *b"hkni", *b"hpea"]);
    }

    #[test]
    fn a_hero_outranks_a_higher_priority_non_hero() {
        let groups = group(&[member(0, b"hfoo", false, 9), member(1, b"Hpal", true, 1)]);
        assert_eq!(groups[0].kind, *b"Hpal");
    }

    #[test]
    fn equal_priority_orders_by_rawcode_not_by_selection_order() {
        let groups = group(&[member(0, b"hrif", false, 6), member(1, b"hfoo", false, 6)]);
        assert_eq!(groups[0].kind, *b"hfoo");
    }

    #[test]
    fn every_hero_is_its_own_subgroup() {
        let groups = group(&[
            member(0, b"Hpal", true, 9),
            member(1, b"hfoo", false, 6),
            member(2, b"Hpal", true, 9),
            member(3, b"hfoo", false, 6),
        ]);
        let sizes: Vec<_> = groups.iter().map(|g| (g.kind, g.units.len())).collect();
        assert_eq!(sizes, [(*b"Hpal", 1), (*b"Hpal", 1), (*b"hfoo", 2)]);
        assert_eq!(groups[0].units, [handle(0)]);
        assert_eq!(groups[1].units, [handle(2)]);
    }

    #[test]
    fn cycling_wraps_in_both_directions() {
        assert_eq!(cycle(3, 0, true), 1);
        assert_eq!(cycle(3, 2, true), 0);
        assert_eq!(cycle(3, 0, false), 2);
        assert_eq!(cycle(3, 2, false), 1);
        assert_eq!(cycle(2, 0, false), 1);
        assert_eq!(cycle(1, 0, true), 0);
        assert_eq!(cycle(0, 0, false), 0);
    }
}
