# Water input and geometry evidence

Water settings exist in the supplied assets. The shipped-byte notes below do not
by themselves prove a native water function. The geometry implemented in
`WaterData` is a labelled recreation of WarsmashModEngine commit
`f9e0aeed4be372d6016519d0e97b384aa873f374` (`Corner.load` and
`SoftwareWaterAndGroundMesh`), placed in this crate's measured `world_z` scale.
It is not a claim that those native internals were re-traced.

`WaterData` still exposes original corner records and explicit source/edition
provenance. Presence is the Warsmash water flag after this parser's nibble
shift. Level and mesh require a caller-selected `Water.slk` row: either an
explicit `waterID` or the W3E tileset byte plus the Warsmash suffix `Sha`.
The row's `height` cell is parsed and scaled by `TILE_XY`. A missing row,
unparsable height, or exhausted caller budget is an error. A dry grid is an
empty mesh, not a stand-in plane. Out-of-grid `water_level` returns `Ok(None)`;
that does not mean dry ground. Colour, texture frames, opacity, and the
animation clock stay raw cells. `Unsupported::WaterLevel` and
`Unsupported::WaterGeometry` remain named variants and are not returned by
this path.

## Shipped-byte observations

The local inputs were `wc3/game/app` (the existing vanilla comparison install)
and `/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)`.
The latter was checked with the explicit source label used by the existing world
import evidence: executable VERSIONINFO `1.29.2.9231`; archive adjacency alone is
not proof of executable/asset edition equivalence. The environment variable
`PEACECRAFT3_LEGACY_DIR` was unset during this check, so that documented local
Legacy directory was selected explicitly.

* `TerrainArt\Water.slk`, resolved from `War3x.mpq` in both installs, is byte
  identical: 12,863 bytes, SHA-256
  `8c5b8580da80d00380db8c144ae8eb64730a07975de99eb11fa3222639bdf699`.
  The existing SLK parser reports 19 rows **including the header**, 51 columns,
  and 969 cells. This establishes 18 authored data rows, not 19 water types.
  Header-string byte offsets within the extracted payload include:

  | Header | Offset |
  |---|---|
  | `waterID` | `0x24` |
  | `height` | `0x35` |
  | `texFile` | `0x59` |
  | `numTex` | `0xab` |
  | `texRate` | `0xbc` |
  | `Smin_A` | `0x268` |
  | `Dmax_B` | `0x367` |
  | `version` | `0x378` |

  The table also names `impassable`, `mmAlpha/mmRed/mmGreen/mmBlue`,
  `texOffset`, `alphaMode`, `lighting`, motion/range fields, shoreline fields,
  and the `Smin/Smax/Dmin/Dmax` A/R/G/B columns. These are authored settings.
  No values from those records are embedded into this crate. Column names alone
  do not establish their W3E join, texture suffix/sampler rule, or world units.

* `UI\MiscData.txt` is identical in the two archive sets: 9,954 bytes, SHA-256
  `d097d3cbb4a03719a97591900b45ab42fdb847385819c3d5e7dd7b47d7d9b516`.
  Its `[Water]` section starts at `0x1b4a`. Authored comments at `0x1b6a`
  describe water-plane vertex colour as dependent on distance to the ground,
  using shallow/deep depth ranges and corresponding `Water.slk` colours.
  Authored keys occur at `MinDepth=0x1cdc`, `DeepLevel=0x1ce9`,
  `MaxDepth=0x1cf7`, `DeepLevelPathing=0x1d7b`, and `WavesDepth=0x1e2b`.
  The last two have comments describing pathing/placement depth and wave depth.
  This establishes that depth settings and a described colour relationship
  ship. It does **not** supply the W3E water height, occupancy mask, or prove
  executed renderer/pathing behavior. No depth values are copied into code.

* `TerrainArt\Terrain.slk` is identical in the compared payloads: 50,162 bytes,
  SHA-256 `ea046ed80be44c984917b0a284fe91144d57e40083ecfcf847c510b3d44cd3ce`.
  Its 178 rows including the header and 14 columns name ground `tileID`,
  `cliffSet`, texture identity and tile properties. Its headers contain no
  `waterID` or water-height column. This bounded header check does not exclude
  a native rule joining terrain identity to the separate water table.

* Legacy `Scripts\common.j` declares `SetWaterBaseColor` at token offset
  `0x27eef`, `SetWaterDeforms` at `0x27f62`, and `PATHING_TYPE_FLOATABILITY`
  at `0x4b03`. The comparison install's token offsets are `0x275fe`, `0x27671`
  and `0x4454`. These are declarations, not native implementations or W3E masks.
  `Scripts\Blizzard.j` contains the colour wrapper name at `0x16f20` in
  Legacy and `0x16adf` in the comparison install. Its wrapper converts the
  supplied percentages before invoking the native colour setter; it does not
  decode W3E water geometry.

