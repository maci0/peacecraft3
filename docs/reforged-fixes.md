# Reforged fixes ledger

The [asset-mode plan](../PLAN.md#39-reforged-assets-and-reforged-fixes) defines
how to document and verify an opt-in gameplay change.

- `vanilla` is the only mode the §6 fidelity gate is measured against.
- The `fixes` behaviour mode may apply only rows marked `adopt`. A trace
  must identify its mode and the active ledger rows.
- Rows default to `defer`. Adoption requires a scripted observation of the
  change, unchanged Legacy reference traces in `vanilla` mode, and a written
  reason for the decision.

Local sources (from `PLAN.md` §3.9, identities only):

- Target: 1.29b (`w3_legacy`, `1.29.2.9231`).
- Reforged: 3.0.0.24268 (`w3`, `RETAIL - RC 3.5 LAUNCH BUILD`), CASC at
  `~/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III`.
  No Reforged bytes enter the repo.

## Ledger

No fixes are admitted.

Adding a row requires: subsystem, symptom, both behaviours, an `ev_…`
id and/or patch note, and a disposition. Do not change `defer` to
`adopt`/`reject` without the three adoption checks above and a scripted
scenario reference. `pcx-diff` must report which mode produced a trace so
a `vanilla` run is never confused with a `reforged` one.
