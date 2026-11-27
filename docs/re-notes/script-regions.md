# Script region requests and subscriptions

## Selected interface and retained state

`ObjectHost::with_regions` admits loaded `CreateRegion`, `RegionAddRect` and
`TriggerRegisterEnterRegion` contracts after rectangle and trigger admission.
`TriggerRegisterLeaveRegion` is supported when its own declaration is loaded.
Signatures, scalar parameter/return types, source identity, no script override
and the selected region/rectangle/boolean-expression ancestry are checked.
`MapStartupBudgets.regions` is an optional caller-supplied `RegionBudget`;
absence keeps these calls unsupported. There is no asset-derived grid size or
native initial state hidden in a default.

Creation uses the same checked identity arena as units, triggers and rectangles.
Each successful `RegionAddRect` retains the actual rectangle identity and its
current finite ordered bounds in an append-only call record. Later rectangle
mutation or removal does not rewrite that historical request. The records are
not a native cell raster or a membership result. Repeated requests are retained
separately under the recreation resource policy. Region count, aggregate
rectangle-addition count, event count and shared identity capacity are bounded
before their corresponding growth; failed calls preserve prior state.

Enter/leave registrations require an actual trigger and region, plus null or an
existing Filter/Condition wrapper. Their event tokens retain the trigger, region,
filter identity and direction. Registration does not evaluate a predicate, run
an action or change the trigger's optional enabled state. Reserved identities
with matching family names are insufficient. No second VM, arena, simulation
world or clock is introduced.

## Evidence boundaries

The selected `common.j` declares the native interface. The selected `Blizzard.j`
`TriggerRegisterEnterRectSimple` helper creates a region, adds its supplied
rectangle, then registers entry with a null filter; the leave helper follows the
same call structure. Supporting those explicit calls does not establish native
cell coverage, boundary inclusion, geometry changes or event ordering.

Synthetic owned-VM tests cover call-time bounds after rectangle movement,
independent and repeated subscriptions, null/filter/condition inputs, disabled
triggers, unexecuted callbacks, invalid and reserved identities, separate budgets,
shared capacity, source/signature/type refusals and an absent leave contract.
The application test uses actual map delivery and the production startup factory;
failed replacement without opt-in or with an exhausted event budget preserves
the previous context, lifetime witness and subscription.

## Unchanged chapter execution

The selected Human01 Program runs through the production factory with the
retained compatibility catalog, qualified startup inputs and explicit verifier
clock origin. Six regions, six rectangle-addition requests and six entry
subscriptions succeed. Every retained rectangle identity and exact `f32` bound
matches its executed constructor; the verifier also checks that no successful
rectangle mutation/removal affects those inputs. No leave subscription is
executed by this prefix; leave support is covered by synthetic tests.

Globals and `config` complete. `main` next refuses `TriggerRegisterDeathEvent`,
map span `184271..184338`, common declaration `81184..81277`. The shared arena
holds 1007 identities, 144 rectangles, 155 units, 33 triggers and 14 condition
wrappers/associations. The earlier explicit disabled state stays assigned, and
the clock does not advance during startup. Main remains incomplete.

Commands, immutable input/source/artifact hashes and outputs are local in
`.scratch/region-root-20261017/`. This is reconstructed script execution and
retention evidence. No new original-client, render or site run was performed.

## Limits at the storage checkpoint

This slice did not qualify removal, rectangle and cell edits, membership, or
event delivery. The later [range-event slice](slice-range-events.md) covers
its registered crossing events. Native region rasterization, repeated
registration identity and rectangle edits remain outside this result. These retained requests alone do not make a trigger fire.
Full campaign execution, rendered gameplay and the 100-consecutive original
baseline remain open. Original files and extracted contents remain local.

## Removed-region subscriptions

`RemoveRegion` now detaches filter references from subscriptions naming that
region. Other regions and triggers sharing a wrapper continue to retain it.
Region polling omits removed regions before reading their live bounds, including
subscriptions whose triggers remain live. Previously, the next poll failed with
`WrongFamily`, and a removed region kept its filter named indefinitely.

The synthetic script regression reproduces the polling failure before the fix.
It removes one of two regions with shared live trigger owners, before and after
membership initialization, then verifies that the surviving region still queues
the expected entering-unit event. A second script regression reproduces the
wrapper-cleanup failure and checks that cleanup succeeds only after the last
region holding the wrapper is removed. Event and arena counts retain their
existing issued-token bounds.

