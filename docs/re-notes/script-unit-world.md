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
`BlzGetUnitCollisionSize` reads another unit virtual method. Further selected
class decoding links that method to a rawcode-keyed unit-type entry. Creation
passes through an engine factory before exposing the JASS handle.

Twenty-one source windows and the selected virtual-call relations are rechecked
against the complete image. These are bounded windows, sometimes including
subsequent functions, not complete function/caller graphs or runtime observations.
No decoded operand or measured collision size is a production constant.

## Catalog collision and creation path (2026-10-18)

Selected MSVC class metadata links the `CUnit` position, collision and initializer
methods. The collision method looks up the unit rawcode and reads its unit-type
entry. The supplied `UnitMetaData.slk` maps the collision field to the `unreal`
column `UnitBalance.collision`. Production SYLK parsing of the supplied footman
row agrees with both footmen's collision queries in the retained range run.
The separately extracted base and expansion table members are byte-identical;
that equality does not identify their archive releases or general priority.

The recreation now admits a declared `BlzGetUnitCollisionSize` alongside the
existing catalog-backed `CreateUnit`. It reads the same retained definition by
metadata source and column, including a correctly typed map override. It neither
opens another archive nor resolves a replacement definition. Missing, ambiguous,
indexed, nonnumeric, nonfinite and negative geometry refuse with the selected
native's source context. Omitted declarations remain unsupported; present ones
must have the loaded signature and share the creation declaration's source.
This is catalog geometry; runtime collision mutation and native clamping of
unusual map overrides remain unqualified.

The CPU hotpath change retains this derivation on each executed unit at creation.
It stores either the exact catalog real or its validation error; callers still
receive errors only when querying geometry, in their own source context. The
definition and provenance remain retained. Combat and motion reuse the same
result, without a second catalog resolution or repeated metadata walks. No
runtime collision setter is admitted; admitting one must also update the retained
value. The [CPU capture](../performance/cpu-hotpaths.md) records the unchanged-map
work-count comparison and its deliberately failing repeated-scan mutant. This
preserves recreation behavior, without adding a native-fidelity claim.

The creation initializer also reaches a position-adjustment method before the
position is stored. One branch passes the unit collision method's result into a
spatial search; another takes a distinct query path. A separate nullable record
can adjust the coordinates again. These relations explain why bypassing
placement is unsafe, but do not establish the search order, blocker policy or
the producer of that optional record. A bounded decoder still leaves indirect
switches unresolved in the larger initializer. No placement algorithm is copied
or inferred from those incomplete graphs. Rechecked bytes, table comparisons,
refused source/geometry mutants and validation receipts remain local under
`.scratch/unit-placement-root-20261018/`.

Validation passes 448 library tests both natively and through the WASI runtime
(six explicit local-input ignores), plus 107 map-loading integration tests
(one explicit ignore). Native and browser-WASM all-target Clippy, the browser
library build and formatting checks pass. These do not establish accepted
placement or rendered gameplay; no fresh browser package or reference run was
needed for this catalog query.

## Creation, position setters and occupancy probe (2026-10-18)

A subsequent fresh isolated run retains the original Human01 `main` and
`config`, with reversible instrumentation after `InitBlizzard`. Nineteen
synthetic position requests each create a footman, call `SetUnitPosition`, then
call both axis setters before removing that probe unit. Three requests have
fractional coordinates and one repeats an earlier request. All 57 operation
records, the bounds record and completion markers are retained. This is map
initialization during loading, not a movement or frame-update experiment.

- Creation and `SetUnitPosition` return identical positions for all 19 requests.
  This agreement does not establish equivalence for other unit classes, existing
  states, removal timing or out-of-map positions.
- The axis setters preserve every requested coordinate in this sample, including
  fractional inputs. They do not apply the observed placement adjustment.
- The queried world bounds match the parsed terrain origin and complete grid
  extent under the scale read from the selected coordinate-normalization code.
  No camera/playable-area restriction is substituted for those bounds.
- The source corner's low final-byte nibble matches all 114 requested/returned
  cliff-level queries, covering three distinct values, with the selected
  routine's nearest-corner indexing. This is a source/runtime join for these
  points, not a qualification of other terrain versions or platform overlays.
- One unchanged creation position has a true `IsTerrainPathable` result for
  walkability. That predicate cannot directly substitute for creation's
  acceptance test, just as `IsUnitInRange` cannot substitute for range-event
  membership.

Further bounded decoding resolves the placement search's four-way dispatch.
It tries the initial point and can expand integer bounds through a mode-selected
perimeter scan. Its grid test combines stored cell flags with occupancy records;
another callback checks a terrain/overlay-derived domain. The footprint loader
reads table-selected image data and builds a cell mask. These are selected code
relations, not a complete live-world model.

A private replay using the terrain raster alone reproduces the first retained
range probe's anchor but fails its other creation. Adding nearby source-derived
stone-wall footprints reproduces both. Testing that same limited model against
the new 19-request probe matches only 14 requests. The misses are preserved:
other placed footprints, executed unit occupancy, terrain acceptance masks,
cliff/overlay rules, rotation and lifetime still need qualification. In particular,
the limited replay must not become a production placement default or a silent
fallback for missing geometry.

Eight mutated logs are refused. The container is stopped and preserved, and its
reviewed capture shows the original loading screen. Probe/member hashes,
reversible edits, isolation, source checks, observations and replay mismatches
remain local in `.scratch/unit-placement-probe-root-20261018/`. No production
placement, range-registration behavior or recreation rendering changes in this
experiment. The unchanged-map four-result RNG factory gate remains open.

## Source placement replay and fresh repetition (2026-10-18)

Further selected-code analysis resolves the movement-mask producer that was
missing above. The unit defaults bind the `movetp` column; the unit-type cache
decodes that source token through a selected string table, then indexed switches
produce its outgoing and incoming cell masks. The footman row parsed from the
supplied table selects the corresponding cases. The masks and coordinate scale
are decoded from the selected executable for this analysis, not copied into
production constants. The terrain predicate separately maps the source-declared
pathing type to its static cell mask and returns whether that mask blocks the
point. It does not include the placement solver's complete occupancy test.

Two additions explain the limited replay's remaining misses. First, the selected
creation call supplies a finite search bound. If the initial point and permitted
perimeters fail, the native path retains the requested coordinates. That explains
the unchanged point whose terrain predicate reports blocked; it does not permit
a fallback when recreation geometry is missing. Second, source-selected placed
object textures, their death-texture alternatives, and one explicitly selected
script-created building contribute footprints. The building is identified by
its actual source global and executed call, not a rawcode/placement-ordinal join.
The private replay with these inputs reproduces all 19 earlier requests.

A fresh isolated reference run then repeats those same 19 requests against a
saved prediction, with an additional query of that source building. Its 61
records reproduce all 19 creation positions and confirm the building's rawcode,
position and catalog collision. All 114 requested/returned terrain-pathing
queries agree with the parsed WPM mask, and all 114 cliff queries still agree
with the source corner nibble and selected indexing. These are repeated
coordinates, not an unseen-coordinate validation or a proof across maps.

