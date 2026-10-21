# Patrol orders on the executed unit

## Identity and evidence

Selected client: Legacy 1.29.2.9231 (plan 1.29b), SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Its order initializer registers the `patrol` name at `0xb115bd`. The comparison
at `0xb64594` uses the same operand in a point-order acceptance routine. The
existing registry and activity readers now decode and join those source records.
The activity binding carries a required patrol comparison window, validated like
the existing move/hold/stop windows. No measured order number is a production
constant. Synthetic tests change the operand and use a different order name.

Human03's `SetupPatrol01` callback issues a patrol location order to each unit in
its selected group (`Human03.j:1997`). Its other endpoint is the unit's accepted
position when the order is issued; the requested destination remains the script's
point. This implementation follows the two-endpoint patrol behavior. The static
identity reads do not establish the native pathfinding or combat algorithms.

## Execution

All four existing point-order forms accept the source patrol identifier: names
or numeric IDs, direct coordinates or locations. Validation and route/animation
preparation precede replacement of the old order. The unit retains both endpoints
and its current leg on the existing activity record; no unit or second world is
created.

Reaching an endpoint reverses the leg. The next motion starts at the actual
arrival time, retaining unused travel time in that projection. Unblocked travel
therefore does not lose distance at turns when projection chunks change. Accepted
spans remain straight waypoint segments with contiguous positions and times.
Routes, occupancy, collision radii, speed, animation selection and pause epochs
reuse the existing readers and runtime.

Patrol uses the existing attack-move acquisition/chase path and retains its leg
through automatic combat. Losing the target resumes that leg; subsequent endpoint
reversals continue. `GetUnitCurrentOrder` reports the patrol intent during those
interruptions. Explicit move/hold/stop, a target order or a successful Holy Light
order replaces it. Guard release sends its existing stop and cancels patrol.
Instant item use and Defend retain the existing movement behavior. Death/removal
discard patrol, while explicit pause preserves it without catch-up. Script
relocation keeps the original endpoints. An active patrol prevents natural idle
sleep, including while its route is blocked.

A blocked leg retains its requested endpoint and retries on later activity passes;
guard AI does not treat that ongoing order as idle wandering. An actual guard
return finishes before patrol resumes. Coincident endpoints stand without a
zero-distance loop. Every route retry/turn draws from the existing work budget;
many cycles in one projection refuse on exhaustion. Failed span output is
truncated under the existing session-stop-on-error contract; world effects are
not rolled back.

## Fidelity limits

Endpoint retention/retry policy, patrol intent during combat, guard interactions,
exact combat-resume ordering and animation phase at reversals are recreation
choices, not original-runtime comparisons. Native turn-rate/acceleration and the
existing blocked-motion partition dependence remain unqualified. No arrival
tolerance, retry timeout or new speed constant is fitted to observations. Queued
patrol routes and a source-derived player command-panel button are not added.
Campaign unit caches still omit transient orders.

## Verification

Synthetic tests cover changed source identities, repeated legs, leftover travel
time, span continuity, chunk composition, all four point forms, pause, refusal
without replacement, stop/move/target-order cancellation, coincident endpoints,
bounded work, acquisition/combat resumption and a blocked guard patrol recovering
after its blocker is removed. These establish the recreation contract.

The selected unchanged Human03 ROC map initializes with three retained scripted
patrol endpoints. The explicit local-file gate checks native order-name round
trip, current orders and stored endpoints. The broader headless gate now finishes
repeated startup, compares roster/unit state and accepts its test move order,
then refuses the first clock tick because blight regeneration lacks terrain
state. That gate exits 101; Human03 is not accepted as playable. No original
client or rendered patrol comparison was run.

Private-target checks: the complete `pcx-play` package passes 1,170 tests with 46
explicit input gates ignored by default. The selected Human01 startup/order/profile
gates and Human03 patrol-startup gate pass. The WASM library check and native
`peacecraft3 --all-targets --features test-fixtures` check pass. These are compile
and host-runtime checks, not rendered or original-client behavior comparisons.
