# Unit health and mana regeneration

## Clock semantics

Each unit has its own regeneration clock, `Regen.through`. It is set to the game time
of creation and moves forward at every projection step. A unit never regenerates for
time before it existed. `advance_unit_regen(now)` runs inside `advance_unit_activity`,
after motion and before combat and inventory. Each step integrates
`[through, now)` per unit, then sets `through = now`.

- Dead units and units at life ≤ 0 keep their pools, and their clock still moves,
  so a later life write cannot regenerate across the dead span. Revive is not admitted: `ReviveHero` and `ReviveHeroLoc` are declared by `common.j` only.
- A step that errors keeps every pool and clock unchanged. The next call integrates
  the same spans.
- A span that starts before the last time-of-day rebase is refused, because the stored
  hour cannot be classified there. Scripts run after the projection to the same
  timer time, so this does not happen in the production order.
- Integration is continuous: a rate × elapsed span. Splitting a span into chunks gives
  the same pools up to f32 rounding of the stored pool.
- Not established: whether the native game integrates continuously or on a discrete
  regeneration tick, and its rounding. Nothing here claims native tick parity.

## Sources

| Input | Source | Use |
|---|---|---|
| `regenHP` (`uhpr`) | `Units/UnitBalance.slk`, column `regenHP` | base life per second; SLK `-` reads as 0 |
| `regenMana` (`umpr`) | `Units/UnitBalance.slk`, column `regenMana` | base mana per second; SLK `-` reads as 0 |
| `regenType` | `Units/UnitBalance.slk`, column `regenType` (Token) | life eligibility: `always`, `day`, `night`, `none`, `blight` |
| `StrRegenBonus`, `IntRegenBonus` | `Units/MiscGame.txt` `[Misc]` | life/mana regen per derived STR/INT point, via `HeroVitals` |
| `bj_TOD_DAWN`, `bj_TOD_DUSK` | `Scripts/Blizzard.j` constants | day window `[6, 18)` hours, read from the admitted program globals |
| `DayLength`, `DayHours` | `Units/MiscGame.txt` | game seconds per hour (existing time-of-day clock) |

### Explicit install gate

`crates/pcx-play/tests/unit_regen_original_gate.rs` (ignored; needs
`PEACECRAFT3_LEGACY_DIR`) reads the selected install directly through `MpqSet`:

- `Warcraft III.exe` SHA256 must be `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
- `UnitBalance.slk`: the three columns exist; every row's `regenType` is admitted.
  Census on the selected install: always 502, blight 41, night 51, none 242 (837 rows).
  No row uses `day`. Parser admits `day` in the editor's token family.
- `MiscGame.txt` through the production `HeroVitals::read`: `StrRegenBonus=0.05`,
  `IntRegenBonus=0.05`.
- `Blizzard.j`: `bj_TOD_DAWN=6`, `bj_TOD_DUSK=18`.

Run: `PEACECRAFT3_LEGACY_DIR="…/Warcraft III (Legacy)" cargo test -p pcx-play --test unit_regen_original_gate -- --ignored`.

The gate covers source fields and constants. It does not run a map.

## Rules implemented

- Life rate = `regenHP` + (hero only) derived STR × `StrRegenBonus`.
- Mana rate = `regenMana` + (hero only) derived INT × `IntRegenBonus`.
- Derived STR and INT use `derive_hero_attribute`, with the current level and held
  item bonus, so level changes and items change the rate.
- `always`: life regenerates for the whole span. `day`: only inside the day window.
  `night`: the complement. `none`: never. `blight`: refuses while the rate is nonzero.
- Mana regenerates regardless of `regenType`.
- Life is capped at `max_life`. Mana is clamped to `[0, max_mana]`.
- The day window requires the admitted time-of-day clock and dawn/dusk constants.
  Without them, a nonzero day or night rate refuses.

Day seconds are an exact integral: the hour is linear between rebases, each window
edge crossing is solved for, and each piece is classified at its midpoint.

## Production ordering

`NewGameInput` sets `activity` and `hero_vitals` together. Map startup
(`map_startup_host.rs`) admits unit activity before `admit_unit_vitals`, and
time-of-day before vitals. `admit_unit_vitals` requires admitted activity and fails
before any mutation. The fresh-game fixture sets `hero_vitals: None`, so it is unaffected.
Synthetic fixtures admit activity before vitals (`combat_tests`, `unit_guard_tests`,
`inventory_powerup_host_tests`).

## Blight trace (not implemented)

- `common.j` declares `SetBlight`, `SetBlightRect`, `SetBlightPoint`,
  `SetBlightLoc` (lines 1697-1700). It declares no blight query such as
  `IsPointBlighted`, so scripts cannot observe blight state.
- No host code admits any `SetBlight*` native. `pcx-tera/src/tiles.rs` reads
  `Terrain.slk` identity columns and documents `blightPri` and `convertTo` as
  uninterpreted.
- So no per-cell or per-player blight state exists. A `blight` unit with a nonzero
  rate refuses. That refusal is interim: the task stays open.

Needed for real eligibility: the initial blight source (terrain tile type and
`blightPri` semantics, or map data), the `SetBlight*` natives with per-player planes
and radius/rect/point/loc inputs, and the eligibility sample point and chunking rule.
Units move, so a moving unit's eligibility changes inside a span. Sampling at step
boundaries is not chunk-invariant for moving units.

## Evidence grade

- Shipped-byte evidence: the `UnitBalance`, `MiscGame` and `Blizzard.j` values above,
  verified through the explicit install gate.
- Inference, not native-verified:
  - Per-second units (the editor labels `regenHP` as a per-second rate).
  - Mana is not gated by `regenType`, because the column sits outside the HP group.
  - STR/INT terms share the gating of the base rate.
  - The native day window matches the `Blizzard.j` `[dawn, dusk)` test.
- Not established: native tick period and rounding, regeneration suppression after
  damage (none is implemented), and native ordering of regeneration against timer
  and event delivery. The current order is motion, regeneration, combat, inventory.

## Tests

- `regeneration_integrates_stock_rates_and_caps_at_maxima_for_any_tick_chunking`:
  one 10 s span and 1/64 s steps give identical pools; caps hold.
- `regeneration_never_applies_to_units_without_rate_or_with_none`.
- `night_and_blight_regeneration_refuse_without_their_source_state`: atomic refusal.
- `dead_units_do_not_regenerate_and_their_clock_still_advances`.
- `daytime_integrates_exact_window_crossings_and_ignores_chunking` and
  `suspended_or_stopped_clock_holds_its_hour`: window integral, chunking, negative
  scale, suspension.
- The refusal for a span before the time-of-day rebase has no direct test yet.

Pure and synthetic tests do not prove native parity. The full native and WASM gates
belong to the integrator.

## Clock ordering around time-of-day changes

Each `SetFloatGameState`, `SuspendTimeOfDay` and `SetTimeOfDayScale` first integrates
unit regeneration to the change time, then mutates the clock. Every regeneration span
therefore lies inside one clock segment. Multiple changes in one projection interval
compose: the composition test splits a span across two rebases and checks the sum.
Clock arms are validated before the advance, so a rejected call has no effect. An
advance that refuses (for example a blight unit) refuses the clock change too.
End-to-end tests (`combat_tests.rs`, fixture `open_clocked`: startup clock at hour 9,
admitted time-of-day, real unit pools):

- `clock_rebases_integrate_each_span_once_for_day_and_night_regeneration`: the clock
  moves to t=6 without projecting units, then scale changes. Day gives 13 and night 17
  from life 5 over 20 s. Without the advance, the span is refused with
  "span predates the time-of-day rebase". The mutation check confirmed the test
  fails on that refusal when the advance is removed.
- `rejected_clock_change_leaves_pools_and_clock_unchanged`: NaN scale and hour are
  refused before any integration; pools, scale and the later result are unchanged.
- `mid_run_units_and_manually_lifed_dead_units_gain_no_retroactive_regeneration`: a unit
  created at t=5 regenerates from 5, and a unit killed at t=10 and set to 5 life at
  t=13 gains nothing until its clock reaches t=20.
- `moving_unit_regenerates_at_its_rate_while_it_travels`: the unit moves 56 units and
  gains its rate unchanged.

Ordering is not evidenced natively. The moving-unit test checks the rate only, since
no blight or motion-dependent eligibility exists yet.

## Dead, manually lifed and created units

- Creation sets `through` to the creation time. No regeneration before creation.
- Dead or lifeless units advance `through` with pools unchanged. A later life write integrates
  from the last advance, which excludes the dead span.
- Native comparison of these policies is not established. This is a recreation policy.

## Terrain blight trace

- Terrain.slk, selected install: 178 rows, 14 columns, `blightPri` and `convertTo`
  present. Eight rows have the blight class letter `B`: `Bdrt`, `Bdrh`, `Bdrr`,
  `Bdrg`, `Bdsr`, `Bdsd`, `Bflr`, `Bgrr`.
- Each row's `convertTo` lists 15 counterparts in the class order
  `F W B A C N Y V Q X D G Z I O`. Index 2 is the `B` class for every ground row
  checked (for example `Ldrt` → `Bdsr`, `Lgrs` → `Bgrr`). Read the entry by class
  letter, not by position.
- The W3E corner nibble selects a map ground slot (`pcx-map` `slot_nibble`). The
  probe report maps Human01 slots to `Ldrt`, `Ldro`, `Ldrg`, `Lrok`, `Lgrs`, `Lgrd`.
  No `B`-class tile appears in those slots.
- `pcx-tera` reads only identity columns and documents `blightPri` as uninterpreted.
- No `SetBlight*` native is admitted. `IsPointBlighted` is not declared in
  `common.j`, so scripts cannot query blight.

Not established: whether native regeneration reads the ground class, a per-player
blight plane, or both; the player affinity of `SetBlight*`; and the native conversion
rule. Those need native evidence or an explicit reviewed source.

Remaining work before blight eligibility can be enabled:
1. A ground-class query at a unit point, from the selected map's W3E slot list and
   the `pcx-tera` slot-to-tile resolution. The map startup must pass it to `ObjectHost`.
2. `SetBlight`, `SetBlightRect`, `SetBlightPoint`, `SetBlightLoc` with per-player
   state, admitted only after the affinity rule is evidenced.
3. Eligibility along each movement span: solve tile-boundary crossings on the
   existing motion path, not sample at step boundaries. Motion and regeneration
   order must match the event delivery order.

Until then, a nonzero blight rate refuses. This refusal is interim and the task stays open.

## Controlled blight runtime probe (prepared, not launched, not evidence)

Question: does original runtime regeneration of a non-hero `regenType=blight` unit
depend on `SetBlight*` state, and for which owner? The probe does not depend on the
initial W3E encoding. It varies only the known `SetBlight*` calls.

Source values, read from the installed archives (`War3.mpq` and `War3x.mpq` hold
byte-identical `Units\UnitBalance.slk`, SHA-256
`93c4f25eb75c46818e11f2569ca11e5efd742e97e68dbce4f96094059fa9f9c2`):

| Unit | Role | `HP` (max) | `realHP` | `regenHP` | `regenType` |
|---|---|---|---|---|---|
| `ugho` | blight unit, non-hero | 340 | 340 | 2 | blight |
| `hfoo` | control, non-hero | 420 | 420 | 0.25 | always |

- Rawcodes are lowercase, so neither unit uses the hero rawcode convention.
  Uppercase rawcodes follow the repo's hero convention and are excluded. `Ucrl` is a hero (`HeroCryptLord`).
- Reset life 100 is valid for both: it is above zero and at most min(340, 420).
- Rate expectation is `regenHP × elapsed`, using the timer that the probe records.
  No gain threshold is used.

Build (`.scratch/blight-probe`, ignored): `src/unit_rows.awk` extracts the rows from
the installed SLK, `src/source_values.py` validates them and writes the receipt,
`src/fill_block.py` substitutes the validated values into `src/probe_block.j`, and
`src/blight_map.py` (adapted from the wave03 packer) patches the Human01 template.
`run_build.sh` runs the chain. `src/plan_launch.py` prints the reference argument
vectors and starts nothing. Receipts: `out/source-values.json`,
`out/PCXB-blight-probe.receipt.json`, `out/launch-plan.json`, `out/compile.txt`.

Probe phases, each 20 s of `TriggerSleepAction` after a life reset of both units to 100:

1. `off`: no blight. Blight unit expected Δ = 0 (`regenType blight`, no blight).
2. `SetBlightPoint(Player(0))` on, then off.
3. `SetBlightPoint(Player(1))` on, then off (affinity).
4. `SetBlightRect(Player(0))` on, then off.
5. `SetBlight(Player(0), radius 256)` on, then off.
6. Move (0,0) to (300,0) with `IssuePointOrder "move"` under `SetBlightRect` x 128..512 on,
   then the same walk with blight off. The probe logs whether the order was accepted.

Each `Preload` line records `t0`, `t1`, `dt`, life before and after for both units,
positions, the expected control delta (`CONTROL_RATE × dt`), and the blight source
rate. Life values come from `GetUnitState` and are printed with `R2S`. The header
also records `R2S` output for `1/3` and `12345.678901`, which measures display
precision. The output file is `PCXB-blight-probe.txt`, written by `PreloadGenEnd`.

Interpretation (no thresholds):

- Control validity: the `hfoo` delta must equal `0.25 × dt` within the `R2S` display
  precision. Any other control residual invalidates that phase.
- Blight: on a point or rect phase, the `ugho` delta is compared with `2 × dt` and
  with 0. A residual matching neither is recorded as unexplained, not as a blight result.
- Affinity: compare Player(0) and Player(1) point phases with their off controls.
- Crossing: the walk logs the first sample where x ≥ 128. The 20 s window is one
  sample, so no crossing time is resolved by this probe. Crossing timing needs the
  motion segments from motion09.
- Uncertainty sources, not yet measured: `R2S` precision (logged), timer read
  latency, and whether life is updated in discrete ticks within `dt`.

Template isolation and interference (verified from the template `war3map.j`):

- The template `main` calls `CreateRegions`, `CreateCameras`, `CreateAllUnits`,
  `InitBlizzard`, `InitGlobals`, `InitCustomTriggers` and `RunInitializationTriggers`.
  The probe's replacement `main` calls none of them.
- Trigger isolation holds: no `InitTrig_*` and no `RunInitializationTriggers`, so the
  template's `gg_trg_*` triggers are never initialized.
- Interference: `config` still runs, and it sets players, teams, start locations and
  slots, which are not unit or trigger effects. `CreateAllUnits` is skipped, so the
  neutral hostile, neutral passive and player-building units are absent. The probe is
  therefore a runtime test in an emptied template world, not in the original placed
  scene. This is a deliberate omission. Include it in any comparison.

Launch plan (not run): `prepare` the run directory, stage the map under
`game/Maps/PCXB-Reference/`, then `launch` 1280×960 at 96 DPI with
`--game-arg=-loadfile` and `--game-arg=Z:\run\reference\game\Maps\PCXB-Reference\PCXB-blight-probe.w3m`.
The plan records `launched: false`. Launch waits for the root GUI queue after Sound03.

## Motion segments requested from root09

Blight eligibility along a moving unit needs exact time-spans inside the active
motion path, not point samples. The minimal request for the motion owner:

- Per active unit: `unit`, `segment` (index), `from` (x, y), `to` (x, y), `start` and
  `end` in the timer clock used by `advance_unit_activity`, `speed` (units per second),
  and a `turn`/`hold` flag that marks non-moving spans.
- Segments are monotonic in time and cover `[projected_through, now]` exactly, with
  no gaps and no overlap.
- Work bound: one pass per advance, O(segments intersecting the span) per unit.
  Segment count per unit is bounded by its path length in points, and the eligibility
  integrator splits each segment only at blight region boundary crossings.
