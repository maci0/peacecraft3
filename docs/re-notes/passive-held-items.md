# Passive held items: source audit, implemented attribute families, refusals

## Implemented behavior

A held passive item (`ItemData` `usable=0` and `powerup=0`) contributes the strength, agility and
intelligence of its `AIab`-class abilities, flat armor from `AIde`, flat attack damage from `AIat`,
maximum life from `AIml`, life regeneration from `Arel`, and a mana-regeneration fraction from
`AIrm` while held. The unit total is the retained powerup strength plus the held passive totals,
derived on demand from the held slots, so no add/remove state exists.

- Derived: damage (primary STR plus held flat damage), armor and speed (AGI), and the level-up
  life delta.
- Equip and drop reprice life by the strength and maximum-life changes with the current/max
  ratio policy, the same native path the powerup grant uses (`b3dd30` → `b3fd00` → mode 1
  `6be8c0`). The ratio is rounded with half upward.
- Equip and drop reprice mana by the intelligence change times the admitted
  intelligence-to-mana coefficient with the same ratio and half-up rounding. Mana may be zero,
  so it has no living-life minimum of one.
- The projected pools are written as prepared. A pure-intelligence change preserves current and
  maximum life exactly, including a living holder below one life, and a pure life change preserves
  unrelated current mana.
- Transfer is a drop from the holder followed by an equip on the receiver. Strength and
  intelligence apply only to heroes; maximum life applies to any receiver with vitals. A pure
  intelligence item is accepted on a non-hero but has no modeled attribute or mana effect.
- Death drops eligible held items through the existing `forget_inventory_unit` path, which
  reprices life, maximum life and mana. A held item that is not dropped retains its
  maximum-life and mana contributions. Death itself does not clear current mana, and removal
  while dead still reprices mana by the rounded ratio instead of forcing zero.
- Orbs (`AIfb`, `AIgd`, `AIcb`) add their flat damage and run their on-hit effect; see "Orb
  families" below. A holder takes at most one orb.
- Usable items (`usable=1`) are held without any passive bonus. Their activation is not
  implemented.
- Passive rows whose abilities are outside the admitted classes refuse the pickup atomically
  with a diagnostic. No slot, event, pool or held state is written before the refusal.

Stored versus derived totals is an internal recreation choice. The observable equip, drop,
transfer, death and level results are what the tests check.

## Source qualification

- Registration: the passive constructor is the window at `0xbc9ce0` (`push 0x11b6694`,
  `push "APbo"`, `push "AIab"`, `call 0x7f6360`, `add esp,0xc`, `ret`), the same layout as the
  powerup window `0xbca0e0` (`AIsm`, base `AIxm`). `Rules::read` decodes the powerup, passive,
  armor, damage and maximum-life (`0xbc9f40`, `AIml` against `APbo`) windows from the reviewed
  client (SHA-256 `3f2ed012…`, `REVIEWED_IMAGE`).
- Attribute metadata: `Iagi`, `Iint`, `Istr`, `Idef`, `Iatt`, `Ilif`, `Ihpr` and `Imrp`
  `useSpecific` lists must admit the item's ability alias. Amounts come from `Data{column}1`
  through `W3A` overlays. In the selected metadata, `Iint` uses Data column 2. No ID or amount
  is written in code. A level-1 W3A entry applies only when its second selector equals the
  field's metadata `data` ordinal, which selects the `Data` letter; the metadata `index` column
  is unrelated. A mismatched pointer, including selector 0 where the data ordinal is not 0,
  refuses rather than applying the entry. This keeps the controlled RoC `Ilif` selector-0
  observation from fabricating a bonus; general native ignore semantics are not yet qualified.
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
`PCX_TOME_CATALOG`, `--ignored`) partitions every selected row. The counts below predate the
armor, damage, maximum-life and intelligence families and are retained as the initial audit;
rerun the gate for current counts.

| initial refusal | rows |
|---|---|
| passive ability of a family other than `AIab` (life, mana, attack speed, and others) | 79 |
| `AIab` row with an intelligence bonus (then the mana path was unqualified) | 12 |
| `AIab` alias not admitted by the attribute metadata | 3 |

The 12 intelligence rows are now admitted through `Iint` and the mana projection path.
Host tests include `held_intelligence_adds_mana_pool_and_is_removed_with_ratio_rounding`,
`pure_intelligence_equip_and_drop_leave_tiny_positive_life_untouched` and
`pure_intelligence_cleanup_keeps_tiny_life_and_matches_sequential_mana`. Campaign placement
impact is not measured. A row still outside the admitted families fails the pickup call with a
diagnostic instead of applying a silent wrong stat.

## Original-source gates

- `hval` (strength 4, agility 4 from source rows) reads as supported with those amounts.
- `hcun` (intelligence bonus) reads as supported with a non-zero intelligence total.
- `crys` (usable) reads as not passive.

## Not verified and not implemented

- The native remove callback for `CAbilityStrengthMod`. Candidate entries `0xbc6c60`,
  `0xbc6d20` and `0xbc6d50` were surveyed without confirmation. The apply callback is `0xbc5520`
  at vtable `+0x430`. This is an internal implementation detail, not a fidelity gap, because the
  observable results are tested.
- Life behavior on drop, and on death of a passive holder, has no original runtime capture here.
  The ratio policy on drop is the same native path as the grant, with the negative delta. For
  life this is a faithful derivation, not a measured observation. Mana drops and dead-holder
  removal are measured in the controlled RoC mana probes.
- The source of the revival current-mana value is not qualified.
- Usable item activation (`UnitUseItem`) and its charges.
- Passive families other than `AIab`, `AIde`, `AIat`, `AIml`, `AImm`, `Arel`, `AIrm`, `AIas` and the
  orbs: mana (`AIrn` auras and other classes), and the rest of the refused rows.
- Non-hero passive equip: pure intelligence is accepted with no modeled effect, while strength
  and agility still refuse. Armor, flat damage and maximum life are accepted for any unit with
  vitals. Native behavior for non-hero strength/agility is not verified.
- Powerup `get_items` gating and non-hero powerup targets remain documented gaps in
  `inventory-powerup.md`. They are fidelity gaps, not permission gates.

## Maximum-life family (`AIml`, Item Life Bonus): implemented and tested

- Registration: window `0xbc9f40` (`push "APbo"`, `push "AIml"`, `call 0x7f6360`), the same
  reviewed layout and base as `AIab`. The class must be distinct from the other registered
  classes. See `passive-max-life.md` for the original-runtime samples.
