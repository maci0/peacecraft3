# Version-1800 stored tangent and weighted-skin streams

## Configured namespace discovery

The local build is `3.0.0.24268`. Discovery enumerated authored TVFS entries
starting from configured `vfs-root`, resolving each entry through the checked
ENCODING mapping and traversing it only when its EKey identified another
configured manifest. It reached 110 manifests with zero refused entries. No
HD suffix, module precedence or shorthand was invented.

The selected stored path is
`war3.w3mod:_hd.w3mod:units/human/footman/footman.mdx`. Its three authored links
are root `war3.w3mod` at node `0x3e` (`vfs-root`/`vfs-1`), `_hd.w3mod` at
node `0xac` (`vfs-2`), and the leaf at node `0x7edb2` (`vfs-43`). The HD manifest
has EKey `ebea75e6ea9974e0c9442abf71ced0ba`, CKey
`5e6819e084c5492ca0fce3793fa86877` and 26,791 entries.

The checked file has 4,424,314 decoded bytes, CKey
`6fcd268161babbd73f4601060be39df5`, EKey
`55ddc4ab832e5eadc526d91d40cdd9dd`, and MDX version **1800**. These identities
and counts are provenance observations, not decoder constants. Original bytes
are imported only in memory and are not retained in the repository.

## Actual GEOS framing and version boundary

GEOS occupies `[0x4400,0x2f622a)` and its 36 inclusive records tile that
3,087,914-byte payload. The observed sub-block sequence is VRTX, NRMS, PTYP,
PCNT, PVTX, GNDX, MTGC, MATS plus its untagged trailer, TANG, SKIN, UVAS and
the declared UVBS sets. GNDX has zero entries in this asset; it is not a
replacement weighted-skin table. There are 44 stored UV sets in total.

- TANG's count measures vertices. Each record is four little-endian f32 bit
  patterns, **16 bytes**, with no conversion or inferred handedness.
- v1800 SKIN's count measures **u16 elements**, eight per vertex. Each record
  is four joint words followed by four weight words, **16 bytes**. The parser
  retains all words; it does not divide, normalize, clamp or map a joint to a
  runtime skeleton.
- Every geoset's TANG count equals VRTX count; SKIN count equals eight times
  VRTX count. Totals are 40,142 tangent/skin records and 321,136 skin words.

For geoset 0, TANG starts at file `0x3b878`, count 6,947, body 111,152 bytes.
SKIN begins at `0x56ab0`, count 55,576 u16 words, also 111,152 body bytes;
its body ends exactly at UVAS at `0x71ce8`. Treating the count as bytes advances
to `0x643d0`, still inside SKIN. No scan or padding skip can repair that width.

Primary cross-checks:
[WhiteoutLib v1800 parser, commit 92887f9](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/mdx/parser.cpp)
and its [Geoset structure](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/include/whiteout/models/mdx/structures.h)
corroborate the tangent vector width, joint/weight grouping and u16 SKIN elements.
That implementation distinguishes byte elements below v1400 from u16 elements
at v1400+. The older
[mdx-m3-viewer parser](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/parsers/mdlx/geoset.ts)
reads SKIN as bytes. That older branch is not applied to this file.

Only **v1800** is enabled by the new accessors. The older version difference
is primary implementation evidence, not a supplied older-HD fixture proof.
v800/900/1000/1200/1400 and unknown versions refuse these accessors rather than
using an assumed width. Classic v800 and SD v1800 mesh behavior is unchanged.
Observed weight sums in this particular file are 255; that observation is not
an enforced parser rule or a normalization denominator.

## Stored API and render boundary

The HD file also has FAFX at `[0x4367c2,0x43691e)` and BPOS at
`[0x43691e,0x43827a)`. Strict `Mdlx::parse` still refuses FAFX, and
`Mdlx::mesh` still refuses TANG/SKIN even when handed the stored container.
Neither importer is made to silently use GNDX/MATS for weighted data.

The additive path is:

```rust,ignore
let model = pcx_mdl::Mdlx::parse_stored_v1800(bytes)?;
for geoset in model.geosets(bytes)? {
    let stored = model.geoset_streams_v1800(&geoset)?;
    // Inspect source streams; render semantics remain a separate task.
}
```

`parse_stored_v1800` checks the MDLX envelope, every chunk's range, VERS and
the exact version gate. Its **checked import policy** requires exactly one VERS
record with exactly four payload bytes. Identical/conflicting duplicates and
short/oversized payloads are rejected. This is a local width-gate policy, not
native malformed-input rejection parity; strict `Mdlx::parse` is unchanged.
Duplicate VERS uses `UnsupportedVersionedChunk` with `chunk: "duplicate VERS"`;
invalid width uses `RecordShape` with the VERS tag offset and expected width four.
It exposes opaque chunks through the existing Chunk
offset/size metadata and payload accessor; it does not decode or approve their
semantics. This is distinct from the strict importer.

`GeosetStreams1800` borrows its Geoset's bytes and exposes version, vertex count,
all `StoredGeosetBlock` entries in authored order, optional TANG and optional
SKIN. Blocks retain tag, count, body and the exact MATS trailer. Their spans
tile the original geoset, preserving both UV sets where authored.

