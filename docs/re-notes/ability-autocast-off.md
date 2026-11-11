# Ordinary autocast OFF orders

The selected Legacy client capability is read from the retained executable, with
its input identity and SHA checked at admission. Named Slow, Inner Fire and Heal
class getters lead to their class-code and on/off-order virtuals. The bound
classes are Slow, Inner Fire, Heal, Raise Dead (`CAbilityRaiseDead`, code `Arai`),
Auto-Dispel Magic (`CAbilityAutoDispelMagic`, code `Aadm`), Web (`CAbilityWeb`,
code `Aweb`), Faerie Fire (`CAbilityFaerieFire`, code `Afae`), Renew
(`CAbilityRenew`, code `Aren`), Bloodlust (`CAbilityBloodlust`, code `Ablo`), Frost
Armor (`CAbilityFrostArmorAuto`, code `AUfu`), Restoration (`CAbilityRestoration`,
code `Arst`), Mana Shield (`CAbilityManaShield`, code `ANms`) and Immolation
(`CAbilityImmolation`, code `AEim`). The reader decodes all thirteen with the same
OFF handler checks and refuses the image otherwise. Each OFF order name is the
`Orderoff` (or, for Immolation, the `Unorder`) of an ability section in the
`*AbilityFunc.txt` members, and each section's `code` (from
`Units\AbilityData.slk`) selects the class: `weboff` in `[Aweb]` and `[ACwb]`,
`faeriefireoff` in `[Afae]`, `[Afa2]` and `[ACff]`, `renewoff` in `[Aren]`,
`bloodlustoff` in `[Ablo]` and `[ACbb]`/`[ACbl]`, `frostarmoroff` in `[AUfu]`,
`[ACfu]` and `[ACf2]`, `restorationoff` in `[Arst]`, `manashieldoff` in `[ANms]`,
`unimmolation` in `[AEim]` and `[ACim]` (`ACim` has code `AEim`). The OFF handler
joins a tested state-bit operand to its complementary clear operand and the
already-clear return branch. The reader derives these operands and IDs; it does
not embed native addresses or order IDs. Synthetic relocated/nonstock fixtures
and malformed/ambiguous/budget refusal tests cover this reader.

Unit ability aliases and map overlays remain owned by the existing ability
resolver. `UnitAbilities.auto` lists ability codes, not aliases: an alias starts
enabled when its `AbilityData` `code` appears there (`ACrd`, code `Arai`, is enabled
by `auto` `Arai`). Checked against `Units\UnitAbilities.slk` and
`Units\AbilityData.slk` in `War3x.mpq`, all 74 `auto` entries resolve this way:
19 name an alias equal to its code and 55 name the code of another alias. The
resolved ability profiles' `Orderon`/`Orderoff` must agree with the admitted class
and registry. A canonical unit stores only changes from that source default.
Missing or malformed authored data is an error, not an invented default.

## Original runtime observations

Two command-only probes used the known-loadable Human02 container and source
Sorceress/Priest/Footman definitions. The isolated original clients were stopped
before collection. Local artifacts and raw records remain ignored under
`.scratch/autocast-off-probe/`; no original data is included here.

Arm A retained 81 records and 18 before/after command pairs. A state-changing idle
OFF returned true and emitted an issued-order callback, with the OFF order visible
inside the callback and zero current order after return. Repeated OFF, authored
initially-disabled Inner Fire and the missing-ability control returned false,
without order/spell events or movement interruption.

Arm B retained 77 records and 16 pairs. The first enabled-to-disabled OFF while
actually moving returned true, emitted the issued-order callback, stopped motion
and left current order zero. Subsequent samples remained stationary. Repeated
OFF did not interrupt a later movement order. No tested OFF emitted spell events.
This is bounded ordinary-unit evidence, not a claim about all autocast abilities
or special native ownership modes.

## Canonical implementation and limits

A changed OFF preflights the frame, activity interruption, event delivery and
state capacity before mutation. It uses the canonical stop/order cleanup and
shared player/unit event contexts. The ephemeral OFF current order lasts through
its callbacks; a callback's explicit replacement order survives settlement.
Already-disabled and missing-ability returns do not reserve event frames or add
state overrides. Removed units return false; unknown and wrong-family handles
retain errors. Whole-unit retirement drops the state.