* `UI\WorldEditData.txt` names deep/shallow water editor brushes. The Legacy
  entries begin at `0x6da1` and `0x6dec`; comparison entries at `0x6d21` and
  `0x6d6c`. They establish editor brush identities, not corner flag meanings.
  No water-name matches were found in the compared `Units\MiscData.txt`
  payloads; this is a bounded text search only.

* `Maps\Campaign\Human01.w3m` in each archive set contains identical
  `war3map.w3e` bytes: version 11, 65,932 bytes, raw FNV-1a64
  `fbddbd5029267caa`. The 9,409 seven-byte corner records start at `0x45`
  and consume the payload exactly. The first second-word field is therefore
  at `0x47`, and its first slot/flags byte at `0x49`. Every original record was
  checked against `Corner::position_word`, `corner_flags`, `byte5` and `byte6`;
  all remained verbatim. Set-bit counts were collected without assigning a
  water meaning to any bit. This selected map's `war3map.j` contained neither
  water-setter token. Neither observation generalizes to all maps or proves
  that this map is dry.

## Existing format/native evidence and its limits

`crates/pcx-map/src/lib.rs` documents the W3E reader chain ending at
`0x6f740610` and writer chain ending at `0x6f740770`, with a seven-byte
corner record. It labels +2 as an opaque word, +4 as slot/flags, and +5/+6
as opaque bytes. The current parser folds no field into a water semantic.
This investigation reproduces byte framing; it does not independently certify
the native-code field meanings described by that earlier source documentation.

The comparison install's `game.dll`, SHA-256
`6d21bd9a0f9fbc8446f455c9e89ac994fed68174426fe608fcb9baefd4dec53c`,
contains string tokens at **file offsets**, not function addresses:
`SetWaterDeforms=0x9432dc`, `SetWaterBaseColor=0x9432ec`, `waterID=0x97c704`,
and the RTTI name `.?AUWaterLevelInfo@@=0xaa1a04`. These only establish strings
in that binary. They do not establish a structure layout, code path, or 1.29b
runtime behavior.

No fresh native disassembly or runtime observation was obtained. REA MCP was
unavailable in this session. No new `ev_` identity is claimed. A future decisive
native investigation would follow the second-word and flag-byte destinations
in the matching target's corner reader, then their water-height/presence consumers
and the `waterID` table lookup. Earlier-build addresses must not be transplanted
into the target build without establishing its identity and corresponding code.

## Geometry helper and validation

`cliff_wall` accepts caller-supplied x/y/z endpoints and a finite nonnegative
step threshold. A step strictly greater than that threshold emits a vertical
rectangle between the endpoints' minimum and maximum z. `cliff_walls` applies
that construction to a caller-supplied tile perimeter. Shared edges must be
enumerated once by a whole-grid caller. There is no implicit threshold, world
scale, UV/material rule or native wall-placement claim. Existing `CliffRule`
and `CliffThreshold` refusals remain intact.

Synthetic tests check raw reserved-bit preservation and record offsets,
provenance requirements, dry/wet/non-flat water meshes, missing profiles,
budget limits, threshold boundaries,
perimeter order, triangle winding/normals, flat edges, reversed edges and
invalid/nonfinite geometry. These prove API behavior, not original rendering.

The explicit original test requires three environment variables and fails when
they are missing; it is ignored during ordinary unit runs. It derives counts,
offsets and digests from the local inputs, with no golden asset values in code.
Each of the two local invocations passed one test. Native and WASM builds use
the workspace's denied-warning policy.

```sh
CARGO_TARGET_DIR=/tmp/pcx-tera-target cargo test -p pcx-tera --locked
CARGO_TARGET_DIR=/tmp/pcx-tera-target cargo build -p pcx-tera --locked
CARGO_TARGET_DIR=/tmp/pcx-tera-target cargo build -p pcx-tera --locked --target wasm32-unknown-unknown

# Set these to an explicitly identified local original install and map member.
PEACECRAFT3_WATER_INSTALL="$install" \
PEACECRAFT3_WATER_EDITION="$edition" \
PEACECRAFT3_WATER_MAP='Maps\Campaign\Human01.w3m' \
CARGO_TARGET_DIR=/tmp/pcx-tera-target \
cargo test -p pcx-tera --locked --test local_water -- --ignored --nocapture
```

Only summaries and this interpretation boundary are recorded. Original payloads,
extracted files, and generated copies of their contents remain local.

## Current geometry integration

The current geometry path borrows the existing parsed SYLK table and selected
row; it adds no parser or copied-row allocation policy. Vertex and index caps
apply before mesh storage is reserved. The in-memory Human01 terrain check
passes with the explicitly selected compatibility install. Synthetic checks
cover ambiguous rows, non-finite or overflowing table heights, independent
vertex/index caps, masked height words and non-flat wet tiles. Native and actual
WASI executions are recorded in `.scratch/ground-water-root-20261016/`.

The separate proposed ground material is not integrated. Its packed-atlas
nearest sampling differs from the pinned reference's filtered, isolated cells
and mipmaps. Ground texture predecode limits, that sampling behavior, water
materials, cliffs, ramps and blight remain work for the renderer. Geometry
checks establish none of those rendered results or native visual parity.
