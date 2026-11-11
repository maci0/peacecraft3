# Ghoul lumber harvest (`Ahrl`, `CAbilityHarvestLumber`)

Image: Legacy `Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed` (1.29.2.9231). Static reads
only. Peasant harvesting is in [worker-harvest-orders.md](worker-harvest-orders.md), wisps in
[wisp-harvest.md](wisp-harvest.md).

## Which unit and ability (observed)

RoC `Undead05` orders `autoharvestlumber` (`Blizzard.j` `IssueImmediateOrderBJ`) on a `ugho`
(Ghoul). Shipped `Units\UnitAbilities.slk` lists `Acan,Ahrl,Aiun` for `ugho`. `Units\AbilityData.slk`
row `Ahrl` (code `Ahrl`, "Harvest Lumber", race undead): `Dur1` 1.35, `Area1` 900, `Rng1` 116, `DataA1` 2,
`DataB1` 20, `DataC1` `-`. The `Ahar` row has `Dur1` 1.1, `DataA1` 1, `DataB1` 10, `DataC1` 10.
`Units\AbilityMetaData.slk`: `Har1` (`DataA`) and `Har2` (`DataB`) apply to `Ahar,Ahrl,Ahr2,Ahr3`;
`Har3` (`DataC`) applies to `Ahar` only.

## Classes (observed)

Class vtables located through RTTI complete-object locators (type descriptors `0x110fc58`
Harvest, `0x110fc78` Lumber, `0x110fc9c` Return, `0x110fcc0` Base):

| Class | vtable | slot 7 class code | slot 32 data loader |
| --- | --- | --- | --- |
| `CAbilityHarvestBase` | `0xf2b37c` | `0xb75820` returns `Ahrb` | `0xb75870` |
| `CAbilityHarvestLumber` | `0xf2ba54` | `0xb77190` returns `Ahrl` | `0xb771a0` |
| `CAbilityHarvest` | `0xf2bda4` | `0xb77c30` returns `Ahar` | `0xb77c40` |

- Lumber loader `0xb771a0`: base ability loader `0x46c8c0`, then `Area` (real) into `[+0x190]`,
  `DataA` truncated by `0x9a07b0` into `[+0x1dc]` (damage to a tree), `DataB` truncated into
  `[+0x19c]` (lumber capacity). `DataC` is not read. The field-name pointers are `0x10dce00`
  (`Area`), `0x10dce1c` (`DataA`), `0x10dce20` (`DataB`), `0x10dce24` (`DataC`).
- `Ahar` loader `0xb77c40` calls the Lumber loader `0xb771a0` and then reads `DataC` into `[+0x198]`
  (gold capacity). So the peasant class is the lumber class plus gold; Lumber carries no gold.
- Virtual `+0x310` (`0xb76640`, "carries gold") is `[+0x198] > 0`; virtual `+0x314` (`0xb76650`,
  "carries lumber") is `[+0x19c] > 0`. Lumber and Harvest share both; for `Ahrl` `[+0x198]` is never
  written, so it is not a gold worker.
- Immediate validator: the Lumber class slot `0xb76a00` handles `0xd0031`, `0xd02b3`, `0xd0034`,
  `0xd02b4` itself and forwards every other order to the base `0xb75d00`. There `0xd0036`
  (`autoharvestlumber`) is accepted when `+0x314` is true and `0xd0035` (`autoharvestgold`) when
  `+0x310` is true; all other values give `0xdd`. A ghoul therefore accepts `autoharvestlumber` and
  refuses `autoharvestgold`. The start routine `0xb7a3f0` is the shared one in
  worker-harvest-orders.md.

## Recreation

`HarvestRules::from_alias_list` (`harvest_rules.rs`) resolves `Ahar` or `Ahrl` by class code. `Ahrl`
reads `DataA` (damage), `DataB` (lumber capacity), `Rng`, `Area` and `Dur` like `Ahar`, with gold
capacity 0 and no `DataC` read. A unit with both classes is refused as ambiguous. Every lumber
path (`autoharvestlumber`, explicit `harvest` on a tree, depot return, upgrades) runs on the same
`HarvestRules`; the gold paths already return false at gold capacity 0.

Inferred or open:

- Ghoul-specific values beyond the data rows (carry animation tags, `Dur` use) are the same as
  for the peasant path; only the class codes and loaders above were read.
- The second function returning `Ahrl` at `0xb79da0` is not referenced by a vtable and was not
  decoded.
- Selection of the upgrade list applied to ghouls (`upgradeList`) is generic and was not checked
  against the shipped ghoul row.
