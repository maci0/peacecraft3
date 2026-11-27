# Region cell geometry

Script regions retain rectangle identities and call-time bounds as history.
With the selected numeric policy, membership and enter/leave polling now use
source-backed cells and ordered geometric edits. Unqualified hosts retain their
earlier continuous predicate and identity-based clearing. `RegionAddCell`,
`RegionAddCellAtLoc`, `RegionClearCell` and `RegionClearCellAtLoc` now require
selected cell rules and edit the same geometry. `IsUnitInRegion` shares the
point predicate and the existing retained unit-position lifetime.

## Measured, and not this cell

`war3map.w3r` records are four signed integers (version below 2) or four floats, plus the versioned name and optional fields in `crates/pcx-map/src/regions.rs`. There is no circle or polygon variant. Those bounds may later be wrapped as ordinary `rect` values. That wrap does not define a region cell.

`region_cells` (`crates/pcx-play/src/native_spatial_chains.rs`) maps one finite ordered rectangle onto a caller-supplied spatial grid: native axis 0 is world Y, cell index is `floor((coordinate - origin) / cell_size)` in the session real arithmetic, the maximum cell is included by adding one after the floor, and the result is clipped to the grid. `cell_size` must be a normal positive power of two. Placement supplies that size as `1 << exponent` from the selected executable. The reach-cache owner uses this conversion when a tracked `RegionAddRect` is inserted (`spatial_pool.rs`, addresses `0x801a50`, `0x801930`, `0x8c23f0` in `ai-construction.md`). That is the spatial-chain rectangle. It is not a measurement of the JASS region cell.

## Source-backed rectangle cell edits

`script_numeric::region_cells` validates the selected image and source windows
for point conversion (`0x8017a0`), rectangle conversion (`0x801a89`) and
floor/inclusive-maximum conversion (`0x801930`). Both coordinate paths must
subtract the same world origin, call the admitted subtraction routine, and use
the same exponent-scaling operands. Pitch is derived from those operands rather
than a copied observation. Floor and integer-conversion call relationships and
selected shapes are checked. Full-map bounds come from the admitted source map.

The selected build's operands describe a 32-unit pitch relative to world minima.
The synthetic `[-4, 2, 8, 10]` rectangle covers X `[-32, 32)` and Y `[0, 32)`
in the controlled map. A rectangle maximum on a cell boundary includes that
cell: `[0, 0, 32, 32]` includes queries up to but excluding 64 on either axis.
These numbers describe that observation, not production constants.

`RegionClearRect` uses its rectangle's current geometry, even with a different
handle from the addition. Moving the original rectangle before clearing does
not clear the old cells. A zero-area rectangle still selects a cell. Additions
and clears form a bounded ordered edit log; the last edit covering a queried
cell determines membership. Clear operations count against
`RegionBudget.max_rect_additions` as well as additions in this selected mode.
Budget or allocation refusal leaves membership unchanged. Historical rectangle
requests remain available independently of current membership.

Point queries, location queries and region-event polling share this predicate.
The placement cache receives geometric clears and replays the ordered edit log
when initialized after script execution. Removing a region excludes its edits
from replay. Native event timing, exceptional numeric inputs, non-aligned map
extents and broad world-edge behavior still need qualification.

## Rect natives checked in the same pass

