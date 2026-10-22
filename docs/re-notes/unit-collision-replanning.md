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

## Blocked destination search termination (2026-10-21)

The recreation's A* stopped on reaching an exact clear destination, but a blocked
destination had no equivalent success condition. Even after finding its closest
standable cell, the search kept exploring until the budget or the reachable
component ended. A valid fallback route could therefore spend the entire shared
projection budget before movement or another unit could run.

For a blocked destination, a bounded best-first scan now proves the minimum
octile distance of any clear cell from that destination. It visits cells in
distance order without treating obstacles as connectivity barriers. Each tested
cell consumes route work, and heap allocation is fallible. The forward A* still
tests actual reachability and diagonal corner clearance; it stops when it reaches
the proven distance. If every nearest cell is disconnected, it continues its
existing bounded search and fallback policy. No timer, retry delay, source speed,
collision threshold or caller work limit is changed.

This preserves the existing grid objective: nearest reachable cell, then shortest
travel with the existing heap tie order. It does not establish that native
pathfinding uses that objective or algorithm. The extra lower-bound scan may
itself refuse when the caller supplies insufficient work; it does not invent a
clear endpoint when that proof is incomplete.

The synthetic regression covers a blocked cell with a clear cardinal neighbor,
a blocked cross whose nearest cells are diagonal, and a larger blocked square.
Both a grid-sized budget and a larger budget must return identical paths and
spend identical work, leaving work available. Every returned segment must be
clear, and occupancy must remain unchanged. The regression fails on the earlier
search because it spends the full small budget. Limits remain covered by the
existing exhausted-route and failed-span tests.

Disabling the proven-distance termination makes the regression fail again on
layout 0: the route spends its complete grid-sized budget. The real condition
was restored before the final aggregate checks.

Selected unchanged Human03 originally exhausted movement work at tick 155 on a
skeleton's blocked destination. An adjacent-cell-only proof passed that point
but found the same defect at tick 230 on a larger blocked area, motivating the
general distance proof. These are recreation diagnostics, not original-client
runtime evidence. The general proof passes the selected Human03 256-tick gate:
two independent ordered starts agree, and the uncommanded control differs.
A longer bounded headless diagnostic reaches tick 257, then refuses the
unimplemented `SetUnitLookAt` at Human03.j:2606. No cinematic-skip input was sent
before that failure. This is not whole-map acceptance or rendered/native parity.
Final private-target checks pass: complete `pcx-play` package 1,180 / 0 failed /
46 ignored, strict all-target clippy, WASM playback library, native app all-target
check with test-fixtures, and formatting on changed Rust files. These compile and
host-runtime checks do not qualify native or rendered pathfinding.
The three selected original-input startup/order/profile/patrol regression gates
also pass, separately from the Human03 progression gate above.

## Large creation radii

The selected Legacy executable's classifier at `0x8bc926..0x8bc979` selects four
bands using the threshold loads already admitted by `campaign_client::placement_rules`.
Its final branch selects the largest band for values at or above the last
threshold. The recreation previously refused any collision radius larger than a
cell before consulting those bands, making that largest band unreachable.

That additional one-cell restriction is removed. Radius validation still rejects
negative/nonfinite values and unsupported arithmetic; the existing derived band
extents, collision raster, occupancy and query budgets remain authoritative.
No new measured radius or threshold is compiled into production code.

This was the first refusal of the selected Human03 startup at
`war3map.j:1097`, creating the authored `nogm`. The loaded UnitBalance collision
record and the selected normalization put that unit in the already-supported
largest band. This static evidence identifies the extra restriction; it does not
prove native placement-search ordering or complete Human03 execution.

A synthetic regression uses four-unit cells and a six-unit radius. It requires
the full outer footprint, prevents overlap with an existing unit, and makes an
obstacle at its outer corner refuse when no further query work is available.

At the placement-only increment, selected Human03 startup passed that creation
site and reached the unsupported `PauseUnit` in `Blizzard.j:3611`. The subsequent
[unit-pause increment](unit-pause.md) advances it to `RestoreUnit`. The map is
still not accepted. Native library tests, strict all-target clippy and the WASM
library check pass for the placement increment.

## Accepted spans for consumers

`ObjectHost::advance_unit_activity_spans(now, work, &mut Vec<AcceptedSpan>)`
projects the same motion as `advance_unit_activity` and appends what it accepted.
`AcceptedSpan { unit, from, to, start, end, moving }` is on host game time:

- A moving span is one straight accepted segment, from its accepted start point to
  its accepted end point. Times come from the same arithmetic that credits distance:
  `started + credited / speed`. Spans never interpolate across a detour.
- A non-moving span (`moving: false`, `from == to`) covers a stop: the unit stood at
  `from` from `start` to `end`. It covers a blocked tail, from the last credited
  point to the tick, including a blocked tail that a replan then leaves.
- Zero-length spans are omitted. After a unit's last span, no span is emitted until its
  next motion. Consumers treat that interval as standing at the last `to`.
- Within one call, each unit's spans are contiguous in time and position. Across
  calls, the first span starts at the previous call's last credited time and
  position. A pause rebases `started`, so contiguity there holds to floating-point
  rounding, not bit for bit.
- Relocations and placements outside motion (`SetUnitPosition`, reveal, creation)
  emit no span. A consumer must reconcile the record position at call boundaries.

Bound: each span is emitted in a branch that has already charged at least one unit
of the caller's `work`, so a call appends at most its budget on entry. A call
that fails removes the spans it appended. Motion effects of the failed call are
not rolled back; the caller stops the session, as before.

