# Version-1800 GEOS trailer

## Shipped-byte observation

The explicit TVFS path `war3.w3mod:units/human/footman/footman.mdx` in the
local CASC install reports build `3.0.0.24268`. The verified decoded file is
105,602 bytes, CKey `3bd0831fad186c0c08767f15a9663e4b`, EKey
`50f7f9185775e0e6f3b1cf18646e2882`. Its VERS word at file `0x0c` is **1800**.
These are provenance measurements, not decoder constants or fixture expectations.
No original contents are stored here.

The scratch-only CASC probe reads the named path through Storage's checked
encoding/TVFS/content chain, retains the MDX only in memory, and reports cursor
arithmetic. Top-level chunks, in order:
`VERS MODL SEQS MTLS TEXS GEOS GEOA BONE HELP ATCH PIVT EVTS CLID`.
GEOS payload is `[0x0d18, 0x6e2a)`. Geoset 0's data begins at `0x0d1c`.

| Geoset-0 field | Geoset.data offset | File offset |
| --- | --- | --- |
| MATS header | `0x3333` | `0x404f` |
| End of 54 matrix indices / materialId | `0x3413` | `0x412f` |
| LoD signed word, after selectionFlags | `0x341f` | `0x413b` |
| Inline name, exactly 80 bytes | `0x3423` | `0x413f` |
| Bounds radius after the name | `0x3473` | `0x418f` |
| Sequence extent count, authored 13 | `0x348f` | `0x41ab` |
| Sequence extent end / UVAS | `0x35ff` | `0x431b` |
| UVBS | `0x3607` | `0x4323` |
| Exact geoset end | `0x4407` | `0x5123` |

The classic reader mistakes a name word at trailer+40 for the sequence count
(zero in this asset) and advances to `0x415b`, still inside the name. Its next
tag is therefore `[0,0,0,0]`. The first semantic divergence is the radius read
at `0x413b`, which is actually the LoD word. The fixed insert is **4 + 80 bytes
after the three selection/material words**, before the existing extent. It is
not padding after the whole trailer. All five geosets tile exactly with that
insert; their file ends are `0x5123`, `0x5b49`, `0x6171`, `0x64b7`, `0x6e2a`.
There are no TANG or SKIN sub-blocks in this named SD asset.

## Primary format cross-check

The independent primary parser source is
[mdx-m3-viewer Geoset.readMdx](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/parsers/mdlx/geoset.ts).
That pinned implementation places a signed LoD word and an 80-byte name after
selection flags for newer models, followed by the extent, its sequence count
and 28-byte extent records. Its optional TANG/SKIN handling also distinguishes
newer SD models without those streams. This corroborates the observed byte
arithmetic; no native v1800 reader or LoD selection behavior was probed.

[Texture.readMdx](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/parsers/mdlx/texture.ts)
has the same leading word, 260-byte path and trailing word as the existing
268-byte TEXS parser. The three authored TEXS records tile this file's chunk.
Existing raw slot/flag interpretations remain unchanged.

The same revision's newer material reader expects a shader-name field that
this v1800 asset does not have at that location. It cannot establish v1800
material semantics. The classic checked layer parser instead fails at
Material.data+44 (file `0x880`, layer+28: emissive data mistaken for a track).
The classic `materials()` accessor retains its typed
`UnsupportedVersionedChunk { version: 1800, chunk: "MTLS" }` refusal. The
separate `materials_v1800()` accessor now checks stored layer/subtexture framing;
see [REFORGED_MTLS.md](REFORGED_MTLS.md). Shader/render semantics remain open.

## API and limits

`Mdlx::mesh` selects only the established v800 and v1800 layouts; other versions
return `UnsupportedVersion`. The v800 fixed trailer, sequence arithmetic and
native-evidence semantics retain their existing path.

`Mesh::lod: Option<GeosetLod>` is None for v800. For v1800 it carries:

- `version`: the parsed MDX version, not an install edition inferred from a path.
- `offset`: the signed LoD word's offset within `Geoset.data`.
- `level`: the authored i32, without choosing a level or inventing its effect.
- `name`: all 80 authored bytes, including NULs and any bytes after them.

The corresponding file offset is GEOS.payload_offset + Geoset.offset + 4 +
lod.offset. The enclosing Geoset retains its source bytes. TANG and SKIN return
`UnsupportedGeosetFeature { version, at, tag }`, with `at` relative to that
Geoset.data; they are never discarded or silently converted to classic skinning.
TEXS records are parsed; importing referenced images is outside this crate.

Synthetic tests use a non-zero, non-UTF8, unterminated LoD name containing a
false UVAS marker. That fixture fails the classic reader and distinguishes the
position and full width of the insert. They also cover classic field equality,
truncated LoD, unknown versions and explicit HD/material refusals. The ignored
direct-CASC fixture requires explicit install and authored asset environment
variables, imports original content only in memory, and reports parsed counts.
Parsing does not establish materials, LoD selection, animation/runtime, rendered
appearance or native parity. The separate geometry-only graphics gate verifies
five geosets and 582 imported Bevy mesh vertices for this supplied asset.