Verification rechecks 13 new bounded code graphs (259 source blocks), directly
decodes the relevant switch cases, and refuses eight additional mutated logs
alongside the previous eight. Other indirect branches and outside graph tails
remain recorded. Source identities, extracted texture data, the saved replay,
reversible script edits, isolation, stopped-container status and checks stay
local under `.scratch/unit-placement-policy-root-20261018/` and
`.scratch/unit-placement-validation-root-20261018/`.

The replay remains private. Automatic placed-object activation versus explicit
script creation, asymmetric texture orientation, non-cardinal rotation, complete
dynamic occupancy and lifetime, platform overlays and out-of-map queries are
not qualified. Loading every descriptor into another arena could duplicate
script-created objects and is not an accepted world model. No production live
placement or range consumer changes here. The capture remains the original
loading screen; recreation rendering and the unchanged-map RNG gate remain open.

## Opt-in static terrain predicate (2026-10-18)

`ObjectHost::with_terrain_pathing` now admits the loaded `IsTerrainPathable`
declaration against the map context already owned by its player host. The
startup factory accepts the same optional input. Omission keeps the native
unsupported. Admission checks the declaration signature and the admitted
`ConvertPathingType` source; queries use that arena's symbolic selector and
the existing context's WPM raster rather than another map or world copy.

`terrain_pathing::TerrainPathing::read` reads the cell scale and selector masks
from caller-reviewed executable windows. Complete-image and window fingerprints,
source identity, byte budgets, executable sections, branch shape, indexed branch
targets and constant-result handlers are checked. The existing selected
normal-real subtraction helper supplies coordinate differences before raster
indexing. No measured scale or pathing mask becomes a production constant.

The query reports whether the selected mask blocks the authored cell. It creates
no handles or occupancy state and does not admit terrain mutation. Nonfinite or
unsupported arithmetic, wrong argument families and missing cells refuse.
Coordinates outside the raster also refuse explicitly: their native edge
behavior is not yet qualified. This is a static terrain predicate, not an
accepted-position, live occupancy or range-membership implementation. Numeric
handle tokens retain the existing host-local arena contract.

Validation passes 454 library tests natively and in the WASI runtime, with six
explicit local-input ignores, and 107 native map-loading integration tests with
one explicit ignore. Strict native/browser-WASM Clippy, native/WASI/browser
library builds and formatting pass. The actual startup factory reads the
selected original image and admitted Human01 context; its new query agrees with
all 114 retained reference observations in both native and WASI executions.
Those full-map runs still stop at `TriggerRegisterUnitInRange` with one of the
required four RNG checkpoints. Their failing gate exits are retained, not
reported as successful startup. Receipts and local factory inputs remain in
`.scratch/terrain-pathing-root-20261018/`. No fresh browser rendering or full
campaign parity is claimed.

## Automatic placed objects and the map's unit stream (2026-10-18)

Selected MSVC class metadata links the placed-object reader's virtual predicate
to `CDoodads`. Its normal-game branch consults the destructable database and
the authored flags: some rows create game destructables through a registered
callback, some remain generic world objects, and script-created rows wait for
their source calls. The callback reaches the live/dead destructable constructors;
the authored Z flag is separate from the life selector. This replaces the
earlier assumption that every descriptor could be loaded the same way.

A fresh reference census brackets the original `CreateAllDestructables` call.
The saved source prediction matches all 1,422 automatic destructables by their
complete type/position multiset. They retain the same handles, life and maximum
life after the call. Five new handles match the five explicit source globals,
bringing the count to 1,427. Four initially dead automatic destructables agree
with their source life bytes. The complete census has 2,858 records and refuses
eight mutated logs. This qualifies that stock-map initialization branch, not
editor mode, arbitrary custom metadata, enumeration order or later lifetime.
The canonical recreation world must avoid loading those five script objects
twice and must keep any automatic actors in the same owned authority.

A separate corrected probe tags the map's direct `CreateUnit` call sites by
their original source spans and records construction inside `CreateAllUnits`.
Its 312 complete records cover 155 distinct handles, 155 source call sites and
53 rawcodes. This is an executed call stream, not a placement-ordinal identity
join. The getter positions differ from the helper's post-native argument
snapshots in 100 cases. All reported positions lie on a quarter-unit grid;
146 cases match flooring those snapshots to that grid and nine have additional
displacement. That observation does not establish an intentional quantizer or
permit a copied coordinate constant in production. The cause must be derived
from the selected coordinate path before implementing it.

There is another unresolved numeric distinction: 129 of 465 logged x/y/facing
inputs differ from a standard decimal-to-binary32 literal cast. Because these
helper arguments were read after native creation, that trace alone did not
isolate literal decoding, parameter writeback or formatting. The subsequent
numeric probe below narrows that distinction.

The first unit-trace attempt produced no records and a native error dialog;
moving the wrapper definition before its first use allowed native execution.
The second attempt completed 155 creations but clipped every long observation
at 259 characters. Both attempts remain rejected datasets. The accepted third
attempt uses two records per observation, checks their payload bounds, and
refuses eight mutated logs. All reference containers are stopped and preserved.
Input/member hashes, bounded code checks and receipts remain local under
`.scratch/unit-world-activation-root-20261018/` and
`.scratch/unit-startup-placement-root-20261018/revision-3/`.

No production placement or range consumer changes here. Complete live
occupancy, walkable-model overlays, rotation and lifetime remain open, as do
the unchanged-map four-result RNG gate and recreation rendering.

## Creation-position cancellation (2026-10-19)

A bounded debugger observation of two instrumented Human01 initialization
requests follows the actual node writes and all four public coordinate returns.
The first request has an exact binary fraction and passes the search unchanged;
the second reaches a different point through the placement search. The native
input words are checked before creation, independently of script formatting.

The node constructor at `0x8baf10` loads its initial normalized coordinate from
the selected image through the operand at `0x8baf2d`. That initial value is far
outside the map. Both observed scales and the initialization-time scalar are
zero. The position setter at `0x800280` subtracts the source terrain origin and
adjusts the exponent by the amount encoded in its normalization instructions.
Its update path computes `sub(cell, base)`, then `add(base, delta)` through the
selected real helpers. The getter applies the inverse exponent adjustment and
adds the terrain origin.

Emulating those selected instructions reproduces both axes of all four observed
position updates and all four public getter words exactly. The first fractional
point loses its fraction in the cancellation through the constructor's large
initial coordinate. The second point is already displaced before that update.
The first case therefore explains a quarter-grid result without adding a
quarter-floor operation. The previous inference that each intermediate had an
in-map magnitude was incorrect: the constructor's initial coordinate invalidates
that bound. This does not prove one fixed rounding grid for every state or map.

