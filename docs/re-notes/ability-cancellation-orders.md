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

These observations qualify only the fresh-unit requests above. They do not
justify returning false for every unimplemented cancellation, or returning true
without a state change. Shield activation, active cancellation, duration,
mana/cooldown changes, militia conversion and rendered effects remain unmeasured.
Ability values and order associations must still come from the selected ranked
ability rows, profiles and map overrides; the experiment does not replace those
inputs with constants.