These are recreation lifecycle fixes. Polling emits no synthetic leave event for
retired subscriptions. Original-client removal side effects, queued-event
cancellation and region-cell geometry remain unqualified. The native differential
corpus still needs matched original traces before it can establish tick parity.
Local failing and final validation receipts are in `.scratch/goal-region-removal/`.

Validation used Rust/Cargo 1.100.0 with `RUSTFLAGS=-Dwarnings`. The full
`pcx-play` library suite passed 3,330 tests with 163 explicit ignores. Strict
native library/test Clippy, formatting and the `wasm32-unknown-unknown` library
build passed. These checks launched neither the original client nor a renderer.

## Location-based membership

`IsLocationInRegion(region, location)` is now admitted when its selected-source
contract and location ancestry match. It reads the live location through the
existing location accessor and delegates to the same predicate as
`IsPointInRegion`. Selected-image cell geometry is described below; unqualified
hosts retain the earlier continuous predicate. Moving the location changes the next query.
Moving or removing a rectangle already added to the region keeps the existing
copied-bounds history. The later selected geometry derives its grid from the
image and source map.

Synthetic VM and host cases cover live coordinate updates, copied region bounds,
missing admission, invalid identities, destroyed locations, optional declarations
and source/signature refusals. Missing or unsupported state remains an explicit
error. Native region-cell equivalence, exceptional inputs and tick parity remain
unverified. Validation receipts are in `.scratch/goal-location-geometry/`.

The shared location-geometry validation passed 3,335 library tests with 163
explicit ignores, strict native library/test Clippy, formatting and native/WASM
library builds under Rust/Cargo 1.100.0 with `RUSTFLAGS=-Dwarnings`.
The owned-VM test also checks that the declared location predicate appears in
the public native-coverage report. These checks establish recreation behavior.