The arithmetic regression uses synthetic operands, including a case where
ordinary host rounding and the selected helpers differ. No measured constructor
value or terrain origin is copied into production. A future world authority
must derive the initializer from the selected source and preserve the ordered
arithmetic; directly storing the request or simplifying the cancellation loses
observable behavior.

The successful trace contains 52 scoped records. Three earlier attempts remain
rejected: a direct debugger launch reached no creation, one observer incorrectly
required the position wrapper before a constructor write, and another exhausted
its limit on repeated internal scalar reads. The final observer samples those
reads only through the initial update and scopes getter reconstruction to the
public query. It completes within the original event and time limits, detaches,
and stops its private container. Eight changed records are refused by the
checker. Source/member identities, reversible instrumentation, static windows,
selected-instruction replay and captures remain local under
`.scratch/unit-position-normalization-root-20261019/`.

The existing production real helpers reproduce all eight stored axis words and
their eight reconstructed world-coordinate words in standalone native and WASI
executions using the same captured input. The 522 native library tests pass
(seven explicit local-input ignores), as do the five real-arithmetic tests in
the actual WASI runtime. Strict native/browser-WASM Clippy, native/WASI/browser-
WASM library builds and formatting pass. These checks qualify arithmetic; they
do not execute a recreation live-position consumer or render the mission.

These are intrusive Wine observations during map loading, not a native Windows
or running-frame equivalence claim. Full occupancy, live movement, rectangle
enumeration and range delivery remain open. The unchanged Human01 startup still
stops at `GroupEnumUnitsInRect`; the exact caller is corrected below. This
evidence alone does not make the mission playable.

## Full creation stream and rectangle caller (2026-10-19)

An exact slice of the selected Blizzard source at the reported failure span
identifies `GetUnitsInRectMatching`, reached through `GetUnitsInRectAll` in the
authored map-initialization trigger. The executed rectangle handle matches the
map global `gg_rct_Starting_Forces` and its executed bounds. The earlier
`NudgeObjectsInRect` label was incorrect; that function is at a different source
offset. Its repositioning callback is not the immediate blocked caller.

The source-derived literal decoder and constructor cancellation reproduce 146
of the 155 retained original creation positions without a placement search.
The remaining nine need collision displacement. The actual recreation Host
resolves all 155 creations to 53 catalog types; their movement inputs use two
moving-unit tokens, and their buildings name six pathing textures. All six
textures are present in the supplied local archives and decode through the
production TGA reader. Archive identities remain separate from the compatibility
catalog's identity.

A private cell-occupancy candidate combines the source terrain/pathing grid,
placed-object footprints, the executed building footprints, creation order and
the selected real normalization. It reproduces all 155 retained creation
positions and all 21 additional requests from the separate nineteen-request
probe and the recent two-request probe. The previously omitted moving-unit
occupancy matters: without it, only 149 creation positions match. A circular
overlap candidate also matches only 149, with a different set of misses. Neither
alternative's failures are discarded. Center containment of the matched
positions gives a nonempty starting-forces selection; original enumeration
order, boundary and filter behavior still require their own checks.

The broad private startup verifier also had an independent omission: it left
the selected numeric policy absent and its observation wrapper did not forward
`real_ops`. That version agreed with only 80 of the 155 exact coordinate pairs.
The corrected verifier supplies the reviewed policy, forwards it to the actual
VM, checks that it is present, and matches all 155 argument pairs exactly. It
retains the same explicit refusal at the rectangle enumeration. The production
Host and recording/localization wrappers already forward this policy; no
production arithmetic change is needed for this verifier correction.

This is a candidate for initialization, not an installed live world. The small
cell-query shape, including radius-boundary cases, footprint alignment/rotation,
search bound, full terrain/platform domain, removal and later movement still
need qualification or explicit implementation limits. In particular, the
candidate's successful initialization replay must not be presented as proof of
running occupancy updates. The implementation must retain the actual executed
handles in one authority and update that authority when objects move or die.

Source/member fingerprints, the frozen actual-Host executions, texture decode,
all model comparisons and deterministic work counts remain local under
`.scratch/campaign-unit-world-root-20261019/`. No new original-game run, playable
mission, native Windows comparison, recreation rendering or browser parity is
claimed by this replay.

## Numeric input isolation (2026-10-18)

Two further fresh reference runs compare 32 selected decimals before any probe
unit creation. References are constructed from exactly representable signed
integer mantissas and repeated scaling by integer two. The second run extracts
scaled integer values with `R2I`; all 32 constructed references recover the
expected integer. This avoids treating decimal formatting or real equality as
an exact representation check.

Twenty-six compiled literals differ from standard nearest binary32 conversion.
Twenty-four selected map literals differ by one step toward zero; the positive
and negative `0.1` controls differ by one step away from zero. `S2R` agrees with
the constructed reference in the other 30 cases, including the 24 selected map
literals. These differences occur before the probe's native creation calls;
they cannot be attributed solely to creation's argument writeback or logging.
All eight later creation calls preserve their before/after formatted arguments.
That observation covers these inputs, not every native or possible argument.

Real equality reports true for all 26 differing literal/reference pairs, while
ordered comparisons and integer comparisons distinguish them. False controls
and distant-value checks also execute. The equality tolerance and complete
literal-conversion algorithm remain unqualified; an assumed epsilon or blanket
round-toward-zero conversion would not follow from these observations.

The runs retain 82 and 146 complete records, with all common observations
repeated. Eight mutated logs refuse. Six selected-code graphs and 50 blocks are
rechecked, including `S2R`'s decimal parser and integer conversion helpers; they
do not yet identify the compiler's literal-conversion path. Both containers are
stopped and preserved. Receipts remain local under
`.scratch/script-real-input-root-20261018/`. That isolation checkpoint did not
change execution; the selected policy below addresses the qualified operations.

## Selected numeric execution (2026-10-18)

The selected compiler converts decimal whole and fractional parts separately,
using its integer-to-real, division and addition helpers. Replaying that code
matches all 32 scaled-integer controls and all 465 formatted constructor inputs.
The selected VM uses a strict image-derived threshold for real equality, but
compares stored representations for real inequality. A fresh 14-record probe
checks 72 results at the threshold and adjacent values, including signed zero;
eight mutated logs refuse. The VM also checks a zero divisor before entering
its shared arithmetic helper.

`NumericPolicy::read` now binds those reviewed roles in one fingerprinted image,
checks their call relationships and bounded windows, and reads the reciprocal
lookup data and equality threshold from that image. Neither the measured table
nor a copied tolerance is a production constant. Edition/version/source remain
explicit, independently of the map and archive identities.

`MapStartupInputs.numeric` admits the policy into the existing `ObjectHost`.
The same authority reaches the VM through startup, neutral, recording and both
localization wrappers, including parked expressions. Decimal literals, `I2R`
and real arithmetic/comparisons use that policy when supplied. An unbound VM
retains its ordinary host arithmetic; it is not evidence of selected native
behavior. Unsupported spellings and number classes report their source context.