- Amounts: metadata row `Ilif` (`field Data`, `slk AbilityData`, `data 1` = `DataA` in the
  selected source). The `useSpecific` list must admit the item's ability alias, and typed W3A
  overrides win only at level 1 with the second selector equal to that metadata `data` ordinal.
  A selector-0 or other mismatched pointer refuses instead of applying. The synthetic parser
  fixture maps `Ilif` to column 2 and puts a decoy in DataA, so a hardcoded DataA or pointer 1
  fails. Negative or unadmitted amounts refuse; held sums use checked addition.
- Held effect: flat maximum-life points, summed over held passive items and derived per unit,
  hero or not. It applies whenever the holder has current vitals; a unit without vitals refuses
  a nonzero contribution.
- Pool policy: `max' = max + delta`, `life' = floor(new_max × life / max + 0.5)`. A living
  holder stays at one hit point when the rounded ratio is zero; a dead holder stays at zero. The
  strength and maximum-life paths share this adjustment. Dead units still cannot pick up items.
- Equip, drop, transfer and cleanup preflight receiver and donor pool projections before slots,
  ownership or drop events change. Held-total overflow refuses before writes.
- Tests: parser (`max_life_points_come_from_the_admitted_ilif_row`,
  `max_life_w3a_override_on_ilif_wins`,
  `max_life_override_with_the_wrong_data_pointer_refuses`,
  `negative_authored_max_life_refuses`, `max_life_row_outside_the_ilif_admission_refuses`,
  `max_life_registration_requires_the_passive_base_and_a_distinct_class`); host
  (`held_max_life_stacks_on_a_hero_and_rounds_the_retained_ratio`,
  `held_max_life_applies_to_a_non_hero_and_transfers_the_pool`,
  `dropping_held_max_life_restores_the_pool_and_rounds_the_ratio`,
  `cleanup_drops_held_max_life_and_restores_the_pool`,
  `held_max_life_total_overflow_refuses_transfer_without_writes`,
  `held_max_life_on_a_typed_override_uses_the_override_amount`,
  `held_max_life_keeps_a_living_holder_at_one_life`, `dead_unit_cannot_pick_up_max_life`).

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
  `AIat` (20); the amount is read from `DataA1`, never a constant, and a typed W3A override
  applies at level 1 only with the matching metadata data pointer.
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

## Mana-regeneration family (`AIrm`, Item Mana Regeneration): implemented and tested

- Registration: window `0xbc9ec0` (`push "Arem"`, `push "AIrm"`, `call 0x7f6360`), the same
  reviewed immediate-class layout and base shape as the `Arel` window. `Rules::read` decodes it as
  an additional class window and all registered classes must be distinct.
- Amounts: metadata row `Imrp` (`field Data`, `slk AbilityData`, `data 1` = `DataA1`, authored
  type `real`), whose `useSpecific` list must admit the item's ability alias. The selected source
  rows give `AIrm` `DataA1=0.5` and `AIrn` `DataA1=0.25`. The value is read at level 1 and a typed
  `real`/`unreal` W3A override with the matching data pointer wins. The editor string is the
  mana-regeneration bonus as a fraction
  of normal, and the probe below is consistent with a fraction rather than points per second.
- Held effect: the finite, nonnegative fractions of held passive items are summed. The unit's mana
  rate is the existing base rate plus the hero INT term, multiplied by `1 + held fraction`.
  Negative, nonfinite, unadmitted and overflowing sums refuse the pickup before any slot, event,
  mana or held-state write. Fractions apply to heroes and non-heroes alike because the held total
  is derived from inventory ownership for any unit.
- Projection: a nonzero held fraction requires the unit's vitals to be projected through the
  current game clock before equip, drop or cleanup, for both the receiver and a donor during a
  transfer. This uses the same boundary as the held life rate.
- Tests: parser (`mana_fraction_comes_from_the_admitted_imrp_column`,
  `w3a_real_override_on_the_imrp_field_wins_at_effect_level`,
  `malformed_negative_and_overflowing_mana_fractions_refuse`); host
  (`held_mana_fraction_scales_base_and_intelligence_rate`,
  `held_mana_fraction_changes_refuse_stale_projection_without_moving_items`,
  `held_mana_fraction_cleanup_refuses_stale_projection_and_clears_the_rate`).
- Original observation: two held `rwiz` items added about `0.810` mana per second and one added
  about `0.4045`, relative to the matched control; dropping both returned the difference to
  approximately zero, and re-equipping one at night restored it. The increments are consistent
  with a fraction of the unit's existing base rate, but the probe does not separate the base,
  time-of-day and INT terms, so the exact native multiplicand remains inferred.
- Not verified: native integer/real rounding of the multiplied rate, whether the native term
  scales the INT contribution as implemented, auras such as `AIrn`, TFT/Reforged differences and
  unchanged Human02 startup. The selected victory/Continue gate has not been re-run after this
  change.

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

The selected Human01 score/Continue gate passed the former `rlif` refusal and
reached `rwiz`, whose `AIrm` passive was unsupported at
`Scripts\Blizzard.j:2640` before this implementation. That gate stimulates the selected victory BJ
and Continue control; it does not prove Human01 mission completion or successful Human02 startup.
The failed run is retained locally as
`.scratch/campaign-continuation/life-item-selected.log` (exit 101). The `AIrm` path is now
implemented, and the gate has not been re-run.

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
under `.scratch/passive-mana-probe/`; the implemented multiplicand is inferred,
not native-qualified.

## Orb families (`AIfb`, `AIcb`, `AIgd`): implemented and tested

Status: admitted. The decode below comes from static disassembly of the reviewed image
(SHA-256 `3f2ed012…`, `objdump`/`rizin`), the shipped rows (`Units\AbilityData.slk`,
`AbilityMetaData.slk`, `ItemStrings.txt`, `ItemAbilityFunc.txt`, `UnitWeapons.slk`, all from
`War3x.mpq`) and the repo's combat code. No runtime capture of an orb hit exists, so every
behavior below is a reading of the machine code and the data, not an observed game. Each fact is
marked **observed** (read from the image or a shipped row) or **inferred**.

### Class structure

- Registration (observed): `AIDB` window `0xca6d00` (`push 0x11b9784`, `call 0xbc92b0` which is
  `mov eax,"AIat"; ret`, `push eax`, `push "AIDB"`, `call 0x7f6360`); `AIcb` window `0xca6ce0`,
  `AIfb` window `0xca6d40` (and `AIDE` `0xca6d20`, `AIzb` `0xca6d60`), each with base `AIDB`. `AIgd`
  has no registration: its row's `code` is `AIfb`. `Rules::read` decodes the three windows
  (`with_orbs`), requires the base chain `AIfb`/`AIcb` over `AIDB` over `AIat`, and requires all
  classes to be distinct.
