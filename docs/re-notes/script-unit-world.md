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
implicit coercion of declared real storage and parameters. Live occupancy,
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
acceptance run. No production range-registration stub, world-id adapter, copied
position/collision constants or assumed polling interval is added. Live placement,
session-scoped world identity, world-driven event delivery and the required
four-result RNG factory gate remain open. The factory still refuses
`TriggerRegisterUnitInRange` after one exact RNG result.

## Accepted-position gap (2026-10-18)

`CreateUnit` at `0x663ca0` calls the factory `0x687ca0`. The placement routine
at `0x6be040` clamps the request through `0x7f4a60`, then builds pathing-cell
indices with trunc-toward-zero of `(coordinate - origin) / 2^5`. The division
is the exponent adjustment `add eax, 0xfd800000`, guarded by the `0x3000000`
underflow test. That is the same power-of-two cell size the terrain predicate
reads from the image. Collision remains virtual slot `0x168`. `GetUnitX` still
reaches virtual slot `0xB8` and `0x7fe810`, which multiplies the stored
cell-space value by `2^5` and adds the origin.

The search wrapper `0x7fce70` calls `0x8aef10`. When its limit argument is `-1`,
the immediate `0x20` is substituted. Success writes `cell_space * 2^5 + origin`
back to the request pointers (exponent add `0x2800000`). Failure leaves the
clamped request, and `0x4e00f0` runs only when both the shape lookup and that
search result are non-null. `0x4e00f0` aligns a truncated coordinate to a
multiple of 64 and can add the runtime float at `0x113bde4`. That address is
past the file image.

Reclassifying the retained 155 constructor pairs against the parsed terrain
origin and this cell size leaves two results. 146 getters equal
`floor(logged_request * 4) / 4`. The other 9 are pathing-cell centers. Their
rawcodes are `nshe` (five, collision 16), `nvil` (two, collision 16), `nC22`
(collision 16) and `nban` (collision 31). 63 logged axes match that floor and
disagree with truncation toward zero, so the getter quantum is a different
operation from the search's cell index. 85 of the quarter matches are also
finer than an integer floor.

No decoded instruction in this chain is that quarter floor. File-backed floats
on the nearby path are `0x10db050` = `0x38d1b717` (nearest `0.0001`),
`0x10db120` = `0x33d6bf95` (nearest `1e-7`), `0x10db0ac` = `0.5` and
`0x10db0d4` = `1.5`. The last two select a deformation band in `0x8bc900`.
The band offsets `0x112d810` and `0x112d83c` are outside
the file image, as is `0x113bde4`. The position write continues through
`0x800280`, `0x8bc740` and `0x8c3050`. Those offsets and that call are the open
branch. A blanket quarter snap would also miss the 9 cell-center moves.

`ExecutedUnit.x` and `ExecutedUnit.y` stay the executed request. Window
listings and the pair classification remain local under
`.scratch/grok-active-three-20261018/unit-placement/analysis/`. No new runtime
was launched.

### Bounded probe

One init-time run on this image and Human01, two creations:

1. Create one footman at a fractional point the 19-request probe accepted
   without moving, chosen so the request is not a multiple of `1/4`. Record
   the `GetUnitX`/`GetUnitY` bit patterns, the two dwords `0x8b6ea0` copies
   from spatial slots `+0x40` and `+0x44`, and the floats at node `+0x78` and
   `+0x80`.
2. Repeat for one creation that same probe moved onto a cell center.
3. After load, record the runtime floats at `0x112d810`, `0x112d83c` and
   `0x113bde4`.

That separates a cell-space fixed-point quantum from the deformation offsets.
It does not need the 155-stream replay, range registration, or another map.