Native and actual WASI checks cover 32 exact literal representations, 465
formatted source inputs and 72 comparison results through the owned VM. The
unchanged original-map factory also verifies all 155 constructors' numeric
arguments. It still refuses `TriggerRegisterUnitInRange` and fails the required
four-result RNG gate at one result. Correct creation arguments are not accepted
live positions or a world identity join.

Validation passes 457 play-library cases on native and actual WASI (six explicit
local-input ignores), 112 native script cases and 111 WASI script cases (three
explicit ignores each), plus 107 native map-loading cases (one ignore). Strict
native/browser Clippy, affected library builds and formatting pass. Receipts
remain local under `.scratch/script-numeric-policy-root-20261018/`.

This does not add `S2R` execution, qualify non-normal arithmetic or complete
implicit coercion of declared real storage and parameters. The later
[native real-argument boundary](campaign-presentation.md) converts integers for
declared native real parameters, preserving the current VM callback path;
storage and user-function coercion remain open. Live occupancy,
placement, range delivery, the original startup gate and rendered fidelity
remain open.

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
acceptance run. No world-id adapter, copied position/collision constants or
assumed polling interval is added. Live placement, session-scoped world identity,
world-driven event delivery and the required four-result RNG factory gate remain
open.

`ObjectHost::with_unit_in_range` can retain the executed trigger, unit, finite
range and nullable filter. The call returns an event and does not move the unit
or run the filter. Omitting the budget leaves the native unsupported, so an
unchanged factory still stops there. Registration is not range delivery.

## Deferred range consumer (2026-10-20)

Normal new-game preparation now admits a bounded range consumer as well as its
registration budget. Its event identity comes from the selected common's typed
conversion global, checked against the registration source. The runtime reads
accepted positions and catalog collision radii from the actual executed unit
records. It does not substitute creation arguments, `IsUnitInRange`, map-placement
ordinals or a separate world/VM.

The reconstruction samples a candidate-collision circle at explicit logical
ticks. Its radius is the executed range plus the candidate's catalog collision
radius; the anchor's collision radius is not added. This distinguishes the two
retained boundary cases. Inclusion at exact equality, other eligibility rules,
native spatial ordering and update cadence remain unqualified. The implementation
uses a registration/creation-order snapshot and samples final positions, so an
out-and-back change between samples does not invent an entry. Negative finite
registrations are still retained, but delivery refuses their unqualified geometry.

Registration never runs the filter. The simulation tick samples membership,
then executes new entries through the owning VM. Startup itself does not flush
entries at zero game time, and binding/loading time advances no logical ticks.
The first candidate incorrectly flushed on completion of `main`; that caller
was removed after comparison with the retained nonzero-time initial delivery.
No measured delay constant replaces the still-open native cadence.

The filter sees the actual candidate through `GetFilterUnit` and `GetTriggerUnit`,
with null triggering trigger and event. Accepted filters establish the owner and
selected range event for conditions/actions; the candidate stays visible through
both getters and nested rectangle predicates restore it afterward. The optional
`GetEnteringUnit` declaration is signature/source checked. Selected native getters
show that it gates on the same context field as `GetTriggerEventId`, then reads
the same unit field as `GetTriggerUnit`: it is null during the range filter and
exposes the candidate during range conditions/actions. This implements no region
event producer or general unit-event context.

All registered conditions run before deciding whether to run actions, preserving
the retained trigger evaluator behavior even when an earlier condition is false.
Trigger lifetime/enabled state is rechecked after filter effects. Destruction or
disablement suppresses actions; a retained registration can still run its filter.
The original post-destruction observation covers only one later entry, so complete
native cleanup timing is not claimed. Callback-list mutation uses snapshots.
New registrations/births during dispatch join a later sample. False filters still
join geometric membership; native reevaluation of such filters remains open.

Membership/pending-entry storage and scan/callback work are bounded. A failed
scan preserves the previous membership and generation. Entries carry an arena
witness and generation rather than trusting coincident handle numbers. An active
timer delivery or unfinished range callback cannot be polled over. Callback
errors preserve earlier effects, clear context and do not replay the same edge.

The native suite passes 604 Host, 26 library, 154 map/application and 419 binary
cases. Ten range and four trigger cases execute under WASI; strict native/browser
lint and both full application builds pass. Tests cover shared globals, deferred
and repeated entries, new births, nested predicate context, disabled/destroyed
owners, false filters/conditions, callback failures, work limits and stale tickets.
The application tick test matches state at the same final logical tick under
steady and uneven frame chunks, including retained tick debt and excluded loading.
The actual Cargo-built browser-target test module also executes under Node:
three range cases and fifteen related timer/direct-trigger/runtime regressions
pass. This is WASM CPU execution, not browser rendering. The normal placement
loader still matches all 155 retained original creation requests and accepted
getter bits in the separate creation-cut diagnostic; observed positions are
expectations only.

Unchanged Human01 through the actual application now gets past range registration
and refuses `IssueImmediateOrder` at selected Blizzard.j line 7817 during map
initialization. It publishes no session. The authored initialization issues
`holdposition`; the existing Defend-only immediate-order branch cannot implement
that world activity. Live movement, removal/death, collision-cache updates,
startup suspension, actual order/AI/combat consumers and first-mission playability
remain open. There is no new original run, rendered/audio comparison or complete
original/native/WASM simulation claim. Local checks and rejected attempts are
under `.scratch/campaign-range-root-20261020/`.


## Creation placement on executed identities (2026-10-19)

`ObjectHost::admit_unit_placement` now installs an explicit initialization collision
cache before the first executed unit. Each successful birth stores its accepted
position on that same `Entry::Unit`; `x` and `y` still retain the creation request.
`GetUnitX`, `GetUnitY` and the supported rectangle query consume that record.
There is no placement-ordinal, rawcode or external world-id alias.

The selected complete-image fingerprint and reviewed instruction windows supply
the cell exponent, constructor coordinate, radius thresholds, integer-initialized
geometry scalars and ground mask. Static collision cells, terrain-domain labels
and decoded footprint offsets arrive with explicit selected-source identities.
The module does not select archives or substitute missing textures. Source real
arithmetic preserves the constructor's position cancellation. Both occupancy and
footprint updates wait until placement, work/allocation budgets and the actual
unit allocation succeed. A refused birth does not stamp a phantom occupant.

This is an opt-in creation slice. The private caller installs the derived cache
at the first `CreateUnit`, after earlier stationary creations. Later destructable
creation refuses because this admission cannot rebuild stationary occupancy.
Movement, death, removal and overlapping-footprint removal are not implemented.
Only the admitted ground/nonmoving classes, in-map normal arithmetic and canonical
quarter-turn unit footprints are supported. Deterministic perimeter search has
an explicit query budget; its failure behavior is not qualified as the native
finite-search policy. Platform coverage and general footprint geometry remain
open. The ordinary application does not assemble this cache automatically yet.