- Object factories (observed): the class descriptors written by the static initializers at
  `0x426da0`..`0x426e30` (`0x11b988c` = `AIcb`, `0x11b9784` = `AIDB`, `0x11b98d4` = `AIDE`,
  `0x11b98ec` = `AIfb`) lead through descriptor slot 0 to the factories `0xca6150` (`AIcb`, vtable
  `0xfbefe8`), `0xca6200` (`AIDB`, `0xfbf7bc`), `0xca62b0` (`AIDE`, `0xfbfbf4`) and `0xca6360`
  (`AIfb`, `0xfbff38`). Each calls the shared base constructor `0xbc7870`, which is also the whole
  `AIat` factory (`0xbc1ec0`), and then stores its own vtable. The base constructor's last vtable
  store is `0xf50450`.
- Slot 7 of each vtable is the class-code getter (`0xc9e2a0` `AIDB`, `0xc9d590` `AIcb`, `0xc9ef20`
  `AIDE`, `0xc9f7b0` `AIfb`). `RTTI` names `CAbilityDamageBonusBase`, `CAbilityFireDamageBonus` and
  `CAbilityCorruptionDamageBonus` exist in the image; their mapping to the vtables above is not
  checked.
- Overrides against the `0xf50450` vtable (observed, slot index = offset / 4): `AIDB` replaces 3, 10,
  14, 15, 17, 18, 19, 32, 35, 36, 41, 76, 77, 78, 81, 82, 83, 84, 85 (event, serialization,
  attach/detach, order handlers). `AIfb` and `AIcb` differ from `AIDB` only in slots 1 (destructor),
  7, 22 (class descriptor), 83, 84, 85 (`+0x14c`, `+0x150`, `+0x154`) and 203 (`+0x32c`). All four
  vtables are 205 slots long.
- Ability instance fields (observed): `[this+0x8c]` is the value `AIat`, `AIde` and the orbs call
  `DataA1` (`0xbc2eb0` returns it; the `AIde` apply at `0xbc865e` adds it to armor), `[this+0x50]`
  is the level, `[this+0x4c]` the target-class word, `[this+0x20]` flag bits (`0x100` armed).
  Level-record getters read `Area` at `+0x18`, `DataA` `+0x20`, `DataB` `+0x24`, `DataE` `+0x30`
  from records of `0x70` bytes (`0x46e900`, `0x46ee20`, `0x46f030`); the order matches the
  `AbilityData` column order and the field-name table at `0x10dce00`.

### What runs on a hit (observed)

The attack code calls three virtuals per ability. Slot 77 (`+0x134`, `AIDB` `0xc9e4a0`) arms the
ability: it returns 0 without arming when the attack record's flag `0x80` is set, when the target
check `0x682450` (which passes the ability's class word `[this+0x4c]` to the target-allowed test
`0x6adb90`) reports an error, or when the target-side veto `0x682180` fires; otherwise it sets
`[this+0x20] |= 0x100` unless the owner's flag word `[owner+0x164]` has bit `0x40000000`. Slot 85
(`+0x154`) is the handler for a unit target: the base version (`0xc9e550`) only clears the flag.
The orb versions run their effect first and then apply the attack record to the target through the
unit virtual `[unit+0x120]`, so the attack damage is the same call as an ordinary hit.