The later controlled original-client probe found that this continuous predicate
is incompatible with region membership outside the authored rectangle. Both
point and location queries and polling now share the selected cell policy that
repairs this mismatch in the controlled boundary cohort;
see [observed containment mismatch](slice-regions.md#observed-containment-mismatch).

## Selected rectangle cell membership and clearing

The selected image policy now quantizes rectangle additions and point/location
queries using common world-relative cell rules. Geometric clears use current
rectangle bounds, independent of handle identity. Event polling calls the same
membership predicate. The placement cache applies these clears and replays
ordered edits when created later. Historical addition records stay retained.
All edits, including clears, share the selected mode's aggregate edit budget;
refused edits preserve existing membership. Unqualified hosts preserve their
earlier behavior. See [region cell geometry](slice-regions.md#source-backed-rectangle-cell-edits)
for source validation, the 84-row matched rectangle cohort and open limits.

## Single-cell edits and unit queries

Optional selected-source contracts now admit `RegionAddCell`,
`RegionAddCellAtLoc`, `RegionClearCell`, `RegionClearCellAtLoc` and
`IsUnitInRegion`. The four edit calls require source-backed cell rules and the
admitted world extent. They copy finite coordinates into the existing bounded
edit history and update the same placement and event-membership paths as
rectangle edits, without allocating rectangles or inventing request identities.
Location calls require actual live admitted locations.

Unit membership reads the accepted position, including the retained position
at the removal clock boundary. It ignores visibility and health and returns
false for null arguments or a removed unit after that boundary. Unknown unit
positions, missing removal clocks/positions and invalid families refuse. The current first-cleanup-boundary model
is shared with `GetUnitLoc`; native checks cover immediate removal and a
one-second later sample, not the exact first cleanup frame. See
[cell source relationships](slice-regions.md#cell-native-source-relationships)
and [unit membership](slice-regions.md#unit-membership-and-deferred-removal) for
source, controlled observations, synthetic tests and remaining timing limits.

## Region event response identities and notification ordering

When the selected common source declares `EVENT_GAME_ENTER_REGION` or
`EVENT_GAME_LEAVE_REGION`, region admission now resolves that constant through
its checked `ConvertGameEvent` initializer. The queued thread exposes that
identity through `GetTriggerEventId`, then restores the prior context. The
numeric event identity is never a runtime default. Missing globals retain the
unqualified host's null response; malformed or mutable globals refuse admission.
Synthetic session tests use nonstock negative event values and both directions,
check unit/trigger responses and idle restoration, and reject mutable constants
and null initializers.

Three isolated original-client runs used the selected build 9231 executable,
modified local Human01 input and a private Wine display/null audio sink. The
82-row first run repeated exactly within the 199-row second run; those 199 rows
repeated exactly within the 289-row third run. The final run also checked filter
response context and condition timing. Receipts, modified inputs and observation
exports remain local under `.scratch/goal-native-region-events-12/`,
`.scratch/goal-native-region-events-13/` and
`.scratch/goal-native-region-events-14/`. All three containers were stopped and
their zero process IDs verified. These are controlled Wine observations, not
native Windows or complete campaign acceptance.

Observed behavior in this cohort:

- Registration over a stable existing unit did not fire entry. Creating a unit
  inside after registration did fire entry at the later script boundary.
- Moving outside then inside, or inside then outside, within one callback
  retained both crossings. Final-position coalescing is incompatible.
- Adding or clearing region geometry under a stationary unit emitted nothing.
  A crossing captured before a geometry clear still delivered later.
- Hiding, killing and removing a unit inside did not emit a leave notification.
  Creating inside and immediately removing or hiding still retained its entry.
- Filters ran during the mutations. None ran in the deferred phase. They saw
  no region trigger, event identity or trigger-unit response in this timer
  callback cohort. Conditions and actions ran later. Enter/leave action contexts
  matched their declared event constants, triggers and unit responses.

The current recreation polling path still differs: it seeds at first poll,
compares only final positions, excludes hidden/dead units, turns geometry edits
into crossings and evaluates filters during queued delivery with trigger context
already installed. The event-identity repair does not fix those ordering and
membership-history errors. Mutation-time notification capture and filter
continuations must cover script unit creation/movement and projected physical
movement, preserve every crossing, and defer conditions/actions. Nested filter
mutations, disabled/destroyed registrations, sleeping callbacks and the exact
native dispatch tick still need qualification. Tick identity remains open.

Validation passed 3,366 library tests with 163 explicit ignores, strict native
library/test Clippy, formatting and native/WASM library builds with warnings
denied on Rust/Cargo 1.100.0. The unchanged Human01 production-startup test also
completed two matching 60-second no-input simulations; the source hero retained
650 life, the clock reached 60 seconds and neither run requested an exit. This
is recreation repeatability, not an original-client tick comparison. Private
commands, logs and source/artifact closure are in
`.scratch/goal-region-event-context/`.

## Mutation-time script region notifications

Owned-VM calls to the supported creation, position, visibility and revival
mutators now capture region transitions at each accepted call. Registration
seeds the units currently inside immediately. Subsequent script births are
therefore observed before any boundary poll, and an out-and-back script move
retains both events. Captured notifications keep their unit and source event
identity even when later geometry edits or removal change current membership.

Filters run on the existing native-callback stack before the mutator returns.
`GetFilterUnit` uses the innermost region or rectangle predicate. Region filters
do not install the later trigger/action context. Accepted notifications queue
without another filter evaluation; conditions and actions remain deferred.
Nested filter mutations restore the prior predicate unit. Callback failure
unwinds predicate context while preserving the already accepted movement.
Pending region predicates share the existing arena-sized event queue bound.
The startup host prepares its source-backed placement before an intercepted
first creation, preserving the normal lazy-loader contract.

Rectangle and cell edits silently refresh membership for the affected
subscriptions. Hidden and dead retained units remain eligible for containment;
removal alone no longer generates a leave notification. The former removal
regression expected such a leave and has been replaced with a real movement
leave after silent removal, matching the controlled native cohort.

The published-session regression covers nine cases: out-and-back movement,
creation after registration, creation followed by removal, movement followed by
clear, addition followed by movement out, clear followed by movement, hiding
inside, filter rejection and nested filter movement. It checks synchronous
filter counters, deferred conditions/actions, event identities, ordered action
traces, idle restoration and absence of duplicate delivery on the next poll.
A separate owned-VM regression checks failure cleanup and retained position.

This does not complete spatial event parity. Direct host calls and physical
movement still use boundary delivery for filters, and physical movement is
sampled at its final position rather than every accepted span. Special births,
revival, hide-driven relocation, disabled/destroyed subscriptions, nested native
ordering and the exact dispatch tick need dedicated original-client acceptance.
The synthetic nested case proves callback isolation, not native nested ordering.
Native `SetUnitX` and `SetUnitY` are not admitted by this implementation.

A further original-client check replaced the axis setters with
`SetUnitPosition`. The first variant changed the initial placement and failed
73 of the existing controls; its results are retained as mismatched evidence in
`.scratch/goal-native-region-events-15/`, not accepted as a cohort replay.
The controlled follow-up retained the initial axis placement, disabled unit
pathing and changed only the tested moves to `SetUnitPosition`. All 289 rows
matched the earlier axis-setter run, including containment controls, immediate
filters, deferred conditions/actions, contexts and intermediate crossings.
This result is specific to that pathing-disabled placement cohort; it does not
qualify collision-aware placement. Receipts remain local in
`.scratch/goal-native-region-events-16/`.

Final validation passed 3,368 library tests with 163 explicit ignores, strict
native library/test Clippy, formatting and native/WASM library builds with
warnings denied. The unchanged Human01 production Escape test also passed,
restoring control/interface and running the real footman region callback.
Commands, logs, input hashes and the source/artifact closure are retained in
`.scratch/goal-region-mutation-events/`. No original input or generated copy is
published. The two native containers were stopped and their zero process IDs
verified.

## Region events during patrol projection

Activity movement now checks region membership after each accepted movement
endpoint, including a partial step stopped by collision. This preserves an
entry and a leave when a patrol reaches an inside waypoint and returns outside
during one projection. Each registration checked consumes projection work.
The boundary scan shares the same membership state and does not emit a second
notification for those transitions.

The synthetic patrol regression exercises movement with and without pathing.
Filters on these events run during queued delivery before trigger context is
installed. They use the predicate callback stack, preserve the outer predicate
unit during nested mutations and release that context on failure. The session
regressions cover null trigger/event/unit responses during filtering and a queued
filter that moves another unit. Straight segments
with both accepted endpoints outside can still pass through a region without
producing a notification.

## Physical movement filter phase observations

A controlled flying-unit probe recorded 40 ordered rows with no export overflow.
All three move orders were accepted. At the first entry filter, the other two
units already exposed their updated positions. The three entry filters completed
before any entry condition or action. A stop issued in one entry predicate held
that unit at its entry position through the final sample. Another entry predicate
changed its unit's X coordinate. Its entry action observed the changed coordinate,
and a leave predicate ran on the following movement update.

The probe selected its flying unit from the supplied UnitData movement column.
Private inputs, generated scripts, exported decimal timer/position responses and
control assertions are retained under
`.scratch/goal-native-region-physics-20/`. The preceding ground-unit run accepted
its orders but routed outside the test region, so it did not qualify crossing
ordering. Both runs have stopped-container receipts. These observations cover one
controlled unit cohort and leave cross-unit traversal order and exact tick
acceptance open.

Queued delivery now filters a contiguous region-event batch before invoking
its conditions/actions. Region predicate context stays active only during the
predicate. Accepted events return to the queue if a later callback fails. The
session regression checks two queued predicates, nested unit mutation, unchanged
condition/action counters during filtering and subsequent ordered actions.
Physical projection still needs to yield between movement updates and filtering
so a stop or teleport changes subsequent movement within a larger advance.

The bounded-work retry case exhausts delivery work after one predicate accepts
and before the next predicate starts. The accepted event remains pending and
runs on retry without repeating its predicate. The final tree passed 3,369
library tests with 163 explicit ignores, strict native library/test Clippy,
formatting and native/WASM library builds with warnings denied. Final commands
and logs are retained in `.scratch/goal-region-filter-phase-final-retry/`.

## Delivery at selected physics boundaries

Pending physics now delivers queued region events before completing its visual
continuation and advancing to another physics update. This uses the existing
training/mirror callback boundary. A region predicate can therefore stop a unit
at the accepted entry point before the next movement update. Predicate failure
marks the pending physics continuation as failed, preserving prior effects and
preventing callback replay.

The selected-arithmetic session regression uses synthetic flying units and a
synthetic physics cadence. Its stop case advances one second and checks one
filter, one action, an inside stop coordinate, unchanged subsequent position and
an empty current order. Its failure case checks the retained position, pending
physics, absence of actions and refusal on retry without repeating the filter.
The fixture reads PCX_WAR3_EXE and used a 16 MiB test thread stack. Commands and
logs are retained under `.scratch/goal-region-physics-boundary/`.

Intermediate timer projections, legacy clock behavior, cross-unit traversal
order and complete native tick agreement remain open.
