# Admitted map ground sheets

`pcx-tera::tile_texture_selectors` and `pcx-gfx::spawn_admitted_ground`
consume the admitted W3E corner grid and the BLP sheets named by the selected
`TerrainArt\Terrain.slk`. `peacecraft3::map_ground` installs the material and
runs its publisher after `MapRequestSet::Deliver`. It does not create a script
VM, simulation world, gameplay camera or playable chapter.

The publisher requires the successful admission's context, binding revision,
map identity and exact receipt arc, a matching `SelectedMapSources` reader,
and explicit `GroundSupply` read and texture budgets. `install` retains the
caller-supplied budget contract. The launcher uses `install_with_budget` to
supply its application policy after successful delivery and before publication:
16 MiB per source read, 128 MiB retained texture data and 64 MiB decode scratch.
These are resource ceilings, not source terrain values or original engine limits.
The policy follows the binding revision on replacement; it chooses no archive
or substitute tile. Native `replace_selected` and a file drop of
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

Cliffs, blight, water, terrain lighting, dynamic unit/building shadows, fog, gameplay camera activation,
world/session synchronization and complete terrain fidelity remain open.
Automatic ground preparation does not supply the other required campaign Host
inputs or make a refused initialization playable. The existing `--tile` viewer remains a
single-texture preview.

An explicit local source check also constructs Human01's draw from each selected
compatibility and Legacy archive set: the real W3E, Terrain.slk and six BLP
winners produce a 96×96 mesh and a 64×64×192 array with seven mip levels
(4,194,048 retained bytes). The two source sets have different map/BLP bytes;
the check records their archive identities and member fingerprints separately.
Those dimensions are observations, not production constants or release inference.
No original assets are written by that check or published with the code.

The automatic budget caller is checked through actual map delivery, including
changed binding revisions, cached updates, missing replacement textures and
read/texture budget refusal before publication. All five ground cases pass on
native and the actual Cargo WASM artifact. Native application suites (550 tests,
30 explicit skips), seventeen targeted WASM cases, strict native/WASM lint/builds
and formatting pass. Evidence: `.scratch/campaign-ground-budget-root-20261019/`.
This caller check adds no original-game or rendered campaign comparison.

## Shadow and colour comparison work (2026-10-27)

The ground publisher now reads `war3map.shd` from the admitted map receipt and
`FogOfWar/ShadowImageColor` from the selected `UI\MiscData.txt`. The grid retains
source row order and derives world bounds from the admitted W3E geometry. A
missing or malformed optional shadow input is reported separately through
`GroundPublication.shadow_error`; it does not invalidate an independently
admitted ground sheet. This does not supply dynamic unit or building shadows.

The scene and sky cameras now use the existing numeric-channel composition path
with tonemapping disabled. This addresses an unwanted colour conversion; it is
not a replacement for source terrain lighting or a claim of matching colours.

Comparison acceptance requires equal effective settings, not equal labels.
Low must match stock Low, High must match stock High, and enabled effects must
remain enabled in stock. Unsupported recreation settings remain reported as
unsupported; turning off the stock effect is not a fidelity fix. Pair captures
must identify the client/edition, map bytes, resolution, aspect, render scale,
gamma, quality controls, shadows, occlusion, camera, time of day and scene state.
A source-fallback preview is not an effective runtime setting.

The isolated stock baseline was saved at 1280×960, gamma 1.0, all five quality
controls High, unit shadows On and occlusion On. Public UI and saved registry
were both observed in `.scratch/shadow-fix-20261027/settings-original-3/`.
This is an observed baseline, not a production default. The original client ran
under isolated Wine/software rendering; native Windows parity is unmeasured.
Earlier captures with different settings or scene framing are diagnostic only.

Removing model draws from a diagnostic recreation capture removed the black
building patches while retaining the baked mask, isolating them to model
rendering. Under the previous JPEG rule every lower mip of `Textures\Base.blp`
decoded black (see `003-blp.md`, which also counts the other textures the
changed rule touches). A rendered capture after the decoder change was not
rerun in the verification below. Gameplay projection scale is covered by
`campaign-camera-projection.md`. Dynamic unit and building shadows,
ubersplats and terrain lighting remain open.

## Rerun on 2026-10-27

Headless, no display, one shared cargo lock, logs in `.scratch/verify/`:

- `cargo test -p pcx-tera`: 24 passed, 2 ignored. Ignored
  `human01_mesh_matches_its_terrain` (`PEACECRAFT3_FORMAT_INSTALL` set to the
  Legacy install): passed; it now compares every mesh position with
  `corner_world`, so it covers the elevation-layer term. Ignored
  `sweep_every_shipped_terrain`: 132 maps, 132 clean manifolds, 0 failures.
- `cargo test -p pcx-map`: 55 unit tests and the integration tests passed. The
  ignored original-input tests passed (`grid_alignment`, `local_pathing_regions`,
  both sweeps) except `campaign_item_sets`, which needs two extracted files
  (`PEACECRAFT3_CAMPAIGN_UNITS`, `PEACECRAFT3_CAMPAIGN_SCRIPT`) and was not run.
- `cargo test -p peacecraft3 --lib --features test-fixtures map_ground`: 5 passed,
  including the shadow grid orientation, value and device-limit test.
- `cargo test -p pcx-gfx -p pcx-objects -p pcx-play --lib`: 69, 73 and 2070 passed.

Not rerun: the GPU readback and capture tests, the WASM builds, the full
application suite quoted above, and any comparison with the original client.
The terrain layer term and the shadow-mask composition therefore have unit and
original-map mesh coverage here but no new rendered comparison.
