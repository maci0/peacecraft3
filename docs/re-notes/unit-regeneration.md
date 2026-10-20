# Unit health and mana regeneration

Live units integrate per-second life and mana regeneration over the game time
projected by `advance_unit_activity`. The integrator runs before combat and
inventory at each projection step, so a span split into any number of steps gives
the same pools as one step (up to f32 rounding of the pool, each step rounds once).

## Sources

| Input | Source | Use |
|---|---|---|
| `regenHP` (`uhpr`) | `Units/UnitBalance.slk`, column `regenHP` | base life per second; SLK `-` reads as 0 |
| `regenMana` (`umpr`) | `Units/UnitBalance.slk`, column `regenMana` | base mana per second; SLK `-` reads as 0 |
| `regenType` | `Units/UnitBalance.slk`, column `regenType` (Token) | life eligibility: `always`, `day`, `night`, `none`, `blight` |
| `StrRegenBonus`, `IntRegenBonus` | `Units/MiscGame.txt` `[Misc]` | life/mana regen per derived STR/INT point, via `HeroVitals` |
| `bj_TOD_DAWN`, `bj_TOD_DUSK` | `Scripts/Blizzard.j` constants | day window `[6, 18)` hours, read from the admitted program globals |
| `DayLength`, `DayHours` | `Units/MiscGame.txt` | game seconds per hour (existing time-of-day clock) |

The values read from the selected source copy at
`.scratch/campaign-time-of-day-root-20261020/frontier-1/audit-execute/selected-source-copy/units/`
are `StrRegenBonus=0.05`, `IntRegenBonus=0.05`, and 837 `UnitBalance` rows with
`regenType` in {always 502, blight 41, night 51, none 242}. No row uses `day`;
the parser admits it because the editor token is the same name family.
Blight rows carry `regenHP` 2 (`unitbalance.slk` line 8139 onward). That copy's
build edition is not recorded here; the integrator must confirm it against 1.29b.

## Rules implemented

- Life rate = `regenHP` + (hero only) derived STR × `StrRegenBonus`.
- Mana rate = `regenMana` + (hero only) derived INT × `IntRegenBonus`.
- Derived STR and INT use the same `derive_hero_attribute` as live combat, with the
  current level and the item bonus, so level changes and held items change the rate.
- `always`: life regenerates for the whole span. `day`: only inside the day window.
  `night`: the complement. `none`: never. `blight`: refuses, see below.
- Mana regenerates regardless of `regenType`.
- Dead units and units with life ≤ 0 do not regenerate.
- Life is capped at `max_life`; mana is clamped to `[0, max_mana]`.
- A nonzero life rate under `day` or `night` needs the admitted time-of-day clock
  and dawn/dusk constants; without them the advance refuses.
- A nonzero life rate under `blight` refuses the advance. Blight ground state is not
  modelled, so such a unit cannot be given a regeneration span it did not earn.

Day seconds are computed exactly: the hour is linear between clock rebases, so each
window edge crossing is solved for, and each piece is classified at its midpoint.

## Evidence grade

- Shipped-byte evidence: the `UnitBalance`, `MiscGame` and `Blizzard.j` values above.
- Inference, not native-verified:
  - Per-second units (`regenHP` is applied per second, as the editor labels it).
  - Mana is not gated by `regenType`, because the column sits outside the HP group.
  - The STR/INT terms are gated together with the base rate under `regenType`.
  - The day window in game.dll matches the `Blizzard.j` `[dawn, dusk)` test.
  - Continuous integration matches the native periodic timing for the pool values.
- Not established: the native regen tick period and rounding, regeneration suppression
  after damage (none is implemented), and the native treatment of dead-boundary life.

## Callers and boundaries

- `ObjectHost::advance_unit_regen` is called from `advance_unit_activity`, after
  motion and before combat. A refused span leaves pools and `regen_through` unchanged.
- `admit_unit_vitals` now requires admitted unit activity, since regeneration uses its
  projection clock. A missing clock fails at admission rather than silently skipping.
- `prepare_unit_vitals` refuses creation when the `regenHP`/`regenMana`/`regenType`
  columns are missing or malformed, as it does for `HP`/`manaN`.

## Tests

- `regeneration_integrates_stock_rates_and_caps_at_maxima_for_any_tick_chunking`:
  one 10 s span and 1/64 s steps give identical pools; caps hold.
- `regeneration_never_applies_to_units_without_rate_or_with_none`.
- `night_and_blight_regeneration_refuse_without_their_source_state`: atomic refusal.
- `daytime_integrates_exact_window_crossings_and_ignores_chunking` and
  `suspended_or_stopped_clock_holds_its_hour`: window integral, chunking, negative
  scale, suspension.

Pure tests do not prove native parity. The asset-gated census test and the full
native/WASM gates are the integrator's responsibility.