- Fire, `0xc9fa10`: one call of the area-damage routine `0x6eb640` with the target's position,
  one ring `{radius = Area1 (0x46e900), amount = [this+0x8c]}`, damage-type word `0x4000000` (the
  orb's `+0x32c` getter `0xc9fe60`; `AIDB` returns `0x10`), the ability class word with bit
  `0x10000` ORed in, and the filter `0xca60f0` (`unit != target`). Then the target takes the
  attack record.
- Corruption, `0xc9d5a0`: the duration is `0x46f500` when `0xcaf450` says the target qualifies
  (inferred: hero, `HeroDur1`) and `0x46f3a0` otherwise (`Dur1`); the amount is `0x46ee20`
  (`DataB1`) or 0 when owner bit `0x40000000` is set. It then looks for the target's existing
  `BIcb` buff (`0x6d6570`); if present it calls buff virtual `+0x360`, else it creates the buff
  (`0xca3c40`), calls `+0x35c` and attaches it (`0x6d4880`). Then the target takes the attack
  record.
- The `BIcb` buff class (vtable `0xfbf33c`): apply `0xc9dc80` stores the amount and adds the negated
  amount to the target through `0x680070`; refresh `0xc9dcf0` restarts the timer (`0x6f6c30`),
  adds `old - new` through `0x680070` and stores the new amount; removal `0xc9dd70` adds the
  stored amount back. `0x680070` adds a float to the unit field `[unit+0x1e8]` and refreshes the UI.
  The damage routine `0x6677c0` reads `[unit+0x1e8]` as the armor for `0x67d960` (the
  `DefenseArmor` curve), and the `AIde` apply and removal use the same call, so this field is the
  armor bonus.

### Rules decoded

1. **One damage instance plus the flat bonus.** The orb's `DataA1` is the same property `AIat`
   adds to the holder's attack (the orb classes derive from `AIat`), so the primary hit is the
   ordinary attack damage with the bonus, armored once (damage type `0x10` is the only type the
   damage routine `0x6677c0` armors, at `0x667b2d`). The splash and the penalty are separate.
   Implemented as held flat damage (`AttributeBonus.damage`) plus the on-hit effects below. That the
   flat bonus applies to every target, including those the orb's class list excludes, is inferred
   from the inheritance.
2. **Splash (`AIfb`, `AIgd`).** Every living enemy of the attacker within `Area1` of the target's
   position, except the target, takes the full `DataA1`. There is a single ring, so no falloff
   (`0x6ea0a0` takes the smallest containing ring's amount). The damage type is universal (bit 26 of
   the damage-type mask, `0x4000000`): the routines at `0x6ea0a0` and `0x667855` skip the spell
   immunity checks for the types `0x10`, `0x20`, `0x400000` and `0x4000000`, and `0x6677c0` does
   not armor it. The record's attack type is 0 (`0x6a9940` zeroes `+0x20`), which indexes the
   `DamageBonusSpells` row of the table loaded by `0x67b920` (table base `0x113e9e8`, entry
   `attack * 8 + defense`, load order Spells, Normal, Pierce, Siege, Magic, Chaos, Hero), so the
   splash is `DataA1 * DamageBonusSpells[defense]`. The splash runs before the primary hit.
   - Target filter: class tokens of `targs1` (bits `ground` 0x2, `air` 0x4, `structure` 0x8, `ward`
     0x10, from the token table at `0x10dce70`) plus the enemies bit `0x10000` the handler ORs in.
     The shipped rows list `ground,air,ward`, so structures are not hit and are not armed (the
     arming check uses the same word). Whether `alive` and `vulnerable` are implied when absent is
     inferred: the recreation uses live, targetable units (`weapon_target`).
   - Reach: `0x7ff460` tests the unit position against the radius plus the unit field `[+0x90]`.
     Treating that field as the collision radius is inferred; the recreation uses
     `Area1 + collision radius` of the struck unit, from the target's position.
3. **Air switch.** Attaching an orb (slot 32, `0xc9e3a0`) reads `DataE1` (`0x46f030`), truncates it
   (`0x9a07b0`) and, when nonzero, calls `0xb1c6e0` on the owner's sub-object `[owner+0x3f4]`, which
   increments a counter at `+0x234` and on the first enable sets flags `0x8100000`; detach
   (`0xc9e2f0`) calls `0xb22960` to undo it. The call takes no attack index, so the value only has
   to be nonzero for the call, and `Enabled Attack Index` (`Iob5`, `maxVal` 2) is the editor name of
   the field. That the sub-object is the second weapon is inferred: the stock melee heroes author
   attack 2 as a ranged missile weapon that targets air (`Hpal`: `weapsOn` 1, `targs1`
   `ground,structure,...`, `targs2` includes `air`, `rangeN2` 500, `weapTp2` missile), the item
   texts say "attacks also become ranged when attacking air", and every shipped orb row has
   `DataE1` 2. Implemented: with an orb held, a target with `movetp` `fly` that weapon 1's `targs1`
   cannot reach is attacked with weapon 2 (its own range, cooldown, damage point, dice, attack type
   and missile) when weapon 2 exists (`weapTp2` is not a placeholder) and its `targs2` names air.
   Otherwise weapon 1 is kept. `DataE1` other than 2 refuses at pickup.
4. **Armor penalty (`AIcb`).** The first hit that arms the orb attaches `BIcb` and lowers the
   target's armor by `DataB1` before the attack damage is applied, so that hit already sees the
   lowered armor (instant weapons: applied before the roll; missile weapons: the roll is redone
   against the lowered armor on impact). The duration is `HeroDur1` for a hero target and `Dur1`
   otherwise. A later hit refreshes: the penalty is replaced by the new value (not added) and the
   timer restarts. Expiry, `UnitRemoveBuffs` and death restore the armor. A non-positive duration
   refuses at pickup (the base refresh at `0x6f6c30` starts no timer for it, and that is not
   modeled). The buff is attached through the unit buff host with only the negative flag, like the
   curse buffs; its dispel class is not decoded.
5. **`AIgd` (Orb of Kil'jaeden).** Its row's `code` is `AIfb`, so it uses the `AIfb` class, and the
   instance value comes from the row's own `DataA1` (6). The image reads the level record by
   offset, not through the editor metadata, so the `Idam` `useSpecific` list that omits `AIgd` is
   an editor-only admission (inferred). The amount is read from the row for any alias whose code is
   a registered orb class, and a map that modifies the row refuses.
6. **Missiles.** A missile weapon carries the orb armed at launch and applies it on impact
   (inferred: the effect and the attack record are applied by the same handler, which the engine
   calls when the attack lands). The target is checked for liveness at impact.

### Tests

Parser: `crates/pcx-play/src/inventory_orb_tests.rs` (`fire_orb_reads_damage_area_and_target_classes_from_its_row`,
`kiljaeden_orb_takes_its_amount_from_its_own_row_without_metadata_admission`,
`corruption_orb_reads_penalty_durations_and_buff_from_its_row`,
`orb_rows_outside_the_decoded_shape_refuse`, `an_item_with_two_orb_abilities_refuses`,
`structure_class_in_targs_covers_buildings`,
`orb_registration_must_chain_through_the_attack_damage_base_and_stay_distinct`,
`without_the_orb_registration_an_orb_row_stays_refused`). Combat host:
`crates/pcx-play/src/inventory_orb_host_tests.rs` (splash area, collision reach, allies, spells row
and no armor, Kil'jaeden amount, drop, structure targets, corruption timing, refresh without
stacking, hero duration, second-orb refusal, missile impact for both orbs, second weapon against
air, no second weapon, no orb). All inputs are synthetic rows built in the tests.

### Open (not decoded, so not modeled)

- The meaning of the attack-record flag `0x80`, the owner flag `0x40000000` and the target-side
  veto `0x682180` (it calls virtual `+0x26c` of the target's abilities); the recreation arms on the
  class list alone.
- Slots `+0x14c` and `+0x150` (`AIfb` `0xc9fcb0`, `0xc9fba0`; `AIcb` `0xc9d8d0`, `0xc9d750`) run the
  same arming test with a different damage call (`[ebx+0xc8]`); the target kinds they serve were not
  identified. Orb attacks on destructables and items are outside the recreation's combat.
- How several orbs on one unit fire on one hit. A second orb refuses at pickup, so no ordering is
  guessed.
- Damage modifiers after the armor step in `0x6677c0` (calls `0x6a0630` and the ethereal and
  hero tables) beyond the damage-bonus matrix.
- The selected-unit panel armor does not show the corruption penalty.
- The `Targetart`, `Missileart` and `Specialart` of the orb rows are not shown, and the orb missile
  model does not replace the unit's missile.
- Map-modified orb rows, `DataE1` other than 2, multi-level orb rows and orb rows with relationship
  or state tokens in `targs1` refuse.
- The `ward` class token is accepted and has no effect (the recreation does not model ward units as a
  separate class). The enemies bit is the only relationship the handlers add.

### Locked-chapter probe

Each row is the result of `selected_install_locked_chapter_startup_probe` on the selected
install, before this change (orb refusal at `UnitAddItemToSlotById`) and after. Exit 0 means the
probe passed.

| Chapter | Before | After |
|---|---|---|
| RoC Undead02 | `AIfb` on `ofir` | `AIms` on `bspd` (unsupported passive ability) |
| RoC Orc02Interlude | `AIfb` on `ofir` | passes |
| RoC Undead01 | `AIfb` on `ofir` | passes |
| RoC NightElf06 | `AIfb` on `ofir` | `IssueTargetOrder` `entangleinstant` (only the attack target order is supported) |
| TFT UndeadX01Interlude | `AIfb` on `ofir` | `AIdf` (code `AIsb`) on `odef` |
| TFT HumanX05 | `AIcb` on `ocor` | `AIll` (code `AIsb`) on `oli2` |
| TFT UndeadX08 | `AIgd` (code `AIfb`) on `gldo` | `UnitSetUsesAltIcon` is unsupported by this host |
| TFT UndeadX02 | `AIfb` on `ofir` | `AIpb` on `oven` |

All eight chapters are past the orb pickups; the new stops are other families or natives.

## Maximum-mana family (`AImm`, Item Mana Bonus): implemented and tested

All addresses are in the reviewed image (SHA-256 `3f2ed012…`), read with `objdump` and `rizin`.
Items: Pendant of Mana (`pmna`, ability `AIbm`, `DataA1` 250).

- Registration (observed): window `0xbc9f80` (`push 0x11b6848`, `push "APbo"`, `push "AImm"`,
  `call 0x7f6360`), same layout and base as `AIml`. `Rules::read` decodes it and requires a class
  distinct from every other held class.
- Class (observed): descriptor `0xf4c718`, slot 0 factory `0xbbecb0`, constructor `0xbc7c60`, vtable
  `0xf55b94`. Against the `APbo` vtable `0xf4f884` it overrides slots 1, 7 (`0xbc75e0`, returns
  `AImm`), 22, 32 (`+0x80`, attach `0xbc75f0`), 41 (`+0xa4`, `0xbc7690`) and 182 (`+0x2d8`,
  `0xbc76d0`, copies `[this+0x8c]` to a result field, the UI value).
- Effect (observed): attach calls `0x680350(unit, &[this+0x8c], 1)` and slot 41 calls it with the
  value's sign bit flipped. `0x680350` is the unit mana-pool routine documented for the `rmnx`
  upgrade effect in `upgrade-effects.md`: mode 1 moves the maximum and sets the current pool to
  `round(current / old maximum * new maximum)`. `[this+0x8c]` is the same `DataA1` instance value
  `AIde` and `AIat` read. The class code has no hero test.
- Amount: metadata row `Iman` (field `Data`, `AbilityData`, integer, `useSpecific`
  `AImm,AImb,AIbm`), read at level 1 with the same typed W3A override rule as `Ilif`. Negative or
  unadmitted amounts refuse.
- Implementation: `AttributeBonus.max_mana`, summed over held items. Equip, drop, transfer, death
  cleanup and the cached-unit restore apply it through the same pool function the upgrade effect
  uses (`reprice_max_mana`), on any unit with vitals, and after the intelligence change when both
  apply. The order between an intelligence change and a maximum-mana change inside one equip is
  inferred (sequential), since a single item never carries both.
- Tests: parser (`max_mana_points_come_from_the_admitted_iman_row`,
  `max_mana_w3a_override_wins_and_a_wrong_pointer_refuses`,
  `negative_or_unadmitted_max_mana_refuses`,
  `max_mana_registration_requires_the_passive_base_and_a_distinct_class`); host
  (`held_max_mana_moves_the_maximum_and_rounds_the_retained_ratio`,
  `held_max_mana_applies_to_a_non_hero_and_transfers_the_pool`).

## Orb of Frost (`AIob`): implemented and tested

Decoded from the same image (SHA-256 `3f2ed012…`) and the shipped rows; no runtime capture.

- Registration (observed): window `0xca6d80` (`push 0x11b9994`, `push "AIDB"`, `push "AIob"`),
  vtable `0xfc0b80`, class getter slot 7 `0xca0f50`. It differs from the `AIDB` vtable in slots 1, 7,
  22, 83, 84, 85 (`0xca0f80`) and 203, the same set as `AIfb` and `AIcb`.
- Hit handler (slot 85, `0xca0f80`, observed): when armed (`0x100`, cleared as in the other orbs) it
  takes the duration `0x46f500` (`HeroDur1`) when `0xcaf450` qualifies the target and `0x46f3a0`
  (`Dur1`) otherwise, and the owner flag `0x40000000` as a boolean. It looks up a buff of class
  `Bfro` (`0xc5e2e0` returns `0x4266726f`) on the target (`0x6d6570`). Present: buff virtual `+0x368`
  (`0xc5b670`, refresh). Absent: `0xc11c90` creates the buff by class id, then `+0x32c`, `+0x364`
  (`0xc5b5e0`) and attach `0x6d4880`. Then the attack record is applied through unit virtual `+0x120`.
- `Bfro` (observed): registration window `0xc5e4b0` (base from `0x6f4c00` = `BSPD`), vtable
  `0xf9f71c`. Apply (`0xc5b5e0`) reads `Misc` keys `FrostMoveSpeedDecrease` (string `0xfa12a4`) and
  `FrostAttackSpeedDecrease` (`0xfa12bc`) through `0x430620` (section string `0xe69798`, default 0),
  or 0.0 for both when the owner-flag boolean is set, negates them and calls the `BSPD` apply
  `0x6f0630`. That stores them at `+0xd4`/`+0xdc`, adds the movement value to the movement
  object (`unit+0x3f8`, `0xb67720`) and the attack value to the attack-side object (`unit+0x3f4`,
  `0xb1e490`). Removal (`0x6f0700`) adds the negated values back; refresh (`0x6f07d0`) restarts the
  timer (`0x6f6c30`), undoes the old values and applies the new ones.
- Values: `MiscGame.txt` has `FrostMoveSpeedDecrease=0.5` and `FrostAttackSpeedDecrease=0.25`.
  The row `AIob` has `DataA1` 6, `DataE1` 2, `Dur1` 3, `HeroDur1` 1, `BuffID1` `Bfro`, `targs1`
  `ground,air,ward`, no `Area1` or `DataB1`. The item text says the attack slows "the movement
  speed and attack rate" for `Dur1` seconds.

Recreation: `AIob` reuses the orb machinery (flat `DataA1` damage, air switch, `targs1` class
filter, missile impact). A hit attaches `Bfro` (negative flag only) and sets the target's move
speed to `base * (1 - 0.5)` through the speed host, and subtracts 0.25 from the attack speed
multiplier the combat step uses. The duration is `HeroDur1` for a hero target (inferred from
`0xcaf450`, as for `AIcb`) and `Dur1` otherwise. A second hit restarts the timer; the decreases
never stack. Expiry, `UnitRemoveBuffs` and death end it. The decreases come from the `Misc` profiles
(stock then map), a missing key reads 0, and a value outside `[0, 1)` refuses (kept as a bound on
the `Misc` input; the speed model itself accepts a multiplier at or below 0, see
`unit-move-speed-model.md`).

Speed model (updated): the movement decrease is a delta on the unit's speed multiplier
(`unit-move-speed-model.md`), so frost and a slow add on one multiplier and both can be active. A frost
hit on a slowed unit and a slow on a frosted unit no longer refuse. A polymorphed unit still refuses
(the speed writes of the polymorph type change are not decoded), as does a polymorph on a frosted
unit. That the attack-side object is the attack speed multiplier is inferred from the key name and
the item text. The `Bfro` dispel class and
`Targetart`/`Specialart` are not modelled. Tests: `inventory_orb_tests.rs` (frost rows, `Misc`
profiles, registration) and `inventory_frost_host_tests.rs` (speeds, timers, refresh, removal,
missile impact, death).

## Spell resistance (`AIsr`, Runed Bracers `brac`): stored, one consumer

- Registration (observed): window `0xbc9ee0` (`push 0x11b649c`, `push "AIat"`, `push "AIsr"`), vtable
  `0xf4d964`, slot 7 `0xbc0af0`. The base is the flat attack-damage class, so `DataA1` (0 in the
  row) is added as flat damage like the orbs. Against the `AIat` vtable `0xf50450` it overrides
  slots 1, 7, 22, 62, 63, 203 and 204.
- Slot 62 (`+0xf8`, `0xbc0b30`) returns 6. The damage routine `0x6677c0` calls slot 62 on each
  ability of the damaged unit (list at `unit+0x3e8`) and slot 63 (`+0xfc`) on the ones that
  answer 1 at once and on the rest in ascending order of that number, before the armor step
  (`0x667b2d`, damage type `0x10` only). A zero-amount record skips all of it.
- Slot 63 (`0xbc0b60`, observed): returns 0 without effect for a null record, when the holder's
  flag word `[unit+0x164]` has `0x40000000`, when the record's flags `[+0xc]` have `0x10`, or when
  its attack type `[+0x20]` is nonzero. Otherwise it computes `1.0 - DataB1` (`0x46ee20`; `0x9a00b0`
  subtracts, `0x112d810` is 1.0) and multiplies the record's amount fields `[+0x10]` and `[+0x18]`
  (`0x6b8170`). Several abilities multiply in turn. Record layout from its constructor `0x6a9940`:
  `+0` source, `+0xc` flags, `+0x10` amount, `+0x14` damage type, `+0x18` second amount, `+0x20`
  attack type (0 is the first row of the damage-bonus table, the spells row). Flag `0x10` reads as
  "zero amount" (the area routine `0x6e9ed0` sets it from a float equality at `0x6ea001`;
  inferred), so it never blocks a real hit.
- Row: `DataB1` 0.33. The item text says "Reduces Magic damage dealt to the Hero by 33%".

Recreation: the held state is `AttributeBonus.spell_damage_taken`, the product of `1 - DataB1`
over held `AIsr` abilities (a fraction outside `[0, 1]` refuses; map-modified rows refuse). Its
only consumer is the Fire orb splash (`orb_splash`), whose record has attack type 0 and a nonzero
amount, so a holder takes `splash * (1 - 0.33)`. Not consumed: the other spell damage paths
(`damage_unit` for Holy Light and Chain Lightning, attack-ground splash), because the record
fields they carry (attack type, flags) are not decoded. The holder flag `0x40000000` is not
modelled (meaning not decoded). Tests: `inventory_orb_tests.rs` (rows, product of two, refusals,
registration) and `spell_resistance_scales_the_spells_type_splash_the_holder_takes`.

## Attack speed (`AIas`, Gloves of Haste `gcel`): implemented and tested

Decoded from the reviewed image (SHA-256 `3f2ed012…`) and the shipped rows; no runtime capture.

- Registration (observed): window `0xbc9cc0` (`push 0x11b64e4`, `push "APbo"`, `push "AIas"`), the
  same layout and base as `AIab`. The descriptor global is set to `0xf4fbb0` at `0x41c18a`. Vtable
  `0xf51020`, class getter slot 7 `0xbc32e0`. Against the `AIat` vtable `0xf50450` it overrides
  slots 1, 3, 7, 14, 15, 22, 32, 41, 181, 182, 185, 186, 199 and 202 to 209.
- Attach (slot 32, `0xbc3310`, observed): formats `"%s%d"` with the string at `0x10dce1c` (`DataA`)
  and level 1, reads that cell of the ability row through `0x6b06a0`, stores the float at `+0x88`,
  calls virtual `+0x2fc`, then `0xbc8560`. The `AIat` attach `0xbc2a20` is the same sequence
  ending in the damage routine `0xbc84b0`.
- Apply (`0xbc8560`, observed): once per ability (flag `0x80` at `+0x20`), takes the holder's
  attack-side object (`unit+0x3f4`; absent means no effect). When the unit flag word
  `[unit+0x164]` has `0x40000000` it first reads the `Misc` key `IllusionsGetAttackSpeedBonus`
  (string `0xea7500`, section `0xe69798`, default 0) and does nothing when it is 0. Otherwise it
  calls `0xb1e490(value, 0)` with the stored amount. Detach (`0xbc3410` to `0xbca250`) clears the
  flag and makes the same call with the sign flipped, under the same gate.
- Multiplier (`0xb1e490`, mode 0, observed): the new value is the stored multiplier at `+0x1b4`
  plus the argument (`0x9a01c0`), snapped to exactly 1.0 when within 0.001 of it (`0x10db054`),
  then written through the field object at `+0x1b0`; a change above about 2e-7 (`0x10db124`) posts event
  `0xd01b0`. The Frost buff reaches the same routine with its negated `FrostAttackSpeedDecrease`
  (see the Orb of Frost section), so held bonuses and frost and slow decreases are one sum, not
  factors.
- Rows (shipped): `AIsx` (code `AIas`, `Isx1` metadata row: `Data`, `AbilityData`, data 1, unreal,
  `useSpecific` `AIas,AIsx,AIs2`) has `DataA1` 0.15; `AIs2` has 0.2. `gcel` lists `AIsx`. `MiscGame.txt`
  has `IllusionsGetAttackSpeedBonus=1`.

Recreation: `AttributeBonus::attack_speed` is the sum of the held `Isx1` amounts (typed W3A override
at level 1 with a matching data pointer wins, as for `Imrp`; a non-finite sum refuses). The combat
step's multiplier is `1 + AGI * AgiAttackSpeedBonus + held - slow - frost`, which
`derive_hero_combat` and `attack_speed_penalty` already feed into the swing and cooldown, and into
attack-ground. Non-heroes get `1 + held`. A multiplier at or below 0 still refuses at the swing.
Tests: `inventory_powerup_tests.rs` (column, override, refusals, registration) and
`inventory_frost_host_tests.rs` (held sum, removal, held plus frost).

Not decoded or not modelled: that agility's attack speed also goes through `0xb1e490` (assumed
additive, as before this change); the 0.001 snap to 1.0 (float noise only); the illusion gate, which
the recreation does not need because the shipped key is 1 and an illusion keeps the holder's bonus;
that `[unit+0x164]` bit `0x40000000` is the illusion flag is inferred from the key name; and any
cap or floor the cooldown consumer applies to the multiplier.

## Open held classes (decoded in part, still refused at pickup)

Each class below is registered in the reviewed image and its first-level structure is read. The
effect is not fully decoded or its host mechanism is absent, so the pickup refuses with the reason
in `UNMODELED_PASSIVE_CLASSES` (`crates/pcx-play/src/inventory_powerup.rs`), test
`decoded_but_unmodeled_classes_refuse_with_their_reason`.

### `AIsb` (Orb of Slow, Orb of Darkness `AIdf`, Orb of Lightning `AIll`)

- Registration (observed): window `0xca6e20` (`push 0x11b98a4`, `push "AIDB"`, `push "AIsb"`),
  base `AIDB` (the orb base). Descriptor `0xfbf6b4`, factory `0xc9dff0`, constructor `0xca3e70`
  (calls the shared base constructor `0xbc7870`, adds a list at `+0xa8` and fields at `+0xbc..+0xdc`),
  vtable `0xfc026c`. It overrides slots 1, 7 (`0xc9edf0`, `AIsb`), 10, 17, 18, 19, 22, 32, 34, 35,
  41, 44, 46, 73, 74, 83, 84, 85, 185, 186, 203, 204 of the `AIDB` vtable.
- Attach (slot 32, `0xc9ee50`, observed): runs the `AIat` attach `0xbc2a20`, sets flag `0x200`,
  runs the same `DataE1` air switch as the other orbs, then `0xca70c0`, which builds the ability
  whose raw code is `UnitID1` of the orb row (`0x46fb50`), stores its handle at `+0xc8` and sets
  flag word `0x6000000` on it. Detach (slot 41, `0xc9ee00`) releases it.
- Unit-target handler (slot 85, `0xc9f3d0`, observed): when the orb is armed (`0x100`) and flag
  `0x400` is set, it asks the wrapped ability's virtuals `+0x354`, `+0x3e4`, `+0x3f4` and the
  routine `0xbebae0` whether it may act on the target, ORs `0x10000` into the wrapped ability,
  calls `0xca7060` and `0xb4a940` on it, then applies the attack record through the unit virtual
  `+0x120`. Where flag `0x400` is set is not traced.
- Shipped rows: `AIsb` `UnitID1` `AIos` (code `Aslo`), `AIdf` `ANbs` (Black Arrow, summon `ndr1`,
  buffs `BNba`, `BNdm`), `AIll` `AIpg` (code `Aprg`, Purge). `DataA1` is the orb damage (6, 6, 5).
  `DataB1`..`DataD1` (15/5/35, 100/100/100, 35/10/35) are read by the wrapper or the wrapped
  ability; which is not decoded.
- Why it stays refused: the effect is the wrapped ability (slow, Purge, Black Arrow with its
  summon). The recreation has a slow buff but no Purge, no Black Arrow and no summon-on-kill, and
  the roles of `DataB1`..`DataD1` are open.

### `AIpb` (Orb of Venom `oven`, with `Apo2`)

- Registration (observed): window `0xca6e00` (`push 0x11b98bc`, `push "AIDB"`, `push "AIpb"`).
  Descriptor `0xfc0eb4`, allocator `0xca6620`, vtable `0xfc1848`. It differs from `AIDB` only in
  slots 1, 7 (`0xca2a80`), 22, 83, 84, 85 and 203 (`0xca2ca0`, returns `0x4000000`), the same slot
  set as `AIfb` and `AIcb`.
- Handler (slot 85, `0xca2ab0`, observed): when armed it runs `0x4705f0`, clears the flag and, unless
  the owner flag `0x40000000` is set, runs the callback `0xca6bf0` over the owner's abilities with the
  target and attack record (`0x6d6460`). The callback is not decoded; it is how the poison reaches
  the target. `oven` also lists `Apo2` (code `Apo2`, `Dur1` 6, `DataA1` 9, `DataD1` 1, buffs
  `BIpb`, `BIpd`), a second class that is not decoded.
- Why it stays refused: poison damage over time and its stacking and kill rules are not decoded or
  hosted. `DataA1` (5) is the flat orb damage and `DataE1` (2) the air switch, which are the
  already-implemented orb parts.

### `AIrc` (Ankh of Reincarnation `ankh`)

- Registration (observed): window `0xb8c1e0` (`push 0x11b5a5c`, `call 0x46f5b0` which returns
  `abil`, `push "AIrc"`). Descriptor `0xf34978`, factory `0xb8a9b0`, vtable `0xf35200`, slot 7
  `0xb8b7c0` (`AIrc`).
- Reincarnation entry (slot 161, `+0x284`, `0xb8b870`, observed): resolves the item that carries the
  ability from the handle at `+0x70`, requires its use count (`[item+0x18c]`) to be nonzero, the
  owner's flag word `[owner+0x164]` bit `0x40000000` clear, no pending reincarnation marker (an
  object lookup for `XOre`), a location test (`0x7f6a00`), then reads `DataB1` (`0x46ee20`, clamped
  against the float at `0x112d810`) and `DataC1` (`0x46eed0`, negative selects a unit-derived
  value via `0xb20d20`), creates a revive record (`0xb8bfc0`), calls its virtual `+0x78` with the
  unit and the level values, sets owner flag `0x1000` and calls `0x682c80`. Shipped row `AIrc`:
  `DataA1` 7, `DataB1` 500, `DataC1` -1, `EfctID1` `XOre`.
- Why it stays refused: the delay, the restored life and mana, where the hero returns, the item
  charge use and the death path that calls slot 161 are not decoded, and the recreation has no
  reincarnation host.

### `AIms` (Boots of Speed `bspd`)

- Registration (observed): window `0xbc9fc0` (`push 0x11b6860`, `push "APbo"`, `push "AIms"`).
  Descriptor `0xf4cf18`, factory `0xbbf6b0`, constructor `0xbc7cd0`, vtable `0xf4c7b4`. It overrides
  slots 1, 3, 7 (`0xbbee70`, `AIms`), 14, 15, 22, 32, 41, 97, 181, 182, 185, 186 and 199 of the `APbo`
  vtable.
- Value (observed): slot 97 (`+0x184`, `0xbbf220`) returns `[this+0x8c]` (`DataA1`, 60) where the
  base returns 0.0 (`0x46a810`). `0x6d91f0` walks the list headed by the handle at `unit+0x3e8`
  (next handle at `+0x24` of each member), calls `+0x184` on every member and keeps the maximum,
  starting from 0.0. Several speed bonuses therefore do not stack: the largest applies.
- Consumer (observed): the movement class getter at `0xb681b0` (class code `Amov`, `0xb681a0`)
  computes `(maximum + [this+0x74]) * [this+0x7c]` (the add is `0x9a01c0`, the multiply `0x99ffe0`),
  then limits the result with the globals at `0x113e...` and the unit-type fields at `+0x1d8` and
  `+0x1dc` (`0x6975b0`, `0x697410`). Attach (`0xbbeee0`) and detach (`0xbbefc0`) call the recompute
  `0xb69430` on the movement object at `unit+0x3f8`.
- What the consumer fields hold (observed unless marked): the movement object (constructor
  `0xb66360`, vtable `0xf24fc8`) keeps two tracked floats, `[+0x74]` and `[+0x7c]`.
  `SetUnitMoveSpeed` (native handler `0x4ab880` -> `0x69cf90` -> `0xb69520`) writes `[+0x74]` and
  recomputes, so `[+0x74]` is the base speed. The init virtual (slot 196, `0xb65260`) sets
  `[+0x78]`'s value `[+0x7c]` to `0x112d810`, which is 1.0 (static initializer `0x401460` converts
  the integer 1; `0x112d88c` is 0.0 from `0x401300`). `0xb67720` adds a delta to `[+0x7c]`, tracks
  whether it is below 1.0 (counter at `[+0x84]`) and snaps to 1.0 within 0.001, so `[+0x7c]` is a
  speed multiplier whose neutral value is 1.0. Buffs shift it additively (the Frost buff passes
  `-FrostMoveSpeedDecrease`). The unit-type `spd` column reaches `[+0x74]` at creation (the init call
  at `0x68115c` follows the unit-type speed read via `0x67b780`, which reads field `+0x1d4` of the
  unit-type record; that this field is `spd` is inferred from the unit-creation call order).
  Result: `speed = limit((max(AIms DataA1) + base) * multiplier)`, so the bonus is added before the
  slow multiplier, and `SetUnitMoveSpeed` leaves the bonus in place.
