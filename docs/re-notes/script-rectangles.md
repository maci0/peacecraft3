# Script rectangles in the shared arena

`ObjectHost::with_rectangles` admits the loaded program's direct-coordinate
rectangle contracts: `Rect`, `SetRect`, `MoveRectTo`, `RemoveRect`, and the six
bound/center queries. It validates positional parameter and return types,
constant qualifiers, declaration sources and `rect` ancestry using the existing
object contract checker. Formal parameter names are not an ABI requirement.

Executed min-x, min-y, max-x and max-y values live on the existing bounded
`EnumHost` arena entry. There is no second handle pool or region loader.
`rectangle_bounds` exposes those current values to a future spatial consumer.
Mutation preserves identity; removal invalidates all aliases. Removed tokens are
not recycled, and rectangle creation consumes the same total identity capacity
as timers, players, groups, sounds and quests. Incorrect-family, null, stale and
unknown handles refuse with source-qualified errors.

Finite corner coordinates, including zero-area rectangles, are supported. Each
axis is normalized independently when corners are reversed. Non-finite inputs
remain explicit refusals. Basic hosts move rectangles using wider intermediates
and preserve extents around the requested center. The selected policy below
instead uses the source's center/delta arithmetic, including its rounding effects.
Unsupported arithmetic refuses before changing bounds. Exceptional-input and
complete native parity remain unqualified. No authored coordinate or map dimensions are
embedded in the implementation.

## Verification

Six synthetic tests cover source contracts and renamed formals, actual JASS VM
execution, alias mutation/removal, timer/rectangle identity separation, capacity
after destruction, integer promotion, invalid mutations, and finite extreme
coordinates. All six also execute under `wasm32-wasip1` through Wasmtime. The full
`pcx-play` suite passes 356 tests with six explicit ignores; strict native and
WASM Clippy, native/WASM application checks and formatting pass.

The existing original Human01 gate was rerun against current production rlibs,
with only rectangle admission added to its composed Host. The selected map is
SHA-256 `f5b155615dbce93b88fff80214a413b137c6fbf8c12abf357976a6ff42040ee9`;
its supplied `common.j` is
`3c21e998bf80d1e04718bd3f2ee7ef2bf1b83359e5b303a56cf59cd61a2afdcd` and
`Blizzard.j` is
`7ca28f0ce47712a9ab6832c22bb0d0c9d2844b5831fb465b9d792731a3d5243e`.
The gate retains its explicitly identified compatibility object catalog; that
does not establish the selected Legacy install's stock catalog or native archive
priority.

Globals and `config` still complete. All 58 direct sound creations, parameter
assignments and duration assignments still execute. `main` now creates 98
rectangles. Every retained coordinate is compared by its exact `f32` bits against
the corresponding executed call, and rectangle handles are checked against the
other arena families. The next executed refusal is `SetSoundPosition`, map span
`83728..83801`, declared at `common.j:157048..157147`.

The actual commands, logs and artifact receipts stay local in
`.scratch/cursor-gameplay-20261016/root-rect-integration/`. No original file or
extracted script is part of this change.

## Remaining work

Location-based creation and setters and explicit world-bound admission are now
supported. Enumeration remains unsupported. Explicit region creation, rectangle-addition requests and enter/leave
subscriptions now use the composed Host factory; see [script regions](script-regions.md).
That path does not establish cell geometry, membership or event delivery. This
verifies execution and retained state, not spatial effects, rendered regions,
a live campaign objective or native game parity.

## Location-based movement during Human01 pursuit

The unchanged chapter's periodic pursuit action reaches `MoveRectToLoc` after
actual player attack-move inputs and two authored route points. The previous
source-backed replay stopped at 129.5 simulated seconds because this declaration
had no consumer. This establishes a runtime execution boundary, not chapter
completion or original-game parity.

Rectangle admission now validates that optional declaration, its location ancestry
and source relationship to `Rect`. Execution requires admitted live locations and
copies their current point into the admitted movement path. Selected numeric
hosts now use the source-backed center/delta routine described below. Later
location movement or removal cannot alter the rectangle. Invalid/stale
locations, wrong-family arguments, bad signatures and overflowing moved bounds
refuse without changing the prior rectangle. No second geometry or handle pool
is introduced. Synthetic VM and Host checks exercise these boundaries; native
exceptional-input and arithmetic parity remain unverified.

## Location-backed creation and setters

Rectangle admission now validates optional `RectFromLoc(location, location)`
and `SetRectFromLoc(rect, location, location)` declarations against the selected
rectangle source and loaded location ancestry. The existing `MoveRectToLoc`
admission uses the same optional-contract path. Programs without these
optional declarations keep their prior rectangle support.