`GroupEnumUnitsInRect` reads accepted positions and preserves actual unit
identities. The later predicate slice below now clears a populated destination
and executes supplied predicates. Centre containment and creation order remain
explicit recreation choices; native edge/order equivalence is still open.

The helper's subsequent `DestroyBoolExpr(null)` is now a source-qualified no-op
when that declaration is present with the matching source/signature. The selected
functional wrapper at `0x48f310` calls `0x4810e0`, whose zero-input branch returns
zero; the wrapper then returns without destruction. Non-null expression
destruction remains unsupported, and null cleanup neither releases a wrapper nor
changes its callback or the arena count.

The final actual-Host execution matches all 155 retained original creation
rawcodes, argument pairs and accepted coordinate pairs exactly. Four null-filter
rectangle helpers return successfully; the starting group's first member is the
same handle held by the authored Arthas global. The next refusal is the filtered
query in `GetUnitsInRectOfPlayer` at selected Blizzard span `199211..199272`.
Its loaded predicate requires `GetFilterUnit` followed by the existing ownership
query, so callback candidate context is the next implementation dependency.

Validation passed 529 native library tests, with seven local-input tests still
explicitly ignored, and twelve actual WASI tests: four placement tests, two
shared-identity/refusal tests, null cleanup and five real-arithmetic tests.
Strict native/browser lint and both application library builds passed. The
155-pair original comparison is a native Host run; the WASI runs use synthetic
placement inputs. The 21 earlier control requests remain private model evidence.
No new original-game run, rendered comparison, complete cross-runtime simulation
parity or playable campaign is claimed. Immutable checks and rejected attempts
remain under `.scratch/campaign-unit-world-root-20261019/live-world-1/`.


## Player-owned enumeration (2026-10-22)

The selected `common.j` declares `GroupEnumUnitsOfPlayer(group, player,
boolexpr)`. Its `Blizzard.j` helpers `GetUnitsOfPlayerMatching` and
`GetUnitsOfPlayerAll` create a group, enumerate it and destroy the expression.
An ordinary-input route through unchanged Human01 reached this previously
unsupported enumeration at requested tick 28475, after the ledger reward.

The host now reuses the rectangle/selection predicate pipeline for this query.
Candidates are extant executed units whose current owner equals the validated
player. Hidden and dead unit records remain candidates; removed tombstones do
not. Ownership transfers affect the next query. The destination is replaced,
and `Filter` and `Condition` callbacks execute on the same owned VM, with nested
`GetFilterUnit` context restored on return or error. Null filters require no
callback. Null or invalid players refuse explicitly; their native behavior is
not qualified. The optional declaration must match its loaded signature/source
when placement is admitted.

Creation-order snapshots, including eligibility fixed before predicates run,
are recreation choices shared with the existing enumeration path. Native
enumeration order and mutation during predicates remain unqualified. Filtered
queries preserve earlier callback effects if a later callback fails. Unfiltered
replacement preflights membership capacity before clearing the destination;
this also corrects that ordering in the shared selection path. No RNG draw or
gameplay allowance is added.

Synthetic tests exercise current ownership, hidden and removed identities,
nested predicates followed by real `ForGroup` callbacks, refusal without losing
the previous group, and native-specific predicate diagnostics with context
unwinding. These do not establish native or rendered equivalence.

The retained unchanged-Human01 controller now passes the former query refusal
and reaches requested tick 29361 (step 425), where shared unit activity work is
exhausted. It does not reach chapter victory. The controller still uses ordinary
orders and read-only failure inspection; adaptive commands are not an identical
input-journal comparison. The later work failure remains a separate blocker.

Private validation: 1,139 play library tests pass with 33 ignored; the focused
enumeration suite passes all 11 cases. Strict all-target Clippy, the play WASM
library check and native application all-target check pass. The selected-map
opening repeat, authored Escape/control restoration and authored XP-cap gates
pass. The longer controller fails at the work frontier above, so its combined
run is recorded as three passing gates and one failing route, not a green
campaign acceptance result.

## Filtered rectangle predicates (2026-10-19)

Rectangle queries now evaluate loaded `Filter` and `Condition` callbacks on the
same owned VM. `GetFilterUnit` exposes the current candidate, including across
nested trigger calls. A nested rectangle temporarily replaces that candidate;
returning restores its parent. The declaration, when present, must have the
selected constant signature and source. An empty candidate stack returns null,
as the selected `GetFilterUnit` reader does; it does not retain a stale unit.

The selected rectangle wrapper at `0x498140` enters `0x5024d0`. Its setup calls
the same container clear operation as `GroupClear`, and its candidate callback
at `0x5028e0` establishes the unit context before evaluating the expression.
Only a true result reaches member insertion, which occurs before the next
candidate. The recreation follows that order. A null filter still uses the
ordinary bounded path, with allocation/membership checks before clearing.

Explicit trigger callbacks and rectangle predicates now share one continuation
stack in `ObjectHost`. Frames retain actual identities and loaded function ids;
there is no second VM, copied global state or replacement object arena. Return
routing follows the top frame, including trigger evaluation returning a boolean
inside a unit predicate. Execution errors unwind both kinds of context while
preserving earlier script effects. Thus a later predicate/budget failure does
not replay or roll back an earlier accepted member. Non-boolean predicate
returns and a destination destroyed during its callback refuse explicitly.

Candidate collection remains a creation-order snapshot with centre containment.
Births during a predicate join a subsequent query. Native spatial ordering,
boundaries and mutation during traversal remain fidelity questions; this is
not a claim to reproduce the original spatial-index internals.

The unchanged Human01 now completes its actual owner-filtered rectangle call.
Its loaded predicate runs seven candidate reads, selects five members, and all
155 actual unit handles agree with the authored owner/rectangle test. The prior
155 original creation-position comparisons and four null-filter helpers still
pass. Its next refusal is `ForGroup` in `GroupAddGroup`, at selected Blizzard
span `194012..194066`; that callback needs `GetEnumUnit` and the existing group
membership operation.

Validation passed 535 native library tests (seven local-input tests remain
ignored), six application trigger integration tests, and 21 actual WASI tests.
The new tests cover shared globals, true/false results, destination clearing,
nested and empty queries, incremental membership, budget errors, malformed
returns, destroyed targets, mixed trigger evaluation/execution and abort cleanup.
Strict native/browser lint and both library builds pass. Evidence and rejected
attempts remain local in `.scratch/campaign-filtered-rectangle-root-20261019/`.
No new original runtime, rendered/4K comparison, complete cross-runtime mission
parity or playable campaign is claimed; normal cache assembly and live world
updates remain open.

## Group actions on the owning VM (2026-10-19)

`ForGroup` now invokes loaded zero-argument, nothing-returning actions on the
existing VM and shared unit identities. `GetEnumUnit` returns the current action's
unit, restores its parent after a nested group call, and returns null outside a
group action. Its context remains independent of `GetFilterUnit` and trigger
context when these callbacks nest. Both declarations are optional at group
admission; when present, their signatures and source must match the admitted
group interface. Undeclared APIs and invalid callbacks remain unsupported.

