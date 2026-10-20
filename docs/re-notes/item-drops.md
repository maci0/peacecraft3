# Item drops and drop origins

Status: campaign placed-unit drops are script-generated; the engine path that
runs them is not exercised by a test in `pcx-play`. Destructable drops have no
evidence in the campaign.

## Script drop calls (selected `Blizzard.j`)

- `RandomDistReset` / `RandomDistAddItem` / `RandomDistChoose` (`Blizzard.j:10133-10199`)
  are script functions, not natives. `RandomDistChoose` draws `GetRandomInt(1, sum)`
  over the cumulative chances and returns the first entry whose cumulative sum reaches
  the draw. It returns `-1` for an empty list. One draw is consumed even for a single
  entry. Chances are relative weights.
- `UnitDropItem` (`Blizzard.j:10206`) draws `GetRandomReal` twice for an x and y within
  32 units of the unit, calls `CreateItem`, then `SetItemDropID(item, GetUnitTypeId(unit))`
  and `UpdateStockAvailability`. `UnitDropItemPoint`, `Slot` and `Target` are declared in
  `common.j:1564-1566` but no campaign drop calls them. `pcx-play` does not implement them.
- `UpdateStockAvailability` (`Blizzard.j:9944`) reads `GetItemType` and `GetItemLevel`.
  It never reads the drop origin. No script-visible getter exists for `SetItemDropID`.

## Generated campaign drops

The campaign map scripts (`campaign-placements/j/*.j`, local, not committed) contain
one `Unit<N>_DropItems` function per placed unit with a drop set:

- Registered by `TriggerRegisterUnitEvent(t, <unit>, EVENT_UNIT_DEATH)`.
- Returns early when `IsUnitHidden(trigUnit)`.
- Emits, per binary set, `RandomDistReset`, one `RandomDistAddItem(rawcode, chance)` per
  entry, `RandomDistChoose`, and `UnitDropItem(trigUnit, itemID)`.
- An empty set emits nothing.

Across the twelve campaign scripts (nine contain drops): 96 drop functions, 50 `RandomDistAddItem` calls, all with
chance `100`, all single-entry sets. No multi-entry set and no non-100 chance occurs in
the campaign. Multi-entry behavior therefore rests on the `RandomDistChoose` source
above, not on a shipped campaign case.

## Binary placement drop sets (`war3mapUnits.doo`)

`crates/pcx-map/src/units.rs` reads each set as word pairs. Each pair is
`[item rawcode, chance]`, with the rawcode the little-endian `u32` of the four file bytes.
Gate: `crates/pcx-map/tests/campaign_item_sets.rs` (ignored; set
`PEACECRAFT3_CAMPAIGN_UNITS` and `PEACECRAFT3_CAMPAIGN_SCRIPT` from one map). It checks
that the multiset of binary pairs equals the multiset of `RandomDistAddItem` calls in that
map's script. The order is established by that check, not by the synthetic fixture.

Local Human01 evidence: 161 placed units; four have drop-set words. Creation 12 has one
empty set. Creations 79, 94 and 37 each hold one entry at chance 100 (`pman`, `shea`, `phea`).
These match the three non-empty generated drop functions.

## Link from binary placement to runtime handle

- `CreateAllUnits` in the generated script creates the runtime unit handles
  (`CreateUnit` for each placement). The binary placement's drop sets are not read by
  `pcx-play` and do not attach to those handles.
- The generator names named units `gg_unit_<type>_<creation>`, with `<creation>` equal to
  the binary creation number (79, 94). Drop-trigger numbers (`Unit000025`) are not the
  creation number.
- Units created by `CreateNeutralHostile` have no `gg_` name, so the name link does not
  cover them. The mapping for those relies on the generated order and the sets.
- Runtime drop behavior therefore comes from the generated script, not from the binary
  sets. The binary sets are the generator's input.

## Native behavior (not established here)

Whether the engine also drops binary sets on death, which would double-drop, is not
shown by the campaign scripts. REA queries on the selected executable:

- `0x551cc0`: the containing function is `FUN_00551870`, a race-preference routine that
  calls `RANDOM`, `HUMAN`, `NIGHT_ELF` and `UNDEAD` strings. It does not read item sets.
  The repo citation of `0x551cc0` as drop framing is wrong.
- `0x5520d0`: see the result recorded below.

## Doodads version 8

`crates/pcx-map/src/doodads.rs` accepts version 8: a signed `table_id`, a set count,
then per set an item count and `[word, word]` pairs, ahead of the creation number. No
shipped file is version 8 (132/132 are version 7). The version 8 layout and pair order
are unproven. The parser behavior and the docs now agree on that.

## Natives on the generated path

Implemented in `pcx-play`: `GetTriggerUnit`, `GetTriggeringTrigger`, `DestroyTrigger`,
`IsUnitHidden`, `GetUnitTypeId`, `TriggerRegisterUnitEvent`, `GetRandomInt`,
`GetRandomReal`, `CreateItem`, `SetItemDropID`, `GetItemType`, `GetItemLevel`. Not
exercised end to end: no test runs a generated `Unit<N>_DropItems` in the VM.

## Remaining

1. Execution gate: run a campaign map's drop trigger through the VM, kill the unit, and
   compare the created item and the RNG draw sequence with the original.
2. Native check of double drops (above).
3. Destructable drops: no campaign evidence; doodad version 8 remains unproven.
