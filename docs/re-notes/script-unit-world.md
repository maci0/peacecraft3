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
