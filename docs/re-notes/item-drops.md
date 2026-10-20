# Item drops and drop origins

Status: retained setter state only. No death or destruction drop is implemented.

## Drop origin (`SetItemDropID`)

- `common.j:1448` declares `native SetItemDropID takes item whichItem, integer unitId returns nothing`.
  No getter for it is declared in `common.j` or `Blizzard.j`.
- Only caller in the selected scripts: `Blizzard.j:10226`, inside `UnitDropItem`
  (`Blizzard.j:10206`). It passes `GetUnitTypeId(inUnit)` after `CreateItem`. The item
  position is two `GetRandomReal` draws within 32 world units of the unit.
- `UnitDropItem` then calls `UpdateStockAvailability` (`Blizzard.j:9944`). That function
  reads `GetItemType` and `GetItemLevel` only. It does not read the drop origin.
- `WidgetDropItem` (`Blizzard.j:10233`) creates the item without `SetItemDropID`.
- Native effect of the stored integer: not observed in any shipped script or evidence
  file. The engine behavior that consumes it is unverified.
- Code: `crates/pcx-play/src/item_drop_host.rs` (`SetItemDropID`, budget
  `max_records`), `item_drop_tests.rs`, and the consumed-powerup refusal in
  `inventory_powerup_host_tests.rs`.

## Authored drop sets

Parsed, not consumed by the runtime:

| Source | Parser | Shape | Runtime consumer |
|---|---|---|---|
| Placed unit (`war3mapUnits.doo`) | `crates/pcx-map/src/units.rs` | `item_table: Option<i32>` (subversion > 10), `item_sets: Vec<Vec<[u32; 2]>>` | `ImportedRecord.placement` in `pcx-sim` keeps the value; `pcx-play` does not read it |
| Doodad, version 8 (`war3map.doo`) | `crates/pcx-map/src/doodads.rs` | `ItemDropData { table_id: i32, sets }` | none |
| Map item table (`war3map.w3i`, version 0x18+) | `crates/pcx-map/src/info.rs` | `ItemTable { id, name, sets }` | none |

Pair semantics (`[u32; 2]`): the field order and the meaning of each word are not
established by a shipped-byte or native check. `units.rs` and `info.rs` decode the two
words without naming them.

Discrepancy: `doodads.rs` module prose and `DOO_VERSION` doc say version 8+ is refused,
but `Doodads::from_bytes` accepts version 8 and reads an item-table id and sets before the
creation number. The version-8 layout has no shipped-byte proof in the repo. Either the
prose or the parser changes, not both by guess. Recorded for root.

## Not established

- Roll rule: how a set chooses items (one pick per set, independent per-item chance,
  or cumulative ranges), the `chance` unit, and whether a missing remainder means no drop.
- Table resolution: what `table_id` and `ItemTable.id` mean, and whether `-1` is the only
  "none" value.
- Trigger points: which natives reach the drop path (`KillUnit`, lethal `roll_damage` in
  `combat_host.rs`, `KillDestructable`, `RemoveUnit` with no drop).
- Drop position and the RNG draw order for the two `GetRandomReal` calls in
  `UnitDropItem`. The draws come from `script_random_host.rs`.

## Native evidence needed

A Ghidra function dossier on `Warcraft III.exe` (sha256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, 1.29.2.9231) for the
unit-death drop routine and the item-set reader. Start from the framing addresses
cited in `crates/pcx-map/src/units.rs:1` (`0x551cc0`, `0x5520d0`).
Needs a root Ghidra slot. No such dossier exists in `.scratch` as of this note.

## Next bounded step

1. Native dossier for the roll rule and the trigger points above.
2. Then one vertical slice: `pcx-play` reads the placement's `item_sets` and `item_table`,
   rolls them on `KillUnit` and lethal damage with `script_random_host`, and creates items
   through the existing `CreateItem` admission path. Destructable drops follow the same rule
   through `KillDestructable` and the version-8 doodad record once that layout is proven.
