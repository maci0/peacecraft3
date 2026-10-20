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

Finite ordered bounds, including zero-area rectangles, are supported. Reversed
bounds and non-finite inputs are explicit refusals; native normalization of such
inputs has not been established. Moving a rectangle preserves its extents around
the requested center. Wider intermediates prevent avoidable arithmetic overflow,
and an unrepresentable result refuses before changing the old bounds. These are
recreation arithmetic and safety choices, not a claim of native exceptional-input
or bit-exact arithmetic parity. No authored coordinate or map dimensions are
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

Location-based creation and setters, world-bound admission and enumeration remain
unsupported. Explicit region creation, rectangle-addition requests and enter/leave
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
copies their current center into the existing extent-preserving movement path.
Later location movement or removal cannot alter the rectangle. Invalid/stale
locations, wrong-family arguments, bad signatures and overflowing moved bounds
refuse without changing the prior rectangle. No second geometry or handle pool
is introduced. Synthetic VM and Host checks exercise these boundaries; native
exceptional-input and arithmetic parity remain unverified.
