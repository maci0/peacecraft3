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
(`CAbilityImmolation`, code `AEim`), Repair (`CAbilityRepair`, code `Arep`) and Black
Arrow (`CAbilityBlackArrow`, code `ANba`). The
reader decodes all fifteen with the same OFF handler checks and refuses the image
otherwise. Each OFF order name is the `Orderoff` (or, for Immolation, the `Unorder`)
of an ability section in the `*AbilityFunc.txt` members, and each section's `code`
(from `Units\AbilityData.slk`) selects the class: `repairoff` in `[Ahrp]`
(`NeutralAbilityFunc.txt`, code `Arep`), `weboff` in `[Aweb]` and `[ACwb]`,
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
slice. ON orders for the classes whose ON handler decodes are implemented (see
`ability-autocast-on.md`). Mana Shield and Immolation ON, default-enabled automatic
operation and the later `ravenform` transformation remain refused or unmodelled.

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
Their ON orders are refused, so an inactive toggle stays inactive and its OFF
returns false without a state change, the same return the shared handler
measured for an authored-disabled Inner Fire (Arm A above). That return is
measured for the shared handler on Inner Fire, not for `ANms` or `AEim`
individually; the per-class return is the recreation choice, following the
autocast precedent.

ON orders (`manashieldon`, `immolation`) refuse with `autocast enabling of <alias>
not implemented: its ON handler differs from the decoded shape` for a live unit that
has the ability (`ANms` and `AEim` override the update hook that `ability-autocast-on.md`
decodes). Their effects are not modelled: Mana Shield absorption
(damage, mana cost and timing) and Immolation's burn aura and mana drain.
Missing evidence for the ON effects: absorption/drain formulas and timing.

## Repair (bound), Town Bell and Inner Fire/Web ON

Repair is `[Ahrp]` in `NeutralAbilityFunc.txt` (`Order=repair`, `Orderon=repairon`,
`Orderoff=repairoff`). `Units\AbilityData.slk` gives alias `Ahrp` the code `Arep`
(`Repair (Human)`), which selects `CAbilityRepair`. The class binds with the shared
checks (`selected_original_autocast_classes_bind`, passing). The Undead03 probe
passes `repairoff` on unit `nhem`, which carries `Ahrp` at rank one. The Orc Repair
alias `Arep` (`Repair (Orc)`) has no section in the five `*AbilityFunc.txt` members.
An OFF on a unit with that alias refuses with a profile mismatch: the host reads
`Order`/`Unorder` for it, finds no profile, and returns an error (read from
`autocast_host.rs`; not run).

`townbelloff` is the `Unorder` of `[Amic]` (`Militia Conversion`, code `Amic`,
`Order=townbellon`, `Unorder=townbelloff`, `HumanAbilityFunc.txt`). No
`UnitAbilities.slk` `auto` row lists `Amic`. Two candidate classes failed the reviewed
checks when added temporarily and then removed: `CAbilityMilitiaConvert` (the reader
fails with `neutral-player context: Rule(Range)`) and `CAbilityCallToArms` (`autocast
instruction differs at 0`). Town Bell stays refused. Whether `Amic` is an autocast
ability is unverified.

`innerfireon` (`[Ainf]`, code `Ainf`, `CAbilityInnerFire`) and `webon` (`[ACwb]`
and `[Aweb]`, code `Aweb`, `CAbilityWeb`) are decoded and implemented as state
changes by `ability-autocast-on.md`; their automatic cast is not. Inventory of
what that leaves:

- Present: both classes bind with ON and OFF; the `auto` defaults below are in the tables.
- Default-enabled automatic casting in `UnitAbilities.slk` `auto` (column X4):
  `Ainf` on `ntks`, `Aweb` on `ucry` and `nnwl`.
- Missing: any cast host for the `innerfire` or `web` orders (no `crates` reference to
  `innerfire`, `Ainf`, `Aweb` or `Bweb` outside the autocast reader); the effects in the
  `[Binf]` and `[Bweb]` buff sections (no host reads them); mana cost, cooldown and
  target filters for either cast; the native automatic target-selection rule for either
  class, which no `docs/re-notes` file records.
