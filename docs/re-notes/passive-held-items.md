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
- Orbs (`AIfb`, `AIgd`, `AIcb`, `AIob`, `AIpb`) add their flat damage and run their on-hit effect; see
  "Orb families" below. The spell orbs (`AIsb`) roll a proc per swing and cast Black Arrow or
  Purge, and the lightning orb (`AIlb`) casts Purge on every armed hit; see "Spell orbs" below. A
  holder takes at most one orb.
- A held Ankh of Reincarnation (`AIrc`) revives its holder at the place it died `DataA1` seconds
  after the death and spends its charge; see "`AIrc`" below.
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

## Spell orbs (`AIsb`: Orb of Darkness `AIdf`, Orb of Lightning `AIll`, Orb of Slow `AIsb`): proc, Black Arrow and Purge implemented, Slow refused on a proc

All addresses are in the reviewed image (SHA-256 `3f2ed012…`), read with `objdump` and `rizin`.
"Observed" is read from the machine code; "inferred" is named as such.

### Class and attach

- Registration (observed): window `0xca6e20` (`push 0x11b98a4`, `push "AIDB"`, `push "AIsb"`), base
  `AIDB`. Descriptor `0xfbf6b4`, factory `0xc9dff0`, constructor `0xca3e70`, vtable `0xfc026c`. It
  overrides slots 1, 7 (`0xc9edf0`), 10, 17, 18, 19, 22, 32, 34, 35, 41, 44, 46, 73, 74, 83, 84, 85,
  185, 186, 203, 204 of the `AIDB` vtable; the arming slot 77 (`0xc9e4a0`) is the shared orb one.
- Attach (slot 32, `0xc9ee50`, observed): the `AIat` attach, flag `0x200`, the `DataE1` air switch
  (as every orb), then `0xca70c0`, which creates the ability whose raw code is the row's `UnitID1`
  (`0x46fb50`, factory `0xaf3ba0` by class code), adds it to the owner (`0x6dd220`), stores its
  handle at `+0xc8` and ORs `0x6000000` into its flag word. Last, the three proc counters
  `+0xd4`, `+0xd8`, `+0xdc` are set to 1. Slot 34 (`0xc9eee0`) and 35 (`0xc9ef30`) forward to the
  wrapped ability, detach (slot 41, `0xc9ee00`) releases it.
- Row fields (observed in the handlers below, labels from `WorldEditStrings.txt`
  `WESTRING_AEVAL_IOB2..IOB4`): `DataB1` chance to hit units (%), `DataC1` chance to hit heroes
  (%), `DataD1` chance to hit summons (%), `DataE1` enabled attack index, `UnitID1` the effect
  ability. Shipped rows: `AIdf` 100/100/100 with `ANbs`; `AIll` 35/10/35 with `AIpg` (code
  `Aprg`); `AIsb` 15/5/35 with `AIos` (code `Aslo`).

### The proc roll (observed)

- The attack function (call at `0xb1db59`) calls `0x697250(target, 0.0, 0.0)` as a swing starts. It
  calls slot 73 (`+0x124`) of every ability of the attacker. AIsb slot 73 is `0xc9f2d0`.
