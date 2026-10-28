//! Cell aggregation and lookup for the selected client's reach hierarchy.
//!
//! Callers must supply the native occupancy classes; this does not perform a reach search.

/// The classification of a cell for one selected occupancy mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Open { link: bool },
    Blocked,
    Mixed,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