Static evidence from the selected 1.29.2.9231 image (`3f2ed012…`) establishes the
member snapshot and context separation. The `ForGroup` entry at `0x4906f0` calls
`0x5025c0`, which retains the loaded callback and traverses through `0x502360`.
That traversal copies member references into a temporary vector through
`0x4f5b20` before invoking actions. Callback `0x5026a0` pushes and pops the enum-unit
context around the saved code; `GetEnumUnit` at `0x491f20` reads that context.
This is code evidence, not a new runtime observation. The retained decoder graphs
and image fingerprint remain local in
`.scratch/campaign-group-iteration-root-20261019/`.

The recreation therefore retains a bounded member snapshot per active action
frame. Group additions, removals, clearing or destruction during an action do
not replace that traversal. Nested calls retain their own callback and members.
Execution errors unwind group, rectangle and trigger frames together, preserving
completed script effects. Native traversal also checks unit liveness; actual unit
removal/death remains unsupported here. The existing deterministic insertion order
is still a reconstruction choice, not proof of native group ordering.

The unchanged Human01 now completes two group iterations and eight enum-unit
reads. Its next refusal is `SetUnitInvulnerable` at map span `109548..109592`.
All 155 creation inputs and accepted positions still match the retained original
comparisons; the authored starting Arthas identity remains unchanged. This run
uses the existing explicit private Host inputs and creation cache. Ordinary
application cache assembly, live actors/controls, removal/death and combat remain
open; this does not make the campaign playable.

Validation passes 542 native library tests (seven local-input tests ignored),
six application trigger integration tests and 28 actual WASI tests. The new cases
cover shared globals/handles, empty groups, mutation and destruction during
iteration, nested calls, abort cleanup, malformed callback/declaration contracts,
source isolation, and independent nested group/rectangle/trigger contexts. Strict
native/browser lint and both application library builds pass. No new original
runtime, rendered comparison, 4K capture or full three-runtime determinism claim
is made; the original baseline gate is unchanged.

## Explicit unit protection (2026-10-19)

`SetUnitInvulnerable` now changes explicit protection on the existing executed
unit record. The optional loaded `BlzIsUnitInvulnerable` query reads that state;
aliases share it, repeated assignments are idempotent, and siblings remain
independent. The declaration signatures and sources are checked against
`CreateUnit` before catalog resolution. Each field is bounded by the existing
unit-record limit, with no additional object identity or per-write record.
A null setter is a no-op and a null query returns false, matching the selected
wrappers. Wrong-family and reserved unit tokens still refuse before mutation.

The selected image's setter at `0x4ab600` dispatches virtual slot `0xa8` to
`0x6d8da0`. That implementation finds the ability class returned by `0xba4820`,
then adds or removes it according to the boolean argument. The query at
`0x499fa0` reads the unit protection flag. The recreation uses a boolean state
instead of duplicating that native ability object. This is an implementation
choice supported for the explicit setter path, not a complete ability runtime.
Static graphs, the class/vtable relationship and the complete image fingerprint
remain local in `.scratch/campaign-invulnerability-root-20261019/`. No captured
ability identifier or native field offset is a production constant.

Initial protection is not assumed false. A unit without an explicit assignment
has unknown protection here, and its query reports that unsupported state.
Initial and other ability effects, ability-level/list changes, damage handling
and combat interactions remain open. In particular this field is not evidence
that a combat or health-bar consumer has run.

The unchanged Human01 completes three true assignments on three actual unit
handles; querying each reports its assigned state. Its next refusal is
`SetPlayerHandicapXP` at map span `123764..123825`. The same run retains both
completed group calls, eight enum-unit reads and all 155 original creation-input
and accepted-position comparisons. Its explicit private Host inputs and collision
cache are unchanged; ordinary bootstrap and a playable world remain open.

Validation passes 547 native library tests (seven local-input cases ignored),
six application trigger integration tests and 34 selected actual WASI tests.
These cover explicit true/false assignment, aliases, siblings, unknown initial
state, null handling, invalid/reserved handles, optional interfaces and each
foreign declaration independently. Strict native/browser lint and both
application library builds pass. This turn did not run the original game,
rendered/4K comparisons or a complete original/native/WASM mission replay.


## Initialization ownership transfers (2026-10-19)

The unit catalog admission now accepts an optional loaded `SetUnitOwner` contract
from the same declaration source as `CreateUnit`. The existing unit's `owner`
is its current player handle; `executed_owner` retains the creation-time player
number. `GetOwningPlayer` and Defend's research prerequisite read the current
owner. No new unit, player, catalog resolve, placement or group membership is
created by a transfer. Aliases observe the same change.

The selected Legacy image SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
registers the `(unit, player, boolean) -> nothing` wrapper at `0x4ab930`. It skips
null lookups and calls `0x69d140` with the resolved player ordinal. That function
returns immediately for an unchanged owner; its color branch is conditional on
the supplied boolean. The getter at `0x494410` returns null for a null unit.
These are bounded static observations, not a fresh ownership runtime comparison.
The implementation uses arena identities rather than native pointers or offsets.

False preserves the known color state, including its explicit unknown initial
state. True requires an assigned, source-typed player color and retains that
identity on the same unit. An unchanged owner returns before color lookup.
Initial unit color is not inferred from an assigned player color: the selected
catalog also contains class-specific color fields, and the creation/class policy
is not implemented here. Missing color knowledge refuses before ownership
mutation when a color change is requested. Actual rendered unit tint remains open.

Ownership-event delivery is not wired. Matching live unit/player-unit listeners
therefore refuse before mutation; disabled or source-qualified unrelated
listeners do not block a transfer. The event enumerands are evaluated from the
loaded named ownership globals with bounded literal arithmetic, never copied
from a measured integer. A listener whose ownership-event identity cannot be
qualified also refuses. Control, food, vision, AI and other world-side ownership
effects remain separate work; this is not full `SetUnitOwner` parity.

The unchanged Human01 completes its false-color transfer on its existing `hhdl`
unit. The getter reports the actual target player handle. All 155 creation inputs
and accepted positions still match the retained original observations, and the
prior group, protection and XP-factor records are unchanged. The next refusal is
`SelectHeroSkill` at map span `109856..109898`, on the `Hart` hero and its authored
ability rawcode. The run still uses explicit private Host inputs; ordinary app
bootstrap and playable actors/controls remain open.

Validation passes 564 native library tests (seven explicit input-dependent
ignores), six application trigger tests and seven ownership tests executed in
actual WASI. Coverage includes aliases, siblings, immutable creation records,
current-owner research, nulls, wrong/reserved handles, unknown and explicit color,
source/signature admission, and both unit/player listener guards. Strict native
and browser lint, both application library builds and formatting pass. The real
Human01 gate is native VM execution, not rendered gameplay or a browser mission
run. Local commands, source hashes, rejected fixture/compile attempts and the
removed unqualified creation-color candidate stay in
`.scratch/campaign-unit-owner-root-20261019/`. No original game was launched for
this slice, and no original content was committed or uploaded.

