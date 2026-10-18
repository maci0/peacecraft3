# Live unit positions and range-event boundary

The selected executable is the same 1.29b image qualified in
[the timer evidence](legacy-timer-start.md): VERSIONINFO `1.29.2.9231`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Archive release identities remain independent. The observations below do not
implement a live campaign world or close the unchanged-map startup gate.

## Current ownership

The production `ObjectHost` owns executed `CreateUnit` records and their actual
script handles. `ParsedMapContext.world` contains imported placement records and
catalog definitions. The separate `pcx_sim::World` is the fixed-point movement
harness. There is no existing association between those three identity domains.
Neither a matching rawcode nor a placement ordinal establishes such a link.

Creation records retain the executed request and catalog provenance. Their x/y
fields are not accepted live positions. A campaign consumer must preserve that
distinction and scope any external unit reference to its published session,
including failed and successful replacement. Adding an unconsumed handle/id
table would not supply the missing spatial state.

## Selected-code relations

Bounded static decoding links the functional range-registration wrapper to the
unit's position virtual method, the same method used by `GetUnitX/Y`. The
single-axis setters preserve the other coordinate through that position object
and call its update wrapper. `SetUnitPosition` follows a separate virtual path
with additional arguments; it is not established as two single-axis assignments.
`BlzGetUnitCollisionSize` reads another unit virtual method, not a proved direct
copy of a catalog collision field. Creation passes through an engine factory
before exposing the JASS handle.

Twenty-one source windows and the selected virtual-call relations are rechecked
against the complete image. These are bounded windows, sometimes including
subsequent functions, not complete function/caller graphs or runtime observations.
No decoded operand or measured collision size is a production constant.

## One bounded range probe (2026-10-18)

A fresh isolated Wine/Xvfb/software-Mesa run instruments Human01 around its
authored `InitBlizzard` call. Its original `main` and `config` remain; reversible
edits change only the script member of a private unsigned experiment map.
Synthetic inputs create three probe units, two range subscriptions and a bounded
sequence of single-axis moves. One subscription has an explicit Filter; the other
has null. The completed log has 51 records.

| Condition in this run | Observation |
| --- | --- |
| A tracked unit is already inside when registration returns | Its filter/condition/action sequence appears later, before the first scheduled move. Delivery is deferred beyond the registration call. |
| Move an outside candidate inside | The explicit filter runs, followed by its condition/action and then the null-filter subscription's condition/action. |
| Move within the range | No additional entry callback appears. |
| Move out and back within one timer callback | The intermediate query reports outside, but no new entry callback appears after the final inside position. |
| Compare two collision-relative boundary points | `IsUnitInRange` reports true at both; the range event appears for only the nearer point. That predicate cannot directly substitute for range-event membership. |
| Inspect callback context | The filter sees the candidate as both filter and trigger unit, with null triggering trigger/event. Conditions/actions see their trigger and the selected `EVENT_UNIT_TARGET_IN_RANGE`; the candidate remains visible through `GetFilterUnit`, including the null-filter subscription. |
| Destroy both triggers, then move the candidate inside | One later filter invocation appears, with no condition or action. Immediate cancellation of filter work is not established by trigger destruction. |
| Compare initial placement requests and returned positions | The requested shared y and relative x separation are not preserved by the queried live positions. A creation request is insufficient to initialize accepted world coordinates. |

The two selected boundary points are consistent with candidate collision
contributing to range notification and with a wider two-unit predicate. They do
not prove an exact formula, equality rule, collision derivation or native update
cadence. Likewise, the creation discrepancy does not identify a placement solver,
its inputs, snapping, blockers or search order. Those remain prerequisites for a
faithful live-position backend.

The disabled trigger was enabled again within the same callback that moved the
candidate inside. This does not qualify a crossing that remains disabled through
delivery. Moving the anchor and candidate within one callback also supplies no
independent anchor-motion entry case. The post-destruction filter observation
covers one subsequent entry, not complete event lifetime or removal semantics.

Input/member hashes, reversible edits, script resolution, callback identities,
context/order and the selected boundary counterexample pass verification. Eight
mutated logs are refused. The reference container is stopped and preserved.
The final capture shows the original opening cinematic; it is not recreation
rendering or chapter completion. Source windows, probe, logs, captures and
verification remain local in `.scratch/unit-world-root-20261018/`.

This is one instrumented Wine run, not native Windows truth or an unmodified-map
acceptance run. No production range-registration stub, world-id adapter, copied
position/collision values or assumed polling interval is added. Live placement,
session-scoped world identity, world-driven event delivery and the required
four-result RNG factory gate remain open. The factory still refuses
`TriggerRegisterUnitInRange` after one exact RNG result.
