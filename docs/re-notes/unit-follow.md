# Unit-target Move

## Inputs and evidence

Selected client: Legacy `1.29.2.9231`, executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.

The selected Legacy command strings describe a Move order on a unit as following
that unit using Move orders. The selected `Units/MiscData.txt` contains
`FollowRange` and `StructureFollowRange` in `[Misc]`. Fresh-game preparation reads
those fields through the existing ordered scalar profile reader, with the map's
`war3mapMisc.txt` applied afterward. Missing, duplicate, non-scalar, negative and
non-finite values refuse; no measured distance is copied into production code.
The activity rules retain the ordered profile labels and full input SHA-256s.
Structure classification reads the target's resolved `UnitBalance/isbldg` field,
including typed map modifications; a missing field is an error, not a guessed
non-building class.

The shipped `UnitMetaData.slk` declares `isbldg` as `bool`. The earlier
`IsUnitType(UNIT_TYPE_STRUCTURE)` used a numeric-only balance reader, so it could
not read that field. It now shares the strict Boolean classifier with follow;
both source cells and typed map overrides are covered by synthetic checks.
Other type queries do not require a structure field merely to answer their own
classification.

The Move identifier remains the selected PE/registry-derived activity identifier.
Both `IssueTargetOrder` and `IssueTargetOrderById` reach the same consumer before
the combat target-order handler. This is supported by source text and shared
order identity, not an original-client runtime qualification of follow geometry.

## Execution

An accepted order retains the target and the appropriate source range on the
existing unit activity. It preflights liveness, movability, pause, source fields,
route, animation and active-list capacity before replacing the old order. A
null, self, hidden, dead, removed or non-unit target is refused. Nonpositive-speed
or paused followers refuse without replacing their previous activity. Target
pause does not prevent following its stationary position.

The unit approaches a point on the line to the target at the admitted range.
Placement, collision, accepted positions and movement speed use the existing
runtime. There is no extra world, speed constant, arrival tolerance or RNG stream.
Reaching range stops travel but retains Move intent; `GetUnitCurrentOrder`
continues to report that admitted order. A later target displacement resumes
travel. Follow intent prevents idle acquisition, natural sleep and guard wandering.
Stop, Hold, another movement/patrol/target order, successful spell orders, death
and removal use the shared order/lifecycle cancellation paths.

At an activity projection boundary, all followers observe target positions from
before that projection. This prevents handle iteration order from choosing which
target state they see. A stationary target preserves a current route epoch;
changed targets and blocked-route retries use the existing route/tick work bounds.
Standing followers remain active so later target motion can be observed. A
relocation inside range stops an old route rather than walking back out of range.
An entirely blocked route retains intent without a zero-progress motion, so
retries do not repeatedly restart the standing animation. Removing a blocker can
resume travel on a later projection.
The pre-projection target-position buffer reserves capacity before order commit
and is reused across projections; it is not an accumulating history. Route and
animation allocations retain their existing lifecycle.
Pause retains the intent and rebases the motion epoch without catch-up.
Loss of a target cancels the intent on the next unpaused projection.

The command-card Move target path and friendly-unit right-click reach this native
consumer. Hostile right-click retains Attack, and ground right-click retains Move.
These Move/Attack identifiers and journal names now come from admitted rules and
the inverse registry, rather than copied order names. Only accepted orders enter
the player journal. Item pickup keeps its existing separate consumer.

## Limits and verification scope

Center-distance interpretation, range choice by target structure, boundary
sampling/replan cadence, target-loss behavior, blocked retry policy and animation
phase are recreation choices awaiting an original-client comparison. Moving-target
trajectories are projection-partition dependent; fixed simulation ticks remain the
session contract. Collision avoidance and the existing blocked-motion limitations
remain unchanged. Transient follow orders are not added to campaign unit caches.
Patrol imitation and item-target following are not implemented by this path.
Right-click shop, repair, transport and harvesting context actions remain separate
work; this increment connects the ordinary unit Move/follow case.
Retained profile hashes establish this host's input provenance. The current
headless admission digest identifies its catalog by install path, so it does not
establish equal catalog/Misc contents across peers. That pre-existing session
identity gap remains open; no cross-install lockstep claim follows from these
follow checks.

Synthetic fixtures vary ranges and profile overrides and cover both native forms,
stationary-target projection composition, normal routing and pathing-off travel,
target movement/removal, pause, new-order cancellation, external relocation and
bounded work. A combat fixture checks that an enemy can be followed without
automatic attacks, then that Stop restores acquisition. These establish the
recreation contract; they do not prove native behavior or rendered parity.
Private validation receipts stay under `.scratch/unit-follow/`.

Final private-target checks pass: the full `pcx-play` package has 1,244 passed,
0 failed and 49 ignored (including 1,123 library tests). Its four unignored
campaign-drop tests require explicit local originals; the passing package run
supplies matching script/unit members from the unchanged selected Human01 map
and the selected prelude files. App binary tests pass 590 with 52 ignored.
Three explicitly enabled Human01 component/input gates pass, including retained
follow target and accepted-only journaling. Strict affected all-target Clippy,
core/app native and WASM compilation, and touched-file formatting pass on the
frozen source manifest. No new rendered or original-client follow comparison
was made, and the source range geometry and native replan cadence remain open.
