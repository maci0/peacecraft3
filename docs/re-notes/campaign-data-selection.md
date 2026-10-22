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
  handles (`0x429054` through `0x4290ad`). This is an archive/source setup path;
  the exact missing-member lookup/fallback policy still needs a separate trace.

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
branch above. The unchanged collisions contradict the earlier proposed
collision-zero fingerprint for the melee dataset: a zero in a compared table
is not proof that the native collision getter returns zero. That field's actual
consumer/merge semantics remain unqualified. Do not use it to reject this run
or to infer that namespace mounting did not occur.

The chapter card was inspected before one Space input; the owned container was
stopped after collection. Receipts are retained in ignored
`.scratch/edition-stats-word2-probe`. The replacement script omits authored units
and triggers, so this is source-selection evidence, not campaign acceptance.

## Implementation boundary

No production selector is implemented by this note. A reader should derive
namespace strings, selector operands and the flag mask from the selected image,
and the edition enumerand from the admitted source constants. Copying a
RoC-to-`Custom_V0` mapping into a match arm would conceal those inputs.
Do not switch unrelated rule consumers until their source lookup and receipt
paths agree. In particular, the native RoC speed probe accepted 80 and clamped
zero to 10; the unprefixed MiscGame minimum of 150 cannot qualify that behavior.

Disassembly and local binary bytes remain in ignored
`.scratch/dataset-selection-native`. No original asset content is published.
