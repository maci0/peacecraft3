# Native archive priority and campaign namespace lookup

## Scope

Static analysis only. The target is Legacy `Warcraft III.exe`
1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
No game process, container or native execution was used. These observations
describe one executable's mount path; they do not establish another release or
prove which on-disk archives were shipped together.

## Campaign data selection

- `0x71dbd0` reads the map-header word at `0x1e44`, treating unsigned values
  greater than two as zero. The header initializer writes `0xffffffff` there
  (`0x71d221`), so an absent version-gated word behaves as selector zero.
- `0x71eb93` stores that word for map information versions at least `0x11`.
  This is the word exposed as `MapInfo.loading_id` after the three loading
  strings.
- Selector one chooses index one; selector two chooses index zero. Selector zero
  calls `0x7046a0(0)` and chooses index zero when its result is nonzero, index
  one otherwise.
- `0x7046a0` tests bit `0x4` of the map flags. The explicit-path branch reads
  the same flags at local header offset `0x440` and masks the same bit.
- `0x428fd0` indexes the pointer table at `0x10db190`, whose entries are the
  image strings `Melee` (index zero) and `Custom` (index one). It combines that
  string with the edition global at `0x10db3e0` through the image's `%s_V%d`
  format, producing the selected namespace prefix.
- The edition global is the value returned by `VersionGet` (`0x4afb80`).
  Observed namespace copies include `Melee_V0`, `Custom_V0` and `Custom_V1`.

The map supplies `loading_id` and `flags`; the selected client supplies the
dataset strings, format and edition enumerand. A reader should not copy a
RoC-to-`Custom_V0` match arm into production.

## Mount records and priorities

`0x439fb0` creates an archive record containing a namespace prefix, a priority
and a base archive pointer, then inserts it into the global list at `0x10db4d8`
sorted by descending priority. The records observed on this path are:

| Base archive | Base priority | Selected-namespace priority |
| --- | ---: | ---: |
| `War3Mod.mpq` | 9 | — |
| `War3xLocal.mpq` | 4 | 0xd |
| `War3x.mpq` | 3 | 0xb |
| `War3Local.mpq` | 2 | 0xc |
| `War3.mpq` | 1 | 0xa |
| `Deprecated.mpq` | 0 | — |

The selected-namespace copies mount the same archive objects with the prefix
already applied; they are not separate archive files. `War3Patch.mpq` is not
referenced by this mount path in this executable. Its absence here does not
prove that no other path opens it.

## Lookup and fallback

`0x435cb0` is the `SFileOpenFileEx` wrapper. It walks the global list from the
highest priority, forms `namespace\path` for prefixed records, and calls the
per-archive open path at `0x437040`. It continues to the next record when that
open reports absence (error 2).

Consequences:

- A missing member in the selected namespace falls through to the unprefixed
  base archives in priority order. The lookup crosses namespace boundaries.
- A present member in a higher-priority record wins even if a lower-priority
  record has another copy.
- Once an archive reports a successful open, there is no observed retry of the
  same member against a lower-priority archive. The wrapper's fallback is for
  absence, not for a failed read after a winner has been opened.

This matches the existing `pcx-assets` resolver for an absent member and for a
listed winner that cannot be read (`crates/pcx-assets/tests/layering.rs:449`).
It differs in layering: production currently mounts one global archive chain,
not selected-namespace records layered above the base chain.

## Relation to current constants

`crates/pcx-mpq/src/lib.rs:117` currently orders the install as
`war3patch`, `war3x`, `war3xlocal`, `war3local`, `war3`,
`deprecated`. The recovered mount order is higher-first
`War3Mod` (9), `War3xLocal` (4), `War3x` (3), `War3Local` (2),
`War3` (1), `Deprecated` (0), with campaign namespace copies at
`0xa` through `0xd`.

Those are different policies. The current constant must not be presented as
the native campaign mount order, and it should not be changed globally to
implement campaign selection without checking every existing consumer.

## Minimal scoped implementation path

The following uses existing `pcx-assets` and `pcx-mpq` types and leaves
`DEFAULT_PRIORITY` unchanged.

1. **Derive the namespace in a new reader** under `crates/pcx-play` (beside the
   existing selected-image readers). Inputs: the selected executable bytes,
   `MapInfo.loading_id`, `MapInfo.flags`, and the selected edition. Read from the
   image the selector operands at `0x71dbd0`/`0x71eb93`, the flag mask at
   `0x7046a0`, the dataset pointer table at `0x10db190` and the `%s_V%d`
   format. A malformed image or absent/ambiguous operand refuses; no default
   namespace is substituted.
2. **Select the prefix from source values.** Normalize `loading_id` with the
   native `0xffffffff` initializer when it is absent. Apply selector one/two
   directly, and selector zero through the recovered flag test. Combine the
   chosen dataset string and edition enumerand with the image's format.
3. **Build a scoped resolver before publishing the map reader.** Open the base
   archives individually in the recovered order and mount each with its base
   priority. For the selected namespace, add prefix-mapped `MpqSource` layers
   at `0xa` through `0xd`, backed by the four corresponding archive objects.
   Each namespace layer maps a logical path only when the prefixed member is
   present; absent entries are omitted so the existing resolver falls through
   to the base layers. Exclude `War3Patch` from this scoped path, matching the
   observed mount function.
4. **Parse the map header first.** `native_campaign::configuration`
   (`crates/peacecraft3/src/native_campaign.rs`) currently builds one
   `InstallSource` layer and binds the chapter map through it. The scoped path
   must first read the chapter map bytes with the base reader, parse
   `war3map.w3i`, derive the namespace, then construct the scoped resolver for
   the actual binding. This is a two-stage map read, not a fallback.
5. **Keep the scoped resolver attached to the same receipts.**
   `SelectedMapSources::publish` and `MapRequests::replace_selected` already
   retain the resolver that produced the accepted binding. Consumers such as
   `campaign_inputs::prepare_new_game` then read `Units/MiscGame.txt` through
   that scoped chain without changing their lookup names.
6. **Browser configuration** (`crates/peacecraft3/src/browser_configuration_json.rs`)
   has no map bytes or executable image at decode time. It can express this
   policy only as caller-supplied layers with explicit priorities and exact
   prefixed member mappings, or after an explicit schema extension carrying
   the source inputs. It must not infer prefixes from archive names.

Do not switch unrelated rule consumers to the selected namespace until their
source lookup and receipt paths agree. In particular, the native RoC speed
probe accepted 80 and clamped zero to 10, while the unprefixed `MiscGame.txt`
minimum is 150. Selecting the namespace does not by itself qualify the speed
stack.
