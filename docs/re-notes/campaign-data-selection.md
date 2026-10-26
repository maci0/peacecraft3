# Campaign data namespace selection

## Reviewed image and address correction

Static observations below use Legacy 1.29.2.9231 `Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The PE `.rdata` section has file offset `0xa64e00` and VA `0xe66000`;
the file-offset-to-VA delta is `0x401200`.

| String | File offset | Correct VA |
| --- | --- | --- |
| `%s_V%d` | `0xa67794` | `0xe68994` |
| `Melee` | `0xa6779c` | `0xe6899c` |
| `Custom` | `0xa677a4` | `0xe689a4` |
| `-classic` | `0xa68788` | `0xe69988` |

Earlier negative cross-reference searches used incorrect VAs (`0xea…`). Those
searches do not establish an absence of references. The correct format address
has 26 direct push references in the disassembly. The command-line string has
a direct `movq` reference at `0x42f290`.

## Native selection path

These are static call/data-flow observations, separate from the controlled
edition/stat measurements in `ability-cancellation-orders.md`.

- `0x705060` parses the current map into a local header with `0x703ff0`, then
  obtains a selector with `0x71dbd0`.
- `0x71dbd0` reads header offset `0x1e44`. Unsigned values greater than two
  become zero. The header initializer writes `0xffffffff` at this field
  (`0x71d221`).
- The header reader stores a word at `0x1e44` for information versions at least
  `0x11` (`0x71eb93` through `0x71eba1`). This is the word the current map parser
  exposes as `MapInfo.loading_id`, after the first three loading strings.
  Its comment that the game discards the word is incorrect for this image.
- Back in `0x705060`, selector one chooses index one, selector two chooses
  index zero. Selector zero calls `0x7046a0(0)` and chooses index zero when
  that result is nonzero, index one otherwise.
- `0x7046a0` tests bit `0x4` of map flags. The explicit-path branch reads the
  flags at local header offset `0x440` and masks the same bit (`0x704701`).
- `0x428fd0` indexes the pointer table at `0x10db190`: index zero points to
  the image's `Melee` string, index one to `Custom`. It passes that string and
  the edition global `0x10db3e0` to the image's `%s_V%d` format, calling
  `0x433bb0` at `0x429042`.
- It then passes the resulting namespace to `0x439fb0` for the existing archive
  handles (`0x429054` through `0x4290cb`). The handle indirection matters:
  `0x429009`, `0x429010`, `0x429024` and `0x429017` index tables at
  `0x10db170`, `0x10db178`, `0x10db180` and `0x10db188`, respectively.
  Their entries point to archive-handle output locations from the base mount
  path, rather than directly to archive-name strings.

For this image, following those handle locations gives the namespace mounts
below. The version column is the source version enumerand read by the native
path; this table records the disassembly, not a production edition mapping.

| Priority operand | Version 0 handle | Version 1 handle |
| --- | --- | --- |
| `0x0a` at `0x42905b` | War3 | War3x |
| `0x0b` at `0x42909d` | War3Local | War3xLocal |
| `0x0c` at `0x42907f` | War3Mod | War3Mod |
| `0x0d` at `0x4290bb` | War3Mod | War3Mod |

In particular, the version-zero priority-`0x0b` handle is `0x112def4`,
populated by the base local-archive mount; it is not the War3x primary handle.
Both higher-priority slots point to `0x112d898`, the War3Mod handle. A reader
that collapses identical archive/namespace layers must retain the highest
priority. Missing-member lookup/fallback remains a separate trace from this
mount setup.

The same edition global is returned by `VersionGet` (`0x4afb80`). The controlled
same-map pair observed TFT/grunt HP 700 by default and RoC/grunt HP 680 with
`-classic`, with Uther collision 32 in both. A zero-to-one selector mutation
under TFT preserved the same custom fingerprint, which is consistent with the
default non-melee and explicit-custom branches above.

A further controlled run changed only that information word from zero to two
and the diagnostic run identifier, using the verified `-classic` launch.
Map SHA-256:
`5e21e6acc54d8d083f90b985cb02a2313b5c32139907fd0dd10403b185d12c4a`.
All 16 records arrived. `VersionGet` reported RoC; grunt maximum life became
700 (the zero-selector RoC control had 680). Uther, peasant and footman collision
sizes stayed 32, 16 and 31. The changed life agrees with the explicit-melee
branch above. The earlier proposed collision-zero fingerprint was a source
inspection error, not evidence of different native field semantics. A fresh
extraction from the selected `War3.mpq`, read with the production `pcx-slq`
parser, gives 32/16/31 for those units in `Custom_V0`, `Custom_V1` and `Melee_V0`.
Collision therefore does not distinguish these datasets. The extracted melee
table's SHA-256 is
`9e793e3c0a121c2d8a9fdad1232da93ac537468077ef5ee4ed162ae2e12b4dce`;
the parser helper, cells and remaining source hashes stay in ignored
`.scratch/collision-source-probe`.

The chapter card was inspected before one Space input; the owned container was
stopped after collection. Receipts are retained in ignored
`.scratch/edition-stats-word2-probe`. The replacement script omits authored units
and triggers, so this is source-selection evidence, not campaign acceptance.

## Implementation boundary

Production `campaign_data_set::ImageRules` now derives namespace strings,
selector operands, archive names and mount priorities from the selected image.
`CampaignRecipe` derives the edition enumerand from admitted common-script
constants and scopes a resolver to the actual map information. Native campaign
startup, chapter transition and headless startup use that resolver for both the
unit catalog and startup rule/profile tables. UI ability profiles use the same
admitted ObjectHost tables; logical `Units` paths do not imply unprefixed bytes.

The explicit selected-install recipe test compares the resolved Mirror Image
ranked mana-cost cell and provenance with the actual archive member under the
recipe-derived namespace. It does not assert a copied runtime cost. Existing
synthetic tests cover selector/mode/edition combinations and malformed input.
This qualification must not be generalized to callers that construct an
unscoped resolver independently.
Disassembly and local binary bytes remain in ignored
`.scratch/dataset-selection-native`. No original asset content is published.
