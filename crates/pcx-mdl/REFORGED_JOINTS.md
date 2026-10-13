# Checked v1800 primary-convention joint metadata

This is an explicit **source convention / local import policy, not
native-verified** joint selection. No GPU binding, pose transformation,
normalization, weight conversion, duplicate merge or default is implemented.
Classic parsing, all existing accessors and strict HD mesh refusal are unchanged.

## Primary evidence and the discriminating shipped input

WhiteoutLib is pinned to revision
`92887f9156cdaca8966b3b58e2625628aa0c2c30`.
[ImportSkin](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/wem/converters/mdx_converter.cpp#L791)
looks up an in-range joint in `matrixIndices`, then resolves the resulting
object ID. [WriteGeosetSkin](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/wem/converters/mdx_converter.cpp#L1921)
writes an object-ID palette to MATS and palette ordinals to SKIN.
[ImportNodes](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/wem/converters/mdx_converter.cpp#L605)
selects numbered-node BPOS rows by object ID, independently of BONE record order.
[parseBPOS](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/mdx/parser.cpp#L1539)
and [writeBPOS](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/mdx/writer.cpp#L887)
establish a u32 count followed by twelve f32 words / 48 bytes per row.
Float-weight conversion and fallback behavior in that implementation are
deliberately outside this checked metadata helper.

The configured HD Altar portrait is build `3.0.0.24268`, version 1800:
`war3.w3mod:_hd.w3mod:buildings/human/altarofkings/altarofkings_portrait.mdx`,
CKey `751c8db8dbeced3e7619165e9251aefc`,
EKey `c6f84b1b7f75d22e73f603be03328717`, 912,888 decoded bytes.
These provenance observations are original-fixture assertions, not decoder
constants. Configured CASC resolution and content checks precede parsing.

| Altar geoset 3, vertex 0, lane 0 | Checked source observation |
| --- | --- |
| SKIN joint word | 0 at file `0xdacbc` |
| SKIN weight word | 255 at file `0xdacc4`, kept as an integer |
| Selected MATS word under primary convention | MATS[0] = 3 at file `0xdabe8` |
| BONE inventory | Four records; object ID 3 occurs exactly once, at `0xdcc88` |
| BPOS inventory | 22 rows; count word at `0xde9d4` |
| Selected raw row under primary convention | Row 3 at `[0xdea68,0xdea98)` |

The integer candidates 0 and 3 differ on real shipped bytes. **That difference
does not prove which candidate native selects.** The helper chooses the pinned
primary convention explicitly, never infers a native choice from the fixture.
The identity palettes in the HD Footman alone cannot discriminate the candidates.

## Additive APIs and provenance

All three model methods are v1800-only and take the same input used to parse
the container:

- `Mdlx::bind_pose_stream_v1800(bytes)` returns an optional borrowed
  `BindPoseStream1800`: raw count/payload, absolute tag/payload offsets,
  `data()`, `record_count()` and `get(index)`.
  `BindPoseRecord1800` preserves index, absolute offset, a borrowed `[u8;48]`
  and `bits()` returning twelve u32 patterns. NaNs, signed zeros and every
  other authored bit remain intact. No matrix basis/inverse/pose space is assigned.
- `Mdlx::matrix_palette_v1800(bytes, geoset)` returns an optional borrowed
  `MatrixPalette1800`: version, declared count, absolute geoset/tag/body offsets,
  raw `data()`, `record_count()` and `get(index)`.
  `MatrixPaletteEntry1800` retains the borrowed `[u8;4]`, absolute offset and
  u32 `object_id` under the primary convention. MATS' untagged trailer remains
  separately preserved by the existing stored GEOS API.
- `Mdlx::primary_joint_join_v1800(bytes, geoset)` prepares a
  `PrimaryJointJoin1800` with the checked source BONE inventory. `get(vertex,lane)`
  returns `PrimaryJointReference1800`: version, raw joint/weight, selected
  object ID, absolute joint/weight/MATS/BONE-object-ID offsets and the borrowed
  BPOS record. The BPOS index is that object ID, with no rebase or runtime bone
  ordinal substituted. The context does not borrow the copied Geoset data;
  its raw MATS, SKIN and BPOS spans borrow the original model input.

`SkinRecord1800::weight_diagnostics()` returns
`SkinWeightDiagnostics1800 { sum: u32, nonzero_lanes: u8, max_word: u16 }`.
Four u16 values have a maximum sum of 262,140, so u32 suffices. This diagnostic
changes no word or lane and imposes no expected sum. It performs no float
division or normalization. Zero lanes and repeated joints are checked and
returned independently, without removal or merging.

Absolute GEOS-body base is `GEOS.payload_offset + Geoset.offset + 4`.
For vertex v and lane k, joint offset is SKIN-body base + `16*v + 2*k`;
weight offset is joint offset + 8. MATS word offset is MATS-body base +
`4*raw_joint`; BPOS row offset is BPOS-payload base + `4 + 48*object_id`.
BONE object-ID word offset is BONE-payload base + record offset + 84.
The accessor checks the Geoset's inclusive size and full body against its input
slice before attaching source provenance. Borrowed word/row pointers are tested
against those original ranges.

## Checked policy and typed refusals

This is a local import policy, not native malformed-input rejection parity.
The existing exactly-one, four-byte VERS policy belongs to `parse_stored_v1800`.
These accessors reject other versions, duplicate BPOS/GEOS/BONE chunks and
duplicate MATS blocks. BPOS must tile exactly `4 + count*48`, with checked
multiplication/addition and bounded row ranges. MATS must retain the declared
u32 word width. The source BONE inventory must have complete node/trailer spans;
the helper adds a minimum `BONE_FIXED + BONE_TRAILER` check before trusting IDs.
No existing BONE/parser acceptance policy is altered.
The helper inventories bounded BONE headers directly; animation tracks within
the declared node span stay opaque. It does not call or change the classic
animation reader. The primary
[parseBONE/parseNode](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/mdx/parser.cpp#L829)
corroborates the node-size-plus-eight framing and object-ID field location.

Missing raw BPOS/MATS is `None`; explicitly present empty streams are distinct.
The checked join does not replace absence with an identity pose/palette.
It returns the new `Error::PrimaryJointRefusal`, retaining version, absolute
geoset/joint location, vertex, lane and a typed `PrimaryJointRefusal1800` reason:

- Missing SKIN/palette/BPOS, invalid vertex/lane.
- Joint outside palette range, even when the raw joint is itself a known BONE ID
  or its weight is zero: no direct-ID fallback.
- Object not in the proven BONE inventory: non-BONE and missing-node targets
  remain refused; generic node parsing is not substituted.
- Duplicate BONE object ID: ambiguous references remain refused.
- Selected object ID outside the BPOS rows: no default or neighboring row.

Envelope/range/source-mismatch failures use existing typed errors. Raw stored
streams and independent raw accessors remain available when a semantic join
refuses. Duplicate palette values are not merged; an ordinal still selects its
authored word and all four raw lanes remain separate.

## Discriminating and original tests

`tests/joint1800.rs` contains synthetic fixtures only. Non-identity `[21,10]`
MATS and sparse object IDs 10/21 in both BONE orders discriminate palette,
record-order and rebased-row mistakes. Distinct BPOS bit patterns assert the
selected row; borrowed pointers and every absolute source offset are checked.
Other cases retain full-width integer weights, NaNs/signed-zero row bits, zeros
and duplicate lanes; reject missing/duplicate/ragged/undersized references,
direct-ID fallback, non-BONE/ambiguous IDs, wrong versions, overflowing indexes
and mismatched source Geosets.

The ignored keyed Altar and HD-Footman gates in `tests/local_joint.rs` require
explicit `PEACECRAFT3_REFORGED_DIR`, `PEACECRAFT3_REFORGED_ALTAR_ASSET` and
`PEACECRAFT3_REFORGED_HD_ASSET`. Both verify configured paths, EKeys/CKeys,
raw row/word borrowing, every lane's source offsets and strict HD mesh refusal.
The Altar witness asserts object ID 3 in BONE and BPOS row 3 explicitly.

## Residual unknowns and next probe

Native selection/fallback, non-BONE weighted targets, BPOS transform meaning,
matrix basis/inversion, GPU binding/blending and integer-to-float conversion
remain unproved. Other MDX versions are refused. The next native probe, only
in a separately authorized exact-build task, is to trace Altar geoset 3's joint
0 from SKIN through the native MATS/node/BPOS consumer, recording whether it
selects object 0 or 3 and which stored row/transform it uses. No provider or
runtime change was attempted here.

The malformed-short-node test initially exposed an underflow in the existing
classic `Mdlx::bones` reader at `src/lib.rs:2800`. Its behavior is unchanged.
The new helper checks record spans and inventories bounded source headers
directly, so that eight-byte record now yields a typed `RecordShape` refusal
without entering the classic animation reader.

## Validation and caller handoff

Exact scoped commands, using the unchanged lockfile and a private target:

```sh
CARGO_TARGET_DIR=/tmp/pcx-mdl-join-target cargo test -p pcx-mdl --locked
CARGO_TARGET_DIR=/tmp/pcx-mdl-join-target cargo test -p pcx-mdl --locked -- --ignored --nocapture
CARGO_TARGET_DIR=/tmp/pcx-mdl-join-target cargo clippy -p pcx-mdl --locked --all-targets --no-deps -- -D warnings
```

Final narrow output includes:

```text
test result: ok. 55 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The ten new synthetic joint tests pass; the 24 existing synthetic integration
tests also pass. Both ignored-fixture configurations passed all ten original
tests. The compatibility-tree corpus printed `mdx=3234 parsed=3234 failed=0`;
the legacy corpus printed `mdx=3249 parsed=3249 failed=0`.
Clippy exited 0 with:

```text
Finished `dev` profile [optimized + debuginfo] target(s) in 0.42s
```

The exact keyed Altar gate observed four geosets, 12,459 vertices, 49,836
primary-convention lane joins, four BONE records and 22 BPOS rows, zero reference
refusals. The keyed HD-Footman gate observed 36 geosets, 40,142 vertices,
160,568 lane joins, 106 BONE records and 135 BPOS rows, zero reference refusals.
Both retain the existing strict HD mesh refusal. Every raw row/word pointer,
bit pattern and computed source offset is checked against the input.

The ignored commands explicitly set `PEACECRAFT3_INSTALL` and
`PEACECRAFT3_FORMAT_INSTALL` to each MPQ corpus,
`PEACECRAFT3_LEGACY_DIR` to the legacy install,
`PEACECRAFT3_REFORGED_DIR` to the supplied CASC install,
`PEACECRAFT3_REFORGED_ASSET` to the configured SD Footman, and the two
HD fixture variables described above to their exact configured paths.
Local logs are `/tmp/pcx-mdl-join-target/{tests,originals,originals-legacy,clippy}.log`.
Formatting and `git diff --check` were clean. No root manifest/lock/dependency,
CASC, gfx, application, provider, runtime or git state change was made.
Original game bytes remain local and are not embedded in source or fixtures.