- Conditions: target handle set, `[this+0x3c] <= 0`, virtual `+0x268(0)` true, the target-class
  test `0x682450` (the orb's own class word) without error, the target-side veto `0x682180` clear.
  The roll does not happen for a target outside the orb's `targs1` (the arming test is the same).
- Target kind (`0xca6aa0`): first character of the target's rawcode `A`..`Z` is 1 (hero); else bit
  `0x4` of `[unit+0x454]` is 2; else 0. That bit is the summoned-unit marker is inferred (the
  editor label "Chance To Hit Summons" and the Purge summoned-unit test read the same bit).
- Chance (`0xca6710`): row value `x` by kind (`0x46ee20` `DataB`, `0x46eed0` `DataC`, `0x46ef80`
  `DataD`); step `trunc(x * 0.2 + 0.5)` clamped to 0..20 (`0xcaec30`, reals `0x10db088`, `0x10db0a8`,
  clamp immediate `0x14`, `0x9a07b0` truncates); the increment is entry `step` of a 21-entry
  table the function builds at first use in `0x11b9b30..0x11b9b80`, each `numerator / 10000`
  (divide `0x9a02d0` = multiply by the table reciprocal `0x9a1d00`, table at `0xed6cf0`). The
  numerators are 0, 38, 147, 320, 547, 819, 1126, 1461, 1815, 2184, 2562, 2953, 3333, 3781, 4203,
  4607, 5000, 5799, 6666, 7706, 10000. Chance = counter of the kind (`+0xd4` units, `+0xd8` heroes,
  `+0xdc` summons) times the increment (`0x99ffe0`).
- Draw (observed): `0x69a030(&out, 0x24)`, a real in `[0, 1)` from secondary stream `0x24` of the
  engine generator (the same arithmetic as the primary `GetRandomReal` wrapper `0x494e80` on
  lane `base + 4 + 8 * 0x24`, through `0x991460`). Success is `chance >= draw`: slot 73 sets
  flag `0x400` on the orb (`0xc9f373`) and sets the kind's counter to 1 (`0xca7020`); a miss adds 1
  (`0xca4ba0`) and leaves `0x400` as it was. The counters start at 1, so the first roll of a fresh
  orb has chance `increment`.
- Slot 74 (`0xc9f3a0`) clears `0x400`. `0x68e3a0` calls it on every ability from the attack-stop
  routine (call at `0xb1fe92`).

### The armed hit (observed)

- The attack routine builds the attack record (`0x6a9940`), arms abilities through `0x697620`
  (slot 77), then calls `0x68f950(attacker, target, record)`, which calls slot 85 (`+0x154`) of the
  attacker's abilities in the order of their slot 78 value (`0x1b..0x29`) and stops at the first that
  returns nonzero; only when none does the attack routine apply the record itself. AIsb slot 85
  (`0xc9f3d0`) returns 1 only when it cast the wrapped ability.
- Slot 85: when armed (`0x100`) it runs `0x4705f0(target)` (the wrapped ability's target art, a
  presentation call, inferred), clears the armed bit and, with `0x400` set, asks the wrapped
  ability whether it may act: virtual `+0x354` (flag word; bit `0x4` set for the generic spell
  `0x100004` and for Purge `0x108004`), `+0x3e4(1, 0)` (`0xb4a530`: ability disabled count
  `[+0x3c]`, owner flag `0x40000000`, mana available through `+0x378`, ability flag `0x200`; the
  cooldown test is skipped by the first argument 1), `0xbebae0(target)` (target class test with the
  wrapped class word) and `+0x3f4(target)` (`0xb28750`: in cast range through `0x7ff460`, error
  `0x52`, then `0x682470`). On success it ORs `0x10000` into the wrapped ability, stores the target
  handle in its `+0xe8`/`+0xec` (`0xca7060`), runs the generic spell cast start `0xb4a940`, applies
  the attack record through the unit virtual `+0x120`, clears `0x400` and returns 1. On a failed
  check it returns 0 and leaves `0x400`.
- So the cast is free of cooldown and, for the shipped rows (no `Cost1`), of mana; the mana check
  itself is not skipped. The level is the orb's own level `[this+0x50]`, every value comes from the
  wrapped row.

### Black Arrow (`ANbs`) (observed unless marked)

- Class: registration window `0xc73ab0` (descriptor `0x11b8ef8`, base `AAsm` from `0xb2b1f0`), vtable
  `0xfac378`. Against the `AAsm` vtable `0xf18508` it overrides only slots 1, 7 (`0xc72f20`), 22 and
  196 (`+0x310`, `0xc72f30`, target mask `0xd0261`). Its apply routine `0xc72f40` is referenced from
  a table at `0xfac7a4` beside the name `CBuffDarkMinion`; that the generic cast reaches it is
  inferred from its target argument and the same shape of Purge's `0xb9e350`, the dispatch from
  `0xb4a940` is not traced.
- Apply (`0xc72f40`, target `t`): duration `HeroDur1` (`0x46f500`) when `0xcaf450(t)` says the
  target counts as a hero, else `Dur1 + [this+0xa0]` (a tracked real initialized to 0 by the
  constructor `0xb2ad50`; the recreation assumes it stays 0). A duration of exactly 0 stops here, so
  the shipped row (`HeroDur1` unset) never marks a hero. `0xcaf450` is true for a rawcode
  starting `A`..`Z`, for unit-type class `[+0x1c0] == 8` at level above 5, or when an ability of the
  unit answers `+0x1a0`; the last two are not decoded and the recreation uses the first only. The
  cast also stops when the target holds flag `0x1000` or already has the buff `BNdm`.
  Otherwise it builds the buff descriptor (`0xb3abe0`) and, when `BNba` is on the target, calls
  its refresh `+0x360` (timer restart `0x6f6c30`), else creates `BNba` (`0xc731f0`), calls
  `+0x32c` and `+0x35c` and attaches it (`0x6d4880`). Arguments: caster, owner player
  (`+0xec`), `UnitID1` (`0x46fb50`), count `DataB1` (`0x46ee20`, truncated), summon duration
  `DataC1` (`0x46eed0`), and the second `BuffID1` entry (`0x46ea70(level, 1)`, `BNdm`).
- `BNba` (vtable `0xfaba84`): apply `0xc71fe0` stores caster handle `+0xd0/+0xd4`, player
  `+0xdc`, unit type `+0xe0`, count `+0xe4`, summon duration `+0xe8`, minion buff `+0xf0`. Slot
  120 (`+0x1e0`, `0xc72130`) returns 1 (negative), slot 118 (`+0x1d8`, `0xc72140`) returns 0.
  Slot 160 returns 3 and slot 161 (`+0x284`, `0xc72160`) is the death handler `0x6d8060` runs per
  stage. For each of `count` units it creates a unit of the stored type for the stored player
  (`0xcaf240` -> `0x687ca0`, mode 2 when the summon duration is not 0, else `0x502`), tells the
  caster (`0xcaf230`, `0x68ab70`) and attaches `BNdm` for the summon duration; it then sets flag
  `0x1000` on the dead unit and clears `0x200` (a re-entrancy guard in the other users).
- Damage bonus: `DataA1` of the row is not read by the apply routine (it is 0 in the shipped row);
  the orb's own `DataA1` is the flat bonus.

### Purge (`Aprg`, wrapped as `AIpg`) (observed unless marked)

- Class: registration window `0xba1310` (descriptor `0x11b5dc8`, base `AAsm`), vtable `0xf3c8ac`
  overrides slots 1, 7 (`0xb9e2e0`), 22, 64 (`0xcb2660`, an autocast target test), 124, 196
  (`0xd008f`) and 197 (`0x108004`).
- Apply routine `0xb9e350` (inferred as the cast effect, as above): reads `DataF` (`0x46f0e0`) and
  `DataC` (`0x46eed0`), calls `0xba0870` with them and the target, then, when target flag
  `0x100` is clear and `0x696a90` is 0, applies buff `Bprg` through `0xba01b0` (it removes an
  existing `Bprg` first) with duration `HeroDur1` for the hero-class test above, else `Dur1`
  (`0xba0d70`), and `DataA`, `DataB`, `DataD`, `DataE`. Labels (`AEVAL_PRG1..6`): `DataA` movement
  update frequency, `DataB` attack update frequency, `DataC` summoned unit damage, `DataD` unit
  pause duration, `DataE` hero pause duration, `DataF` mana loss.
- Entry (observed, `0xb9e350`): reads `DataF` (`0x46f0e0`) and `DataC` (`0x46eed0`), then calls
  `0xba0870` (cdecl, 12 arguments) with the caster, the caster's owner player (unit virtual
  `+0xec`, a getter of `[unit+0x160]`), the target, the class word of virtual `+0x350` (a jump to
  `+0x310`, `0xb9e2f0`, `0xd008f`), the ability, the constants 1, 1, 0, 1, `&DataC`, `&DataF` and
  `0x40`. A target with flag `0x100` (dead) gets only the dispel and damage call and then a
  notification (`0xb9e482`, `0x4704d0`). Otherwise, when the caster's owner is not allied with the
  target's owner (`0x696a90` is `0x478e00`, the alliance bit of the caster's owner towards the
  target's owner, the same bit table `IsUnitAlly` reads at `0x499980`), it applies `Bprg`.
- `0xba0870` (observed): `enemy` is `0x696780(target, owner)`, true when the target's owner is not
  allied with the caster's owner (the `IsUnitEnemy` predicate, `0x499a30`). `ams` is a buff of
  class `Bams` (getter `0xbac9d0`) on the target. `summoned` is target flag `[+0x454] & 4`.
  `blocked` is 0 when the target flag `[+0x164] & 0x40000000` is set, else nonzero when the unit
  virtual `+0x11c` is nonzero or the veto `0x682180(target, mask, ability)` is.
  - An enemy that is `summoned`, without `ams` and not `blocked` takes an attack record
    (`0x6a9940`) of `DataC` damage, damage type `0x4000` (bit 14, magic; the poison tick uses bit
    26, universal), attack type 0 (spells), source the caster and flags `0x40`, plus `0x20` when
    the configuration lookup `0x4304c0(0xe69798, 0xf3e718)` is nonzero. It reaches the target through
    the unit virtual `+0x120` (`0x6677c0`, the damage path).
  - Any other target takes a record of zero damage with flags `0x10` through the same virtual, and
    the cast stops there when the target is `blocked`.
  - Then the dispel: `0x6d87a0(target, 0, 1, 0, 0, 0, 1, 1, caster, 1)`, stdcall with nine
    arguments in the order the call pushes them. The helper walks the target's buffs and removes
    each that is negative (`+0x1e0`) or positive (`+0x1e4`) as arguments 7 and 6 allow (both
    here), whose magic bit (`+0x1d8`, argument 2) is set, whose timed-life bit (`+0x334`,
    argument 3 is 0) and aura bit (`+0x1e8`, argument 4 is 0) are clear. There is no test of the
    caster or of the target's relation inside the helper. So a Purge removes positive and negative
    magic buffs from allies, enemies and neutrals alike; the original does not choose a polarity
    by relation. Buff base class `0xea5cc8` answers `+0x1d8` with 1 (`0x6f1e80`), `+0x1dc`,
    `+0x1e0`, `+0x1e4`, `+0x1e8` and `+0x334` with 0; a class decides the rest.
  - Last, when the target's `[+0x20] & 8` is clear, `ams` is false and `DataF` is not zero, it calls
    `0x69e310` (mana loss, not decoded; every shipped row has no `DataF`).
