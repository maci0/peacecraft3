# Shop slot configuration

`InitBlizzard` calls `InitNeutralBuildings`. That function passes the values of the two
script globals `bj_MAX_STOCK_ITEM_SLOTS` and `bj_MAX_STOCK_UNIT_SLOTS` to
`SetAllItemTypeSlots` and `SetAllUnitTypeSlots`, then creates
`bj_stockUpdateTimer` and calls `TimerStart`. The declarations are the four
non-constant slot natives in common.j beside `AddItemToStock` and
`RemoveItemFromStock`. There is no slot getter in that declaration block.

`ObjectHost::with_stock` admits those four natives together. Each one must
return nothing, use the declared parameter types, and share `CreateTimer`'s
source. A function of the same name, a second declaration source, or a
different signature refuses admission and publishes no slot state. The
per-unit record bound is checked with `Layout` before the state exists.
`usize::MAX` refuses as capacity. Admission does not allocate a handle.

Once admitted, the host stores the integer the call actually passed.
`Ok(None)` means that assignment has not succeeded. `Some(0)` is an executed
zero, not a default. A later successful call to the same global or the same
unit and kind replaces the integer. A failed call leaves the previous integer
in place. Distinct `(unit, item-or-unit kind)` records share one budget.
Replacing an existing pair does not consume another record. A new pair at the
limit refuses before `try_reserve`. Slot calls do not allocate handles.

A per-unit record does not read the global integer, and a global assignment
does not rewrite per-unit records. That separation is the retained state, not
a measured rule about defaults, ceilings, or existing marketplace units.
`AddUnitToStock`, the all-stock natives, `RemoveUnitFromStock`, restock
delays, item categories, and marketplace enumeration stay unsupported.

## Item stock records

`AddItemToStock(unit, itemId, currentStock, stockMax)` and
`RemoveItemFromStock(unit, itemId)` are admitted with the slot natives, under the
same declaration-source rule. Each `(unit, itemId)` pair keeps the two integers
its last successful add passed. A repeated add replaces them. `RemoveItemFromStock`
deletes the pair and refuses `"no item stock record"` when none exists, so no
removal is reported as success without a record. Item records and per-unit slot
records share `StockBudget::max_records`; a new record past the bound refuses with
`"stock item records"` or `"stock unit slots"` before `try_reserve`. The unit must
be a live `unit`, and other families refuse as `WrongFamily`. A `unit` accepts any
live unit, not only a marketplace: the native rule for that is unmeasured.

The stored integers are the call arguments, not a measured stock state. The
native merge rule for a repeated pair, the effect of `currentStock` and
`stockMax` on sale and restock, and whether `AddItemToStock` is refused for a
non-marketplace unit are unverified.

`UpdateStockAvailability` (Blizzard.j 9944) reads only `GetItemType` and
`GetItemLevel`, which were already admitted, and is not affected by this slice.

Still unsupported on the purchase path: `ChooseRandomItemEx`, `IsItemIdSellable`,
`GetRandomInt`-driven `PerformStockUpdates` through `StartStockUpdates` (the
timer callback needs `TimerStart` expiry delivery), `GroupEnumUnitsOfType` over
`"marketplace"`, `EVENT_PLAYER_UNIT_SELL_ITEM` with `GetSellingUnit`/`GetSoldItem`,
and the `RemovePurchasedItem` action. Purchase gold transfer and item transfer
are native behavior with no evidence in this slice, so no sale is simulated.
Sources: `common.j` 1683-1691, 1231-1236, 1443, 1990; `blizzard.j` 9944-10079.

The same source-qualified Human01 Program/localization gate now retains both
executed global integers, checks them against the observed call arguments,
and refuses `TimerStart` in Blizzard.j at `447183..447286` (common.j declaration
`61987..62103`). The stock timer is the next identity on the existing arena;
repeated initialization preserves the post-main timer and group values and
makes no new native calls. The input uses the previously qualified explicit
compatibility catalog, not an inferred target-native catalog.

All 376 native and 376 executed WASI play tests, strict native/browser Clippy,
78 map-loading tests, four app-library tests, warning-denied documentation and
format checks pass. Evidence is local under
`.scratch/stock-slots-root-20261017/`. The initial test compile failure and old
verifier snapshot failure are retained there. Shop simulation and native
acceptance of zero/negative inputs or global/per-unit precedence remain open.
