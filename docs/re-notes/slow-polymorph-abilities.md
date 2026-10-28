# Slow and Polymorph

Sorceress spells `Aslo` (Slow) and `Aply` (Polymorph), cast by the Human02 interlude
script with `IssueTargetOrder( unit, "slow" | "polymorph", unit )`. Code:
`crates/pcx-play/src/unit_curse_host.rs`. Admission is `admit_unit_curses()`, called by
`map_startup_host` next to `admit_chain_lightning`.

## Source

Read from the selected install (Legacy 1.29.2.9231) and gated by
`cargo test -p pcx-objects --test curse_original -- --ignored` with
`PEACECRAFT3_LEGACY_DIR` set:

- `Units\AbilityData.slk`: one rank each. Slow: `targs` `air,ground,enemy`, `Cast` 0,
  `Dur` 60, `HeroDur` 10, `Cool` 1, `Cost` 50, `Rng` 700, `DataA` 0.6, `DataB` 0.25,
  `BuffID` `Bslo`. Polymorph: `targs` `air,ground,nonhero,enemy,organic,neutral`, `Dur`
  and `HeroDur` 60, `Cool` 3, `Cost` 220, `Rng` 500, `DataA` 5, `DataB`..`DataE`
  `nshe`/`nshf`/`nsha`/`nshw`, `BuffID` `Bply`.
- `Units\AbilityMetaData.slk`: `Slo1` and `Slo2` (unreal, `data` 1 and 2) and `Ply1` (int)
  and `Ply2`..`Ply5` (unitList, `data` 2..5), each with `useSpecific` listing the class.
  The editor strings of the install name them: `WESTRING_AEVAL_SLO1` Movement Speed
  Factor, `SLO2` Attack Speed Factor, `SLO3` Always Autocast, `PLY1` Maximum Creep
  Level, `PLY2`..`PLY5` Morph Units - Ground, Air, Amphibious, Water.
- `Units\HumanAbilityFunc.txt`: `[Aslo]` `Order=slow`, `Orderon=slowon`,
  `Orderoff=slowoff`, `Hotkey` W via the strings file; `[Aply]` `Order=polymorph`,
  `Requires=Rhst`, `Requiresamount=2`; `[Bslo]` `Targetart` `slowtarget.mdl`.
- `Units\UnitData.slk`: `hsor` `movetp` `hover`; `nshe` `movetp` `foot`.
  `Units\UnitWeapons.slk`: `hsor` `castpt` 0.45, `nshe` `weapsOn` 0. `UnitAbilities.slk`:
  `nshe` `abilList` `Awan`.

## Binding

A row is bound when its parent row's `code` is `Aslo` or `Aply` (a custom alias binds
through its parent, as for chain lightning). The class codes and the metadata ids
(`Slo1`, `Slo2`, `Ply1`..`Ply5`) are the implementation's identity. The role of each
column is the editor string above; the column letter is the metadata `data` ordinal, so
a nonstock ordinal moves the column (tested). Admission refuses a metadata row of another
type or class, an unregistered order, and a missing buff row. Rows that share the
metadata but not the class (`ACsw`, `AIos`, `Aasl`, `ACpy`, `AOhx`, `AChx`) are not bound.

Orders resolve through the selected order registry from the row's `Order` profile.
`IssueTargetOrder` and `IssueTargetOrderById` reach `order_curse` before the attack
fallback; `GetUnitCurrentOrder` reports the order while a cast is pending.

## Cast

Per learned rank: `Cost`, `Cool` (through the shared cooldown reader, so script cooldown
natives and `UnitResetCooldown` apply), `Rng`, `targs`, `Dur`, `HeroDur`, `BuffID`,
`Requires`/`Requiresamount` and the effect columns come from the tables and W3A edits.
The order is accepted when the caster has the learned rank, the target passes `targs`,
the owner meets `Requires` (through the current-research model), and the caster has the
mana and no cooldown. Otherwise it returns false and changes nothing.

- `targs`: movement (`air`/`ground`), owner relation (`enemy`, `neutral`, `friend`),
  `self`, `organic` (not mechanical), `nonhero`, `nonancient`, `structure`, and the
  stance tokens `invu`/`vuln` (absent means protected units are refused). Unknown tokens
  refuse the whole row. Polymorph also refuses a neutral-hostile target above `Ply1`
  creep level (`UnitBalance` `level`, `>`; the comparison is a reading of the label).