- `Bprg` (observed): class getter `0xb9d5e0`, vtable `0xf3c528`, base `0xea5cc8`. `+0x1d8` (`0xb9da00`)
  is 0 (a Purge does not remove it) and `+0x1e0` (`0xb9d9f0`) is 1 (negative). `0xba01b0` removes a
  `Bprg` already on the target (`0x6d6570` then `0x6d8530`), builds the buff and calls its slot
  `+0x35c` (`0xb9d5f0`) with the target, the duration, the caster handle (stored at `+0xd0/+0xd4`)
  and `DataA`, `DataB`, `DataD`, `DataE`. The duration is `HeroDur1` (`0x46f500`) for the hero
  test above and `Dur1` (`0xba0d70`) else.
  - Slot `+0x35c` starts the buff duration at `duration + [0x10db058]` (`0.01`, the same global the
    poison buff reads) through `+0x328`. It stores `A != 0` as `1.0` or `0.0` at `+0xe0`, `1 / A` at
    `+0xf0`, `B != 0` as `1.0` or `0.0` at `+0xe8`, `1 / B` at `+0xf8`, `DataD` at `+0x100` and
    `DataE` at `+0x108` (the divide is `0x9a02d0`).
  - Movement: `0xb67720(unit+0x3f8, -[+0xe0])` adds `-[+0xe0]` to the movement multiplier, the
    object `unit_speed.rs` models, so a nonzero `DataA` removes the whole speed (the speed floor of
    the unit applies). The attack side does the same with `0xb1e490(unit+0x3f4, -[+0xe8], 0)`.
  - The step timer at `+0x10c` starts at `Dur / A` (`HeroDur / A`) seconds with event `0xd01c1`,
    and the attack timer at `+0x120` at `Dur / B` with `0xd01c3`. That the timer repeats is
    inferred from the step arithmetic below, not from the timer routine.
  - Step (`0xba1130`, observed): when `|+0xe0|` is under `[0x10db054]` (`0.001`) nothing happens;
    otherwise `+0xe0 := +0xe0 - +0xf0` (`0x9a00b0` subtracts) and `0xb67720(unit+0x3f8,
    old - new)` gives that much back. After `A` steps the slow is gone, and the buff ends at
    `Dur + 0.01`. The attack step (`0xba1060`) mirrors it for `B`.
  - Pause (`0xb9d7e5` onward, observed): for `DataD` (a non-hero) or `DataE` (a hero) not zero,
    the movement object's disable counter (`[movement+0x80]`) is incremented through `0xb666b0`,
    which stops the unit's motion (`0x7fcd60` on `unit+0x26c`) when the counter leaves 0, and a
    one-shot timer at `+0x134` of that length posts event `0xd019c`. The handler `0xba0f30`
    decrements the counter (`0xb665d0`), stops the timer and applies a zero-damage record
    (flags `0x210`) from the caster.
  - Event dispatch (slot 3, `0xb9dca0`, observed): `0xd019c` runs `0xba0f30`, `0xd01c1` runs
    `0xba1130`, `0xd01c3` runs `0xba1060`, everything else goes to the base.
  - Removal (slot 209, `0xb9dcf0`, observed): stops the three timers, ends an active pause, gives
    back what `+0xe0` and `+0xe8` still hold, zeroes both, and applies a zero-damage record
    (flags `0x10`) from the caster when it still resolves.
- Cast through an orb (observed): the lightning orb handler (`0xca1610`) finds the `AIlp` ability of
  the owner, asks its `+0x434` (`0xb28470`, an error code, not decoded) and, when that is zero,
  calls its `+0x42c` (`0xca2810`, a jump to `0xb9e350`) with the target; it never runs the generic
  cast start `0xb4a940`, so no mana is spent and no cooldown set whatever the row's `Cost1` and
  `Cool1`. The spell orb path (`0xc9f3d0`) runs `0xb4a940`, which spends the mana of virtual `+0x3dc`
  through `0x69e310` and sets the cooldown; the shipped `AIpg` row has neither. The base class
  `0xb29c89` calls `+0x42c` from the cast completion, and the Purge class's own `+0x42c` slot is
  `0xb9e350` (the table entry at `0xf3ccd8`), so the generic cast reaches the same code. That the
  orb's `0xb4a940` reaches that completion in the same call is inferred, as for Black Arrow.
- Open: `+0x434` (`0xb28470`), the veto `0x682180`, virtual `+0x11c` and target flag
  `0x40000000` (the recreation treats the cast as not blocked); the zero-damage records and what
  the target does with them (retaliation, damage events); the damage path of the magic type
  (`0x6677c0`); the mana loss `0x69e310`; the movement disable counter and what a disabled unit
  does with its orders; the hero test beyond the rawcode rule (`0xcaf450`, see Black Arrow); and
  the magic bit of every buff class other than `Bprg`, the poison buffs and `BNba`.

### Recreation

- `crates/pcx-play/src/inventory_spell_orb.rs`: the registration windows, the stream (read from
  the push before the `0x69a030` call), the increment table (the 21 stores of `0xcaec30` and the
  reciprocal table of `0x9a1d00` read from the image), the step constants, and the row reader. The
  numerators, the stream number and the table are never constants of the crate; the counter rules
  (start 1, reset to 1 after a proc, +1 after a miss) are named constants citing the addresses
  above. Rows: `DataB1`..`DataD1`, `UnitID1` and the wrapped row; a map-modified orb or wrapped
  row refuses, and so does a wrapped `ANbs` row with `Cost1` or `Cool1`, because the generic cast
  start (`0xb4a940`) spends mana and sets a cooldown that are not decoded.
- Pickup admits the class for every wrapped row (the held effect is the flat damage and the air
  switch, as for every orb). The roll runs at the start of each swing at a covered target, draws
  one real from the secondary stream (`script_random`), updates the counters and keeps the proc
  until a hit uses it or the attack stops (`clear_combat_target`). An unseeded or too small
  random state refuses the roll.
- A hit that finds a held proc casts the wrapped ability before the attack damage. Black
  Arrow: the target must satisfy the wrapped `targs1` (class, relationship, `organic`); the buff
  `BNba` (negative) is attached for `Dur1` (zero for a hero) unless the target already has the
  minion buff; a second cast restarts the timer; expiry and `UnitRemoveBuffs` end it. The death of
  a marked unit refuses before any change (`require_black_arrow_unmarked`), because the summon
  placement `0xcaf240` -> `0x687ca0` is not decoded. A proc of the Purge class casts it (see
  "Purge recreation" below); a proc of any other wrapped class (Slow `Aslo`) refuses at the hit with
  the class named.
- Refused or open: the wrapped ability is not added to the holder's ability list (the original
  adds it through `0x6dd220`), so `GetUnitAbilityLevel(holder, 'ANbs')` reads 0; the cast range
  test (`+0x3f4`) is replaced by a refusal when the weapon range exceeds the wrapped `Rng1`;
  `[this+0x3c]`, `+0x268`, the veto `0x682180` and flag `0x200` are not modelled (the roll and
  cast assume they pass); the hero-class test is the rawcode rule only; summoned units use the
  recreation's `summoned` record flag; `RemoveUnit` of a marked unit drops the mark (whether the
  original runs the death handler is not traced). The first draw needs a seeded random state: the
  original generator's initial state is open (`original-rng-control.md`).
