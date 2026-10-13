# Version-1800 stored MTLS framing

## Checked source and cursor arithmetic

The explicit CASC/TVFS asset is
`war3.w3mod:units/human/footman/footman.mdx`, from install build `3.0.0.24268`.
The checked decoded file is 105,602 bytes, MDX version 1800, with CKey
`3bd0831fad186c0c08767f15a9663e4b` and EKey
`50f7f9185775e0e6f3b1cf18646e2882`. These are provenance observations, not
decoder constants or real-fixture count assertions. Original content stays local
and is imported only in memory.

MTLS has 404 payload bytes at `[0x850,0x9e4)`. Its self-inclusive material
records and layers tile as follows:

| Material file offset | Inclusive bytes | Layer file offsets / inclusive bytes |
| --- | ---: | --- |
| `0x850` | 88 | `0x864` / 68 |
| `0x8a8` | 156 | `0x8bc` / 68; `0x900` / 68 |
| `0x944` | 160 | `0x958` / 140 |

Each material has priority and flags after its size word, then `LAYS` and
its authored layer count; this file has no 80-byte material shader-name field.
The count is a count, not a layer identifier. Each layer starts with its own
self-inclusive size. Its layout relative to that size word is:

| Offset | Stored field / width |
| --- | --- |
| `0x04` | filter word / 4 |
| `0x08` | shading flags / 4 |
| `0x0c` | legacy texture word, retained unchanged / 4 |
| `0x10` | texture-animation word / 4 |
| `0x14` | coordinate word / 4 |
| `0x18` | alpha / f32 |
| `0x1c` | emissive gain / f32 |
| `0x20` | fresnel color / 3 f32 |
| `0x2c` | fresnel opacity / f32 |
| `0x30` | fresnel team-color field / f32 |
| `0x34` | shader identifier / u32 |
| `0x38` | subtexture count / u32 |
| `0x3c` | subtexture records: texture word + stored second word / 8 each |

A subtexture may be followed immediately by a `KMTF` track. After all declared
subtextures, layer-level tracks continue to the inclusive layer end. In this
asset each layer has one subtexture. The last layer's `KMTA` starts at `0x99c`
(layer+`0x44`), with seven linear scalar keys. The equality
`60 + 8 + 16 + 7*8 == 140` closes that layer exactly at `0x9e4`; the other
layers close with `60 + 8 == 68`.

The old error `chunk: "LAYS", at: 44` used **Material.data-relative** offsets.
For material 0 it is file `0x854 + 44 == 0x880`, or layer+28: the emissive
field mistaken for a track tag. It is not literal LAYS-tag+44. This resolves
the material-framing blocker recorded in REFORGED_GEOS.md without changing
that note's separate classic-accessor refusal.

## Primary implementation cross-check

Primary source:
[WhiteoutLib parser.cpp, pinned commit 92887f9](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/mdx/parser.cpp).
That implementation explicitly reads through version 1800. `parseMaterial`,
`parseLayer` and `readTrackChunk` corroborate this field order, the removed
material shader name, subtexture records, optional KMTF and float-track widths.
Their generic integer reader consumes time/value pairs regardless of stored
interpolation, whereas cubic float keys carry both tangents.

Only stored framing is adopted. Its rewriting of the legacy texture word,
replacement of stored subtexture semantics with ordinals, interpolation
normalization and unknown-track skipping are deliberately not applied.
This is primary implementation evidence plus checked shipped-byte arithmetic,
not exact-target native-code or render proof.

## Additive API and preservation

The existing `Material`, `MaterialLayer`, `TexLayer` and classic methods retain
their shapes, signatures and behavior. The v800 parser implementation is
unchanged. `Mdlx::materials()` still refuses v1800 so its callers cannot silently
project newer fields into classic material semantics.

Use the explicit new API:

```rust,ignore
let model = pcx_mdl::Mdlx::parse(bytes)?;
for material in model.materials_v1800(bytes)? {
    let layers = material.checked_layers_v1800()?;
    // Consume stored fields explicitly; shader/render interpretation is separate.
}
```

`materials_v1800` checks the parsed version and validates inclusive records and
layers before returning raw-backed `Material` entries. Other versions refuse
this accessor. `Material.data` retains every original record byte except its
inclusive size word, retained in `Material.len`.

`MaterialLayer1800` exposes the original classic prefix separately from the
six extra floats, raw shader id, ordered subtextures and ordered layer tracks.
The prefix's legacy `texture_id` is **not** substituted for a subtexture's
binding. `SubTexture1800` retains the texture id, raw `stored_semantic` word and
optional track; vector position preserves ordinal without replacing that word.
No missing field is filled with a default.

Layer, subtexture and track offsets are relative to `Material.data`. The file
offset is `MTLS.payload_offset + Material.offset + 4 + stored_offset`.
`MaterialTrack1800` retains tag, count, interpolation, global-sequence word,
key stride and exact raw keys. KMTF keeps its authored interpolation header and
8-byte integer key width; no stepped/render behavior is asserted by parsing.

Known layer track framing is KMTA/KMTE/KFC3/KFCA/KFTC/KMTF. Unknown track tags
return `UnknownTrackMagic` with their exact Material.data offset, rather than
guessing a width. Invalid counts, record extents or interpolation kinds also
return typed errors before allocation/indexing. The source bytes remain the
caller-owned input, and raw-backed records preserve any uninterpreted fields.

## Validation boundary

Synthetic records have distinct non-default floats, unrelated legacy/subtexture
ids, unknown raw shader/semantic words, multiple subtextures, an inline integer
track with a cubic header, scalar/vector cubic tracks, and malformed framing.
The discriminating valid fixture fails `checked_layers()` and succeeds through
the explicit v1800 reader without changing its bytes.

The ignored direct-CASC fixture requires explicit install and authored path
environment variables, verifies content identity, checks every fixed field/key
against its original input slice and reports exact tiling/counts. Both classic
corpus sweeps remain separate regression evidence. None proves shader selection,
texture-slot role, blending, fresnel/emissive appearance, animation evaluation,
runtime/native fidelity, image resolution/import or rendered parity. Those
semantics require a separate exact-target/caller scope; no render defaults or
projection are supplied here. TANG/SKIN remain outside this material reader.
