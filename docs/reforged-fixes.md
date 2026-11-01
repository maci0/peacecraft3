# Reforged fixes ledger

Process ledger, not game behavior. No shipped-byte, native-code, or runtime
evidence is claimed by this file's existence. It records the adoption policy
from `PLAN.md` §3.9 so a future fix has a place to land with its evidence.

Rule (from `PLAN.md` §3.9):

- `vanilla` is the only mode the §6 fidelity gate is measured against.
- `reforged-fixes` mode applies only rows marked `adopt`, and the game
  prints or logs which rows were active for a run. A run's trace header
  carries that list.
- Rows default to `defer`. Adoption requires the fix to be (i) observable
  in a scripted scenario, (ii) not break any 1.28c golden trace in
  `vanilla` mode, and (iii) justified in writing.

Local sources (from `PLAN.md` §3.9, identities only):

- Target: 1.29b (`w3_legacy`, `1.29.2.9231`).
- Reforged: 3.0.0.24268 (`w3`, `RETAIL - RC 3.5 LAUNCH BUILD`), CASC at
  `~/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III`.
  No Reforged bytes enter the repo.

## Ledger

| id | subsystem | symptom | 1.28c behaviour | Reforged behaviour | evidence | disposition |
|---|---|---|---|---|---|---|
| — | — | No fix admitted yet. Categories named in `PLAN.md` §3.9 (stuck units, desyncs, pathing, UI, Classic-HD fallback) remain unmeasured placeholders, not rows. | — | — | none — unmeasured | defer |

Adding a row requires: subsystem, symptom, both behaviours, an `ev_…`
id and/or patch note, and a disposition. Do not change `defer` to
`adopt`/`reject` without the three adoption checks above and a scripted
scenario reference. `pcx-diff` must report which mode produced a trace so
a `vanilla` run is never confused with a `reforged` one.