- Tests: `inventory_spell_orb_tests.rs` (rows, steps, stream, registration, image shapes, counters),
  `inventory_orb_host_tests.rs` (roll per swing on its own stream, mark, refresh, expiry, hero,
  death refusal, Purge refusal), `script_random/real.rs` (truncate, reciprocal, divide),
  `script_random/state_tests.rs` (secondary real). The ignored
  `local_original_image_supplies_the_stream_the_table_and_the_classes` reads the real image
  (`PEACECRAFT3_INSTALL`): stream `0x24`, the numerators above, and the divide within 2e-6 of the
  exact quotient.

### Purge recreation

- Rules: `crates/pcx-play/src/inventory_purge.rs` reads the registrations (`Aprg` over `AAsm`,
  `AIlp` over `Aprg`, `AIlb` over `AIDB`), the duration slack (`0x10db058`) and the reciprocal table
  from the image; the step `1 / DataA` and the intervals `Dur / DataA`, `HeroDur / DataA` go
  through the same divide helper as the spell orb table, so the arithmetic is the reviewed
  routine's. `DataB` and `DataF` must be zero (the attack-speed slow and the mana loss are not
  hosted), `DataA` zero leaves a buff without a slow, a repeating timer of no length refuses, and
  a map-modified row refuses (`effect_row`). A spell orb that wraps a row with `Cost1` or `Cool1`
  refuses (the generic cast start); the lightning orb ignores them, as the original does.
- Host: `crates/pcx-play/src/purge_host.rs`. One cast on a living target: a summoned enemy refuses
  (the magic damage path is not decoded) unless `DataC` is zero; a nonzero pause refuses (the
  disable counter is not hosted); a buff that is positive or negative and whose magic bit nobody
  read (`BuffTraits::magic_decoded`) refuses; a polymorphed target refuses when the slow would
  apply. Then the dispel runs through the buff host (`PURGE_DISPEL`, the `0x6d87a0` filter), a
  non-allied living target gets `Bprg` (negative, magic bit 0, decoded) and its movement
  multiplier loses `1.0` through `shift_unit_speed`, and the steps and the expiry run from
  `advance_purges` in the combat step. A step on the expiry runs first. `UnitRemoveBuffs`,
  death and removal end the slow and give back what is left. A `Bprg` on the target is replaced,
  never stacked, and a second cast restarts the decay.
- Decoded magic bits: only `Bprg`, the poison buffs (`BIpb`, `BIpd`, `BIpi`) and `BNba` declare
  `magic_decoded`. The frost, corruption and curse buffs are negative with the bit unread, so a
  Purge cast on a unit that carries one refuses by name until its class is read.
- Tests: `inventory_purge_tests.rs` (rows, registrations, refusals, the real image in an ignored
  test), `inventory_purge_host_tests.rs` (slow and steps for a unit and a hero, replacement
  by later hits, the dispel filter, the unread-buff refusal, an ally, a summoned enemy, removal and
  death, the spell orb path and the shipped pause row).

## Open held classes (decoded in part, still refused at pickup; `AIpb` below is implemented)

Each class below is registered in the reviewed image and its first-level structure is read. The
effect is not fully decoded or its host mechanism is absent, so the pickup refuses with the reason
in `UNMODELED_PASSIVE_CLASSES` (`crates/pcx-play/src/inventory_powerup.rs`), test
`decoded_but_unmodeled_classes_refuse_with_their_reason`.

### `AIpb` (Orb of Venom `oven`, with `Apo2`): implemented and tested

Decoded from the reviewed image (SHA-256 `3f2ed012…`, `objdump`/`rizin`) and the shipped rows; no
runtime capture. Observed unless marked inferred.

- Registration: orb window `0xca6e00` (`push 0x11b98bc`, `push "AIDB"`, `push "AIpb"`), vtable
  `0xfc1848`, slots 1, 7 (`0xca2a80`), 22, 83, 84, 85 and 203 (`0xca2ca0`, returns `0x4000000`) differ
  from `AIDB`. Poison class `Apo2`: window `0xca6de0` (`push 0x11b994c`, `call 0xbe2590` which
  returns `Apoi`, `push eax`, `push "Apo2"`, `call 0x7f6360`), descriptor `0xfc11f8`, factory
  `0xca1c20`, constructor `0xca3e00`, vtable `0xfc1b8c`, class getter `0xca2d20`. `Rules::read` decodes
  both windows and requires the orb over `AIDB` and the poison class over `Apoi`.
- Item: `oven` lists `AIpb,Apo2` (`ItemData`). The orb handler (slot 85, `0xca2ab0`) runs, when the orb
  is armed (`0x100`) and the owner flag `0x40000000` is clear, the callback `0xca6bf0` over every
  ability of the owner (`0x6d6460`). The callback keeps the abilities whose class is `Apo2`
  (`0x7f55f0` against getter slot `+0x1c`) and whose slot `+0x90` equals the orb's, which pairs the
  two abilities of one item. For that ability it calls slot `+0x350` (`0xbdcf40`, the target-class
  veto `0x682450` over the poison row's own `targs1` word `[this+0x4c]`; nonzero skips the whole
  hit, including the attack damage), then slot `+0x354` (`0xca2d70`, apply poison), then applies the
  attack record through the unit virtual `+0x120`. The poison is therefore applied before the hit's
  damage. The unit-target slots 83 and 84 (`0xca2c00`, `0xca2b60`) find `Apo2` by class instead and
  call `+0x358` and `+0x35c`; their target kinds are not decoded.
- Row values (slot `+0x354`, owner flag clear): `DataA` (`0x46ed70`, level record `+0x20`) damage per
  second, `DataB` (`0x46ee20`, `+0x24`) move slow, `DataC` (`0x46eed0`, `+0x28`) attack slow, `DataD`
  (`0x46ef80`, truncated by `0x9a07b0`), `Dur1` (`0x46f3a0`, `+0x8`) or `HeroDur1` (`0x46f500`, `+0xc`)
  when `0xcaf450` qualifies the target (inferred: hero). The owner flag zeroes `DataA`..`DataC`.
  `DataD & 8` indexes `BuffID1` (`0xb3abe0` passes `(DataD & 8) != 0` to the buff-id getter
  `0x46ea70`); `DataD & 7` is the stacking mode. Mode 0 looks for one `BIpb` buff on the target and
  refreshes or creates it (`+0x388`, `+0x380`); that path runs through the base refresh `0x6f5020`,
  which has a flag-`0x80` branch that is not decoded, so mode 0 stays refused. Modes 1 to 7 are the
  stacking path below. `DataD` bits bound which of `A`, `B`, `C` a stack carries (bit 1 `A`, bit 2
  `B`, bit 4 `C`; the other values read as 0).
- Row `Apo2`: `DataA1` 9, `DataB1` and `DataC1` unset, `DataD1` 1, `Dur1` 6, `HeroDur1` 6, `BuffID1`
  `BIpb,BIpd`, `targs1` `ground,air,organic`; row `AIpb`: `DataA1` 5 (the flat orb damage), `DataE1` 2
  (the air switch), `targs1` `ground,air,organic`.
