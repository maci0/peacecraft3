# Holy Light ability execution

Holy Light is the paladin ability `AHhb` in the campaign Human01 script
(`SelectHeroSkill( udg_Arthas, 'AHhb' )`, unit `Hart`). The cast is
`IssueTargetOrder(unit, "holybolt", target)`, where the order name comes from the
ability's `AbilityFunc` `Order` profile and the order number from the Legacy order
registry.

## Source

- `Units\AbilityData.slk`, row `AHhb` (Legacy 1.29 copy, read with `objx unit`):
  `levels` 3, `reqLevel` 1, `levelSkip` 0, `Cost1` 65, `Cool1` 5, `Rng1` 800,
  `DataA1`..`DataA3` 200, 400, 600, `targs1`
  `air,ground,organic,notself,invu,vuln,nonancient`.
- `Units\HumanAbilityFunc.txt` `[AHhb]` `Order=holybolt`.
- `Units\HumanAbilityStrings.txt` `[AHhb]` `UberTip`: heals a friendly living unit
  for `DataA` hit points, or deals half damage to an enemy Undead unit.

The half-damage ratio is prose in the tooltip. No numeric column carries it. It is
recorded as `UNDEAD_DAMAGE_DIVISOR` with that provenance.

## Behavior

`cast_holy_light` in `holy_light_host.rs` runs from the `IssueTargetOrder` dispatch
in `combat_host.rs` when the order number is the admitted Holy Light order.

- The learned ability whose `Order` profile names the admitted order supplies the
  alias. Its rank must be above zero.
- Cooldown, mana (`Cost`), reach (`Rng` plus both collision radii, as attacks use)
  and the `targs` filter are checked. Failed checks return `false` with no change.
- Friendly, not undead, not mechanical targets gain `DataA` life up to max life.
- Enemy undead targets lose `DataA / 2` life through the weapon-damage path, so
  death and kill events follow that path.
- A successful cast spends mana, starts the cooldown and clears an attack target.

The `targs` tokens are validated. Any unlisted token, or a list without both
`air`/`ground` and `invu`/`vuln`, refuses the cast.

## Admission

`ObjectHost::admit_holy_light(order)` requires combat, hero skills and a registered
order name. `map_startup_host` calls it with `"holybolt"` after `admit_combat` when
hero skills are admitted. It is not admitted for maps without hero skills.

## Evidence and gates

- `cargo test -p pcx-play --lib holy_light` passes 4 tests on synthetic AbilityData
  and AbilityFunc tables. The tests cover heal and cooldown, undead damage, enemy
  organic, out-of-reach and low-mana refusal, and the `targs` filter.
- `cargo test -p pcx-play --lib` passes 823 tests.
- Not yet run against the original install. The values above come from the Legacy
  copy read by `objx`, not from a runtime path through the campaign.

## Not implemented

- Walking into reach. An out-of-reach cast returns `false` and does not move the
  caster.
- Cast animation, projectile and target art, and spell-effect events.
- Air/Ground filters beyond requiring both tokens.
- Ranks above one in the tests. Learning through `SelectHeroSkill` is covered by
  the existing hero tests only.
- UI command-card hotkey `T`. Only a script `IssueTargetOrder` reaches the cast.
- Undead classification through `IsUnitType(UNIT_TYPE_UNDEAD)` is not used here. The
  placement reader filters `UnitBalance` fields to numeric kinds, so the `type`
  list reads as absent through that path. This holy light code reads the column
  directly. The same defect affects `unit_type_host`, which is outside this change.

See [script hero skills](script-hero-skills.md) for ability learning and
[hero combat attributes](hero-combat-attributes.md) for the hero combat path.
