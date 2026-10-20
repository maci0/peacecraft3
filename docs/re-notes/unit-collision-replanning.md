# Live unit collision replanning

Point orders (`IssuePointOrder*`, `IssuePointOrderLoc*`) and internal steering
(`steer_unit`) store the requested destination as `Motion.goal`. Routes come from
`Placement::route` on the live collision cache: unit footprints, placed footprints
and authored pathing. `UnitActivity::destination()` and attack-move destinations
return `goal`, not the last waypoint. A route that stops short keeps the requested
point visible while the unit moves, and reports no destination once it stops.

Movement samples the segment to this tick's target every half cell. When a sample
is blocked, the unit walks to the last clear sample and commits its footprint
there. It is credited only the distance up to that sample, so distance beyond it
is not credited. The unit then replans once from that point to `goal`. The replan
draws from the tick's work budget, capped by the admitted `route_work` per route,
and subtracts the work it spends. The replan starts with zero distance at the
tick's time, so the unit never spends travel already credited on the blocked
route. A second block in the same tick pauses the unit until the next tick. A
route that makes no progress stops the unit, which then stands. A stopped unit
spends no route work on later ticks.

Footprints use the selected collision extents. The collision value is normalised
by the cell size before the radius bands are compared. With the synthetic fixture
(cell size 4, collision 2, so normalised radius 0.5, thresholds 0.5, 1, 1.5), a
unit covers cells `c - 1 ..= c` on each axis. Two footprints overlap when their
centre cells differ by at most one on both axes.

Shortcuts (reconstruction policy, not measured original behavior): a half-cell
sample grid, one replan per tick, A* over the 8-neighbourhood with the existing
tie order, and an immediate stop when no route progresses. Original path choice,
steering, local avoidance, and whether units wait for a moving blocker instead of
routing around it are unverified.

Tests in `unit_activity_tests.rs` use the synthetic fixture only. They cover a
blocker created after the order (per-tick displacement bounded by source speed
times elapsed time), an obstruction discovered after a ten-second tick (stops at
the last clear sample, x 42, and spends no further distance on that tick), an
enclosed unit that stops after a partial step and then spends a fixed work count
with zero work on later ticks, and an unreachable destination that keeps its goal
and ends short. The fixed work count is derived in the test. Second-block-in-a-tick
behavior has no reproducing fixture yet.

These results do not establish original collision radii or native movement.