- Stacking path (mode 1, `0xca2d70`, then `0xca4d60`, `0xca53f0`): the callback `0xca4d60` walks the
  target's buffs for class `BIpi` whose source handle (`+0xd0/+0xd4`, compared by `0xbe29e0` with the
  holder's handle) is the holder; found, it restarts that buff's timer with the new duration (`+0x358`
  = `0x6f6c30`) and the hit adds nothing else. Not found, a `BIpi` buff is created (`0xca53f0`, class
  vtable `0xfbec7c`, base `Bpsi`) and attached (`0x6d4880`), storing the holder, the masked
  `A`/`B`/`C`, `DataD` and the buff id (`0xbdf970`). So there is one stack per holder: the same holder
  refreshes, another holder adds a stack.
- Aggregate: attaching a `BIpi` buff (slot `+0x35c`, `0xc9d190`) looks for a `BIpd`-class buff on the
  target (`0x6d6570`, class `Bpsd`). Absent, it creates one (`0xca52f0`, vtable `0xfc2480`, buff id
  copied from the stack, default `BIpd`), applies the stack's `A`, `B`, `C` with duration 0 (no
  expiry) and the stack's holder as source, and attaches it. Present, `0xbe2740` adds 1 to a counter
  at `+0x104`, adds the stack's values to the sums at `+0xf0` (`A`), `+0x10c` (`B`), `+0x114` (`C`),
  and replaces the source with the new stack's holder (refresh `+0x388` = `0xbdec40`, which calls the
  base refresh `0x6f5020` with duration 0). Removing a stack (slot `+0x360`, `0xc9d390`) subtracts its
  values and the counter through `0xbe2400` and removes the aggregate (`0x6d8530`) when the counter
  was 0. The counter is therefore stacks minus one.
- Tick (aggregate buff, `0x6efd30`, slot `+0x36c` of the periodic buff base): the first tick comes
  `[0x10db058]` seconds after the aggregate is created (slot `+0x360`, `0x6ef800`, initialized float
  `0.01`), later ticks every `[0x112d810]` seconds (slot `+0x364`, `0x6ef820`, set to the integer 1 by
  `0x401460`) through the timer events `0xd01e2` and `0xd01c1` (handler `0x6ef880`). A tick is
  skipped when the target has a `BUsl` buff (class getter `0xb3ba50`). The amount is the sum `A`
  (`+0xf0`, slot `+0x368`); a negative amount heals (`EtherealHealBonus`, not decoded). Otherwise it
  builds an attack record (`0x6a9940`) with that amount, damage type `0x4000000` (universal, slot
  `+0x37c`, `0xca3c00`), attack type 0 (the spells row), the source unit (handle `+0xf4/+0xf8`, null
  when it no longer resolves) and flags 2 (0 on the first tick), and applies it through the target's
  virtual `+0x120`: no armor, killing allowed. The `MagicImmunesResistDamage` gate does not cover
  the universal type (the masks at `0x10dce64` and `0x10dce68`, read from the image, lack bit 26).
- Buff traits (`BIpb`, `BIpd`, `BIpi`): magic 0 (`+0x1d8`), physical 0 (`+0x1dc`), negative 1 (`+0x1e0`),
  positive 0, timed life 0 (`+0x334`), auto-dispel 0 (`+0x338`, calls `+0x1d8`).
- `organic` (`targs1` token, mask `0x4000000` in the table at `0x10dce70`) is the unit class bit the
  class routine `0x6b1c90` sets when the unit flag `[+0x454] & 0x8` (mechanical) is clear.
- Slows: `+0x380` (`0xbde9a0`) adds `-B` to the movement multiplier (`0xb67720` on `unit+0x3f8`,
  the same object `AIob` uses) and `-C` to the attack-side object (`0xb1e490` on `unit+0x3f4`).
  The shipped venom row has no `B` or `C`, so no slow is hosted and a nonzero `DataB1` or `DataC1`
  refuses.

Recreation: `OrbKind::Poison` (`inventory_powerup.rs`), host `orb_host.rs`. A hit that arms the orb
(the orb's `targs1` classes, `organic` excluding `UNIT_TYPE_MECHANICAL` units) attaches the buff
(negative only) and one stack for the holder, or restarts the holder's stack timer. The first stack
starts the tick schedule: the first tick at hit + 0.01 (read from the image,
`POISON_FIRST_TICK_VA`), then every 1.0 (`POISON_TICK_INTERVAL`, the integer from `0x401460`). Each
tick is the sum of the stack amounts through the spells row of the damage-bonus table, scaled by
held spell resistance (same record fields as the fire splash: inferred), credited to the holder of
the latest added stack, and skipped on a `BUsl` buff or an untargetable unit. A stack ends at its
own expiry; the last one ends the poison and the buff. `UnitRemoveBuffs` of the poison buff and
death end all of it (inferred: removal of the aggregate and every stack). A missile weapon poisons
on impact. Tests: `inventory_venom_tests.rs` (rows, registrations, refusals) and
`inventory_venom_host_tests.rs` (tick schedule, refresh, stacking across holders, hero duration,
kill, mechanical target, sleep, buff removal, missile).

Refused or open: `DataD & 7 == 0` (single-buff mode), `DataB1` or `DataC1` nonzero, a negative
`DataA1` (heal over time), a map-modified `AIpb` or `Apo2` row, orb and poison `targs1` that
differ, an orb row without exactly one `Apo2` beside it, poison buffs of two different ids on one
unit, and a tick whose holder unit was removed (the client resolves a null source). The owner flag
`0x40000000`, the target kinds of slots 83 and 84, and what dispel does to only the aggregate or
only one stack are not decoded.

### `AIlb` (Orb of Lightning old, `olig` with `AIlp`): implemented

- Registration (observed): window `0xca6da0` (`push 0x11b97e4`, `push "AIDB"`, `push "AIlb"`),
  factory `0xc9ebe0` -> `0xca6570`, vtable `0xfc0ec4`. It differs from `AIDB` in slots 1, 7
  (`0xca1600`), 22, 83 (`0xca1930`), 84 (`0xca16d0`), 85 (`0xca1610`), 124 (`0xca1ba0`) and 203.
- Handler (slot 85, `0xca1610`, observed): armed (`0x100`), owner flag `0x40000000` clear, it finds the
  owner's `AIlp` ability (`0x6d6570`, class `0x41496c70`), asks its slot `+0x434` (`0xca2800` ->
  `0xb28470`) whether it may act on the target, runs its slot `+0x42c` (`0xca2810` -> `0xb9e350`) on
  the target, then applies the attack record through the unit virtual `+0x120`. Slots 83 and 84
  (`0xca1930`, `0xca16d0`) do the same for other target kinds with the row values read like the
  poison class (`0xca66d0` is the non-hero duration getter, `HeroDur1` for a hero).
- `AIlp` (observed): registration `0xca6dc0` (`push 0x11b97cc`, `call 0xba0e30` which returns `Aprg`,
  `push "AIlp"`), descriptor `0xfc075c`, factory `0xca0800`, vtable `0xfc13e8`, class getter
  `0xca27f0`. It derives from the Purge class `Aprg` and uses the Purge code (`0xb9e350`) as the
  cast. Row `AIlp` ("LightningPurge"): `Dur1` 3, `HeroDur1` 0.3, `DataA1` 2, `DataC1` 35, `BuffID1`
  `Bprg`, `targs1` `ground,air,ward`. `olig` lists `AIlb,AIlp`; `AIlb` row: `DataA1` 6, `DataE1` 2.
