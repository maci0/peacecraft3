# Live hero vital updates

Previously, hero life and mana bonuses were evaluated only at creation.
`SetHeroLevel` changed counters/skill points while leaving the pools unchanged.
The successful level setter now stages vital changes before mutating any hero
counters. Combat and vitals share the existing integer attribute growth helper;
resolved metadata-backed STR/STRplus and INT/INTplus retain map overrides and
use the admitted initial level as their baseline. Existing fractional growth is
retained until the current attribute is floored. This rounding remains the
previous recreation choice, not newly established native rounding parity.

Coefficients remain the selected stock/map MiscGame values read by HeroVitals.
Each update computes the difference between the old and new attribute
contributions, then adds that difference to the current maximum and current
resource. This preserves an independently set maximum and the absolute amount
missing from the pool. It does not preserve a percentage. Repeated assignments
and lower requested levels keep the existing setter's non-decreasing semantics
and cannot accumulate bonuses. Units without admitted vitals keep their existing
unsupported-vitals state. The GetHeroStr/Agi/Int queries are in [Hero attribute queries](hero-attribute-queries.md).

## Selected native evidence

The reviewed legacy client has SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
VERSIONINFO 1.29.2.9231. The registered SetHeroLevel wrapper at 0x4a88b0
routes through 0x690910 to the hero XP/level updater at 0xb3e0c0. After
updating levels, it invokes 0xb3fb00. That routine computes a difference
between new and retained old contribution values, then calls:

- Life: 0xb3fc2e subtracts, 0xb3fc3e calls 0x6be8c0 with the source mode.
- Mana: 0xb3fcae subtracts, 0xb3fcbe calls 0x680350 with the same mode.

The life contribution routine 0xb3e7a0 resolves Misc/StrHitPointBonus;
the mana contribution routine 0xb3e930 resolves Misc/IntManaBonus. Their
profile-key references independently establish the correspondence to the existing
source profile reader.

The positive selected mode branch at 0x6be984 adds to the maximum through
vtable slot 0x144 and subsequently adjusts current life through slot 0x128.
MSVC RTTI identifies the CUnit vtable at 0xe9c504: the current-life getter
is slot 0x12c (also reached by GetWidgetLife), and the additive current-life
routine is 0x668db0. The analogous mana branch at 0x68042a adds to its
maximum and calls 0x6802d0, which adds the delta to current mana. These
paths establish an absolute adjustment rather than percentage preservation.
This is bounded native-code evidence, not an original runtime observation.

## Boundaries and verification

Negative deltas use the same adjustment while the resulting living life remains
positive; mana is clamped to its valid arithmetic zero/maximum range. The native
negative-life path has additional living/dead floor logic. Its runtime constant
initialization has not been qualified here. A change crossing that boundary, or
a level change from a nonpositive life pool, therefore refuses before changing
vitals or level/skill counters. Nonfinite/invalid derived pools also refuse.
The slice adds no resurrection or death-floor guess.

Synthetic VM tests vary live levels with fractional growth and a nondefault
admitted baseline, preserve explicit maximum overrides and missing resources,
check repeated/lower requests and unchanged ordinary units, and verify atomic
refusal at the unqualified dead boundary. Pure tests cover positive/negative
deltas, mana clamping and invalid arithmetic. Parser/VM tests do not prove HUD
projection or original runtime parity; those remain integrated gates.
