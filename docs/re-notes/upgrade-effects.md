# Upgrade effects

A researched upgrade level changes the units of the researching player. The host applies
`rmnx` maximum mana, `rhpx` maximum life and `rmnr` mana regeneration, and records every other nonzero effect as
unapplied with a reason. Code: `crates/pcx-play/src/upgrade_effects.rs` (slots, arithmetic),
`unit_upgrade_effects_host.rs` (units, ledger), the effect table in
`upgrade_research::Rules`.

## Data

Read from the selected install (Legacy 1.29.2.9231) and gated by
`cargo test -p pcx-play --test upgrade_effects_original -- --ignored` with
`PEACECRAFT3_LEGACY_DIR` set:

- `Units\UpgradeMetaData.slk`: the rows with an `effectType` give the slot layout. Per
  `effectIndex` 1..4 one row each for `EffectID`, `Base`, `Mod` and `Code`; the `field`
  column names the `UpgradeData` column (`effect1`, `base1`, `mod1`, `code1`, and so on) and
  `ID` is the field id a `w3q` edit uses. The host reads the layout from these rows.
- `Units\UpgradeEffectMetaData.slk`: 29 effect ids (`effectID` column) with their `Base`,
  `Mod` or `Code` data types: `ratd rlev rhpx rmnx rmvx rmnr rhpo rman rmov ratx rlum ratr
  rats rspi rhpr rsig ratc radl rmin rrai rent renw raud rasd rasr rroo ruro rart rtma`.
  `rarm` (armor, 11 stock rows) and `rauv` (1 row) are used by `UpgradeData` but are not in
  this sheet.