`rect_host` already executes `Rect`, `SetRect`, `MoveRectTo`, `MoveRectToLoc`, `RemoveRect`, the six bound and center queries, and `GetWorldBounds` after an admitted full-map extent. `rect_host_tests.rs` covers reversed finite normalization and refusals for non-finite, null, stale, wrong-family, and unadmitted calls. No admitted native whose argument is a live rect refuses an ordered finite operation there. `RectFromLoc` and `SetRectFromLoc` now copy live location coordinates through the existing finite-coordinate normalization path. See [location-backed rectangles](script-rectangles.md#location-backed-creation-and-setters) for validation and remaining native fidelity questions.

## Observed containment mismatch

The selected build 9231 executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
reported both `IsPointInRegion(zone, -10, 4)` and
`IsPointInRegion(zone, 16, 4)` true after adding the synthetic rectangle
`[-4, 2, 8, 10]`. Two isolated Wine runs repeated these results. The former continuous
predicate returned false for both outside points. The selected cell policy now
repairs that mismatch. The first probe alone did not establish the grid; the
source-backed conversion and later boundary cohort above qualify this change. Private receipts are linked from
[rectangle normalization](script-rectangles.md#finite-corner-normalization-against-the-selected-executable).

Finite corner clamping is now source-backed for both rectangle constructors and
setters; see [corner clamping](script-rectangles.md#source-backed-finite-corner-clamping).
It uses source world extents and an image-derived maximum inset. It is separate from region cell conversion. A source-backed motion policy now reproduces the controlled finite movement
cohort, including its arithmetic rounding and absence of world clamping; see
[rectangle movement](script-rectangles.md#source-backed-rectangle-movement-and-centers).
Region conversion is qualified separately above.

## Controlled boundary and clear cohort

A fresh isolated build-9231 Wine run exported 138 ordered boolean observations.
The first production replay matched the 84 rectangle-only rows. After adding
all four cell-edit natives, the production host now replays the complete authored
probe and matches all 138 rows exactly, including its ordered begin/end markers.
The matched cohort covers cell boundaries, mixed rectangle/cell edits, location
cell edits and the supplied outside-world point.
The original client used a private display, a null audio sink and no network.
The container was stopped and its zero process ID verified afterward.

Local source/map/output hashes, isolation checks, case indices, original output,
production compiler arguments and rectangle replay are retained in
`.scratch/goal-native-region-cells-9/`. The complete replay and final artifact
closure are in `.scratch/goal-region-cell-natives/`. Modified original map
contents stay local.
Synthetic tests also vary the encoded pitch, reject inconsistent source windows,
check edit-budget refusal and verify placement clear/readdition behavior.
This establishes finite rectangle and cell membership for the controlled
cohort, not native Windows equivalence, complete event timing or campaign
tick parity.


## Cell-native source relationships

The cell-edit coordinate helpers (`0x65d350`, `0x65d390`) must use the same
world-global operand, axis offsets, subtraction call and exponent-scaling
operands as the admitted point and rectangle paths. The shared edit routine
(`0x8019b0`) must call those helpers and the selected single-point rounding
routine (`0x8018e0`), which must call the same floor/truncation routines as
rectangle conversion. Each of the four native call sites is checked for its
argument loads, add/clear flag and common target. Inconsistent source windows,
fingerprints, budgets or relationships refuse admission.

Cell edits record the supplied finite point as a one-cell addition or clear.
Location variants read the live location at the call and copy that point.
They allocate no rectangle handles and do not fabricate rectangle-history
requests. Their edits share the existing aggregate edit budget, event-membership
predicate and placement-cache replay. Source/type/signature errors, missing
rules or world bounds, invalid/removed inputs and exhausted budgets leave
membership unchanged. Existing programs without these optional declarations
retain the earlier admission contract.

## Unit membership and deferred removal

The selected `IsUnitInRegion` handler (`0x499ed0`) resolves a unit, passes its
position owner to `0x801720`, and reaches the same point-membership routine
(`0x801750`). It does not apply the live/shown-unit filter used for event polling.
The host now reads the accepted unit position and uses the shared predicate.
An unknown accepted position or missing removal clock/position refuses rather
than becoming an invented origin.
Null region or unit arguments return false without allocating identities.

Two fresh isolated Wine runs exported 31 and 35 ordered observations. The second
repeated all first-run rows exactly and sampled the removed unit one second
later. A source-derived original-map unit type was used for the controlled
constructor. The unit's authored test positions were confirmed through the
native coordinate getters. Hiding and death preserved membership. Immediately
after removal, both coordinates and unit membership remained available; one
second later unit membership was false and both coordinate getters returned
zero. Point membership at that zero position remained true, so unit queries
cannot be implemented by blindly querying fallback zero coordinates.

The implementation uses the same removal-time position lifetime as `GetUnitLoc`
and `GetUnitX/Y`: the removal clock boundary retains its point, and a later
clock drops unit membership. Native observations establish the immediate and
one-second samples; the exact first cleanup frame remains unverified. Synthetic
owned-VM tests check live movement, hiding, null/type/source refusals, unknown
positions, same-clock removal, later-clock retirement and allocation-free queries.
These are shared-predicate tests, not a full original-unit-catalog replay.

Private map/script hashes, source selection, ordered output and isolation/stop
receipts are in `.scratch/goal-native-region-unit-10/` and
`.scratch/goal-native-region-unit-11/`. Both runs used a private display, a null
audio sink and no network; both containers were stopped and their zero process
IDs verified. No original files or generated game contents are committed.

Validation used Rust/Cargo 1.100.0 and `RUSTFLAGS=-Dwarnings`: 3,364 library
tests passed with 163 explicit ignores. Strict library/test Clippy, native and
`wasm32-unknown-unknown` library builds, formatting and whitespace checks passed.
The final artifact repeated the full 138-row rectangle/cell match. The local
closure receipt identifies dependency sources, features and linked artifacts.
