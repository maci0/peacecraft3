# Campaign AI build-time query

`GetUnitBuildTime(integer unitid) -> integer` reads the selected unit catalog's
`UnitBalance.bldtm` for ordinary IDs. It shares the existing strict integer
quantity parser with gold/wood costs, including resolved map modifications;
it does not create a unit, infer a time from an animation, or copy a measured
duration into code. Unknown IDs return zero. Missing, ambiguous, malformed or
negative ordinary build-time fields refuse with the query name.
The shared quantity path distinguishes a catalog's invalid raw-ID lookup from
a malformed known definition, so ID zero returns zero for cost/time queries
without suppressing broken-record diagnostics.

## Evidence and hero distinction

The selected image is 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
`0xab5260` selects an ordinary data getter at `0x67c3d0`, or a context query at
`0xabe900` according to the requested ID's leading-byte range. The existing
unit-cost policy now reads the build-time handler too, checking that both
branches have the same derived range. Synthetic fixtures vary the range,
reject changed call/branch destinations and refuse every truncated window.

The ordinary getter resolves the cached selected type and reads its integer
value; absence returns zero. The catalog's typed `ubld` metadata identifies
the `bldtm` column. The retained original-client experiment in
[world queries](campaign-ai-world-queries.md) observed the ordinary `hfoo` time
matching its selected row, while an unowned hero returned zero despite a
nonzero authored time. Those observations are not production constants.

The bounded REA follow-up is retained under `.scratch/ai-build-time/`:

| Evidence | Selected code |
| --- | --- |
| `ev_6221c375a75facad1b4a733d12ea9a6458d647abb0783c146136cb2d643baf5d` | Context query `0xabe900` searches linked objects and the owning player's hero records; an absent match returns zero. |
| `ev_b1183de44b37f24e7551015aeaace5c7d6f1521457b9d27024b4aadb21e24b40` | `0xb48a00` searches owned hero records and forwards a match to the revival-time calculation. |
| `ev_2c133695bf925c80c3d064c982fcf1e97050f0afe2e5316c20ddb28881f359b5` | `0xb48ab0` reads `ReviveTimeFactor`, `ReviveMaxTimeFactor` and `HeroMaxReviveTime`, with numeric helper calls and integer conversion. |

These are static-code observations, not a new original-client run. The helper's
complete numeric conversion/order and hero lifecycle behavior are not qualified
by the ordinary getter. Returning a hero's authored `bldtm` would therefore
silently substitute a different result.

## Recreation boundary

The accepted factories create complete canonical units and do not implement
hero revival or a production queue. A hero ID with no matching owned unit
returns zero; a matching owned hero refuses pending a source-derived revival
calculation. The query uses the canonical current type and owner, so another
player's hero does not supply its context. Dropping removed records follows the
existing canonical arena; native retained hero identities after removal remain
a fidelity question. No separate hero or producer store was introduced.

Hero-token discounts apply only to costs. Build-time queries do not require a
hero-token counter or turn an ordinary duration into zero when tokens change.
The internal shared quantity API was renamed from `ai_unit_cost` to
`ai_unit_amount`; existing cost behavior and its validation remain intact.
Owned AI-common declaration checks admit the new integer query before globals
execute, and dispatch validates the runtime argument.

Tests cover nonstock cell/override durations, malformed metadata/value refusal,
empty versus owned/foreign hero context, unknown IDs, query handle immutability,
and exact handler range/branch checks. The published two-program fixture reads
a custom unit's overridden duration before and after a one-player token change.
The final combined gates pass 1,695 core, 55 app-library and 638 app-binary
tests, both published fixtures, the selected-image query reader, selected
Human01 startup and victory Continue, and strict native/WASM Clippy. The
unchanged Human02 opening-minute test remains red, now at `DisplayTextI` at
7.312500 seconds. Final logs are under `.scratch/ai-build-time/gates-final/`;
the preceding failed fixture is preserved under `gates/`. That failure exposed
the invalid-ID versus absent-ID lookup distinction, corrected in the shared
quantity path. Training, owned-hero revival timing and the full campaign remain
unfinished; no new original-client run is claimed.