## Executed visibility and live world membership (2026-10-20)

`ShowUnit` and `IsUnitHidden` operate on the existing executed unit. Fresh
`CreateUnit` records start shown, matching the retained original control. The
optional loaded declarations must share `CreateUnit`'s source, argument/result
types and constant flags. Missing interfaces stay unsupported. Aliases observe
the same state; hiding does not destroy a unit, change its owner, clear its
orders or remove it from existing script groups. Fog, invisibility abilities,
selection and explicit pause remain distinct systems.

The original Legacy client was tested under Wine with reversible diagnostic
copies of Human01. All seventeen retained members were re-read for each copy;
only `war3map.j` changed. Source files stayed unchanged, and the unsigned probes
make no authenticity claim. The first run's four-unit-wide rectangle failed to
select its shown control, so its rectangle result is inconclusive. The second
uses a wider rectangle and verifies the expected shown control before testing
hiding. A third isolates a hidden range anchor. The three stopped runtimes and
59 records are retained locally under
`.scratch/campaign-unit-visibility-root-20261020/` and
`.scratch/original-game-vm/unit-visibility-1280-{1,2,3}/`.

Observed on the tested unit: hiding leaves its current Move order and does not
pause it; a hidden Move request succeeds and position continues to advance. A
second unit can be created at the hidden unit's position, whereas the shown
control displaces that creation. New rectangle enumerations exclude the hidden
unit and include it again after showing. A hidden range candidate produces no
entry until shown. A hidden anchor still delivers entries for visible candidates;
hiding/showing that anchor alone does not repeat an existing membership. Showing
can relocate a moving unit to free space while retaining its order.

The implementation removes only the hidden unit's collision stamp. Hidden
movement retains its activity and updates its accepted position without occupying
or testing other units' dynamic stamps. Authored ground pathing still applies;
this boundary is recreation policy, not a claim about every native hidden-pathing
case. Showing first tests the current position and preserves a free fractional
position. A blocked position uses the existing bounded placement search. That
search reads the selected image's rules and the map/catalog geometry, without
an observed-coordinate table. A relocation rebases remaining motion from the new
accepted position; it does not jump back to the old path origin. A failed reveal
leaves visibility, positions and occupancy unchanged.

New spatial enumerations omit hidden candidates. Existing group memberships are
retained, as required by the map's later `ForGroup(..., ShowUnitShow)` calls. Range
polls likewise omit hidden candidates and keep hidden anchors. Hiding forgets
that unit's candidate memberships, allowing a later visible entry. The previously
documented callback snapshot/order/cadence limits remain; effects inside an
already selected callback are not newly qualified by this experiment.

The actor consumer hides the existing parent entity, so authored geoset opacity
stays independent. It defers creating actors for hidden units and samples the
continuing animation clock on reveal. Native/browser builds check this consumer,
but this slice has no new rendered original/native/WASM comparison. Related
passengers, attachments, selection, general death/visibility interactions,
small-rectangle native raster behavior, all hidden-pathing cases and exact reveal
search parity remain open. This does not establish a playable mission.

Final checks are retained in
`.scratch/campaign-unit-visibility-root-20261020/verification-1.json`: 1,385 native
test executions, 25 actual WASI cases, twenty cases on the actual Cargo
browser-target WASM application under Node, strict checks and both builds pass.
The source-backed unit also passes fresh visibility, spatial exclusion/reinclusion,
hidden collision release and hidden movement/order controls. Its creation-cut
context still matches all 155 original creation requests and accepted positions.
Unchanged Human01 application startup passes ShowUnit and next refuses
`AttachSoundToUnit` at Blizzard.j:2226; it publishes no session. The earlier
standalone diagnostic's missing day/night loader is not that application's failure.

## Selected flight movement admission (2026-10-22)

The supplied `UnitData.slk` row `nser` (Searinox) selects `movetp=fly`,
`moveHeight=280`, `moveFloor=90`, `turnRate=0.1`, `collision=48` and
`pathTex=_`; its `UnitBalance.slk` `collision` value is `48`. These are the
authored inputs for Human02's scripted Searinox creation, not defaults invented
by the recreation.