- `Units\UpgradeData.slk`: the top-level rows use 22 effect ids. `Rhst` (Sorcery Master
  Training, 2 levels) has `rmnx` 100/100, `rmnr` 0.325/0.325, `rhpx` 40/40 and `ratd` 0/0.
  The Reign of Chaos campaign reads the `Custom_V0\Units\` set instead (the recipe's
  selection prefix, `campaign_data_set_tests.rs`), identical in `War3.mpq` and `War3x.mpq`:
  there `Rhst` has `rmnx` 100/100, `rmnr` 0.25/0.25, `rhpx` 75/75 and `ratd` 1/1, and the
  rows use 18 effect ids. The host reads whichever set the caller selected. A cell of `-`
  is empty and reads as zero; `_` in an `effectN` cell means no effect.
- The map's `w3q` edits override cells by field id (last edit wins). A custom upgrade
  derives from its base row. A selector pair other than `[0, 0]`, a non-numeric base or
  modifier edit, an effect id that is not four bytes, or a missing metadata sheet makes the
  effect table unreadable. The counters still work, and a level change refuses with that
  reason.

## Executable evidence

Reviewed image `3f2ed012...0eed` (the same image the counters are bound to). The addresses
identify native routines. Implemented effect routines are pinned by SHA-256 in
`upgrade_effects::PROFILE`, and admission refuses an image that differs.

Bounded Kuna output retained under `.scratch/upgrades2/` agrees with the inspected
assembly on the attack selector, the owner-change ordering, the `inherit` loop, the
`global` dispatch and the unit-level apply and removal calls. It does not establish
the remaining source-field mappings or form-change behavior.

| Fact | Routine |
| --- | --- |
| Each effect class has an id getter and an apply routine `apply(unit, fromLevel, toLevel)` at vtable slot 31. | `0x6a4420`/`0x6a4430` (`rmnx`), `0x6a3960`/`0x6a3970` (`rhpx`) |
| Level value is `base + mod * (level - 1)`, and zero at level 0. | `0x6acc50` |
| The change is `value(to) - value(from)` (subtraction `0x9a00b0`). | all apply routines |
| `rmnx` passes the change to the unit's maximum-mana routine, mode 1. A negative change is applied only when the unit's maximum mana is above zero. | `0x6a4430` |
| `rhpx` multiplies the change by the owner's current health factor (player field `+0x35c`, the `SetPlayerHandicap` store), then the maximum-life routine, mode 1. | `0x6a3970`, `0x692340` |
| Mode 1 of both routines: a zero change stops before any write; the change is rounded (`0x9a1d70`, the selected unit-pool rounder); the maximum moves; the current pool becomes `round(ratio * new maximum)` with `ratio = current / old maximum` (0 when the old maximum is 0). Life adds the living floor of 1. | `0x680350`, `0x6be8c0` |
| A level change visits the owner's units whose type lists the upgrade and applies each effect in slot order. | `0x6e0b80`, `0x6dd240` |
| `rmnr` adds the level-value delta to the unit property at `+0x1dc`, then refreshes the mana regeneration operation. | `0x6a4600`, `0x680310`, `0x6a1070` |
| A unit entering the game applies `(0, current level)` for its owner's upgrades that its type lists. | `0x6dd280`, `0x694230` |

`0x694230` tests the unit type's list at row `+0x234`. The host reads that list from the
`UnitBalance` `upgrades` column (`hsor`: `Rhst,Rhpm,Rguv`), in list order.

## Behavior

- `SetPlayerTechResearched` on the upgrade model computes the new level, projects every
  matching unit one level at a time (regeneration integrated to the current clock first,
  as for the health handicap), then stores the level and writes the pools. Projection
  refuses more than one million unit, slot and level steps. A projection that fails (a maximum
  that would end nonpositive for life or negative for mana) refuses with an error naming the
  upgrade and leaves the level and all pools unchanged.
- A unit created afterwards (ordinary, summoned, construction) applies the owner's current
  levels to its creation pools. The ratio rule starts from those pools: a `hsor` with 75 of
  200 mana under `rmnx` +200 has 150 of 400, not 400 of 400. Corpses, illusions and units
  restored from a game cache are not adjusted.
- Matching covers every retained unit with vitals, dead ones included. Native enumerator
  options 4 and 6 clear `0x100` and `0x18` from the default `0x118` exclusion mask
  (`0x68eacd`, `0x68edad`, `0x68edb9`). With both clear, the state-mask predicate is not
  installed (`0x68eeb5`). The default mask includes the native dead-state bit, so that
  predicate does not exclude corpses here. In a synchronous modified Human01 probe
  (`.scratch/upgrades2/filter-reference/`), live, hidden, dead and just-removed
  Sorceresses all gained the `Rhst` maximum increases; dead life stayed zero. The
  removed handle still returned its original pools immediately after `RemoveUnit`.
  The probe does not establish membership after later cleanup. The recreation removes
  unit records through its own lifecycle, so the just-removed case remains open.
- `rmnr` adjusts the per-second mana regeneration rate, after the host integrates the old
  rate to the research clock. A negative or non-finite resulting base rate refuses.
  Hero attribute and passive item interactions use the existing model and have not been
  compared with the client for this effect.
- Rounding uses the health-handicap rounder when admitted and half-up otherwise, for mana
  as well as life, because both routines call the same helper.

## Unapplied effects

Every other slot with a nonzero change is listed by `ObjectHost::unapplied_upgrade_effects`
(player, upgrade, slot, effect id, levels, reason) and cleared when the level returns to
zero. Nothing is approximated. A zero change of an effect whose decoded routine adds the
delta (`ratd`, `ratx`, `rmvx`, `rmov`, `rhpo`, `rman`) is a no-op and not listed; the
remaining add routines were not all decoded, so their zero-change behavior rests on
the additive shape.

| Effect | Reason it is not applied |
| --- | --- |
| `ratd`, `ratx` | `0x6a61f0` and `0x6a5e60` convert the level delta to an integer with `0x9a07b0`, then use the selection mask from `0xb20150`. `ratd` adds to dice-number fields and `ratx` to base-damage fields (`0xb1e770`, `0xb1e810`). For representable finite values the conversion truncates toward zero. The setters registered as `BlzSetUnitDiceNumber` and `BlzSetUnitBaseDamage` reach those same fields. For these calls, the mask selects each component slot with kind 1 through 8 and classification outside 1, `0x40`, `0x80` and `0x100`; it does not test enabled-weapon bits. Mapping those component values back to authored weapon fields and enums remains unverified, so neither effect is applied. |
| `rarm` | Native adds `levels * ` a unit type value (`0x6894f0`, probably `defUp`) to the armor bonus (`0x680070`). The armor model has no upgrade term, and the column is not confirmed. |
| `rmvx`, `rmov` | Native adds a movement bonus (`0xb67690`); its interaction with the speed limits and the slow buff is not decoded. `rmov` multiplies by a movement property (`+0x74`). |
| `rhpo` | Native scales a value from a virtual query at `+0x138` that is not decoded. |
| `rman` | Native scales a unit type value read at `0x67b740` (`+0x88`); the column is not confirmed. |
| `rlev`, `rlum`, `ratr`, `rats`, `rspi`, `rhpr`, `rsig`, `ratc`, `radl`, `rmin`, `rrai`, `rent`, `renw`, `raud`, `rasd`, `rasr`, `rroo`, `ruro`, `rart`, `rtma`, `rauv` | The apply routine is not decoded. `rtma` and `rlum` style effects are likely player-level, which is not established. |

The host reads `inherit` and `global` from the selected rows and map edits. Stock `Rhst` has `inherit` 1 and
`global` 0. `UpgradeMetaData` maps them to editor labels in the selected
`War3Local.mpq` `UI\WorldEditStrings.txt`: `inherit` is “Transfer with Unit Ownership”
and `global` is “Applies to All Units”. In the selected executable, the owner-change
path removes old-owner effects from the unit (`0x69d3fc`, `0x6e0d40`). When its transfer
flag is set, `0x694c20` examines each listed upgrade's `inherit` value via `0x6df9a0`.
For a nonzero value and a higher old-owner level, it raises the receiving player's level
one step at a time, subject to the native maximum; those steps also affect the receiving
player's existing units. The unit's owner is then changed and its new owner's levels are
applied from zero (`0x69d4ca`, `0x69d4e9`, `0x6dd280`).

For `global`, `0x6e0b80` reads the flag through `0x6b1db0`. A nonzero value dispatches
each effect through its player-level virtual method; zero enumerates units and calls the
unit-level method. The player-level behavior is effect-specific and remains undecoded.
The host refuses a research change with nonzero `global` until those player-level routines
are decoded. The editor labels alone do not establish additional behavior.

For ownership changes the host stages removal of the old owner's supported effects,
inherited research and its effects on the receiving player's units, and application of
the receiving owner's effects. The stage refuses when admitted owner health factors
differ because the native health adjustment on transfer is not yet modeled.
A modified Human01 probe (`.scratch/upgrades2/owner-reference3/`) transferred a
Sorceress from player 0 with `Rhst` level 0 to player 1 with level 2. Her pools moved
from 137/325 life and 73/200 mana to 171/405 and 146/400. On transfer back, the pools
stayed unchanged and player 0's level had risen to 2. This matches the decoded
inheritance path for this case; repeated transfers and other starting levels remain
unobserved.

## Not modelled

- Polymorph replacement forms do not yet re-apply the new type's upgrade list.
  In the inspected Raven Form path, the native form changer removes the old type's
  effects before the type changes and applies the new type's effects afterward
  (`0xbaed20`, `0x6805e0`, `0x6e0d20`, `0x6c06e0`, `0x6dd220`). Each callback
  dispatches the full level range once, including upgrades shared by both forms.
  Removing and reapplying a shared pool upgrade can change its current value through
  rounding. The caller also changes maximum life and mana from type-property differences after the form
  change. Polymorph also reaches this form changer on one branch (`0xb89eab`,
  `0xb8a8b8`); its alternate branch dispatches an event through `0x6bedd0` whose
  receiver and upgrade behavior are unresolved. The flags and pool changes are not
  fully qualified. For admitted ordinary morphs with equal base vitals, the host
  stages removal and reapplication of supported effects over the full level range.
- The order of upgrades when a unit is created follows the unit type's list, not the
  native record order.
- Cached units (`RestoreUnit`): the saved pools are kept. Whether the saved maxima already
  include upgrade effects is unknown.

## Human02 interlude

`selected_human02_continue_through_interlude_restores_human03` (runner
`.scratch/campaign/run.py`, receipt `upgrade-gap-trace-final`) passes. The map sets `Rhst` and
`Rhpt` to level 2 for players 0 to 2 before the units exist, so the Sorceresses take the
creation path: `hsor` `manaN` 200 becomes 400 (`Rhst` +200 from the `Custom_V0` row). A
temporary traces at the accepted orders showed:

| Game time | Order | Before `rmnr` | After `rmnr` |
| --- | --- | --- | --- |
| 67.97 s | Slow | 233.99 / 400 | 267.9731 / 400 |
| 68.97 s | Polymorph | 234.49 / 400 | 268.9731 / 400 |

The earlier trace also observed the Polymorph cast point and effect without refusal.
The sheep expiry would fall near 129 s; the interlude
publishes Human03 before that, so the restore on the real assets was not reached and rests
on the synthetic tests. `ratd` 1/1 of `Rhst` remains unapplied.

## Evidence

- `cargo test -p pcx-play --lib upgrade_effects`: 23 cases. Pool arithmetic (ratio, rounding
  of the change, zero stop, negative result refusal, living floor, handicap factor), the
  slot reader on nonstock layouts, edits and custom upgrades, refusals, and host cases:
  existing units keep their ratios across levels 1, 3 and 0, a wounded unit, later creation,
  an unlisted upgrade, the unapplied ledger, an atomic refusal and the zero-maximum skip.
- `cargo test -p pcx-play --lib morph`: 17 cases pass and one install-gated case is
  ignored. Synthetic shared-upgrade removal and reapplication takes 67/200 mana through
  34/100 to 68/200; a refused projection preserves the cast and its mana.
- `upgrade_effects_original.rs` (ignored, needs the install): the top-level `Rhst` slots
  and the stock effect ids against the metadata.
- A modified Human01 map tested the pool rule on the selected client in Wine. The
  private container used network `none`, Xvfb `:99` and a PulseAudio null sink;
  `.scratch/upgrades2/reference5/` retains the launch and output receipts. After
  map initialization the probe raised Human01's zero `Rhst` maximum allowed level,
  created Sorceresses and set their pools to 137/325 life and 73/200 mana. One
  `SetPlayerTechResearched(..., 2)` produced 171/405 life and 147/400 mana.
  Separate calls for levels 1 and 2 produced 154/365 and 110/300, then 171/405
  and 147/400. This distinguished the observed result from the recreation's former
  one-step 146/400 mana calculation. Native callers at `0x6e0895` and `0x6e0775`
  invoke the effect routine for adjacent levels. The recreation now projects each
  level separately, with synthetic tests for intermediate rounding; applying that
  calculation to the observed pools gives 147/400.
- In that same probe a request for level zero left the reported level at 2 and
  the pools unchanged. The reason is not established. This observation does not
  validate the recreation's undo path or the behavior of other research removals.
- A separate modified Human01 run (`.scratch/upgrades2/regen-reference/`) set two
  Sorceresses' mana to zero, one with `Rhst` level 2 and one without it. Ten game
  seconds later they had 13.165 and 6.666 mana respectively. The 6.499 difference
  is consistent with the selected top-level row's 0.65 rate increase for ten seconds;
  it does not establish item or hero interactions.