`TangentStream1800::bits(index)` returns the exact four u32 float bit patterns.
`SkinStream1800::get(index)` returns `SkinRecord1800 { joint_words: [u16;4],
weight_words: [u16;4] }`; `record_count()` bounds complete records by both the
authored word count and available bytes. Tangent/skin accessors return `None`
for records beyond either bound, including directly constructed public streams.
The stream's `word_count` retains the original header. All block/stream offsets
are header offsets within Geoset.data; body offset is header+8. File offset is
GEOS.payload_offset + Geoset.offset + 4 + stream.offset. No asset value is supplied
as a fallback. Missing optional streams remain absent.

Bad record widths/counts, truncated ranges, unknown tags and duplicate streams
return typed errors. `DuplicateGeosetTag` is the only new error variant.

## Tests and remaining semantics

The discriminating synthetic fixture includes joint and weight words above 255,
unnormalized weights, negative-zero/NaN tangent bits and two UV sets. Its source
bytes reconstruct exactly from stored blocks; the old mesh path refuses it.
The narrow-byte SKIN variant fails the v1800 reader. Tests also cover explicit
version gates, truncation, duplicate streams, out-of-range record access and
opaque container chunks without changing strict parsing.
Four additional tests explicitly reject matching duplicate VERS, conflicting
duplicate VERS, short VERS payloads (0 through 3 bytes) and oversized VERS
payloads (5 and 8 bytes). Classic-accessor acceptance of duplicates/oversized
records remains covered, without making a native rejection claim.

The ignored direct-CASC test requires explicit install and discovered HD asset
environment variables. It verifies identity, every geoset's counts/tiling, each
tangent bit/skin word against its input slice, and the strict mesh refusal.
Classic/SD fixtures remain regression checks, not native or render oracles.

Unestablished: tangent sign/handedness use, joint-word mapping into MATS/global
nodes, BPOS bind-pose semantics, GPU influence selection, weight normalization,
rounding, LoD choice, HD shaders and rendered/runtime/native fidelity. One next
probe for future integration is the exact-target v1800 weighted-skin consumer:
trace its SKIN joint lookup through MATS and BPOS and its integer-weight conversion
before constructing GPU bindings. No REA provider retry was performed here.

## Per-geoset measured counts

| Geoset | Vertices / TANG records | TANG file | SKIN file | SKIN u16 count | SKIN bytes |
| ---: | ---: | --- | --- | ---: | ---: |
| 0 | 6947 | `0x3b878` | `0x56ab0` | 55576 | 111152 |
| 1 | 58 | `0x8da90` | `0x8de38` | 464 | 928 |
| 2 | 682 | `0x93f5e` | `0x96a06` | 5456 | 10912 |
| 3 | 970 | `0xa2612` | `0xa62ba` | 7760 | 15520 |
| 4 | 944 | `0xb38be` | `0xb73c6` | 7552 | 15104 |
| 5 | 405 | `0xc01e2` | `0xc1b3a` | 3240 | 6480 |
| 6 | 384 | `0xc7782` | `0xc8f8a` | 3072 | 6144 |
| 7 | 272 | `0xcdb82` | `0xcec8a` | 2176 | 4352 |
| 8 | 2738 | `0xe6cce` | `0xf17f6` | 21904 | 43808 |
| 9 | 5532 | `0x12cb56` | `0x14251e` | 44256 | 88512 |
| 10 | 58 | `0x16e41e` | `0x16e7c6` | 464 | 928 |
| 11 | 4383 | `0x19075c` | `0x1a1954` | 35064 | 70128 |
| 12 | 32 | `0x1c4560` | `0x1c4768` | 256 | 512 |
| 13 | 3033 | `0x1db66c` | `0x1e7404` | 24264 | 48528 |
| 14 | 16 | `0x1ff53a` | `0x1ff642` | 128 | 256 |
| 15 | 578 | `0x204538` | `0x206960` | 4624 | 9248 |
| 16 | 341 | `0x20cdd0` | `0x20e328` | 2728 | 5456 |
| 17 | 157 | `0x2119e4` | `0x2123bc` | 1256 | 2512 |
| 18 | 814 | `0x219930` | `0x21cc18` | 6512 | 13024 |
| 19 | 556 | `0x225e60` | `0x228128` | 4448 | 8896 |
| 20 | 342 | `0x22e0d0` | `0x22f638` | 2736 | 5472 |
| 21 | 862 | `0x238658` | `0x23bc40` | 6896 | 13792 |
| 22 | 589 | `0x245950` | `0x247e28` | 4712 | 9424 |
| 23 | 359 | `0x24e49c` | `0x24fb14` | 2872 | 5744 |
| 24 | 386 | `0x254fb2` | `0x2567da` | 3088 | 6176 |
| 25 | 311 | `0x25b51c` | `0x25c894` | 2488 | 4976 |
| 26 | 229 | `0x260472` | `0x2612ca` | 1832 | 3664 |
| 27 | 380 | `0x265e02` | `0x2675ca` | 3040 | 6080 |
| 28 | 323 | `0x26c774` | `0x26dbac` | 2584 | 5168 |
| 29 | 233 | `0x271c10` | `0x272aa8` | 1864 | 3728 |
| 30 | 271 | `0x276854` | `0x27794c` | 2168 | 4336 |
| 31 | 245 | `0x27b6f2` | `0x27c64a` | 1960 | 3920 |
| 32 | 143 | `0x27f3c4` | `0x27fcbc` | 1144 | 2288 |
| 33 | 2685 | `0x296980` | `0x2a1158` | 21480 | 42960 |
| 34 | 2271 | `0x2c3480` | `0x2cc278` | 18168 | 36336 |
| 35 | 1613 | `0x2e6602` | `0x2ecada` | 12904 | 25808 |
