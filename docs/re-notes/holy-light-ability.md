# Holy Light ability execution

Holy Light is the paladin ability `AHhb`. The campaign Human01 script selects it with
`SelectHeroSkill( udg_Arthas, 'AHhb' )` on unit `Hart`.

## Source

- `Units\AbilityData.slk`, row `AHhb` (Legacy 1.29, read through `DefaultTables`
  and the explicit gate in `pcx-objects/tests/holy_light_original.rs`): `levels` 3,
  `reqLevel` 1, `levelSkip` 0, `Cost1` 65, `Cool1` 5, `Rng1` 800, `DataA1`..`3`
  200/400/600, `targs1` `air,ground,organic,notself,invu,vuln,nonancient`.
- `Units\HumanAbilityFunc.txt` `[AHhb]`: `Order=holybolt`, `Targetart`.
- `Units\HumanAbilityStrings.txt` `[AHhb]`: `Hotkey=T`. The tooltip reads "heal a
  friendly living unit ... or deal half damage to an enemy Undead unit".

## Binding

The implementation is bound to the `AHhb` row. Nothing in the data names the row as
Holy Light, so the binding is the implementation's identity. Everything else is read
from the row:

- The order string is the row's `Order` profile. Admission refuses a missing,
  ambiguous or unregistered order.
- The player hotkey is the row's `Hotkey` profile, one letter.
- Rank, `levels`, `Cost`, `Cool`, `Rng`, `DataA` and `targs` are read per learned rank.

`map_startup_host` admits the implementation with `admit_holy_light()` after
`admit_combat`, when hero skills are admitted.

## Behavior

`IssueTargetOrder` with the admitted order number reaches `order_holy_light`.

- The caster must be a live unit with a learned rank above zero, a valid target under
  the `targs` filter, a finished cooldown and enough mana. Otherwise the order returns
  `false`.
- An accepted order replaces the caster's other orders. The caster walks toward the
  target until the target is within `Rng`, then casts once. The cast rechecks cooldown,
  mana and target eligibility, and cancels if any fails or another order arrives.
- Target rules: `notself` refuses self-targeting, `nonancient` refuses ancient units,
  and `organic` refuses undead and mechanical friendly units. Enemy targets must be
  undead, so enemy organic units are refused. Heal targets are allies: the same player,
  or a player that has an explicit passive alliance toward the caster's owner. The
  tooltip says "friendly living". Neutral passive units heal only through such an
  alliance; no owner-number rule applies.
- Heal: `DataA` life up to max life. Invulnerable undead take no damage but still
  spend the cast.
- Undead damage: `DataA / UNDEAD_DAMAGE_DIVISOR` through `damage_unit`, the shared
  life-loss path. Kills name the caster as killer. Armor is not applied to spell damage.
- Mana spend, cooldown start and clearing of the caster's attack target happen on a
  successful cast.
- Animation: the `spell` sequence plays when the model has one.
- Events, once per cast: `EVENT_UNIT_SPELL_EFFECT` on the caster and
  `EVENT_PLAYER_UNIT_SPELL_EFFECT` for its owner. `GetSpellTargetUnit` returns the target.
- Command: `holy_light_command(unit)` gives the hotkey letter and the order number of a
  learned Holy Light. The `campaign_input` hotkey arms it, and the next left click on
  a unit issues `IssueTargetOrderById`. Right click cancels.

## Undead classification fix

`unit_position_host::field` filtered `UnitBalance` fields to numeric kinds, so the
`UnitBalance` `type` token column never read. `IsUnitType(UNIT_TYPE_UNDEAD)` and the
other class checks in `unit_type_host` returned `false`. The lookup now takes the
requested kinds: text reads accept `Token`, and `collision` accepts `Real`/`Unreal`.
Covered by `unit_type_undead_reads_the_type_token_column_through_the_placement_reader`
and by the explicit gate, which asserts the `type` column is `Token` and that
`ugho` (Ghoul) is `undead` in the install.

## Evidence and gates

- `cargo test -p pcx-play --lib`: 830 pass. Holy Light tests cover heal and cooldown,
  undead damage, undead type reads, refusals (ancient, enemy organic, mana),
  walk-to-cast, order replacement, learning ranks 1-3 through `SelectHeroSkill` with
  each rank's `DataA`, the spell event and its target, invulnerable undead, and the
  command name.
- `cargo test -p pcx-objects --test holy_light_original -- --ignored` with
  `PEACECRAFT3_LEGACY_DIR` set to the Legacy install passes. Without the variable it
  is ignored.
- `cargo clippy -p pcx-play -p pcx-objects --lib --tests` and
  `cargo check -p pcx-play --lib --target wasm32-unknown-unknown` pass with no warnings.

## Not verified or not implemented

- Cast range is measured between unit positions, without collision radii. Native
  cast-range geometry is unqualified.
- Undead damage is `DataA / 2`. `UberTip` states "half damage" for every rank, and
  admission refuses rows without that text. The native ratio is not separately
  qualified.
- Armor and attack type of spell damage are unverified. The implementation applies
  no target armor. No row column names an attack type for this spell.
- Art: the `Targetart` attaches to the target at `origin`, the model root, because the
  row authors no `Targetattach`. That default is an inference. Its lifetime is the
  interval of the model's only `SEQS` record, which must be non-looping (the install
  has one, `Birth` 0-2800 ms, checked by the pcx-mdl gate). Any other layout refuses
  admission, because which sequence the engine plays is not established. Each cast
  attaches its own effect with its own end time. The end is derived from the game clock
  when the effect is read, so it does not depend on combat stepping. Removing the unit
  ends its effects, as the existing removal path does. Script `DestroyEffect` also ends one. The art bytes come from the campaign reader, with `.mdl` mapped to `.mdx`.
- Native and WASM consumers now compile in a private target, with strict app
  Clippy and app tests. This does not establish original-client click/cast parity.

See [script hero skills](script-hero-skills.md) for ability learning and
[hero combat attributes](hero-combat-attributes.md) for the hero combat path.

## Fractional ranked fields (2026-10-22)

Holy Light formerly read cost, cooldown, range and healing through the integer
learning-field helper. Valid fractional source cells were therefore refused.
The cast and readiness paths now use the shared ranked-cell reader: cost keeps
its signed-integer range, while cooldown/range and healing use real-valued reads.
Nonfinite and negative cast values refuse before payment or an effect. That
refusal policy does not qualify native behavior for negative custom values.

The target list also uses the ranked reader. Supported numeric W3A edits no
longer fail merely because the old target-list helper refused every modified
ability. Loaded metadata still selects the field, type and rank; untouched ranks
keep their source cells. The obsolete unranked text helper was replaced, with
all text reads retained through the ranked API.

Synthetic cast tests distinguish a fractional range boundary, fractional healing,
fractional cooldown expiry, typed edits of one rank, and unchanged values at the
next learned rank. Invalid fields, including a cost outside the signed-integer
range, leave mana and target life unchanged. Local receipts are under
`.scratch/holy-fractional/`; original-client fractional-cast parity is not measured.