- `MiscGame.txt` `MoveSpeedBonusesStack` (string `0xf55f08`): attach (`0xbbeee0`), detach
  (`0xbbefc0`) and the other `AIms` handlers read it with `0x4304c0`. With 0 they call the
  recompute `0xb69430` (the maximum rule above). With nonzero they set flag `0x80` on the ability
  and skip the recompute (the update handler slot 15, `0xbbf3c0`, sets the flag from the key and
  clears it for an illusion when `Misc/IllusionsGetMoveSpeedBonus` is 0). While the flag is set,
  slots 181 and 182 (`0xbbf330`, `0xbbf370`) store `DataA1` at `+0x28` of a bonus record that the
  routine around `0x6d6960` (call site `0x6d69c0`, virtual `+0x2d4`) sums over the unit's
  abilities; for the flat damage class the same slot (`0xbc2be0`) fills the attack damage fields
  of such a record. Where the `+0x28` total reaches the speed is not decoded. Values in the shipped
  files: root `Units\MiscGame.txt` 0, `Melee_V0` 0, `Custom_V0` and `Custom_V1` 1. The
  campaign chapters run with the custom rule sets, so they use the undecoded path.
- Implemented (2026-11-11): the full decode of the speed formula, the limit and the multiplier
  writers is in `unit-move-speed-model.md`. The pickup binds `AIms`: each held item sets its `DataA1` as a
  per-item entry of the unit's bonus list, the largest entry counts, dropping the item (also on
  death or removal) takes it out, and the bonus is added to the base before the multiplier and the
  limit. Refused: a nonzero `MoveSpeedBonusesStack` (the stacking path above; this keeps the
  campaign chapters that carry boots blocked, because the custom rule sets set it), a map-modified `AIms`
  row (the overrides are not decoded), a unit without the speed host, position or clock, and a
  polymorphed holder. Tests: `inventory_speed_host_tests.rs` and the `speed_bonus_*` tests in
  `inventory_orb_tests.rs`.

