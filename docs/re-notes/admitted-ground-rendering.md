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
The shader uses implicit-derivative `textureSample`, not a forced LOD. Supplied
BLP mip pixels are split through the common available cell mip range, capped
at one texel per cell. Base-only sheets stay base-only; no replacement mip
pixels are generated. Mip dimensions that cannot preserve the cell grid refuse.
Native terrain filtering, LOD selection and far-LOD atlas behavior remain
unqualified. Height conversion reuses `corner_world`; positions
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
slots. CPU image tests cover authored mip pixels, base-only odd cells, mixed mip
ranges, malformed chains, size budgets and atomic
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

## Terrain-light omission qualified (2026-11-03)

The world color-path audit finds that the ground texture array already uses
numeric `Rgba8Unorm`, and scene cameras already use numeric composition. The
larger omission is lighting: `ground_mesh` uploads positions, UVs and selectors,
while `ground.wesl` samples and blends textures/shadows without a normal or
terrain-light input. `TerrainMesh` already derives corner normals from source
geometry, but this ground draw does not consume them. This source inspection
identifies a missing path; it is not original-client rendered evidence.

The new explicit CPU-only fixture
`campaign_entry_gpu_tests::selected_terrain_lighting_source_receipt` runs the
selected chapter's existing minimal startup host, records its executed day/night
pair and tests the existing sun sampler against both bound models. It opens no
display, GPU or audio backend. Its output requires a new caller-selected capture
directory, and original-derived data stays local.

The Prologue02 RoC run passes. Both the selected terrain and unit model contain
one unparented directional light with a fixed rotation plus animated diffuse
and ambient colors; the existing `BoundModel::sun` supports both. Their source
ambient settings differ, so substituting the unit light for terrain would lose
authored data. The admitted terrain has 12,513 corner normals, 11,241 of them
non-flat by exact component comparison; all are finite. These counts are local
observations, not production constants or performance thresholds.

The terrain model SHA-256 is
`7655f5b6078dabd6d445f67d49c5fccd47a86eb5ba583c32783ba262edac919a`;
the unit model SHA-256 is
`a5736241e6e11a4624f163f53c7aa44dd13ac577aec5a5860f90b0e435efed49`.
The receipt retains script spelling, selected archive provenance, edition,
version, sample time and complete sampled light values at
`.scratch/terrain-light-source-20261103/terrain-light-source.json`.
The process log and strict native all-target Clippy pass are under
`.scratch/terrain-light-qualification-20261103/`.

Next: connect the selected terrain model and canonical day fraction to the
admitted ground draw, preserve source coordinate relationships and interpolate
source-normal-based lighting consistently with the observed renderer. Reuse the
existing sampler and channel policy where qualified. Missing/unsupported models
must be reported, not replaced with guessed white light. The terrain-normal
algorithm, native light submission/packing and rendered parity remain unverified;
CPU source sampling does not establish them. No production shader or lighting
behavior changed in this qualification step, so README images remain unchanged
at 988,243 exact full-frame differences. Browser and native-client rendering were
not run for this step.

## Selected terrain sun connected (2026-11-03)

The campaign now binds the executed terrain day/night model and canonical day
fraction to its admitted ground material. It checks that the script session and
ground publication share the same admitted context. Source light sampling and
packed-channel conversion reuse the existing day/night and unit-light paths;
no light direction, intensity, color, clock or scale is fitted to an image.
Source parameter reads (including failures) are cached per context. Repeated
identical material/sun bindings do not dirty material assets. Missing or
unsupported inputs report a persistent diagnostic and warn only when that
error changes; an existing draw/binding is retained rather than inventing a sun.

`corner_normal` derives a finite-difference normal from the same source corner
positions and clamped neighbors used by the terrain mesh. The ground draw now
uploads those normals, rotated through the existing game-to-Bevy axis mapping.
Allocation is checked before normal-vector construction; invalid normals refuse
before asset publication. This adds one normal per emitted vertex. Lighting is
computed at vertices and interpolated across triangles, then modulates numeric
texture channels before the existing shadow and boundary passes. Normals and
mesh topology remain static after upload; only explicit light uniforms change.
An unbound material retains the generic draw's previous unlit behavior.