Execution requires admitted live locations. Both operations copy the current
corner coordinates into the existing rectangle representation and
reuse its finite-coordinate validation and independent axis normalization. Later location movement or removal
does not change those copied bounds. Setters preserve rectangle identity and
refuse before mutation when either location is invalid or the resulting bounds
are unsupported. Construction allocates only after both points pass validation;
shared arena capacity remains its bound. Inverted corners normalize through the
same path as direct-coordinate rectangles.

Synthetic VM tests cover creation, alias-based setting and copied lifetime.
Host tests cover missing location admission, null/stale/wrong-family inputs,
invalid arity, unchanged state on refusal, creation capacity,
selected-source contracts and absent optional declarations. This extends the
existing geometry; selected center/delta arithmetic is now qualified below,
while exceptional inputs remain unverified. Local receipts are in `.scratch/goal-location-geometry/`.

Native coverage now reports both location-backed rectangle operations and the
location membership query. The earlier trigger condition/action cleanup natives
are also included; their constant-based dispatch had escaped the source audit.
Owned-VM tests check the admitted programs' public coverage results after executing
these APIs.

Validation used Rust/Cargo 1.100.0 with `RUSTFLAGS=-Dwarnings`. The full
`pcx-play` library suite passed 3,335 tests with 163 explicit ignores. Strict
native library/test Clippy, formatting, the native library build and the
`wasm32-unknown-unknown` library build passed. Original-client and renderer
comparisons were outside that validation run. Source/artifact receipts, including the
failed focused and coverage-audit attempts, are retained locally.

## Finite corner normalization against the selected executable

A controlled modified-map probe in the selected build 9231 executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
ran under isolated Wine/Xvfb with a null audio sink and no network. Two completed
runs produced the same first 17 observations. Reversing the synthetic corners
`(8, 10)` and `(-4, 2)` gave bounds `[-4, 2, 8, 10]` for `Rect`,
`RectFromLoc`, `SetRect` and `SetRectFromLoc`. The prior refusal of reversed
finite coordinates was incompatible with this observation and is removed.

Selected-image code at `RectFromLoc` handler `0x4a4cb0` and `SetRectFromLoc`
handler `0x4a99d0` compares the two coordinates separately. Its conditional
moves select the second corner on equal operands. The shared bounds helper uses
explicit comparisons to preserve that tie choice, including signed zero, rather
than relying on floating-point min/max tie behavior. Regression tests cover all
four corner permutations, both setters, aliases, allocation counts and zero ties.
The signed-zero assertion is backed by code inspection; the runtime probe used
ordinary nonzero coordinates.

