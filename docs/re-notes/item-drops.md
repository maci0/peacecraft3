# Item drops and drop origins

Status: campaign placed-unit drops run through the generated death trigger. The gate
below runs that trigger's action body in the VM. The death event itself and the full
map startup are not exercised, because the production Host cannot initialize the
`common.j` globals yet (see Frontier).

## Script drop calls (selected `Blizzard.j`)

- `RandomDistReset`, `RandomDistAddItem` and `RandomDistChoose` (`Blizzard.j:10133-10199`)
  are script functions, not natives. `RandomDistChoose` draws `GetRandomInt(1, sum)`
  over the cumulative chances and returns the first entry whose cumulative sum reaches
  the draw. It returns `-1` for an empty list. It consumes one draw even for a single
  entry. Chances are relative weights.
- `UnitDropItem` (`Blizzard.j:10206`) draws `GetRandomReal` twice for an x and y within
  32 units of the unit, calls `CreateItem`, then `SetItemDropID(item, GetUnitTypeId(unit))`
  and `UpdateStockAvailability`. `UnitDropItemPoint`, `Slot` and `Target` are declared in
  `common.j:1564-1566`. No campaign drop calls them, and `pcx-play` does not implement them.
- `UpdateStockAvailability` (`Blizzard.j:9944`) reads `GetItemType` and `GetItemLevel`.
  It writes only `bj_stockAllowed*`. No script-visible getter exists for `SetItemDropID`.

## Generated campaign drops

The campaign scripts (`campaign-placements/j/*.j`, local, not committed) hold one
`Unit<N>_DropItems` function per placed unit with a non-empty drop set:

- Registered by `TriggerRegisterUnitEvent(t, <unit>, EVENT_UNIT_DEATH)`. The unit is
  either a `gg_unit_<type>_<creation>` global or a local `u` from the creation call.
- Returns early when `IsUnitHidden(trigUnit)`.
- Emits, per binary set, `RandomDistReset`, one `RandomDistAddItem(rawcode, chance)` per
  entry, `RandomDistChoose`, and `UnitDropItem(trigUnit, itemID)`.
- Calls `DestroyTrigger(GetTriggeringTrigger())` at the end.
- An empty set emits nothing.

Across the twelve campaign scripts (nine contain drops): 96 drop functions and 50
`RandomDistAddItem` calls, all with chance `100`, all single-entry sets. Multi-entry
behavior therefore rests on the `RandomDistChoose` source above, not on a shipped case.

## Binary placement drop sets (`war3mapUnits.doo`)

`crates/pcx-map/src/units.rs` reads each set as word pairs `[item rawcode, chance]`. The
rawcode is the little-endian `u32` of the four file bytes. Gate:
`crates/pcx-map/tests/campaign_item_sets.rs` (ignored; set `PEACECRAFT3_CAMPAIGN_UNITS`
and `PEACECRAFT3_CAMPAIGN_SCRIPT`). It checks that the multiset of binary pairs equals the
multiset of `RandomDistAddItem` calls in the same map. Mutation check: swapping the pair
order fails it with `left: [(100, 1634035824), …]`, and the restored gate passes.

Human01 evidence: 161 placed units; four have a set. Creation 12 has one empty set.
Creations 79 (`nban`), 94 (`nban`) and 37 (`ngno`) each hold one entry at chance 100:
`pman`, `shea` and `phea`.

## Link from placed unit to generated trigger

- Creation 79 is `gg_unit_nban_0079` at `(508.5, -1147.9)`. Creation 94 is
  `gg_unit_nban_0094` at `(-348.9, 556.9)`. Both positions equal the placed record.
- Creation 37 (`ngno`, `(301.27, -4120.17)`) matches the unnamed
  `CreateUnit(p, 'ngno', 301.3, -4120.2, …)` in `CreateNeutralHostile`. Its trigger
  `Unit000057_DropItems` registers on the local `u`. The link is by type and position,
  not by name. The placed z (684.16) differs from the script's z (54.6). The script's z
  is not checked against the placed z.
- Trigger numbers (`Unit000025`, `Unit000037`, `Unit000057`) are not creation numbers.
- `CreateAllUnits` creates the runtime handles. `pcx-play` does not read the placed
  `item_sets`. The generated script carries the sets into the runtime, so the binary
  record is the generator's input.

## VM gate

`crates/pcx-play/tests/campaign_drop_execution.rs` (explicit local gate, four env paths).
It loads the selected `common.j`, `Blizzard.j` and the map's `war3map.j`, runs global
initialization, and calls each drop trigger directly. Host natives are recorded, and
random draws return the lower bound. The gate passes (exit 0, 4 tests):

- creation 79: one `RandomDistChoose` draw `(1, 100)`, two position draws around the
  placed `(x, y)`, one `CreateItem('pman')` at the lower bounds, one `SetItemDropID` with
  the `nban` type id, and one `DestroyTrigger`.
- creation 94: `shea`, same shape.
- creation 37 (unnamed): `phea`, same shape.
- hidden unit: no draws, no item, and one `DestroyTrigger`.

Not exercised: the `EVENT_UNIT_DEATH` delivery, the RNG values (the host stream returns
the lower bound), the item's position after creation, and the stock arrays.

Harness values, not evidence:
- Player-slot queries (`GetPlayerNeutralPassive` and related) return `0`. Global
  initialization needs them. The drop-phase assertion requires that none of them run
  during the drop.
- `GetItemType` returns `ConvertItemType(0)`. It feeds only `bj_stockAllowed*`.
- `CreateTimer`, `CreateGroup` and other handle allocations only return a fresh token.
- `EnumHost` supplies the `Convert*` conversions, with `ConvertMapSetting` and
  `ConvertMapVisibility` omitted because `common.j` declares no constants for them.

## Frontier

1. The production Host cannot run `common.j` globals. `GetPlayerNeutralPassive` remains
   unsupported (`docs/re-notes/native-neutral-players.md`), so no campaign script,
   including this trigger, starts end to end in the Host.
2. `EVENT_UNIT_DEATH` delivery from `KillUnit` and lethal damage to the registered
   trigger. The `ObjectHost` death path (`combat_host.rs:508`) is the hook. Delivery is
   not gated.
3. Random values. The gate fixes the stream at the lower bound. Value parity needs the
   selected RNG policy in this path.
4. Native double-drop: whether the engine also drops binary sets on death. Not established;
   REA query `0x5520d0` (see Native) does not answer it.
5. Destructables: no campaign drop evidence. Doodad version 8 is unproven.

## Native

Two REA queries on the selected executable (`3f2ed012…570eed`, cached Ghidra snapshot,
`REA_ANALYSIS_PROVIDER=ghidra`; results in `.scratch/rea-0x551cc0.txt` and
`.scratch/rea-0x5520d0.json`):

- `0x551cc0` resolves to `FUN_00551870`, a race-preference routine that reads
  `AW4RACE_PREFERENCE` and the `RANDOM`, `HUMAN`, `NIGHT_ELF` and `UNDEAD` strings.
- `0x5520d0` resolves to `FUN_00552040`, which reads game option flags
  (`LockTeamsValue`, `RandomRacesValue`, `MapVisibilityValue`, and others).

Neither reads placed-unit records. The earlier citation of these addresses as the item
drop framing was wrong, and `units.rs` no longer cites them.

## Doodads version 8

`crates/pcx-map/src/doodads.rs` accepts version 8: a signed `table_id`, a set count,
then per set an item count and two-word pairs, ahead of the creation number. No shipped
file is version 8 (132/132 are version 7). The version 8 layout and pair order are
unproven. The parser and the docs agree on that.
