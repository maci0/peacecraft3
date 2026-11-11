# Map-defined objects on the campaign startup paths

Reviewed executable: `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, ImageBase `0x400000`.
Map data: `Maps\FrozenThrone\Campaign\HumanX02.w3x` and `UndeadX05.w3x` from `War3x.mpq`,
read from the nested archive (`HM3W` header, 512 bytes, then the MPQ).

## Object-file members

The seven `war3map.w3?` members and their families, confirmed against shipped bytes:

| Member | Family | Evidence |
|---|---|---|
| `w3u` | units | `HumanX02` base `haro`, custom `uaco` over `unpl` |
| `w3t` | items | `UndeadX05` custom `rat9`..`cnob` as `I000`..`I005` |
| `w3a` | abilities | `HumanX02` base `AIgo` |
| `w3b` | destructables | `HumanX02` base `LTe3`, `OrcX03b` bases `BTtw`, `LTba`, `DTrc` |
| `w3d` | doodads | |
| `w3h` | buffs | `OrcX03b` custom `B000` over `BHwe` with `fnam`, `ftip` |
| `w3q` | upgrades | |

`campaign_placement.rs` previously read `w3h` as the destructable member and used `w3b`
only when `w3h` was empty. Two shipped chapters carry both members
(`OrcX02_10`, `OrcX03b`), so their destructable overlay was the buff file. The catalog now
reads `w3b` alone. Test: `destructables_come_from_w3b_and_never_from_the_buff_member`.

## UndeadX05 `I003`: item rows read through the ability overlay

`UndeadX05`'s `war3map.w3t` has no base-table records and six custom records
`I000`..`I005`, each over a stock item (`I003` over `oven`) with one modification, `ipri`
(priority) set to 999. The map script gives `I003` to a unit with `UnitAddItemByIdSwapped` (`war3map.j:2103`).

`MetadataCatalog` already resolves the custom item through the item overlay: the
`Definition` carries `rawcode` `I003`, `base_rawcode` `oven` and the authored overrides.
`inventory_powerup::Rules::item_row` ignored that and looked up `rawcode` in the ItemData
table after checking it against the ability overlay, so any custom item raised
`Item: no shipped object I003`.

Fix: `item_row` reads the stock row named by `base_rawcode` and refuses a definition that is
not an item. `item_text` and `item_flag` return the definition's override for an `ItemData`
column when the map authored one, and the stock cell otherwise. An indexed override and two
fields naming one column refuse. Tests: `custom_items_read_their_stock_base_row`,
`custom_item_overrides_replace_the_base_cell`,
`custom_item_with_a_missing_base_row_or_wrong_family_refuses`.

Stock items with W3T overrides were affected the same way: their overrides were not read.

## HumanX02 `A001`: an id with no object data in a unit's ability list

`war3map.w3u` sets `nbal`'s `uabi` to `ACcr,Adsm,Awrs,A001`. No shipped table, no
`war3map.w3a` record (the member holds one base record, `AIgo`, and no custom records) and no
other member of the archive defines `A001` (byte search of all members: one hit, the `uabi`
string). The campaign archives carry no `war3campaign.w3a`.

Decoded from the executable (observed in code, not run):

- `UnitAddAbility` (registered with handler `0x4ae8f0`) calls `0xaf3ba0(unit, id, 0, 0, 0)`.
  That routine calls `0x6aefb0(id, 0)`, a lookup in the object table at `0x113f048`. A miss
  calls `0x6ace00(id)`, which inserts a node, loads the row, and returns 0 when no row
  exists (`[node+0x2c]` zero). `0xaf3ba0` then returns 0 without creating an ability. The
  nonzero flag argument of `0x6aefb0` would call `0x43eca0(0x57)` on a miss; `0xaf3ba0` passes 0.
- The unit creator at `0x66ffda` reads the unit row's ability list (offset `+0x178`),
  splits it on the separator set at `0xe9ddac` (space, comma, `-`, `_`, tab), converts each
  token with `0x467bb0` and calls `0xaf3ba0` on it. The result is not used. A token that
  names no object-data row therefore adds nothing and raises nothing.

Fix (`unit_hero_host.rs`, `Admission::defined`): at unit creation an `abilList` id with no
map record (custom or modified stock) and no shipped row is skipped. Every other failure of
that id still refuses, including a custom id over a missing base
(`a_custom_ability_over_a_missing_base_still_refuses_creation`). Tests:
`listed_ability_ids_without_object_data_are_skipped_at_creation`,
`listed_ability_ids_defined_by_the_map_are_still_resolved`.

Not decoded, still refused: `heroAbilList` entries with no row (the loop at `0x69b9e8` is a
different list walk), and `UnitAddAbility` on an id with no row (the recreation refuses it;
the original returns false from `0xaf3ba0`, but the caller's boolean is unverified).

## Other kinds

- `w3q` upgrades: `harvest_rules`, `upgrade_research`, `upgrade_effects` already resolve
  `overlay.new_object(alias)` before the table lookup. No change.
- `w3b` destructables and `w3d` doodads: `destructable_host` and `placed_doodads` resolve
  custom ids; only the member mapping above was wrong.
- `w3h` buffs: `ability_cooldown.rs` (`ability_buff_exists`, `placement_buff_code`,
  `buff_profiles`) reads the shipped table only, so a custom buff id would refuse with
  `no shipped object`. Not changed: only `OrcX02_10` and `OrcX03b` of the 97 probed chapters
  carry a `w3h`, and neither is in the selected install's campaign index, so no reachable
  chapter exercises it.
- `item_sales::derive` builds its rows from the shipped ItemData table only. A custom item
  has no sale row. Not reached yet: `UndeadX05` stops earlier (below).

## Remaining first refusals

- `HumanX02`: `CreateUnit` for `ubsp` (`war3map.j:1987`): `UnitBalance regenMana` is `-3`
  in the shipped row; the drain rule is the one `unit-regeneration.md` records as undecoded.
- `UndeadX05`: `UnitAddItem` (`Blizzard.j:2640`): `unsupported passive ability AIpb with
  code AIpb on oven`. The Orb of Venom poison on-hit effect is not decoded.
