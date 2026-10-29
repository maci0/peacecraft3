//! Reach hierarchy built from a tracked session's current spatial chains.

use crate::native_path_hierarchy::{
    BaseLink, BaseNeighbor, Cell, CellRect, CoarseInput, Lookup, Node, SearchInput, base, distance,
    expand_base, expand_coarse, lookup, parent, search,
};
use crate::unit_placement::Placement;
use pcx_script::HandleId;

/// Whether the caller has established that the source has no auxiliary warp links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkEvidence {
    Absent,
    Unavailable,
}

/// One query-local hierarchy. The builder's current collision rectangle is cleared.
pub struct Reach {
    levels: Vec<([usize; 2], Vec<Cell>)>,
    bounds: [[f32; 2]; 2],
    cell_size: f32,
    group: usize,
    priority: u32,
    result_scale: u32,
    removal_limit: usize,
    movement: i32,
    count: u32,
}

impl Reach {
    /// Build the hierarchy from the live fine grid and the authored terrain raster.
    pub fn from_placement(
        placement: &Placement,
        worker: HandleId,
        links: LinkEvidence,
    ) -> Result<Self, String> {
        let rectangle = placement
            .movement_stamp_rectangle(worker)
            .map_err(|error| error.to_string())?;
        Self::from_rectangles(placement, &[rectangle], links)
    }

    pub(crate) fn from_rectangles(
        placement: &Placement,
        rectangles: &[CellRect],
        links: LinkEvidence,
    ) -> Result<Self, String> {
        if links != LinkEvidence::Absent {
            return Err("AI reach auxiliary warp-link state is not admitted".into());
        }
        let pool = placement
            .spatial()
            .ok_or("AI reach spatial pool is missing")?;
        if let Some(refusal) = pool.refusal() {
            return Err(format!("AI reach spatial pool refused: {refusal:?}"));
        }
        pool.placement_rules()
            .ok_or("AI placement rules are missing")?;
        let (terrain, (mask, priority, result_scale, removal_limit, movement, count)) = pool
            .reach_inputs()
            .ok_or("AI reach source inputs are missing")?;
        let group = usize::try_from(result_scale).map_err(|_| "invalid reach sample group")?;
        if group != 2 || count == 0 {
            return Err("unsupported AI reach sample grouping or hierarchy depth".into());
        }
        let bounds = placement.world_bounds();
        let cell_size = placement.rules().cell_size();
        let dimensions = [terrain.width as usize, terrain.height as usize];
        let size = dimensions.map(|axis| axis.div_ceil(group));
        let mut levels = Vec::new();
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(size[0].checked_mul(size[1]).ok_or("reach grid overflow")?)
            .map_err(|error| error.to_string())?;
        for y in 0..size[1] {
            for x in 0..size[0] {
                let mut samples = [true; 4];
                for (i, sample) in samples.iter_mut().enumerate() {
                    let p = [x * group + i % group, y * group + i / group];
                    if p[0] < dimensions[0] && p[1] < dimensions[1] {
                        let flags = terrain
                            .cell(p[0] as u32, p[1] as u32)
                            .ok_or("reach terrain cell missing")?;
                        *sample = flags & mask != 0
                            || pool
                                .live()
                                .blocked([p[1], p[0]], 0, u32::from(mask))
                                .map_err(|error| error.to_string())?;
                    }
                }
                let cleared = rectangles.iter().any(|rectangle| {
                    (0..2).all(|axis| {
                        let coordinate = [x, y][axis] * group;
                        let native_axis = 1 - axis;
                        coordinate < rectangle.upper[native_axis]
                            && coordinate + group > rectangle.lower[native_axis]
                    })
                });
                cells.push(base(samples, false, cleared));
            }
        }
        levels.push((size, cells));
        while levels.len() < count as usize {
            let (size, previous) = levels.last().ok_or("reach hierarchy missing")?;
            let next_size = size.map(|axis| axis.div_ceil(2));
            let mut next = Vec::new();
            next.try_reserve_exact(
                next_size[0]
                    .checked_mul(next_size[1])
                    .ok_or("reach hierarchy overflow")?,
            )
            .map_err(|error| error.to_string())?;
            for y in 0..next_size[1] {
                for x in 0..next_size[0] {
                    let children = std::array::from_fn(|i| {
                        let p = [x * 2 + i % 2, y * 2 + i / 2];
                        (p[0] < size[0] && p[1] < size[1]).then(|| previous[p[1] * size[0] + p[0]])
                    });
                    next.push(parent(children));
                }
            }
            levels.push((next_size, next));
        }
        Ok(Self {
            levels,
            bounds,
            cell_size,
            group,
            priority,
            result_scale,
            removal_limit,
            movement,
            count,
        })
    }

