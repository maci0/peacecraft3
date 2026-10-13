# MDX material layers

## Native evidence

Fresh REA analysis of the local Legacy World Editor, SHA-256
`d198d2ca47fc29d4901f3de27ff17619824afef5af7e42b9ff17cbe03723d7b7`:

- Material reader `0x68fe90`, `ev_f37b525e5b4ba2d5abcd0de0ae6922035bcab4997085fd9c8a0cd2bc80a7bef2`,
  reads priority plane and material flags, then `LAYS` and its count, calling the
  layer reader once for each declared layer. This word is a count, not an index.
- Layer reader `0x68fdb0`, `ev_3a81e18f3e9583faade57a7d5b621c997ae6bfa687dc90328078b0ed208cc443`,
  reads an inclusive size, five integer fields, then a float. Thus static alpha
  is layer-record+24, or first `LAYS`+32. The preceding word is coordinate id.
- Layer keyword reader `0x68f4b0`, `ev_452c56a2fab8fe0e5ac3a742b1ad3d05d769d842f4839b106f06e18040745b62`,
  binds `Alpha` to the float destination, `CoordId` to the fifth integer,
  `FilterMode` to the first integer, and shading flags to the second integer.
  Keyword identities were derived from that executable's token/pointer table,
  seeded by its `GeosetId` string. They are not taken from the newer PDB.
- Filter parser `0x68f170`, `ev_3c35e2506a18e41964d24ee2fe052deb3b6ca8b18aa062754281cac9bf139677`,
  maps None=0, Transparent=1, Blend=2, Additive=3, AddAlpha=4, Modulate=5,
  Modulate2x=6. These are format/algorithm identifiers, not authored materials.
- `KMTA` reader `0x68e2c0`, `ev_eedfce4d09de3781c37c4d327e2c7fb671b6cb94740549b245c6eafd1eb82c7b`,
  uses scalar floats and appends tangents for interpolation types above one.
  `KMTF` reader `0x68e3c0`, `ev_08f9a2b910532c1600017dfbe1660f769934c53bc43e384d98796e02615a3894`,
  uses signed times and integer texture references at an eight-byte key stride,
  without tangent fields regardless of the stored interpolation kind.

## Implementation and caller handoff

`Material::checked_layers()` validates every group count, inclusive record and
track. `Mdlx::materials()` performs this validation before returning materials.
Legacy `tex_layers()` remains a compatibility projection of the decoded layers;
manual constructors should call the checked accessor to obtain typed errors.

`MaterialLayer` exposes each layer's properties, alpha tracks and texture-frame
tracks. `TexLayer.index` is now the derived ordinal; it is not read from the
`LAYS` count. Alpha is parsed at its proven offset. `filter_mode`,
`shading_flags`, `coord_id`, material priority and material flags are authored.
The older `shader_id` and `blend_mode` names retain their historical raw values
for compatibility; those words are actually filter mode and shading flags.
Callers must switch to the correctly named fields and render every layer.

The Legacy corpus contains 7,860 decoded layers in 7,057 materials, recovering
803 secondary layers omitted by the tag-scanning accessor. There are 161 layers
with texture-animation references, rather than the earlier first-layer-only 152.
The initial full model sweep after this correction passed 53 tests and its gated
original-binding check. Further synthetic material/PRE2 checks are being added.
These counts are measurements, not assertions hard-coded into the decoder.

No rendered blending, texture-frame switching, ordering or alpha oracle was run.
Native reader proof and exact corpus framing establish parsing, not rendering.
