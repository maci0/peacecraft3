# Hero attributes in live combat

The source weapon-identity fix exposed a second independent gap: heroes used only
weapon dice/base damage, base armor and raw attack timing. The hero's Primary,
STR/AGI/INT and fractional level-growth fields did not reach combat. The selected
MiscGame profile already supplied the corresponding coefficients, but CombatRules
previously did not read them. Hero life/mana initialization alone did not implement
these effects.

CombatRules now requires `StrAttackBonus`, `AgiDefenseBonus`, `AgiDefenseBase` and
`AgiAttackSpeedBonus` through the existing ordered stock/map profile parser. It
retains the supplied profile identities in order, including later map overrides.
No measured coefficient is a production constant. Unit values come through the
resolved metadata-backed UnitBalance columns, so supported map object overrides
retain their existing precedence.

Live hero attributes are derived as:

```
attribute = floor(base + growth * (live hero level - admitted initial hero level))
primary damage = attribute named by Primary * StrAttackBonus
hero armor contribution = AgiDefenseBase + AGI * AgiDefenseBonus
attack speed factor = 1 + AGI * AgiAttackSpeedBonus
```

The baseline comes from the existing source-qualified hero counter admission;
level one is not copied into this calculation. Current level comes from the same
HeroProgress used by native SetHeroLevel/GetHeroLevel, including its existing cap.
Fractional growth remains available until deriving the integer attribute at that
level. Repeated level assignments do not accumulate bonuses. Hero identification
uses the same rawcode convention as existing ability/vitals admission.

Primary damage is added before attack-type and armor multipliers. Target armor
adds the current agility contribution to its retained base armor. At the start of
a new swing, attack cooldown and damage-point delay are divided by the current
speed factor. Ordinary units keep their existing base damage, armor and timing.
Missing hero admission, missing/invalid fields, unsupported Primary tokens,
levels below the admitted baseline, non-finite results and non-positive speed
factors refuse rather than receiving invented values.

Synthetic tests mutate all primary types, fractional growth, the admitted
baseline, live SetHeroLevel calls, source coefficients and map profile overrides.
They check actual hit damage, target armor and deterministic damage-point/cooldown
boundaries as well as ordinary-unit behavior and source refusals. The focused
native combat suite passes all 13 tests, and the enhanced source-profile test
passes (2026-10-20). Strict native/browser checks and runtime validation remain
with the coordinated verification run.

These formulas are recreation choices informed by the authored relationships;
integer rounding, original attack-speed caps and exact native timing are not newly
qualified by these tests. A level change does not retime an already running swing
or remaining cooldown. The existing attack animation clock is not rescaled, and
missile damage/target armor still use the existing launch-time roll rather than a
new impact-time reevaluation. The Get/SetHeroStr/Agi/Int queries are in
[Hero attribute queries](hero-attribute-queries.md). General item/ability stat
bonuses, XP awards and level-up effects remain separate work.

## Level/vitals integration

Live level changes now stage life/mana contribution differences through the same
attribute helper, preserving explicit maxima and absolute missing resources.
See [Live hero vital updates](hero-vital-level-updates.md) for selected native
current-resource evidence, tests and the explicitly unsupported living/dead
minimum boundary. General item/ability attribute effects remain separate work.