    /// Return the selected distance and removal count, or `None` for the failure sentinel.
    pub fn query(&self, center: [f32; 2], point: [f32; 2]) -> Result<(Option<u32>, usize), String> {
        self.query_with_limit(center, point, self.removal_limit)
    }

    pub(crate) fn query_with_limit(
        &self,
        center: [f32; 2],
        point: [f32; 2],
        removal_limit: usize,
    ) -> Result<(Option<u32>, usize), String> {
        if center.iter().chain(&point).any(|value| !value.is_finite()) {
            return Err("nonfinite AI reach point".into());
        }
        let cell = |level: u32, point: [i32; 2]| -> Option<Cell> {
            let (size, cells) = self.levels.get(level as usize)?;
            let [x, y] = point.map(|value| usize::try_from(value).ok());
            let (x, y) = (x?, y?);
            (x < size[0] && y < size[1]).then(|| cells[y * size[0] + x])
        };
        let coordinates = |point: [f32; 2]| -> [i32; 2] {
            std::array::from_fn(|axis| {
                ((point[axis] - self.bounds[0][axis]) / (self.cell_size * self.group as f32))
                    .floor() as i32
            })
        };
        let start = coordinates(center);
        let goal = coordinates(point);
        if cell(0, start).is_none() {
            return Err("AI reach start is outside the map".into());
        }
        let resolve = |level, position| lookup(level, self.count, position, cell);
        let open = |position| Some(matches!(cell(0, position), Some(Cell::Open { .. })));
        let start_node = resolve(0, start).ok_or("AI reach start lookup failed")?;
        let goal_node = match resolve(0, goal).ok_or("AI reach goal lookup failed")? {
            Lookup::Blocked => Lookup::Open {
                level: 0,
                position: goal,
            },
            value => value,
        };
        if start == goal || matches!(start_node, Lookup::Blocked) {
            return Ok((
                Some(distance(start, goal, self.result_scale).ok_or("AI reach distance overflow")?),
                0,
            ));
        }
        let (
            Lookup::Open {
                level: a,
                position: s,
            },
            Lookup::Open {
                level: b,
                position: g,
            },
        ) = (start_node, goal_node)
        else {
            return Err("unresolved AI reach node".into());
        };
        let node_limit = self
            .levels
            .iter()
            .try_fold(0usize, |n, (_, cells)| n.checked_add(cells.len()))
            .ok_or("reach node count overflow")?;
        let input = SearchInput {
            start: Node {
                level: a,
                position: s,
            },
            goal: Node {
                level: b,
                position: g,
            },
            priority_scale: self.priority,
            result_scale: self.result_scale,
            link_priority_cost: 0,
            link_result_cost: 0,
            removal_limit,
            node_limit,
        };
        let result = search(input, |node| {
            if node.level == 0 {
                expand_base(
                    node.position,
                    self.movement,
                    self.movement,
                    BaseLink::Absent,
                    resolve,
                    open,
                )
            } else {
                expand_coarse(
                    CoarseInput {
                        node,
                        movement_width: self.movement,
                        clearance_width: self.movement,
                        max_visits: node_limit,
                    },
                    resolve,
                    open,
                )
                .map(|nodes| {
                    nodes
                        .into_iter()
                        .map(|node| BaseNeighbor {
                            node,
                            via_link: false,
                        })
                        .collect()
                })
            }
        })
        .map_err(|error| error.to_string())?;
        Ok((result.distance, result.removals))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_start_and_work_limit_are_preserved() {
        let mut reach = Reach {
            levels: vec![([2, 2], vec![Cell::Blocked; 4])],
            bounds: [[0., 0.], [128., 128.]],
            cell_size: 32.,
            group: 2,
            priority: 24,
            result_scale: 2,
            removal_limit: 0,
            movement: 2,
            count: 1,
        };
        assert_eq!(reach.query([0., 0.], [64., 0.]), Ok((Some(2), 0)));
        reach.levels[0].1.fill(Cell::Open { link: false });
        assert_eq!(reach.query([0., 0.], [64., 0.]), Ok((None, 0)));
        assert!(reach.query([f32::NAN, 0.], [64., 0.]).is_err());
    }
}
