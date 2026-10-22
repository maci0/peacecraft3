# Fresh-unit cancellation orders

## Source and bounded runtime observation

The selected Human02 script creates Uther (`Huth`), requests hero level and mana,
learns his authored skills, then issues `undivineshield`. It also issues
`militiaoff` to newly created peasants (`hpea`). These calls currently reach an
unsupported immediate-order path in the recreation.

A local diagnostic copied Human02's archive members and creation statements,
replaced only its script, and omitted placed-unit creation and authored
initialization triggers. Its timer callback creates the source units, performs
the authored pre-order calls, reads their types, and records each off-order
result twice. Each checkpoint has a separate preload output file. This is an
isolated stimulus, not an unchanged-mission playthrough.

Reviewed executable: Legacy 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Diagnostic map SHA-256:
`8c2cd972bce2d2b4aa09b6037411c231f5e889a5953b0d815dfbdc0da7f585a0`.
The staged script and map matched their build receipts before launch. The run
used the private Wine reference at 1280×960 and 96 DPI, with software rendering.

All ten expected checkpoints were recorded. Both type readbacks matched their
source rawcodes. At printed game time 5.000:

| Fresh source unit | Order | First return | Repeated return |
| --- | --- | --- | --- |
| Uther | `undivineshield` | false | false |
| Peasant | `militiaoff` | false | false |

The chapter card was observed before one Space input. Main-entry and main-return
checkpoints were already present; the callback and completion checkpoints were
observed afterward. This does not establish a general timer or cinematic-pause
rule.

Receipts, generated map/script and output files remain ignored locally in
`.scratch/inactive-order-probe`; `result.json` links the source/build/staging
hashes and all record hashes. The container was stopped after capture.

## Implementation boundary

That first run qualifies only the fresh-unit requests above. It does not
justify returning false for every unimplemented cancellation, or returning true
without a state change. It does not measure activation, active cancellation,
duration, mana/cooldown changes, militia conversion or rendered effects.
Ability values and order associations must still come from the selected ranked
ability rows, profiles and map overrides; the experiment does not replace those
inputs with constants.

## Shield activation and sampled protection

Two further isolated Human02 diagnostics used the same reviewed executable.
The five prepared MPQ hashes also match the selected Legacy installation;
`archive-identity.json` retains that comparison locally. The first diagnostic
map has SHA-256
`c02e5129c8cc6bd4afefa4520db7edfbf95af58d7d7032e5236fbc9831ce5724`,
and the damage follow-up has SHA-256
`4998d44e5e83d5d9ffb13e8dbb4efb825c02deced3cc742474fda61e7ea65bd8`.
Both staged map/script pairs matched their build receipts, reached their final
checkpoint, and had their private containers stopped afterward. Generated
content and receipts remain in ignored `shield-probe` and `shield-followup`
directories.

In the first run, three source-created, rank-three Uthers accepted
`divineshield` at printed game time 5.000. Mana changed from 495 to 470 and the
cooldown query immediately reported 65 seconds. The active cancellation calls
at 5.250 and 5.500 returned false, as did the repeated cancellation at 5.750.
These observations do not establish successful active cancellation.

The reviewed `BlzIsUnitInvulnerable` getter is inverted relative to explicit
protection: a `SetUnitInvulnerable(false)` control reads true, while a
`SetUnitInvulnerable(true)` control reads false. Its primary registration at
`0x4a17ce` names handler `0x499fa0`; the handler resolves the unit, shifts
`[unit+0x20]` by three bits, inverts it, and masks bit zero. Keep the internal
protection flag distinct from this getter's observable result.

In the follow-up, the unprotected controls changed mana, cooldown and order
state during the initial damage stimulus, before their planned cast requests.
Automatic shield activation is an explanation consistent with those records;
the precise native reaction path has not been traced. Those initial damage
calls therefore do not supply a neutral damage control.

At game time 41.750, the naturally expiring unit still read false through that
getter and lost no life to the sampled damage call. At 51.750 and 61.750 it
read true, and the same damage stimulus reduced life from 1250 to 1244.891.
The independently protected unit continued to read false and lost no life.
This supports the protection distinction and bounds the observed effect end
between the two checkpoints; it does not identify an exact retirement tick.

## Game-data variant remains open

The selected archives contain multiple authored balance tables. For `AHds`,
unprefixed `Units/AbilityData.slk` has ranked `Dur`/`HeroDur` values 15, 25 and
35. `Custom_V0/Units/AbilityData.slk` has 15, 30 and 45. The latter rank-three
value is consistent with the sampled original lifetime; matching a value does
not prove the native table-selection rule. Both tables retain the same
rank-three mana cost and cooldown, so those observations cannot discriminate
them.

Do not compensate by changing rank arithmetic, copying a duration constant,
or silently selecting a table from the observed duration. The map's game-data
selection and native namespace policy still need to be derived before claiming
production duration fidelity. The existing ranked-cell reader can consume the
selected table once that policy is qualified.

A controlled follow-up changed only the version-18 Human02 information word
currently exposed as `MapInfo.loading_id` from zero to one, plus the diagnostic
run identifier. The staged map SHA-256 was
`4651017750ea452fec9825656c7ac29b70df3c0723c8ae219eda67a5a58a1496`.
All 39 records arrived, including the final checkpoint: rank-three activation
at 5.000 again spent 25 mana and started a 65-second cooldown, and the natural
shield still read false at 41.750. This change did not discriminate the
35-second and 45-second table variants or establish a namespace mapping.
The private container was stopped and the receipt retained in ignored
`dataset-probe/result.json`. The meaning and consumer of that information word
remain a separate source-policy question.

## Recreation shield boundary

The self-cast host reads the learned rank's duration, cost, cooldown and Can
Deactivate field from the admitted ability tables and overrides. Stock
cancellation returns false; the edited Can Deactivate branch is a recreation
choice pending a native edited-ability comparison. Shield protection contributes
separately from explicit `SetUnitInvulnerable`, including combat impact checks.
Cooldown query, end and reset use the same cast deadline, without ending the
shield. Protection is computed from the game clock, avoiding a per-tick scan;
one retained entry per caster is replaced on recast and removed on unit removal.

Ten synthetic tests cover ranked values, malformed admission/arguments, mana,
active cancellation, independent explicit protection, expiry, cooldown controls,
session isolation and protection acquired during attack windup. These tests do
not qualify rendered art, edited cancellation, automatic casting or the unresolved
production balance-table selection.

The Can Deactivate name/type mapping is shipped-byte evidence:
`Units/AbilityMetaData.slk` row `Hds1` specifies `field=Data`, `data=1`,
`slk=AbilityData`, `index=-1`, `repeat=4`, `type=bool`, `useSpecific=AHds`.
Its `WESTRING_AEVAL_HDS1` display key resolves to `Can Deactivate` in the
supplied WorldEdit strings. The unqualified part is the edited runtime
cancellation behavior, rather than the field's identity.