The location handlers also call `0x7f4a60` before ordering coordinates. The
source-backed corner policy below now reproduces that clamp, also used by direct
creation and setting. The initial probe established only the in-world finite
cohort. Non-finite behavior, native Windows execution and full tick parity remain
unqualified. No world limits are inferred from the probe or embedded as constants.
The same probe disproved continuous region containment equivalence; see
[slice regions](slice-regions.md#observed-containment-mismatch).

Private probe scripts, modified maps, original input copies, output checksums and
isolation receipts remain under `.scratch/goal-native-geometry-reference-2/`
and `.scratch/goal-native-geometry-reference-3/`. They are not repository inputs.
The initial attempt in `.scratch/goal-native-geometry-reference/` failed the
original client's map-script CRC check and is retained as failed evidence.

Normalization validation passed 3,337 `pcx-play` library tests with 163 explicit
ignores, strict library/test Clippy, native and `wasm32-unknown-unknown` library
builds, formatting and whitespace checks. Rust/Cargo 1.100.0 used
`RUSTFLAGS=-Dwarnings` and a checkout-specific target directory. Receipts in
`.scratch/goal-rectangle-normalization/` identify the dependency closure and
source/artifact hashes. The first focused filter matched zero tests; the full
suite above subsequently ran both new regressions.

## Source-backed finite corner clamping

A 39-row controlled probe in the same selected executable showed that `Rect`,
`RectFromLoc`, `SetRect` and `SetRectFromLoc` all clamp their finite corners.
The minimum comes from the full world rectangle. The maximum subtracts an
image-derived inset from that world's maximum. Exact-world and reversed outer
corners follow the same rule. The input locations retain their original values.
`GetWorldBounds` itself still returns the full extent, without the corner inset.
The diagnostic includes four IEEE controls and an unsupported-number flag;
all records completed and the controls passed. This is isolated Wine observation,
not an unchanged-map or native Windows acceptance result.

The shared helper is `0x7f4a60`, its inset initializer is `0x4015b0`, and the
initializer writes the operand loaded by that helper. The reader validates the
complete image fingerprint, bounded window hashes, both calls to the selected
numeric subtraction routine, min/max selection and stores, world-coordinate
field relationships, and the two helper calls from each of the four native
paths. The initializer is decoded through the existing numeric policy's integer
conversion reader. Neither its measured value nor map coordinates are copied
into a production constant. Synthetic fixtures use several different initializer
values and reject changed globals, calls, field relationships, identities and
budgets even when their test window hashes are updated.

`campaign_client` now attaches this policy to the selected numeric input.
`ObjectHost` uses the admitted full-map extent and the existing selected real
subtraction when applying its limits. Both constructors validate before arena
allocation; both setters validate before mutation. Missing world extent or an
extent too small for the selected inset refuses contextually. Rectangle identity,
location lifetime and alias rules remain unchanged. Basic rectangle hosts without
this optional numeric policy retain their earlier, unqualified finite geometry;
they do not establish native clamp behavior. The selected campaign path supplies
the policy and derives its extent from source terrain/pathing and placement data.

The exact self-authored probe script also ran in the recreation with the selected
executable's numeric/clamp policy. Its world extent was independently assembled
using the original map's parsed terrain origin, parsed pathing dimensions and
selected image-derived placement cell size, following `campaign_inputs`.
That extent and all 39 observation rows matched the original-client bits.
The capture wrapper only records the probe's four preload export operations;
rectangle/location operations and arithmetic use production code. This bounded
comparison does not prove rendered geometry, out-of-world rectangle movement,
non-finite inputs, other maps/editions or complete tick parity.

Private source copies, probe inputs, native records and stopped-process receipt
remain in `.scratch/goal-native-world-clamp-7/`. Synthetic tests, the matched
production replay, extracted source terrain/pathing and source/artifact receipts
remain in `.scratch/goal-world-clamp/`. Original assets and generated map/script
copies remain excluded from the repository. The first synthetic fixture lacked
location getter declarations, and the first replay harness used two unavailable
APIs; those failed attempts are retained separately from passing runs.

Corner-clamp validation passed 3,350 `pcx-play` library tests with 163 explicit
ignores, strict library/test Clippy, formatting, whitespace checks, and native and
`wasm32-unknown-unknown` production library builds. Rust/Cargo 1.100.0 used
`RUSTFLAGS=-Dwarnings` and the checkout-specific target directory. The matched
replay links the selected image, exact probe, production bindings, source map
members, binary and observation hashes; the build closure includes the new
reader source even while it remains uncommitted. These are bounded corner
checks, not full tick-parity or campaign completion.

## Source-backed rectangle movement and centers

A 55-row controlled probe in the same selected executable measured direct and
location-backed moves beyond both sides of the world, large-coordinate round
trips, repeated moves and both center queries. Four IEEE controls and the
unsupported-number flag passed. The native did not clamp movement to the world.
Its core at `0x4a33a0` computes each previous center with the selected add and
multiply routines, subtracts that center from the requested point, then adds the
delta independently to each existing bound. The center factor is read from its
bound image data operand. `MoveRectTo` and `MoveRectToLoc` both call this core;
the center getters use the same source add/multiply operations and factor.

The former wide-intermediate recreation path differed in 12 bit-valued rows
after large-coordinate round trips. Preserving an exact geometric half-extent
did not reproduce these observable source arithmetic effects. The selected
numeric policy now admits a rectangle-motion reader that validates image and
window fingerprints, the wrapper/core call relationships, arithmetic calls,
bound stores, both center getter paths and the shared data operand. It derives
the factor from those bytes rather than substituting a measured constant.
Synthetic fixtures vary the operand and reject changed calls or operands even
when their test hashes are updated.

Motion and center queries use the existing selected real operations. Every bound
is computed before mutation, and unsupported arithmetic refuses with rectangle
and source context. Moving preserves the rectangle token and reads a location's
current point without altering that location. The source's finite rounding can
change a representable width or leave a computed center different from the
requested point; the implementation retains those observed outcomes. Basic hosts
without this numeric policy retain their prior unqualified geometry. No movement
world clamp is introduced.

The exact probe also ran through production rectangle/location code and the
selected numeric, corner and motion policies. World input was derived from the
same original terrain/pathing and selected placement policy used by the prior
corner replay. All 55 rows matched the native bits after the change, including
the 12 previously differing rows. This is controlled Wine evidence for these
normal finite cases; it does not establish exceptional-number, rendered,
other-map/edition or full tick parity. Region cell geometry remains unresolved.

Private probe inputs and native observations remain in
`.scratch/goal-native-rectangle-move-8/`. The earlier mismatch report, matched
production replay, synthetic tests and dependency/source/artifact receipts remain
in `.scratch/goal-rectangle-motion/`. The native container was stopped and its
process ID checked zero. The first synthetic test compile used an invalid
repeated non-Copy value; the corrected test and passing run are retained
separately from that failed attempt.

Motion validation passed 3,354 `pcx-play` library tests with 163 explicit ignores,
strict library/test Clippy, formatting, whitespace checks, and native and
`wasm32-unknown-unknown` production library builds. Rust/Cargo 1.100.0 used
`RUSTFLAGS=-Dwarnings` and the checkout-specific target directory. The private
closure records both uncommitted reader sources, dependency features/artifacts,
selected image/probe identity and matched replay hashes. These checks do not
establish full campaign or tick-parity completion.
