# Unit health and mana regeneration

## Clock semantics

Each unit has its own regeneration clock, `Regen.through`. It is set to the game time
of creation and moves forward at every projection step. A unit never regenerates for
time before it existed. `advance_unit_regen(now)` runs inside `advance_unit_activity`,
after motion and before combat and inventory. Each step integrates
`[through, now)` per unit, then sets `through = now`.

- Dead units and units at life ≤ 0 keep their pools, and their clock still moves,
  so a later revive cannot regenerate across the dead span.
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