The existing cache format and Mirror Image snapshot do not encode altered
autocast state, so their preflights explicitly refuse it rather than drop it.
There is no new cache version. Per-ability removal is not implemented by this
slice. ON orders explicitly refuse before mutation: automatic effects of the thirteen
bound classes, their default-enabled automatic operation, and the later
`ravenform` transformation remain required work.

## Mana Shield and Immolation toggles

`manashieldoff` is the OFF order of `[ANms]` (`Orderon=manashieldon`,
`Orderoff=manashieldoff`). `unimmolation` is the OFF order of `[AEim]` and `[ACim]`
(`Order=immolation`, `Unorder=unimmolation`; no `Orderon`/`Orderoff` keys). Both
classes read from the selected client with the shared OFF handler checks. The
reader checks the class names, both on/off immediates and the handler shape
(`autocast_rules_tests::selected_original_autocast_classes_bind`, run through
`.scratch/run-asset-gate.sh`, passes with all thirteen classes).

The host picks the toggle key pair per ability: `Orderon`/`Orderoff` when
`Orderoff` is authored, else `Order`/`Unorder`. Both ability sections match their
`code` against the class, so the same ID check covers `[AEim]` and `[ACim]`.

Default state: `UnitAbilities.slk` `auto` (column `X4`) lists no `ANms` or `AEim`
in any of its 837 rows, so both toggles start inactive. The host's
`unit_autocast_enabled` derives state from `auto` and the explicit disabled list.
An ON order never takes effect, so an inactive toggle stays inactive and its OFF
returns false without a state change, the same return the shared handler
measured for an authored-disabled Inner Fire (Arm A above). That return is
measured for the shared handler on Inner Fire, not for `ANms` or `AEim`
individually; the per-class return is the recreation choice, following the
autocast precedent.

ON orders (`manashieldon`, `immolation`) refuse with `autocast enabling/effects
not implemented for class <code>` for any live unit of the class, whether or not
the unit has the ability. Their effects are not modelled: Mana Shield absorption
(damage, mana cost and timing) and Immolation's burn aura and mana drain.
Missing evidence for the ON effects: absorption/drain formulas and timing.

## Frost Armor base class and Burrow (not bound)

`CAbilityFrostArmor` (code `AUfa`, alias `[AUfa]`, which has no `Orderoff`) is not
the class for `frostarmoroff`. Its OFF virtual is `mov eax, [global]`, not an
immediate, and the reader does not read initialized data for a class constant.
The `AUfu` alias binds through `CAbilityFrostArmorAuto`, whose constants are
immediates.

`burrow` is the order of `[Abur]`, `[Abu2]`, `[Abu3]` and `[Abu5]`
(`Order=burrow`, `Unorder=unburrow`). None of these sections has an `Orderoff`,
so the autocast OFF profile check cannot apply. `CAbilityBurrow` is a morph
ability. The repo's form-change path is `prepare_form` in `unit_host.rs`, reached
by Raven Form (`unit_morph_rules.rs`, `script-unit-morph.md`) and by Polymorph
(`unit_curse_host.rs`). No burrow state or its movement and targeting effects
exist in the repo. Burrow stays refused until that state and its effects are
established.

## Root and unroot (not autocast)

`unroot` is the uproot order of the Night Elf root ability, not an autocast OFF.
`[Aroo]` in `NightElfAbilityFunc.txt` and `[Aro1]`/`[Aro2]` in
`NeutralAbilityFunc.txt` declare `Order=root` and `Unorder=unroot`; their `code`
is `Aroo`, and the executable carries a `CAbilityRoot` class name. The
locked-chapter probe for NightElf01 reaches `unroot` on unit `etol`, which
carries `Aro1` at rank one, and receives the generic unsupported
immediate-order refusal.

The root effect is not established. `ai-construction.md` records state bits
`0x80` and `0x8000000` as candidate rooted/uprooted building states with an
unproven mapping to live ability state, and the recreation does not implement
that state. Missing evidence: the state transition, its timing, movement and
attack effects, the cost or cooldown, and the return for an already-uprooted
unit. A native trace or runtime probe must establish these before `unroot` is
bound. Until then the order stays refused. No full autocast or interlude
completion is claimed by these parser and synthetic host tests.
