# Passive held items: source audit, implemented hero attribute family, refusals

## Implemented behavior

A held passive item (`ItemData` `usable=0` and `powerup=0`) contributes the strength and agility
of its `AIab`-class abilities while held. The unit total is the retained powerup strength plus
the held passive totals, derived on demand from the held slots, so no add/remove state exists.

- Derived: damage (primary STR), armor and speed (AGI), and the level-up life delta.
- Equip and drop reprice life by the strength change with the current/max ratio policy, the
  same native path the powerup grant uses (`b3dd30` → `b3fd00` → mode 1 `6be8c0`).
- Transfer is a drop from the holder followed by an equip on the receiver. A non-hero receiver
  gets no life change, because only heroes carry strength pools.
- Death drops held items through the existing `forget_inventory_unit` path, which reprices life.
- Usable items (`usable=1`) are held without any passive bonus. Their activation is not
  implemented.
- Passive rows whose abilities are outside the admitted class, or whose intelligence bonus would
  need the unqualified mana path, refuse the pickup atomically with a diagnostic. No slot,
  event, life or held state is written before the refusal.

Stored versus derived totals is an internal recreation choice. The observable equip, drop,
transfer, death and level results are what the tests check.

## Source qualification

- Registration: the passive constructor is the window at `0xbc9ce0` (`push 0x11b6694`,
  `push "APbo"`, `push "AIab"`, `call 0x7f6360`, `add esp,0xc`, `ret`), the same layout as the
  powerup window `0xbca0e0` (`AIsm`, base `AIxm`). `Rules::read` decodes both windows from the
  reviewed client (SHA-256 `3f2ed012…`, `REVIEWED_IMAGE`).
- Attribute metadata: `Iagi`, `Iint` and `Istr` `useSpecific` lists must admit the item's
  ability alias. Amounts come from `Data{column}1` through `W3A` overlays. No ID or amount is
  written in code.
- The base class `APbo` is a native class with no ability row, so it is not checked.
- The item's ability alias (for example `AIa4`) and its code (`AIab`) differ. The class check uses
  the code, which the registration names.

## Source counts (selected catalog, `units/itemdata.slk`)

273 item rows. Passive rows are `usable=0` and `powerup=0`. Counted from the source flags, not
campaign placements:

| class | passive rows |
|---|---|
| Miscellaneous | 63 |
| Permanent | 37 |
| Artifact | 5 |
| Campaign | 1 |
| Charged | 1 |
| **with an ability list** | **107** |
| no ability list (held, no effect) | 26 |
| **total passive** | **133** |

Census gate `local_original_held_item_census_partitions_every_row` (`PCX_TOME_CLIENT`,
`PCX_TOME_CATALOG`, `--ignored`) prints `census passive rows with abilities=107 supported=13`
and, over all 273 rows, `none=140 supported=39 refused=94`. The 39 includes the 26 empty-list
passive rows (no effect). Refusals are all passive rows with an ability list (13 + 94 = 107):

| refusal | rows |
|---|---|
| passive ability of a family other than `AIab` (damage, life, mana, armor, attack speed, and others) | 79 |
| `AIab` row with an intelligence bonus (mana path unqualified) | 12 |
| `AIab` alias not admitted by the attribute metadata | 3 |

Campaign placement impact is not measured. These are catalog facts. The consequence for play is
that picking up any of the 94 refused rows now fails the pickup call. That is a loud,
diagnosed refusal, not a silent wrong stat.

## Original-source gates

- `hval` (strength 4, agility 4 from source rows) reads as supported with those amounts.
- `hcun` (intelligence bonus) refuses with the intelligence diagnostic.
- `crys` (usable) reads as not passive.

## Not verified and not implemented

- The native remove callback for `CAbilityStrengthMod`. Candidate entries `0xbc6c60`,
  `0xbc6d20` and `0xbc6d50` were surveyed without confirmation. The apply callback is `0xbc5520`
  at vtable `+0x430`. This is an internal implementation detail, not a fidelity gap, because the
  observable results are tested.
- Life and mana behavior on drop, and on death of a passive holder, has no original runtime
  capture here. The ratio policy on drop is the same native path as the grant, with the negative
  delta. It is a faithful derivation, not a measured observation.
- Intelligence bonuses and their mana pool (`b3feb0` route unqualified).
- Usable item activation (`UnitUseItem`) and its charges.
- Passive families other than `AIab`: damage (`AIat`), life (`AIml`), mana (`AImm`), armor (`AIsb`),
  attack speed (`AIas`), and the rest of the 79 refused rows.
- Non-hero passive equip: the recreation gives no life or stat change, which matches only the
  observable structure of hero-only attributes. Native behavior for non-heroes is not verified.
- Powerup `get_items` gating and non-hero powerup targets remain documented gaps in
  `inventory-powerup.md`. They are fidelity gaps, not permission gates.

## Armor family (`AIde`, Item Armor Bonus): implemented and tested

- Registration: window `0xbc9d40` (`push "APbo"`, `push "AIde"`, `call 0x7f6360`), same layout and base as
  the `AIab` window. Read by `Rules::read` as the third window; classes must be distinct.
- Amounts: metadata row `Idef` (`field Data`, `slk AbilityData`, `data 1` = `DataA`), `useSpecific`
  `AIde,AId1,AId2,AId3,AId4,AId5,AIda,AIdb`. Source rows `AId0`, `AId7`, `AId8` are not admitted
  and refuse. `forceNonNeg=0`, `minVal=0`; a negative authored value refuses.
- Held effect: flat armor points, summed over held passive items, derived per unit (any unit, hero or not).
  Not stored. Equip, drop, transfer and death follow the same held-slot derivation as strength.
- Interpretation: flat points added to unit armor before the existing armor reduction curve. Basis: the
  source's own comment for `AIde` ("continuous Defense Bonus"), integer amounts 1 to 10, and the existing
  curve in `combat_host.rs`. No native evidence for the exact flat addition was captured; it is a practical
  reading, and the observable damage results are what the gate checks.
- Damage hook: `combat_host.rs` adds `held_armor_bonus(target)` once to the target's armor in
  `roll_damage`. Hero armor derives from agility only, so held armor is not counted twice.
- Tests: parser (`armor_points_come_from_the_admitted_source_row`,
  `armor_row_outside_the_idef_admission_refuses`, `negative_authored_armor_refuses`); host
  (`held_armor_on_a_hero_is_derived_and_removed_on_drop`, `held_armor_on_a_non_hero_is_supported_and_derived`,
  `armor_transfer_moves_the_points_to_the_receiver`, `death_drops_held_armor_and_removes_its_points`,
  `armor_row_outside_the_admission_refuses_atomically`, `held_armor_changes_real_damage_and_its_drop_restores_it`).
  The damage test runs `roll_damage` with the held item, then after the drop. Overflow of the held armor
  sum is guarded (`checked_add`) but not tested.
- Non-hero refusal: now applies only to nonzero strength or agility. Non-hero armor is accepted.