This is a source-bound recreation implementation, not native terrain-normal or
packing proof. The native normal algorithm, precise color quantization,
interpolation and shadow/light pass ordering still require rendered
qualification. The implementation does not replace the selected terrain model
with the distinct unit model, introduce an ambient floor or use PBR defaults.

Validation passes:

- Synthetic corner-normal equivalence and invalid-coordinate check; the explicit
  Prologue02 source receipt also compares every corner normal with its terrain
  mesh counterpart (`corner_normal_matches_mesh: true`).
- Four ground-material/unit checks, five ground integration checks (one GPU
  check is explicitly ignored in that ordinary run), and nine existing menu
  lighting checks (one explicit original-input ignore).
- The isolated Vulkan ground readback, now including opposite light directions
  to distinguish diffuse-plus-ambient from ambient alone, plus restoration to
  unbound rendering before its existing sampling/mask checks.
- Strict native/WASM Clippy for all affected crates, native/WASM application
  builds, and the final headless/muted mounted campaign capture. The capture
  requires an actual terrain-light submission as well as an empty error state.

Logs are under `.scratch/terrain-light-render-20261103/`; the new source-normal
receipt is `.scratch/terrain-light-source-connected-20261103/`. No original
client was launched during this change. The final capture is
`.scratch/pixel-terrain-light-final-20261103/`, including full diff, mask,
overlay and side-by-side artifacts. Its selected frame matches the initial
lighting capture byte for byte. All thirteen acquisition clock receipts hold,
and same-checkpoint recreation repeats match byte for byte.

Every RGB pixel of the unregistered 1280x960 frame remains in the comparison:
exact differences fall by 36, from 988,243 to 988,207 of 1,228,800. Mean absolute
channel error falls from 12.803221571180556 to 12.529151746961805. The change
modifies 533,316 RGB pixels within [0,33,1280,751]. All 2,640 upkeep diagnostic
pixels still match. No registration, resizing, tolerance or excluded pixels are
used. Five README screenshots were reviewed and refreshed, and provenance now
covers the changed ground/terrain libraries plus the new campaign binding.
Complete world/UI fidelity and native tick/animation/RNG identity remain open;
browser rendering and other native graphics backends were not exercised.

## Authored terrain mip correction (2026-11-03)

Shipped-byte evidence: the selected RoC Prologue02 terrain sheets contain nine
or ten BLP levels, with seven separable cell levels. The explicit local-source
`selected_terrain_authored_mips_reach_renderer` check found 777,076 differing
channels in the old upload. It now finds zero. The source receipts, edition,
paths and dimensions remain local in `.scratch/terrain-mips-after-20261103`;
no decoded source textures are tracked.

The renderer splits the supplied mip pixels into each variation layer, using
the common available range through one texel per cell. It preserves base-only
inputs, rejects noncontiguous/excessive chains and inseparable cell grids, and
budgets the actual retained prefix. It reuses the existing bounded mip decoder.
Synthetic checks cover authored pixels that differ from base averages, odd
base-only cells, mixed ranges, malformed/truncated chains and pre-decode budget
refusal. Four unit tests and seven integration/GPU tests pass. Strict native
and WASM Clippy and both application builds pass.

The isolated full production capture in `.scratch/pixel-terrain-mips-20261103`
passes. Against the unchanged original reference, exact whole-frame differences
fall from 988,207 to 988,203 / 1,228,800 and MAE from 12.529151746961805 to
12.52657958984375. The renderer change affects 70,838 pixels in [0,33,1280,299].
All 13 capture clocks hold, repeated selected frames are byte-identical, and all
2,640 upkeep diagnostic pixels remain exact. Full diffs, overlays and side-by-side
images were reviewed; README captures and hashes are refreshed.

This establishes preservation of supplied mip pixels, not native terrain LOD,
filter selection or atlas behavior below one texel per cell. Native normal,
lighting pass order and original animation/RNG identity remain open. No new
dependency or screenshot-derived layout value was introduced.