- Consequence: `ntks`, `ucry` and `nnwl` have no automatic casts. A tick armed by an
  ON order refuses when game time reaches it instead of skipping the cast silently.

## Frost Armor base class and Burrow

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
(`unit_curse_host.rs`). The class shares the Raven Form morph base and is bound by
`unit_morph_rules.rs`; see [unit-root-burrow.md](unit-root-burrow.md). No burrow state or its
movement and targeting effects exist in the repo, and the form swap of a form without the
immediate-change bit (`Abur` `DataB=1`) is not decoded, so burrow on `Abur` stays refused.

## Root and unroot (not autocast; decoded in unit-root-burrow.md)

`unroot` is the uproot order of the Night Elf root ability, not an autocast OFF.
`[Aroo]` in `NightElfAbilityFunc.txt` declares `Order=root` and `Unorder=unroot`;
`Aro1` and `Aro2` have no section of their own and their `code` is `Aroo`; the
executable carries a `CAbilityRoot` class name. Before the class was bound, the NightElf01
probe reached `unroot` on unit `etol` (`Aro1` at rank one) and received the generic unsupported
immediate-order refusal.

The state transition, return values, attack mask and the refusals that remain are decoded in
[unit-root-burrow.md](unit-root-burrow.md): `unroot` is accepted once for a rooted unit and the
uprooted state is recorded. The candidate bits above are now read as: `0x80` set at attach,
`0x8000000` rooted (cleared by uproot). Uprooted movement, the point order `root` and the deferred
order path stay refused.

## Town Bell (`Amic`) immediate order: native decode (not implemented)

Reviewed exe 1.29.2.9231. Town Bell is not an autocast class; `townbelloff` is an immediate order.

Observed:

- `Amic` is class `CAbilityMilitiaConvert` (name at `0xd1908c`, constructor `0xbf6fa0`, vtable
  `0xf6aa60`). Its class code getter `0xbf62b0` returns `Amic`.
- Order IDs: `townbellon` `0xd0072` (getter `0xbf62c0`), `townbelloff` `0xd0073` (getter `0xbf62d0`).
- Slots `+0x328` to `+0x334` call one routine, `0xbf6b40(dryRun, toMilitia)`. It scans the issuer's
  owner's units carrying ability `Amil` (`0x416d696c`) through `0x7f4ba0` with callback `0xbf7740`.
- The dry-run off check (`+0x32c`, dry-run mode) returns error `0xb3` when no militia is found. The
  match flag is written only when a unit matches, so with no militia nothing changes.

Inferred: with no militia active, `townbelloff` is refused with `0xb3` and changes no state. The
script's `IssueImmediateOrder` return value is not checked by the map.

Not decoded: the militia conversion callback (`0xbf72f0`, `[Amil+0x338]`/`[Amil+0x33c]`), the range
check `0xbf7640`, and the dispatcher that consumes `+0x328`/`+0x32c`.

Repo: still refused, `IssueImmediateOrder townbelloff is not bound`. Implementing the no-militia branch
needs a per-unit ability scan for `Amil` that the host does not have, so it is not done.

## Town Bell off: no-militia branch implemented (2026-11-11)

`townbelloff` on a unit listing `Amic` scans the owner's units. The callback `0xbf7740` skips a carrier
unless it belongs to the issuer's owner, and for the off request (`[ctx+4] == 0`) matches only a
carrier whose type is the militia form (`Amil` `DataB1`, `[ability+0x74]`; the base form is
`DataA1`, `+0x70`). With no match the dry run returns `0xb3`, the order is refused and nothing changes,
so the native returns false. `unit_host.rs` `town_bell_off_order` implements this scan over live
units whose type is the militia form and which list `Amil`. When one exists the call refuses: the
conversion callback `0xbf72f0` and the current-order tests at `0xbf77e2`..`0xbf78a7` are not decoded.