### `AIau` (Legion Doom-Horn `lgdh`, code `AUau`): still refused

Decoded so far (no runtime capture):

- Class (observed): `AIau` is a row whose `code` is `AUau`, the class of the hero spell Unholy Aura.
  Registration window `0xc657d0`: descriptor `0x11b8be0`, base class id `aura` (from `0xb90db0`),
  class id getter `0xc65710`. The buff is `BUau` (getter `0xc65720`), registered at `0xc657f0` over
  base `BAUR` (from `0x6f4b60`), a different base from the `BSPD` of slow and frost.
- Rows (shipped): `AIau` has `Area1` 900, `targs1` `air,ground,friend,self,vuln,invu`, `DataA1`
  0.1, `DataB1` 0.5, `DataC1` 0, `BuffID1` `BUau`. The editor metadata rows `Uau1`, `Uau2`, `Uau3`
  (data columns A, B, C) are labelled "Movement Speed Increase (%)", "Life Regeneration Increase (%)"
  and "Percent Bonus" (bool). The item text says it grants the Hero and friendly nearby units
  increased life regeneration and movement speed and does not stack with Unholy Aura.
- Buff movement half (observed): the `BUau` apply routine `0xc65560` sets flag `0x200` on the buff
  and shifts the unit's speed multiplier by the value at buff `+0xe8` through `0xb67720`, unless the
  unit has flag `0x40000000` and `Misc/IllusionsGetMoveSpeedBonus` (string `0xea7520`, stock 1) is
  0. The flag is the one `IllusionsCanRestoreLife` pairs with illusions (inferred). The speed model
  of `unit-move-speed-model.md` can take this delta; the recreation has no aura host to call it.
- Life regeneration half: the class getter `0xc65750` reads the `Misc` key
  `IllusionsCanRestoreLife` for illusions. Where `DataB1`/`DataC1` reach the regeneration is not
  decoded.
- Open, hence refused with a message naming the class (`UNMODELED_PASSIVE_CLASSES`): the base
  class `aura` scan (period, the unit filter against `targs1`, how a unit leaving the range loses the
  buff), the `BUau` duration and refresh, the stacking rule with the hero spell, and the life
  regeneration addition.
