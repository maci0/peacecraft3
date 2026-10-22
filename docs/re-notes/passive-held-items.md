# Passive held items: source audit, implemented attribute families, refusals

## Implemented behavior

A held passive item (`ItemData` `usable=0` and `powerup=0`) contributes the strength and agility
of its `AIab`-class abilities, flat armor from `AIde`, flat attack damage from `AIat`, and
life regeneration from `Arel` while
held. The unit total is the retained powerup strength plus the held passive totals, derived on
demand from the held slots, so no add/remove state exists.

- Derived: damage (primary STR plus held flat damage), armor and speed (AGI), and the level-up
  life delta.
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
  powerup window `0xbca0e0` (`AIsm`, base `AIxm`). `Rules::read` decodes the powerup, passive,
  armor and damage windows from the reviewed client (SHA-256 `3f2ed012…`, `REVIEWED_IMAGE`).
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

The counts below predate the armor and damage families and are retained as the initial audit;
rerun `local_original_held_item_census_partitions_every_row` for current counts.

| refusal | rows |
|---|---|
| passive ability of a family other than `AIab` (life, mana, attack speed, and others) | 79 |
| `AIab` row with an intelligence bonus (mana path unqualified) | 12 |
| `AIab` alias not admitted by the attribute metadata | 3 |

Campaign placement impact is not measured. These are catalog facts. The table is the initial
audit, before armor and damage rows were admitted; rerun the gate for current counts. A row still
outside the admitted families fails the pickup call with a diagnostic instead of applying a
silent wrong stat.

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
- Passive families other than `AIab`, `AIde`, `AIat` and `Arel`: life (`AIml`), mana (`AImm`), armor
  (`AIsb`), attack speed (`AIas`), and the rest of the refused rows.
- Non-hero passive equip: strength and agility still produce no hero pools, while armor and flat
  damage are accepted for any unit. Native behavior for non-hero strength/agility is not
  verified.
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

## Damage family (`AIat`, Item Attack Damage Bonus): implemented and tested

- Registration: window `0xbc9c80` (`push "APbo"`, `push "AIat"`, `call 0x7f6360`), the same
  reviewed layout and base as `AIab` and `AIde`. `Rules::read` decodes it as the fourth window;
  all four classes must be distinct.
- Amounts: metadata row `Iatt` (`field Data`, `slk AbilityData`, `data 1` = `DataA`), whose
  `useSpecific` list admits the item's ability alias. Shipped aliases include `AItf` (15) and
  `AIat` (20); the amount is read from `DataA1`, never a constant, and typed W3A overrides win.
- Held effect: flat attack damage, summed over held passive items and derived per unit (hero or
  not). Negative amounts, aliases outside `Iatt`, and an overflowing sum refuse the pickup before
  any write.
- Pickup and transfer also check the receiving holder's combined passive totals
  before changing slots or ownership. Two individually valid items whose sum
  overflows refuse at that boundary; a refused transfer retains the donor's item.
- Damage hook: `combat_host.rs` carries the held total as `HeroBonuses.held_damage` and adds it
  once to `roll_damage`, after the primary-attribute damage. It is not derived from strength, so
  it does not change the attribute query or the life pools.
- Tests: parser (`flat_damage_comes_from_the_admitted_iatt_column`,
  `w3a_override_on_the_iatt_field_wins_at_effect_level`,
  `damage_row_outside_the_iatt_admission_refuses`, `negative_or_malformed_authored_damage_refuses`,
  `repeated_damage_aliases_overflow_atomically`, `damage_registration_must_name_a_distinct_class`);
  host (`held_flat_damage_stacks_transfers_and_removes`, `death_drops_held_flat_damage`,
  `damage_row_outside_admission_and_overflow_refuse_atomically`,
  `held_flat_damage_changes_real_damage`).

## Next selected startup refusal: life regeneration

The selected Human01 victory/Continue test on `5d1f3ef` passes the former
`ratf` damage-item refusal and stops at `rlif`, whose passive ability is
`Arel`. The diagnostic is emitted by `UnitAddItem` in `Blizzard.j:2640`.
This is a real startup run with the selected Legacy install, not a static
prediction. Its exit status is 101; the local receipt is
`.scratch/campaign-continuation/held-damage-selected.log`.