Blocked stops sample the segment every half cell at each tick, so their positions
depend on the tick partition. Unblocked movement is partition-independent in
position. Span boundaries follow the projection calls, so a consumer that needs
blocked stops at a fixed cadence must project at that cadence.

Order retention after a stop is not modelled here. `GetUnitCurrentOrder` reads
the moving state, and the spans carry no order data.

## Attack regions and zero-time replans (2026-10-22)

An ordinary-input Human01 itinerary exposed shared movement-work exhaustion at
requested tick 5377. Inspection identifies unit 825 as a bandit (`nban`) chasing
footman 829, correcting the earlier tentative footman label. The bandit's chase
was aimed at the occupied target centre. Its bounded search returned a partial
route after consuming the remaining work; the immediate projection of that new
route then failed with zero elapsed time. Removing that redundant projection
exposed the following unit's starvation at the same tick, so it was insufficient
by itself.

Successful blocked replans now retain their new epoch until the next projection.
They cannot move again at the same time. The synthetic obstruction needs 136
work for the block and route, compared with 138 including the old zero-time
loop/check. The exact-budget regression fails on the old code and passes with
zero work remaining, the same stop point, retained destination and next-tick
resumption. Existing accepted-span and speed checks remain in force.

Combat now supplies its already computed weapon range plus both collision radii
to the bounded router. A clear straight approach is a fast path; if that point
is obstructed, the same search can reach another clear cell centre inside the
attack region. Its heuristic measures remaining distance to the region, and it
does not search for an unoccupied copy of the target's centre. Collision masks,
occupancy, diagonal-corner checks and work charges remain authoritative. Range
survives blocked replanning; point Move, Follow and Patrol keep zero reach and
their existing point-goal rules. Motion retains the target coordinate as its
intent, separately from the endpoint found inside reach.

A second regression blocks the straight contact point, requires another approach
angle inside reach, checks every segment and confirms unchanged occupancy.
A combat regression verifies that the attacker approaches within the fixture's
range and subsequently deals damage. No asset values or larger work allowance
are substituted for these tests.

The same controller on the unchanged Human01 map now clears the former failure,
earns the authored XP cap and ledger reward, and reaches a later unsupported
`GroupEnumUnitsOfPlayer` call at requested tick 28475. Earlier point-only approach
experiments and their failures are retained alongside the current route in local
`.scratch/campaign-route/`; none is chapter acceptance. The controller's commands
respond to current state, so this is not replay equality for an identical input
journal. Native steering/path choice, exact range geometry and complete campaign
victory/transition remain unqualified. General searches may still refuse their
explicit operating bounds; this change does not remove those bounds.

Private checks for this increment: 1,135 play library tests pass with 33 ignored;
318 focused unit-creation tests and the region-routing test pass. Strict
all-target Clippy, the WASM library check and native application all-target check
pass. Timing is report-only; the 136-work regression is deterministic.

## Crowded point destinations and disconnected pockets (2026-10-22)

After owner enumeration was admitted, the same unchanged-Human01 controller
stopped at requested tick 29361. Failure context identifies villager 766 (`nvlw`),
not a combat chase: its blocked replan consumed the remaining work before
villager 767 could advance. Their destination is the script's caravan-removal
rectangle. The loaded script separately controls collision-disabling and removal
triggers; this change does not bypass those triggers or move their rectangles.

A retained read-only diagnostic traversed the recreation's collision graph at
the failure. The mover's component has 31,593 cells and reaches a closest octile
distance of 28 from the requested goal. The earlier lower-bound candidate at
distance 18 belongs to a separate 540-cell component. The optional 128-query
probe could not certify that component, leaving the forward search to visit
13,320 cells before spending the remaining work. These are local recreation
observations, not measurements of native pathfinding or constants in the code.

The optional component proof now shares the supplied route allowance, reserving
half its remaining work for forward search and deducting only actual inspections.
It caches clearance for each queried cell centre while collision is immutable,
including blocked cells and diagonal corners. It visits pending cells toward
the mover first, so a connected destination can establish its connection without
flooding away from the mover. A completed disconnected proof excludes only its
proven component; an incomplete proof still excludes nothing. The overall route
and shared activity allowances are unchanged. There is no persistent clearance
cache to survive occupancy changes, no skipped actor, and no additional RNG draw.

Synthetic closed-pocket cases compare identical endpoints and identical work
with smaller and ample allowances, check all returned segments, and retain the
occupancy snapshot. The smaller regression exhausts its 1,000-work allowance on
the old implementation. Existing point, seed-edge and diagonal-corner checks
remain required. A cache-only intermediate passed that fixture but did not clear
the campaign failure; an undirected adaptive probe regressed existing work
checks. Both rejected attempts remain in local receipts.

With the directed proof, the ordinary-input campaign itinerary passes the old
work failure and reaches game time 507.703125, where the Warlord kills Arthas.
The selected opening-repeat, authored Escape/control and XP-cap gates pass; the
long controller remains a failed player route, not chapter victory. It still
uses adaptive ordinary commands, not an identical input journal. Native route
choice, cinematic timing, complete victory and next-map transition remain open.

Final private checks: 1,140 play library tests pass with 33 ignored, including
explicit nearest-outside-edge assertions for both synthetic pocket sizes.
Strict all-target Clippy, the play WASM library check and the native application
all-target check pass. The three selected-map diagnostic gates pass; the longer
route remains a failed boss fight. No wall-time/FPS threshold or native-lockstep
claim is used for this increment.
