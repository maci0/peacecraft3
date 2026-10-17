# Admitted map ground sheets

`pcx-tera::tile_texture_selectors` and `pcx-gfx::spawn_admitted_ground`
consume the admitted W3E corner grid and the BLP sheets named by the selected
`TerrainArt\Terrain.slk`. `peacecraft3::map_ground` installs the material and
runs its publisher after `MapRequestSet::Deliver`. It does not create a script
VM, simulation world, gameplay camera or playable chapter.

The publisher requires the successful admission's context, binding revision,
map identity and exact receipt arc, a matching `SelectedMapSources` reader,
and explicit `GroundSupply` read and texture budgets. There is no default
archive, budget or substitute tile. Native `replace_selected` and a file drop of
a resolver-backed binding publish that reader with the exact receipt arc. A
binding that did not retain its resolver does not. Missing inputs report a
ground error. A failed replacement keeps
the previous mounted entity, source context and catalog/BLP receipts. A
successful replacement retires the previous entity. Asset handles are inserted
only after selectors, geometry and every texture decode succeed.

Attempt deduplication includes the caller's read and texture budgets. Correcting
an insufficient budget retries the same admitted map; unchanged inputs do not
rebuild it each update. Temporarily removing a required budget preserves the
mounted draw and reports the missing input; restoring it permits publication.

## Evidence and profile

W3E framing, ordered slot fourccs and corner words are shipped-byte evidence
already described in `terrain-preflight.md` and the `pcx-map`/`pcx-tera` module
documentation. The variation and corner-mask composition uses the public
Warsmash loader at commit `f9e0aeed4be372d6016519d0e97b384aa873f374`:

- [Corner.java](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/parsers/w3x/w3e/Corner.java)
- [Terrain.java](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/w3x/environment/Terrain.java)
- [GroundTexture.java](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/w3x/environment/GroundTexture.java)
- [TerrainShaders.java](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/w3x/environment/TerrainShaders.java)

This is a reconstruction-source profile, not observed Warcraft III execution.
Slot indices come from the corner nibble; variation comes from `byte5 & 0x1f`.
The lowest slot uses the bottom-left corner's variation with that slot's own
sheet shape. Later slots use the source corner-mask order. Out-of-range slots
refuse instead of clamping to slot zero. `GroundTiles::blend` continues to
refuse numeric blend weights; the draw composes the authored sheet alpha.

Each four-row sheet has four or eight columns. Its cells become isolated array
layers; the right half of an eight-column sheet starts at layer 16. The sampler
uses clamp-to-edge and linear magnification, minification and mip filtering.
The shader uses implicit-derivative `textureSample`, not a forced LOD. CPU box
mips keep odd-sized parent spans, so an edge texel is not dropped. That mip
filter is an implementation choice; equality with the original driver's mip
filter remains unmeasured. Height conversion reuses `corner_world`; positions
map from source `(x,y,z)` to Bevy `(x,z,-y)`.

The header, grid shape, full retained mip bytes, transient RGBA/cell scratch,
and available GPU texture dimension/array limits are checked before pixel
decode. Non-finite geometry refuses before publication. `GroundMemory` bounds
retained texture bytes and decode scratch separately; it is not a total bound
on compressed source buffers, meshes, allocator overhead or GPU copies. The
four-bit slot format bounds the number of sheets. The resolver's per-read
limit bounds each selected member's returned bytes.

## Verification boundaries

Synthetic native/WASI tests cover selector order, variation mapping and invalid
slots. CPU image tests cover separate cells, odd mips, size budgets and atomic
refusal. Registered application tests drive the actual configuration reader
and map-delivery path, then check successful publication, failed replacement,
stale revision and missing reader/budget behavior.

An explicit local Vulkan readback uses the production material for a large
full-UV quad and a one-pixel full-UV quad. It checks the green cell edge, red
center and the cell's minified average without blue from its neighbour. A
forced-base-level sampler mutant fails the minification pixel assertion; the
source is restored afterward. This validates the Bevy pipeline on that
adapter. It does not establish native terrain appearance.

Cliffs, blight, water, terrain lighting, shadows, fog, gameplay camera activation,
world/session synchronization, automatic launcher budget admission and complete
terrain fidelity remain open. The existing `--tile` viewer remains a
single-texture preview.

An explicit local source check also constructs Human01's draw from each selected
compatibility and Legacy archive set: the real W3E, Terrain.slk and six BLP
winners produce a 96×96 mesh and a 64×64×192 array with seven mip levels
(4,194,048 retained bytes). The two source sets have different map/BLP bytes;
the check records their archive identities and member fingerprints separately.
Those dimensions are observations, not production constants or release inference.
No original assets are written by that check or published with the code.