The reviewed image registers `Arel` at `0xbca040`, with base `AREB`, through
the same immediate class-registration shape. Shipped metadata `Ihpr` maps
an integer Data field to `Arel,Arll`; its editor string
`WESTRING_AEVAL_IHPR` is `Hit Points Regenerated Per Second`. This establishes
the source field and its unit, not eligibility, stack interaction or timing.
The existing regeneration consumer integrates base rates and hero attributes
on the simulation clock. A held-item contribution must respect equip/drop/
transfer boundaries, avoid integrating a new rate over time before pickup,
and keep native interaction with the unit's regeneration type explicit.

## Life-regeneration implementation and original observation

The class and amount now follow the reviewed registration and `Ihpr` metadata,
including typed level-one W3A overrides. Held rates are checked integer sums;
negative, malformed, unadmitted and overflowing amounts refuse. Rates change
neither current nor maximum life immediately. The existing logical-clock
regeneration consumer applies the held contribution while alive and unpaused,
clamps at maximum life, and derives ownership from current inventory records.
Pickup, transfer and drop require existing pools to be projected to the current
game clock before changing a nonzero held rate. This is a host boundary that
prevents retroactive regeneration; the normal activity path projects pools
before processing inventory orders.

A controlled run on the reviewed executable used `-classic` and read back RoC.
Map SHA-256:
`0e860b51f81ab4d9456e30b2bfa30b0586d98c0f9d9fa9d572203a3c9504ce3f`;
script SHA-256:
`a9060752c9a30d3e5a5925c24a791328d382e1e996a30fd6409b180b306240a4`.
The Human02 data stayed intact, but authored units/triggers were omitted and
two matching fresh Demon Hunters were created. One received two rings; the
other was the unordered control. Both began at the same life. Time of day was
held at noon, then midnight; rings were dropped and one was re-equipped.
All 16 bounded marker records arrived and all three pickup calls returned true.

| Two-second interval | Life gain relative to control, per second |
| --- | --- |
| Day, two rings | approximately 4 |
| Day, one ring | approximately 2 |
| Day, no rings | approximately 0 |
| Night, no rings | 0 |
| Night, one ring re-equipped | approximately 2 |

The records use three-decimal native string output; tiny differences in the
last decimal are retained in the receipt rather than treated as exact rates.
The selected `Custom_V0` table gives the ring ability's amount as two. The
Demon Hunter has night-only base regeneration of 0.5 and strength 19; the
selected Misc profile supplies strength regeneration 0.05. Control gains were
approximately 0.95 per second by day and 1.45 at night. Thus base eligibility
gates the unit's base rate, while the strength and held-item contributions
continue independently. The consumer now preserves that separation.

Local inputs, member hashes, exact records and derived differences are under
`.scratch/passive-life-probe/`. This is modified-map behavior evidence on the
Wine reference, not unchanged Human02 acceptance, TFT qualification, native
Windows parity or a rendered comparison. Other regeneration eligibility modes,
negative map edits and mixed unmodeled passive classes remain separate limits.

The selected Human01 score/Continue gate now passes the former `rlif` refusal
and reaches `rwiz`, whose `AIrm` passive remains unsupported at
`Scripts\Blizzard.j:2640`. This gate stimulates the selected victory BJ and
Continue control; it does not prove Human01 mission completion or successful
Human02 startup. The failed run is retained locally as
`.scratch/campaign-continuation/life-item-selected.log` (exit 101).

A separate controlled RoC run repeated the matched-hero protocol with `rwiz`.
Map SHA-256:
`7374aeed7be4dd946097f4b5fdd743aed4de1257439648526e1b05f7e756b57a`;
script SHA-256:
`e600439fc832220361da96a5a47bc50bd87ac5f215ec1ccb677faa0b057806bc`.
All 16 records arrived. Relative to the control, two held items added about
0.810 mana per second and one added about 0.4045, with approximately zero
difference after both were dropped. Re-equipping one at night restored the
one-item difference. These observations do not alone determine which base and
attribute terms the percentage modifies. Exact records and limitations remain
under `.scratch/passive-mana-probe/`; the mana passive is not yet implemented.
