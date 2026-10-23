# Unit-type food queries

`GetFoodMade(integer)` and `GetFoodUsed(integer)` return the requested unit
type's authored food values. They query the admitted object catalog directly;
they do not count units or apply a player's food ceiling. Map globals and
campaign AI use the same implementation, including map object overrides.

## Selected-source evidence

The selected `common.j` declares both natives as constant integer-to-integer
queries. The reviewed 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.

| Native | Primary registration | Handler | Type lookup | Cached integer |
| --- | --- | --- | --- | --- |
| `GetFoodMade` | `0x49d2ad` | `0x492580` | `0x690270` | `+0x60` |
| `GetFoodUsed` | `0x49d2c1` | `0x492590` | `0x6902a0` | `+0x5c` |

The primary handlers forward the integer rawcode to the unit-type cache lookup.
The successful path returns the cached integer; a missing record returns zero.
These paths do not read a player, live unit count or food limit. Separate generic
stub registrations are not the basis for this behavior.

Bounded REA observations retain the two lookup bodies:

- Made: `ev_2381e9a5432f28d4aacf9d23150d4e318af8891a22ca454d5d75a362b498e6c3`.
- Used: `ev_c803e61c018e1446799325c0afc83d356d945f7299cd3a47ddbb9d5693abb4e0`.

The existing typed `UnitBalance.fmade`/`fused` metadata and
[source-derived marker conversion](script-player-resources.md#authored-food-markers)
supply the values. No cache layout or second food store is recreated. Evidence,
mapped-image receipts and disassembly remain local under `.scratch/food-queries/`.
No new original-client execution is claimed.

## Runtime boundaries and checks

Both declarations must come from the owned common source, be unambiguous and
match the constant scalar-integer contract. AI admission checks this before
globals or player activation, then forwards calls into the shared world.

Unknown or invalid rawcodes and non-unit object IDs return zero. Known malformed,
missing or ambiguous food fields refuse with native/rawcode/column context.
Typed integer metadata and integer map overrides remain integers throughout;
live player totals now reuse that path with checked addition, avoiding loss of
precision through a float intermediate. Recognized `-`/`_` markers require the
admitted source conversion. Arbitrary strings are still rejected, an explicit
strict recreation boundary relative to the native integer reader.

Synthetic checks cover distinct made/used values, map overrides, values above
the exact-float integer range, nonstock marker conversion, unknown/non-unit IDs,
malformed data and declaration/argument refusal. Queries allocate no unit or
handle and remain unchanged after unit creation/removal; live totals follow
those lifetimes. The published two-program session checks map-global calls and
AI calls before and after sleep, with different owned unit counts.

Combined checks pass 1,703 core tests, 55 app-library and 638 app-binary tests,
the published two-program fixture, selected-image admission, Human01 startup,
victory Continue, strict native/WASM Clippy and the default desktop build.
The unchanged Human02 opening-minute test passes the food calls and now refuses
`ClearHarvestAI` at 8.312500 seconds. It remains red; neither this check nor the
synthetic fixtures establish a complete campaign strategy or rendered cinematic.
Check receipts remain under `.scratch/food-queries/gates/`.