Selected executable evidence (Legacy 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`):

- the movement-token decoder's string table at `0x10dccc0` maps `fly` to
  movement enumerand `2`;
- the constructor path beginning at `0x67ce7d` indexes a case table at
  `0x67d90c`: movement `1` (`foot`) reaches `0x67cebc` (`mov ecx,2`) and
  movement `2` (`fly`) reaches `0x67cec3` (`mov ecx,4`); the selected value is
  stored at the unit's `+0x1b0` pathing-mask field;
- the separate pathing-type switch at `0x494460` maps pathing enumerand `1` to
  mask `2` and enumerand `2` to mask `4`, matching `PATHING_TYPE_FLYABILITY`;
- the fly predicate at `0x767c30` compares the `movetp` field with `fly`.

The recreation now derives both masks from those reviewed instruction windows.
`unit_positions::geometry` classifies `foot`, `horse` and `fly`; air keeps the
authored collision radius and `pathTex`. Placement selects the `fly` mask instead
of the ground mask for air, skips ground terrain-domain adjacency, ignores dynamic
unit occupancy and stationary footprints, and does not insert air stamps into
either ground occupancy array. `prepare_motion` reuses the stamp's class, so
routes and segment samples also test the fly mask. An all-fly-masked map refuses
creation rather than falling back to an invented position.

Recreation choices still requiring native qualification: whether air units
collide with each other, the exact radius/footprint test against no-fly cells,
`moveFloor` altitude handling and turn-rate-limited facing. The actor consumer
already reads `moveHeight` as model height above terrain; it does not consume
`moveFloor` or `turnRate`, and this slice does not change rendering.

A local Human02-derived diagnostic omitted five unsupported
`IssueImmediateOrder` calls and the later trigger initialization, then reached
and admitted the authored `CreateUnit('nser', ...)` at script line 858. The
resulting `gg_unit_nser_0048` global answered `GetUnitTypeId == 'nser'` and
`GetUnitX/GetUnitY == 2612.25/-5883.25`, the authored 2612.3/-5883.1 request
snapped to the selected quarter-cell grid. The unmodified map still stops
earlier at script line 815 on unimplemented `IssueImmediateOrder`, so this is a
targeted creation-path probe, not an unmodified campaign gate, playability or
rendered-parity claim.

Synthetic tests cover flight admission, ground-mask/occupancy/domain bypass,
fly-mask refusal, air stamps leaving ground occupancy unchanged and scripted fly
movement. Focused library tests, strict Clippy and formatting pass locally.

### Hover creation and movement (2026-10-25)

The unchanged RoC prologue reaches its authored `ushd` creation after rectangle
fog initialization. The selected `Units\\UnitData.slk` assigns that type
`movetp=hover`; the extracted member SHA-256 is
`34a0c8083f146b5d3e25d92a19a8822151d4b26dfe9c1c325a6229466d1b04d6`.
No unit-specific compatibility exception is added. Activity admission, collision
geometry and guard eligibility now accept the movement token together, retaining
the authored speed, radius, model height and pathing footprint.

The current reconstruction uses the existing ground-mask, ground-occupancy and
terrain-domain route for hover. This agrees with the primary
[Warsmash movement implementation](https://github.com/Retera/WarsmashModEngine/blob/main/core/src/com/etheller/warsmash/viewer5/handlers/w3x/environment/PathingGrid.java),
where hover and foot test the same terrain channel. It is not a newly measured
original-client collision/altitude comparison. Synthetic coverage creates a
hover unit, routes it around an occupied cell at a nonstock speed, and checks
computer guard admission. The later floating/amphibious entry below adds their
separate water-pathing and surface path.

## Placed destructable life and stationary death (2026-10-22)

Selected `.doo` records in the reference census expose life bytes `0..=100` and
`255`. Byte `0` creates a dead object, `1..=100` is the authored percentage of
catalog HP, and `255` is full. Bytes `101..=254` are unobserved in that census;
clamping them to full is a recreation inference pending native evidence, not a
measured rule.

Placed rows retain the complete authored record (type, variation, position,
angle, all three scale components, flags and raw life byte) by handle. The
script-facing creation fields remain a separate projection. Nonfinite position,
angle or any scale component is refused before a row is produced. Life getters,
setters, kill and damage also refuse while the selected catalog has no HP for a
script-created record; they do not invent zero life or a zero maximum.

Script-created destructables are admitted only before the stationary collision
cache is built. Later creation is refused because this cache cannot rebuild
stationary occupancy. Death and life mutation share one path, and a dead object
swaps only its own live/dead footprint layer without erasing terrain bits or
other overlapping stationary objects.

A fresh native probe on the pinned Human02-derived map changes only the
authored flag byte of four separated `LTlt` records and records destructable
counts around one matching `CreateDestructable` per record. Flag `0` and flag
`3` produce no automatic object; flags `2` and `6` do, and each script call
adds a new handle rather than returning the pre-existing one. The pinned map
contains only flags `0`, `2`, `3` and `6`; the wider corpus also contains `4`
and `7`, which remain unqualified. The recreation now admits automatic
placement only for `2` and `6`, leaves `0` and `3` to script creation, and
refuses other destructable flags with the rawcode and creation number. These
are observed admission counts, not a claim about the individual flag bits.

The Human02 `.doo` stream contains `LTcr` records at `(-4608, -3264)` and
`(-2112, -4800)` with flag `0`, while `CreateAllDestructables` allocates
matching scripted destructables at those positions. The recreation retains
each skipped `.doo` record as source, but creates neither an arena object nor
an initial stationary layer for it. Each authored script call produces exactly
one script-origin arena record and one `StationaryKey::Destructable` layer.
Non-destructable decorative doodads keep their existing static-footprint
handling regardless of their flags. Killing one script identity changes only
that identity's stationary layer.

## Explicit unit team color (2026-10-23)

`SetUnitColor` is admitted optionally alongside the selected `CreateUnit`
declaration, with the exact `(unit, playercolor) -> nothing` signature and
same-source `playercolor` type. It retains the admitted symbolic color on the
existing unit's `team_color` field. Owner, creation color, catalog, position,
orders and other units remain unchanged; there is no new palette or unit.
The existing actor projection prioritizes this field over class/creation color
and retires/rebuilds an actor when its effective palette index changes. Palette
paths and material binding continue to come from the selected assets.

The reviewed image above registers the working wrapper at `0x4ab260` through
the registration at `0x49d5fb`. It resolves the unit with `0x483940`, skips a
null lookup, passes the color through vtable slot `+0x64`, then calls the unit
color update at `0x6a2a70` and refreshes presentation through `0x5f6e00`.
The second registration table points to `0xcbda30`, a zero-return stub; that
table alone is not edition-specific behavior evidence. The local registration
receipt and disassembly are retained in `.scratch/unit-color/` in the private
continuation checkout.

Null or removed units have no effect, without resolving the unused color.
Malformed arguments, wrong-family/reserved handles and foreign declarations
refuse before mutation. A null color on a live unit is explicitly unsupported
rather than being assigned an invented palette index. The retained-field implementation
is a recreation choice supported by the native setter path and existing
renderer consumer, not a fresh native tint comparison. Exact visual refresh
phase, null-color behavior and rendered parity remain unqualified.

## Floating and amphibious movement (2026-10-25)

The selected Legacy image `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
resolves movement tokens through the string/enumerand records at `0x10dccc0`.
The constructor uses **byte selectors** at `0x67d90c` followed by a jump table
at `0x67d8f4`; treating the byte table as a direct array of addresses is wrong.
Its ground, flight, float and amphibious branches load separate masks at
`0x67cebc`, `0x67cec3`, `0x67cece` and `0x67ced5`. The existing checked placement
reader now derives all four operands and refuses empty, non-single or overlapping
masks. No selected mask value is copied into the runtime.

Creation, show, relocation, pathing changes, live movement and guard eligibility
retain the source movement class. Floating units use the swim channel; amphibious
units may use walkable **or** swimmable cells, while respecting the separate
amphibious exclusion channel. This shoreline rule is a reconstruction consistent
with the primary Warsmash implementation linked above, not a new original-client
trajectory observation. Ground and water dynamic occupancy are separate;
amphibious units participate in both. Non-air unit footprints and stationary red
footprint cells block both media. Flight keeps its existing separate behavior.

The terrain reader's coordinate conversion supplies the new water-mover height
path too: `0x735a87` multiplies a corner coordinate by a selected data float;
`0x7361b4`/`0x7361bf` encode height with an additive bias and multiplicative factor
in the older-format corner reader. The same packed low height field is retained
by the current-format branch. Those data operands are read at runtime and their
inverse maps raw corner indices to world height. The application selects the
unique `Water.slk` row matching the map tileset, derives its height offset using
the checked tile scale, and interpolates the map's stored water levels. Floating
models require a water surface; amphibious models use the higher of floor and
water, then add the authored model `moveHeight`. Look-at targets use the same
surface. Legacy ground/water mesh helpers still carry their older measured-default
API; the new surface path does not copy those defaults.

Synthetic checks use different masks, tile scale, height bias/factor and water
row names. They cover shoreline routes, collision release/show/relocation,
stationary layers, real scripted movement and surface/model height. The unchanged
TFT first chapter now passes its floating-unit creation and reaches later startup
calls. Original boat trajectories, bank/pitch, bridge elevation and rendered water
textures/shorelines remain unverified or unfinished. Water surface sampling is
not a claim that the missing water renderer has been implemented.
