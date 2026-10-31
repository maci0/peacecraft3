# Region cell shape is unmeasured

Script regions still store the rectangle identity and the bounds copied at `RegionAddRect`. They do not rasterize cells, test membership, or deliver enter and leave events. `RemoveRegion`, `RegionClearRect`, and `IsPointInRegion` stay unsupported in `region_host_tests.rs` (`owned_vm_retains_call_time_bounds_and_both_subscription_kinds_without_evaluation`). `RegionAddCell`, `RegionAddCellAtLoc`, `RegionClearCell`, `RegionClearCellAtLoc`, `IsUnitInRegion`, and `IsLocationInRegion` have no host.

## Measured, and not this cell

`war3map.w3r` records are four signed integers (version below 2) or four floats, plus the versioned name and optional fields in `crates/pcx-map/src/regions.rs`. There is no circle or polygon variant. Those bounds may later be wrapped as ordinary `rect` values. That wrap does not define a region cell.

`region_cells` (`crates/pcx-play/src/native_spatial_chains.rs`) maps one finite ordered rectangle onto a caller-supplied spatial grid: native axis 0 is world Y, cell index is `floor((coordinate - origin) / cell_size)` in the session real arithmetic, the maximum cell is included by adding one after the floor, and the result is clipped to the grid. `cell_size` must be a normal positive power of two. Placement supplies that size as `1 << exponent` from the selected executable. The reach-cache owner uses this conversion when a tracked `RegionAddRect` is inserted (`spatial_pool.rs`, addresses `0x801a50`, `0x801930`, `0x8c23f0` in `ai-construction.md`). That is the spatial-chain rectangle. It is not a measurement of the JASS region cell.

## Missing measurement

These are unread, so the region host does not implement them:

1. World shape of one JASS region cell: whether it is a square, its side lengths, and whether the pitch is the placement-cell size or another value.
2. Origin and axis orientation of that grid in map coordinates.
3. Which cells `RegionAddRect` and `RegionClearRect` cover, including a minimum, a maximum, and a coordinate that lands on a cell boundary.
4. Which single cell `RegionAddCell` and `RegionClearCell` select for a point on a boundary.
5. Whether `IsPointInRegion` tests that cell or the continuous rectangle, and whether a later `SetRect` or `MoveRectTo` changes cells already stored.

A 32-unit square, or reuse of `region_cells`, would invent that shape. Runtime membership and cell edits stay refused until the selected executable shows the instruction window, the operands, and at least one boundary observation.

## Rect natives checked in the same pass

`rect_host` already executes `Rect`, `SetRect`, `MoveRectTo`, `MoveRectToLoc`, `RemoveRect`, the six bound and center queries, and `GetWorldBounds` after an admitted full-map extent. `rect_host_tests.rs` refuses unordered, non-finite, null, stale, wrong-family, and unadmitted calls. No admitted native whose argument is a live rect refuses an ordered finite operation there. `RectFromLoc` appears only as two nulls in the unsupported-context test, which is not a specified rectangle. `SetRectFromLoc` is not admitted. Neither was added.