- The caster walks to `Rng` (center distance, no collision radii), faces the target,
  plays `spell` and applies the effect after `castpt`. Mana, cooldown, the effect and
  the unit/player spell-effect events happen at that moment; `GetSpellAbilityId` and
  `GetSpellTargetUnit` are set. A target that stops being eligible, a changed rank, a
  new order, death, removal or pause cancels without payment (pause shifts the timer).
- A nonzero ranked `Cast` refuses the cast with an error: the casting time of a
  non-channeled spell is not decoded.

## Slow

`Bslo` is attached through the unit buff host (negative flag only; the dispel class is
not established). Move speed becomes `base * (1 - DataA)` through the shared speed
setter, so the structure limits clamp it; attack speed is the existing multiplier
(`1 + agility bonus`) minus `DataB`, and a non-positive result refuses the swing with an
error. Duration is `Dur`, or `HeroDur` for a hero. A recast replaces the effect and
refreshes the duration without compounding. Expiry, `UnitRemoveBuffs` on the negative
buff, death and removal end it; expiry restores the base speed if the unit still has the
definition it was slowed in. The buff's `Targetart` shows on the target at `origin` (the
row authors no `Targetattach`; the default is an inference) until the effect ends.
`SetUnitMoveSpeed` and `GetUnitMoveSpeed` on a slowed unit refuse with the existing
"active ability speed modifiers are not admitted" error.

## Polymorph

The target's definition, model animation, movement class and speed, weapon and owned
abilities are replaced by the ground form (`DataB`, `nshe`), through the same
same-handle form path the Raven Form morph uses (`prepare_form` with
`FormChange::Replacement`, `commit_replacement_form`). Life, mana, armor and defense
type stay with the unit, per the tooltip ("retains its hit points and armor"). The
original definition, abilities and movement profile are restored at expiry or when
`UnitRemoveBuffs` removes `Bply`. `movetp` `foot`, `horse` and `hover` select the
ground slot (every stock `hover` row has `targType` ground; the mapping is an
inference). A recast on a polymorphed unit refreshes the duration.

Refused with an error, because the semantics are not established: the air, amphibious
and water forms (flight height and water placement), hero targets, targets that hold
items, are mid-morph, mirror or construction state, illusions, a slowed target and a
slow on a polymorphed target. `Specialart` and `Effectart` of the ability and buff
(`PolyMorphTarget`, `PolyMorphFallingSheepArt`, `PolyMorphDoneGround`) are not shown:
the attach points and the sequence that plays are not decoded. The unit's decay timing
and sleep state keep the original unit's values.

## Not modelled

- Backswing (`castbsw`): orders cancel it at once; only the `spell` sequence interval
  keeps the caster out of the attack loop.
- Automatic Slow (the `Orderon` autocast, default-on for `hsor`) and the
  player command-card buttons. See [autocast off](ability-autocast-off.md).
- Native range geometry, attack-speed arithmetic, speed-limit order and the buff
  classes. The factors and formulas above are recreation readings of the tooltip.
- Unapplied upgrade effects. `Rhst` adds maximum mana and life, which the host applies
  (see [upgrade effects](upgrade-effects.md)); its mana regeneration (`rmnr`) and attack
  dice (`ratd`) terms are listed as unapplied.

## Human02 interlude

Run: `selected_human02_continue_through_interlude_restores_human03` through
`.scratch/campaign/run.py` (ignored receipts `curse1`..`curse4` in `.scratch/campaign/`).
The refusal at tick 4350 is gone and the interlude publishes Human03 with the earned
carryover. The maximum mana of `Rhst` level 2 (`rmnx` +200, so 400) is now applied by the
upgrade effects, and both casts reach their cast point: Slow is ordered at 67.97 s (mana
233.99 of 400, cost 50) and Polymorph at 68.97 s (mana 234.49 of 400, cost 220), with the
effect applied at 69.44 s and no refusal (receipt `upg2`, temporary trace not kept). The
sheep expiry would fall near 129 s; the interlude publishes Human03 first, so the restore
path ran only in the synthetic tests.

## Evidence

- `cargo test -p pcx-play --lib`: 2100 pass; `unit_curse_tests.rs` has 17 cases on
  nonstock tables: data factors, cast point, payment and cooldown, refusals, refresh,
  hero duration, walking into range, order cancellation, buff removal, speed-native
  refusal, ordinal and custom-alias binding, class binding, nonzero `Cast`, events,
  polymorph form swap and restore, hover and air targets, slow/polymorph combinations.
- `cargo clippy -p pcx-play -p peacecraft3 --all-targets --features test-fixtures
  -- -D warnings` and the WASM clippy for `peacecraft3` pass.
- No original-client comparison was made. Pixel or native parity is not claimed.
