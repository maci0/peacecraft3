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
The all-stock natives, `RemoveUnitFromStock`, restock
delays, item categories, and marketplace enumeration stay unsupported.

## Item stock records

`AddItemToStock(unit, itemId, currentStock, stockMax)` and
`RemoveItemFromStock(unit, itemId)` are admitted with the slot natives, under the
same declaration-source rule. Each `(unit, itemId)` pair keeps the two integers
its last successful add passed. A repeated add overwrites them. `RemoveItemFromStock`
deletes the pair; an absent pair is a silent no-op. Item records and per-unit slot
records share `StockBudget::max_records`; a new record past the bound refuses with
`"stock item records"` or `"stock unit slots"` before `try_reserve`. The unit must
be a live `unit`, and other families refuse as `WrongFamily`.

Native evidence (Warcraft III.exe, SHA256 3f2ed012..570eed, read from the
existing objdump `.scratch/item-class-re/text.objdump`; addresses are VAs):

- Registration: `push $name; push $impl; call 0x8ccc80` at `0x49e1d3`/`0x49e1d8`
  registers `RemoveItemFromStock` (name `0xe72e24`) with impl `0x4a5160`;
  `AddItemToStock` (name `0xe72dd8`) with impl `0x489690`.
- `0x489690` and `0x4a5160` both convert the unit handle with `0x483940`. A null
  object returns silently. Otherwise they call `0x6d6570` on the unit object with
  a field id from `0xb37ba0`. A zero result returns silently, and a non-zero result
  is the shop component the add or remove runs on.
- Remove helper `0xb37f40`: scans 12 `item id` entries at shop `+0xd0`. A match
  clears the entry and calls `0x7fbbd0` and `0x46e070`. No match returns with no
  error path. Consequence: an absent pair is a silent success, so the host no-op
  matches native and is not a refusal.
- Add helper `0xb37430`: scans `count = [shop+0x480]` entries. A match skips
  insertion and goes to the stock write at `0xb374e2`. With no match and a full
  table, it selects an existing entry index by comparing per-entry values at
  `0x3b4` and `0x484`, then overwrites that entry. A full table therefore evicts
  rather than refusing. This host still refuses at `StockBudget` and is a known
  deviation.

Unqualified, so not modelled as native: the exact write of `currentStock` and
`stockMax` on a repeated add (the stock write at `0xb374e2`-`0xb37600` calls
helpers `0x9a1620`, `0x9a00b0` and virtual calls not yet read), the eviction key
meaning, whether a non-shop unit ever reaches a non-zero `0x6d6570` result, and
the effect of `currentStock` and `stockMax` on sale and restock.

## AddUnitToStock

Registration push at `0x49e1ab` (name `0xe72dfc`), handler `0x489dc0`. It differs from
`AddItemToStock` (`0x489690`) only in the component class: `0xb37bc0` returns `Asud`
(`0x41737564`), where `0xb37ba0` returns `Asid` (`0x41736964`) for items. A null unit or a
unit without that component returns silently. Otherwise it calls `0xb37690`, which
compared instruction by instruction with the item add `0xb37430` has the same scan,
eviction, stock write and age update at the same offsets (`+0x480` count, `+0xd0`
cells, `+0x484` ages). `SetUnitTypeSlots` (`0x4abc30` to `0xb38340`) likewise mirrors
`SetItemTypeSlots` (`0x4a9160` to `0xb38260`), including the clamp to `0xb` and the
clearing of cells beyond the new count. The same unsigned clamp and the 12-cell table of
`shop_table` therefore apply.

Recreation: the stock table of a unit is keyed by unit and kind, so `AddUnitToStock` and
`SetUnitTypeSlots` act on a second table that shares the item rules and the record budget
(`stock unit records` on overflow). `ObjectHost::unit_stock` reads a cell.

### Default slot count

Both component initializers store a default into the slot count field (`+0x480`): the `Asud`
class (name stub `0xb35380`, vtable at `0xf1b950`) at `0xb353d8`, and the `Asid` class (name
stub `0xb36140`) at `0xb36198`. Each is `mov dword [esi+0x480], 0xb`, so a shop that never
got a setter holds 11 cells of each kind. The initializers (`0xb35390`, `0xb36150`) sit in
the vtables (`0xf1b9b4`, `0xf1c058`). Observed: no other function of either class writes an
immediate into `+0x480`; the other writers are the slot setters (`0xb38340`, `0xb38260`) and
the save loaders (`0xb35474`, `0xb36234`). No code of either class reads a `UnitData` or
`ItemData` column into the count, and the helpers `0xb37430`, `0xb37690` and `0xb38340` are
called only by the natives and by `0xb380f7`..`0xb3818c`, so no data column overrides the
default.

Recreation: `shop_table::derive` reads the default from the executable. From the add handler
(`0x489690`, `0x489dc0`) it takes the class-code stub callee (`mov eax, code; ret`) and the
count field of the helper's `cmp [ebx+disp32], edi`, finds the vtable slot holding the second
copy of that stub, and takes the one vtable function that stores an immediate to that field.
`ShopTable::item` and `ShopTable::unit` each carry `slot_ceiling`, `entries` and
`default_slots` (`0xb`, `0xc`, `0xb` for both in 1.29.2.9231). A shop with no setter record
uses `default_slots` of its kind. TFT `UndeadX03` adds units to two `nmrk` marketplaces that
only received `SetItemTypeSlots` and now passes startup.

Open: the native returns silently for a unit that has no `Asid` or `Asud` ability (`0x6d6570`
finds none). The recreation does not check the ability list, so a unit without the ability
still stores stock.

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