- Recreation: the pickup is admitted. `OrbKind::Lightning` carries the Purge of the `AIlp` row on the
  same item; the orb arms on its own `targs1` and the sibling must list the same classes. Every
  armed hit casts the Purge before the attack damage, free of mana and cooldown (see "Purge
  recreation").

### `AIrc` (Ankh of Reincarnation `ankh`): implemented

Decoded from the reviewed image (SHA-256 `3f2ed012…`, `objdump`/`rizin`) and the shipped rows; no
runtime capture. Observed unless marked inferred.

- Registration: window `0xb8c1e0` (`push 0x11b5a5c`, `call 0x46f5b0` which returns `abil`, `push
  "AIrc"`). Descriptor `0xf34978`, factory `0xb8a9b0`, vtable `0xf35200`, slot 7 `0xb8b7c0`. The
  attach (slot 34, `0xb8b7d0`) only stores the handle of the carrying item at `+0x70/+0x74`. Beside
  it the image registers the hero class `AOre` (window `0xb8c200`, base from `0xb4b180`, which
  returns `AAsp`; vtable `0xf34a10`), the creep class `ACrn` (window `0xb8c1c0`, base `AOre`,
  vtable `0xf34e08`) and the revive record class `XOre` (window `0xb8c220`, vtable `0xf34988`,
  constructor `0xb8bdf0`). Shipped rows: `AIrc` `DataA1` 7, `DataB1` 500, `DataC1` -1, `EfctID1`
  `XOre`; `AOre` `DataA1` 5 (four levels, `Cool` 240); `ACrn` `DataA1` 7. Item `ankh`: class
  `Charged`, `uses` 1, `perishable` 1, `usable` 0, `drop` 0, `cooldownID` `AIrc`.
- Where it runs: the kill routine `0x6896d0` returns for a unit that is already dead, asks the
  death-prevention handlers (`0x6d7580`: abilities whose slot `+0x274` equals the stage and whose
  slot `+0x278` answers 1; neither is `AIrc` or `AOre`), then sets the dead flag (`[unit+0x164] &
  0x100`) and calls the unit death routine `0x6884f0(1, 1)`. That routine runs the death handlers
  of the unit's abilities with `0x6d8060`: for stage 1 to 4, each ability whose slot `+0x280` returns
  the stage has its slot `+0x284` called. It posts the death events `0xd0154` and `0xd0155` after
  the handlers (`0xd0154` and `0xd0156` on the branches that skip `0xd0155`), never before
  and never conditionally on a handler. `AOre` returns stage 2 (`0xb8b2d0`), `AIrc` stage 3 (`0xb8b860`).
- Handler (`0xb8b870`): resolves the item from `+0x70` and requires its charge count `[item+0x18c]`
  to be nonzero; the owner flags `[owner+0x164] & 0x40000000` and `[owner+0x20] & 1` clear (their
  meaning is not established); no pending revive record of the unit (the lookup `0xb8c0c0` runs
  `0xb8c100` over the objects of class `XOre`, comparing the record's unit handle); and the
  location test `0x7f6a00(position, position, 2, 0)` to answer 0. That test is the walkability query
  of `0x7f6890` (`ai-construction.md`: `IsTerrainPathable` maps walkability to mask 2): it answers 1
  when the cell of the unit's position is blocked for walking, by terrain or by a dynamic spatial
  entry whose state and low pathing mask `0x8bdbe0` accepts. A blocked position stops the handler.
- Effects, in order (`0xb8b870`): `0x69cf30` with no unit empties the handle pair
  `[unit+0x45c]`, `[unit+0x460]` (the revive routine empties it too; what reads it is not
  established, the recreation does not model it); `0x6c6f10` spends the item's charge; `DataB1` is
  raised to at least the global `0x112d810` (zero); `DataC1`, when below zero, becomes the unit's
  current mana (`0xb20d20` reads the stat object at `+0x1c0` through `0x800d80`); `0xb8bfc0`
  creates the `XOre` record (vtable `0xf34988`); its start virtual `+0x78` (`0xb8aab0`) is called
  with the unit, `DataA1`, the ability's effect id (`+0x34`), `DataB1` and `DataC1`; the unit flag
  `[unit+0x164] |= 0x1000` is set and `0x682c80` runs (a presentation and event routine that
  posts events `0xd0144` and `0xd0162`, not decoded).
- Charge (`0x6c6f10`, observed): with item flag `[item+0x20] & 0x1000` clear and a positive count it
  decrements `[item+0x18c]`; it notifies the holder (`0x68dcb0`); and when the flag is clear, the
  item flag `0x40` (perishable) is set and the count is zero it destroys the item (`0x6c4f70`,
  `0x5f6d40`). So the charge is spent at the death, no cooldown is set, and the shipped `ankh`
  (`uses` 1, `perishable` 1) is gone at once.
- Record (`0xb8aab0`, `0xb8aeb0`, `0xb8ad60`, observed): the start stores the unit handle at
  `+0x3c/+0x40`, the effect id at `+0x48`, `DataB1` at `+0x50` and `DataC1` at `+0x58`, creates the
  effect `0x5c17f0`/`0x6f78d0` (presentation) and starts the timer at `+0x24` with the delay `DataA1`,
  the event `0xd01c4` and the record as handler (`0x7fbe90`). Virtual `+0x7c` (`0xb8ad10`) clears
  `[unit+0x16c]` bits `0x0a10` (the mask `0xfffff5ef`) for a dead unit. The event handler (slot 3, `0xb8aeb0`) ends the
  effect, runs the revive `+0x80` (`0xb8ad60`) and destroys the record. The revive requires the
  unit's object to be live and the dead flag `0x100` set, then reads the unit's own position (unit
  virtual `+0xb8`) and puts the unit back at it (virtual `+0x180`), so it returns where it died.
  It calls `0x69bd90(owner, &life, &mana, 0x500, 1)`, the revive core `ReviveHero` also uses
  (`0x69c4f2`, flags `0x502`), then `0x6a0d10(0)` and `0x5f5990(unit)` (presentation).
- Revive core (`0x69bd90`, observed): life is the maximum (`+0x130`) when `|DataB1|` is under
  `[0x10db054]` (0.001), else the value raised to `[0x10db0a8]` (0.5); it is set through unit virtual
  `+0x124`, whose stat object clamps the reading to the maximum. Mana is set to `DataC1` (or the
  mana at death). The dead flag is cleared, the supply bookkeeping flags `0x500` re-add the unit
  type's food, and the unit's death events are not posted again.
- Priority against `AOre` (observed): the hero handler (`0xb8b2e0`, stage 2) runs first. It needs
  its own enable count `[+0x3c] <= 0`, the usable test `+0x268`, the owner flags above, no pending
  record, the mana check `+0x378` and ability flag `0x200` clear, and the same location test. When
  it fires it sets its cooldown (`+0x2ec`, `+0x3ac`) and starts a record (`0xb8b3e0`) with the
  row `DataA1`, the maximum life (`+0x130`) and the maximum mana (the stat object's `+0x84`). The
  `AIrc` handler then finds that record pending and does nothing, so the ankh keeps its charge.
  The cooldown and mana state of `AOre` decide which of the two fires.
- Not decoded: the flags `0x40000000` and `[+0x20] & 1`; `0x682c80`; the dynamic part of the
  location test; the handle pair at `+0x45c`; whether a dying unit's items drop before or after
  the handler and whether the client drops them for a unit that will return; the revive's hero
  events; the effect at `EfctID1`; which of several ankhs fires first (the order of the unit's
  ability list).

Recreation: `crates/pcx-play/src/inventory_reincarnation.rs` reads the three registrations from the
image and the `AIrc` row (`DataA1`, `DataB1` raised to zero, `DataC1`, `EfctID1` as a rawcode);
the pickup is admitted and the row is held per item (`AttributeBonus::reincarnation`).
`crates/pcx-play/src/reincarnation_host.rs` plans the handler at the top of `kill_unit`, before
any change: it refuses a unit that also has a hero reincarnation ability (`AOre`, `ACrn`, from the
unit's ability list), several held ankhs, a flier, a host without the item-charge or the terrain
pathing admission, and an item without a charge to spend. A blocked walkability cell
(`terrain_unwalkable`, the static raster only) or a pending record lets the unit die as usual and
keeps the charge. Otherwise the death spends the charge through `spend_item_charge`, removes a
perishable item that reaches none, and schedules the revive at `now + DataA1`. The rest of the
death is the existing path, so the death events fire, the unit stays dead, and its items follow the
unchanged drop rule. The combat step revives the unit (`revive_unit`, the core shared with
`ReviveHero`) at its death position with `Reincarnation::restored_life` and `restored_mana`; a
revive the placement refuses is an error, a unit that was removed meanwhile keeps no record.
Tests: `inventory_reincarnation_tests.rs` (rows, amounts, registrations, the real image in an
ignored test) and `reincarnation_host_tests.rs` (the whole cycle, the unwalkable stop, one charge,
removal, and each refusal); `terrain_pathing_host_tests.rs` covers the raster query.

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
- `MiscGame.txt` `MoveSpeedBonusesStack` (string `0xf55f08`), shipped values: root `Units\MiscGame.txt`
  0, `Melee_V0` 0, `Custom_V0` and `Custom_V1` 1 (the campaign chapters run with the custom sets).
  Attach (`0xbbeee0`), detach (`0xbbefc0`) and the other `AIms` handlers (`0xbbf020`, `0xbbf140`,
  `0xbc8670`) read it with `0x4304c0`. With 0 they call the recompute `0xb69430` (a jump to
  `0xb6ab70`) and never set the ability flag `0x80`. With nonzero they set flag `0x80` and skip the
  recompute; the update handler slot 15 (`0xbbf3c0`) sets the flag from the key and clears it for an
  illusion when `Misc/IllusionsGetMoveSpeedBonus` is 0.
- Where the stacking total goes (decoded 2026-11-11, observed in the image): it does not reach the
  speed formula. The formula `0xb681b0` gets its bonus only from `0x6d91f0` (its single caller, at
  `0xb681d5`), which calls slot 97 (`0xbbf220`) on every ability of the unit and keeps the maximum.
  Slot 97 returns `DataA1` whether or not flag `0x80` is set, so the formula is
  `limit((max(DataA1) + base) * multiplier)` under both settings. Slots 181 and 182 (`0xbbf330`,
  `0xbbf370`) store `DataA1` at `+0x28` of a stat record only while the flag is set. The sole callers of
  virtual `+0x2d4` and `+0x2d8` are `0x6d69c0` and `0x6d73cb`, both inside `0x6d66d0`, which sums the
  records of all abilities into two output records (positive and negative). `0x6d66d0` has eight
  callers: the unit info panel update `0x6487a5` (function `0x6484a0`) and a second panel reader
  `0x659f8a` (reads `+0x38` and `+0x40`), the hero attribute getters `0x690cad`, `0x6917ff`,
  `0x692b2d` (they read `+0x50`, `+0x54`, `+0x58`), `0x69ca1d`, `0x69dbbd` and `0xb3e010`. Only
  `0x6487a5` reads `+0x28`: it copies `+0x24` and `+0x28` of both records into the value pair that it
  passes to the panel element at `this+0x158`, the third icon created at `0x632c5c` from the string
  `SimpleInfoPanelIconArmor`. The `BUau` buff writes its speed fraction into the same field (`0xc64ae0`,
  `0xc64b20`, value `0.1`), so the field mixes flat and fractional amounts and is display data. What the
  panel element does with the pair (vtable `+0x70`) is not decoded.
- What the flag changes, then, is the cache refresh: `0xb6ab70` computes `0xb681b0` and pushes it to the
  unit's mover (unit virtual `+0xb8`, then `+0x18`). The move-order start `0xb69ec5` and the other
  `0xb681b0` callers (`0xb69fa4`, `0xb6a1e2`, `0xb6a25e`, `0xb6a364`, `0xb6a862`, `0x654fe4`,
  `0xad25d0`, `0xb65c50`) read the formula live. Under a set flag, a bonus that attaches or detaches
  while the unit walks leaves the mover at its old speed until the next order or recompute (inferred
  from the call structure; not observed at runtime). `GetUnitMoveSpeed` (`0xb68160`) reads the
  formula live.
- Implemented (2026-11-11): the speed formula, the limit and the multiplier writers are in
  `unit-move-speed-model.md`. The pickup binds `AIms`: each held item sets its `DataA1` as a
  per-item entry of the unit's bonus list, the largest entry counts, dropping the item (also on
  death or removal) takes it out, and the bonus is added to the base before the multiplier and the
  limit. A nonzero `MoveSpeedBonusesStack` is admitted with the same formula. The recreation re-anchors
  a walking route at once, which the skipped refresh would not do, so under a set flag a pickup or drop
  on a walking holder refuses (`require_unit_not_walking`). Also refused: a map-modified `AIms` row
  (the overrides are not decoded), a unit without the speed host, position or clock, and a
  polymorphed holder. Tests: `inventory_speed_host_tests.rs` (`stacking_rule_sets_*`) and the
  `speed_bonus_*` tests in `inventory_orb_tests.rs`.

### `AIau` (Legion Doom-Horn `lgdh`, code `AUau`): decoded in part, still refused

Decoded from the image (no runtime capture). Rows (shipped, same in root and `Custom_V0`): `Area1` 900,
`targs1` `air,ground,friend,self,vuln,invu`, `DataA1` 0.1, `DataB1` 0.5, `DataC1` 0, `BuffID1` `BUau`,
`Dur1` 0, `HeroDur1` 0, `Cool1` 0. The editor metadata rows `Uau1`, `Uau2`, `Uau3` label DataA, DataB, DataC
"Movement Speed Increase (%)", "Life Regeneration Increase (%)", "Percent Bonus" (bool). The hero spell
`AUau` has the same `Area1` and `targs1`, with `DataA` 0.1/0.2/0.3 and `DataB` 0.5/1/1.5.

Classes (observed). The ability is `CAbilityAuraUnholy` (vtable `0xfa4340`, class getter slot 7 `0xc64f50`
returns `AUau`), derived from `CAbilityAura` (vtable `0xf36710`, class id `aura`, `0xb90db0`). The buff is
`CBuffAuraUnholy` (vtable `0xfa3fac`), registered at `0xc657f0` over `BAUR`. The Unholy ability overrides
slots 1, 7, 22, 202 (returns `0xd00f7`, base `0xd0142`) and 212 to 214 (`0xc64f70`, `0xc64fb0`, `0xc65000`).

Aura base mechanics (observed, labelled where inferred):

- Attach (slot 32, `0xb8f890`): reads `Area` (string at `0x10dce00`) of level 1 into the tracked field at
  `+0x90` (value `+0x94`) and `DataA` into `+0x88` (value `+0x8c`), sets flag `0x10000000`, and registers
  the ability in the unit's lists (`0x6c0010`, `0x6bffb0`, `0x6c00a0`, `0x698470`). If flag `0x80` (active)
  is set it starts.
- Start (slot 57, `0xb8fd10`; run from event `0xd01a7`): creates a periodic timer at `this+0x9c` with
  period 2.0 s (`0x9a1620` converts the integer 2; event `0xd01c1`; `0x7fbe90`), and a range registration at
  `this+0x98` (`0x800070`, class `CRangeEventReg`, vtable `0xe7d854`, constructors `0x8035e0`, `0x8030e0`)
  with radius `Area1` (`0x8037a0` writes it at `+0x50`), event `0xd01c7` and a second period argument
  2.0 / 4.0 = 0.5 s (`0x9a02d0` divides; `0x112d820` is 4.0 from the static initializer `0x401610`).
  `0x8bc260` stores that argument at registration `+0x44` and `0x8b7ea0` schedules the next firing at game
  time plus that value; that 0.5 s is the registration's repeat interval is inferred from those two
  routines.
- Events (handler slot 3, `0xb90490`): `0xd01c7` (unit handle at event `+0x10`, resolved by `0x691940`)
  runs the target test and application `0xb90c10(this, unit)` while flag `0x80` is set; `0xd01c1` sets bit
  2 of the registration's flags (`0x803840(1)`) and runs `0xb90c10(this, owner)`; `0xd01a7` starts;
  `0xd01a0` stops (slot 56, `0xb8fcc0`: ends the effects through virtual `+0x310`, then releases
  the registration); `0x80261` re-evaluates the active flag (`0xb90ea0`).
- Target test (`0xb90c10`): reject when virtual `+0x348` (`0xb90440`) returns nonzero. That routine calls
  `0x682450(owner, unit, this+0x4c)`, which forwards to `0x6adb90` (the allowed-target test against the
  ability's `targs` mask; the body of `0x6adb90` is not read) and returns its result when nonzero, else
  returns `0xdd` when `0x682180(unit, slot 202 value)` is nonzero (a scan of the unit's abilities through
  their virtual `+0x26c`; not decoded, inferred to be the no-stacking test against the group id
  `0xd00f7`). Also reject: target flag `[+0x20] & 0x4000000`; owner flag `[+0x164] & 0x10`; owner flag
  `[+0x164]` bit 31 while `Misc/AnimatedUnitsBestowAuras` is 0; owner `[+0x20] & 0x4000000` while
  `BurrowedUnitsBestowAuras` is 0; for a target other than the owner, owner `[+0x164] & 0x40000000`
  while `IllusionsBestowAuras` is 0 and owner `[+0x164] & 0x1000000` while `InvisibleUnitsBestowAuras` is
  0; owner polymorphed (`0x6d7d50`) while `PolymorphedUnitsBestowAuras` is 0. The `Misc` key
  `FlyingHeroesBestowAuras` is read and its result discarded. All six keys are 0 in root, `Melee_V0`,
  `Custom_V0` and `Custom_V1`; what each flag bit means is inferred from the key names.
- Application (`0xb90c10`, rest): the buff id comes from `BuffID` of the level (`0xb905f0`, `0x46ea70`).
  `0x6d6570` looks for an existing buff of that id on the target. None: slot 214 (`0xc65000`) builds the
  `BUau` buff (`0xc65610`), sets its source, and calls buff virtual `+0x374` (`0xc647d0`) with `DataC1 != 0`,
  which sets buff flag `0x100` ("percent"); then slot 212 (`0xc64f70`) calls buff virtual `+0x378`
  (`0xc64820`) with `DataA` and `DataB`, which stores them at buff `+0xe8` (through the tracked field
  `+0xe4`) and `+0xf0` (`+0xec`) and applies both halves. Existing buff: slot 213 (`0xc64fb0`) calls
  `+0x37c` (`0xc64860`), which compares the new values with the stored ones and, on a change, removes
  and re-applies the matching half.
- Speed half (observed): `0xc65560` adds `+0xe8` (`DataA1`, a fraction) to the unit's speed multiplier
  through `0xb67720`, once per buff (buff flag `0x200`); `0xc65890` subtracts it. Both are skipped when
  the target carries unit flag `0x40000000` and `Misc/IllusionsGetMoveSpeedBonus` is 0 (the shipped
  custom sets set it to 0). The multiplier is the additive one in `unit-move-speed-model.md`.
- Life regeneration half (observed): `0xc65420` runs once per buff (flag `0x400`), skipped for a target
  with unit flag `0x40000000` while `Misc/IllusionsCanRestoreLife` is 0 (`Custom_V0`, `Custom_V1` set 0).
  The amount is `+0xf0` (`DataB1`, 0.5); with buff flag `0x100` (`DataC1` nonzero) it is multiplied by
  the result of the target's virtual `+0x130`, which is not identified. `0xb3f600` hands the amount to
  `0x800f40` on the life pool at `unit+0x1a0` with duration 0.0, which creates a permanent extra rate
  operation (the same constructor as the unit's own regeneration, `unit-regeneration.md`); its handle is
  kept at buff `+0xf4`. `0xc65810` ends it through the handle's virtual `+0x8` and releases it. The
  pool sums the rates and evaluates `value + rate * dt` continuously, so the unit's own regeneration
  (armed only for unit flag `0x40`, i.e. under its `regenType` rule) and this operation are
  independent terms. The amount is not an integer, so the integer held-regeneration sum of the
  recreation cannot carry it.
- Buff end (observed): buff slot 209 (`0xc64a20`) removes the speed half (`0xc65890`) and then the
  regeneration half (`0xc65810`).

Not decoded, so `AUau` stays refused (`UNMODELED_PASSIVE_CLASSES`):

- When the range registration fires `0xd01c7` (on entry only, or at each interval for every unit in
  range) and what its flag bit 2 changes; the aura and buff code read no duration (`Dur1`, `HeroDur1` are 0),
  and no path that removes a `BUau` buff when its unit leaves the area was found, so the buff lifetime
  and removal are open.
- The body of the allowed-target test `0x6adb90` and the stacking test `0x682180`/`0x682210`/`0x6822e0`
  against the hero `AUau` (the item text says the two do not stack).
- The target virtual `+0x130` used by the percent mode.

Recreation consequence: an aura host needs the removal and lifetime rule; none is built, because the rule
is not decoded.

### Damage admission by class code

The `AIat` attach `0xbc2a20` reads `DataA1` of any ability whose class code is `AIat` and does not
consult `Iatt.useSpecific`. That list omits the shipped `AItx` (`AttackBonus +20`, `DataA1` 20), so
the damage branch admits by class code alone; the column and the W3A override pointer still come
from `Iatt`.
