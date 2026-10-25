# Campaign sky dome

## Correction and own-light connection (2026-10-25)

The older inventory below incorrectly said this same hashed model had no global
sequence or light records. A fresh PE/archive-backed extraction and chunk walk
finds `GLBS` and `LITE`: one directional `FDirectSun`, a fixed rotation key, and
animated diffuse/ambient colour tracks. The older illumination conclusion is
superseded. These values are read from the model; none are copied into rendering
constants.

The selected executable's SetSkyModel path creates its own sprite/context. Its
sky pass disables the terrain/unit DNC contexts and enables the sky model's
lights. The registered clock callback maps the world time of day into the model
sequence; it is not a colour callback. Local REA evidence and exact window reads
are retained in `.scratch/campaign-sky-20261025/rea-snapshot.json` (38 records).
The selected image identity is
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.

A headless/muted original-client control pair retains the same source map, sky
texture, camera and frozen diagnostic noon. Its only model change sets the
material's unlit bit. The source-lit sky is darker than that unlit control. A
third null-sky control is black in the same sky patches while a sampled terrain
patch is unchanged. Thus forcing the material unlit is contradicted by the
observation; this is not a screenshot-derived brightness constant or a full pixel
parity claim. Raw maps, scripts, model/member identities and captures remain
ignored under the same local directory.

`campaign_sky.rs` now binds the selected sky's own model through the existing
bounded `day_night::Assets` reader, preserving source identity, bytes and tracks.
It samples that model's light at the canonical executed day fraction and sends
it through the existing source-derived Gouraud/colour-packing path used by model
actors. Terrain/unit sun selection is not substituted. Missing or unsupported
own-light records and unavailable clocks remain explicit diagnostics. The model
and light binding retire with the same sky/session state. Other sky animation
clock behavior and camera-relative placement still need broader qualification.

Focused source-clock, identity and lifecycle checks pass (12 tests), as does the
explicit original-model reader gate (1), full app suite (686), strict native/WASM
Clippy and both builds. The fresh production offscreen replay reaches restored
campaign control. Its retained diagnostics now identify doodad-particle ownership
and terrain/tree sight occlusion rather than the old sky-lighting refusal. The replay passed its restart, quit, console retirement and relaunch assertions,
then failed its final renderer assertion on those two remaining diagnostics
(exit 101). The full gate is not green. This camera capture does not itself
establish visible-sky pixel parity, and synthetic
or parser checks do not establish original-client rendered parity.

## Historical inventory (superseded where corrected above)

## Source evidence

- Human01 `Trig_Map_Initialization_Actions` calls
  `SetSkyModel("Environment\\Sky\\LordaeronSummerSky\\LordaeronSummerSky.mdl")`
  (local extracted script `.scratch/campaign-placements/j/Human01.j:1870`).
  `SetSkyModel(null)` runs at lines 2047 and 2656 (cutscene and ending).
- Legacy `War3.mpq` member `Environment\Sky\LordaeronSummerSky\LordaeronSummerSky.mdx`,
  4,062 bytes, SHA-256 `60fcd135b70240019b21b2da34e97448417e62289def6b26e982b8d4314fdf2c`
  (Warcraft III Legacy 1.29.2.9231 tree; local copy in `.scratch/sky-root/`, uncommitted).
- Checked with a private, Bevy-free probe on `pcx-mdl`/`pcx-play` (not committed):
  - `VERS` 800. `SEQS`: one sequence `Stand`, 333 to 60333. `GLBS`: present (the earlier “none” inventory was incorrect; see correction).
  - `BONE`: `SkyInnerClods`, object 0, parent -1, flags 0x100, no tracks.
  - `HELP`: `Dummy01`, object 2, parent -1, flags 0, one `KGRT` track: 8 linear keys,
    frames 333 to 60333, rotations as authored (no global sequence).
  - `GEOS`: one geoset, 42 vertices, 42 normals, `GNDX` all 0, `MTGC` [1], `MATS` [0].
    Vertex 0 is near (-1946, -1954, 1954) in model units, so the dome is roughly 1,950
    units from the model origin (hand decode of the raw floats, not a checked extent).
  - `MTLS`: one layer, shading `0xf0`, filter mode 0, texture
    `Environment\Sky\LordaeronSummerSky\LordaeronSummerSky.blp`. Bit 0 clear (lit).
    `0x20` Unfogged, `0x40` NoDepthTest, `0x80` NoDepthSet, `0x10` TwoSided per the
    menu material notes. No `TXAN`, no `GEOA`, no `ATCH`, no `CAMS`, no `EVTS`.

## What the rig does

- `JointPlan` includes HELP nodes as joints (`pcx-gfx` `mdx_runtime`), and the
  synthetic helper-track test `pcx-gfx/tests/mdx_runtime.rs`
  (`bone_under_animated_helper_inherits_the_authored_helper_track`) covers that
  evaluation.
- The only geoset binds `MATS` [0], which is bone `SkyInnerClods` with no tracks.
  `Dummy01`'s `KGRT` is evaluated but no vertex references it, so the dome is drawn at
  rest. Its rotation has no visible effect for this model.

## Implementation (`campaign_sky.rs`)

- `ObjectHost::environment().sky_model`: an executed path shows the dome.
  `Some(None)` (null) and unset show none. No fallback sky is invented.
- The script path maps to its binary `.mdx` member and is read through the campaign
  resolver with source identity. Animation, geoset, texture and material handling
  reuse `campaign_actors::Actor`.
- Placement: the root sits at the scene camera eye at unit scale (`MODEL_SCALE`, the
  identity transform, not a measured dome radius).
- Drawing: one `SkyCamera` (`Camera3d`, order -1, `RenderLayers` 2). Each frame while
  shown it takes the scene camera's `viewport` and `sub_camera_view` (both public
  `Camera` settings), its `is_active`, its `RenderTarget` component and its `Transform`
  and `Projection`. Output mode, invert culling and MSAA writeback are not copied (they
  keep Bevy defaults). Computed camera state is not copied; Bevy recomputes it.
- Scene clear: the scene camera's previous `clear_color` is saved before the sky sets
  `None`. It is restored only if the value is still `None` when the sky hides, so a
  caller change made while shown is kept. Null, load failure, session replacement and
  teardown all take this path. A camera that started as `None` stays `None`.
- Failure: a load or spawn error retires the dome and camera, records the script in
  `failed`, and reports once. The next frame restores the scene clear.
- Historical lighting gap (superseded by the own-light connection above): the material
  kept its authored shading flags (lit, bit 0 clear), and no light was applied. Illumination source is unqualified, so spawn sets the diagnostic
  `sky lit layer drawn without a qualified illumination source`. Under the campaign's
  ambient light alone the dome renders dark. That is a known gap, not a chosen look.
  Qualifying it needs source or native evidence (see below).

## Authored map sky (no `SetSkyModel`)

Not established. The `pcx-map` `war3map.w3i` layout doc, `war3map.w3e` and the
`MiscData` files that were checked list no sky-model field. That is evidence about
those documented layouts only, not proof of engine behaviour. The implementation shows
no sky in this case, and Human01 always calls `SetSkyModel`.

## Layers and cameras

- Render layers in the campaign: 0 (default), 1 scene (`campaign_camera::RENDER_LAYER`),
  2 sky (new, no other user), 7 menu sprites. The sky camera renders only layer 2.
- Orders: sky -1; launcher letterbox -2 (inactive outside the menu); menu background -1
  (`mainmenu_scene`); campaign UI 1; menu sprites 0. The menu background and sky both
  use -1. They are not expected to be active together. Not verified against
  `mainmenu_scene` teardown.
- Scene camera suspension (`CampaignCamera::suspend`) sets `is_active`, which the sky
  camera copies, so score and suspended views hide the sky with the scene.

## Not verified

- Dome centre and far-plane relationship to the native camera. Eye-centred placement
  is an inference. Fitting inside Human01 `FARZ` 2,500 is plausibility, not evidence of
  the native placement. Native camera-relative placement needs an original observation.
- Scale: identity assumes the model units are world units. The source world mapping is
  not established.
- Illumination of the lit layer, including whether the terrain day/night model
  (`Environment\DNC\DNCLordaeron\...`) applies. See Lighting.
- Rendered result, GPU behaviour and cross-camera order: not run.

## Tests

Not compiled or run yet. The heavy Bevy build waits for a root slot.

- `campaign_sky::tests`, narrow camera checks in a minimal `App`: explicit custom
  colour, initially `None`, replaced scene camera, caller edits to other fields kept,
  viewport and layer mirroring, one sky camera. These call the sync and failure
  functions directly. They do not cover the production reset or load path.
- `tests/support/campaign_sky_cases.rs`, integration: the executed `SetSkyModel` path
  is published with its source spelling, null is distinct from unset, a replacement
  session publishes its own state, and a refused replacement keeps the prior state.
  These go through the production map request and startup path with the synthetic
  fixture. The campaign actor module that draws the dome is in the application binary
  and is not reachable from integration tests.
- Production `project` (bin, `test-fixtures` feature, `campaign_sky::production`): the
  shared startup fixture (`binding`, `fixtures`, `campaign_startup_cases`) is included
  for native test-fixtures builds, the same way the wasm32 test build already includes
  it. Four tests: a shown dome with a hidden clear, null replacement restoring the
  caller clear, load failure, and same-sky replacement with one camera and one saved
  clear. Lighting parameters and the palette are seeded on `UnitActors` with the labelled
  synthetic values, because reading them needs the installed client. Real parameter
  reads stay an explicit original-input gate.
- The dome fixture is a real synthetic classic model (`tests/support/synthetic_sky.rs`):
  one geoset with three vertices, normals, one triangle, one group on node 0, a material
  layer, one 4×4 direct BLP1 texture, one bone, one `Stand` sequence and a pivot. A private
  non-Bevy run parsed it through every `pcx-mdl` decoder (container, sequences, bones,
  pivots, geoset mesh, materials, textures) and decoded the texture with `pcx-blp`. The
  production tests assert owned geosets, decoded image bytes, vertices and mesh entities,
  and that retirement releases them.
- Found and fixed by these tests: session replacement dropped the dome without restoring
  the scene clear. The reset now calls `sky::release`, which restores the clear first.
- Not covered: rendered camera output, and the real lighting parameters from an install.
- `model_member` path mapping (synthetic).
- `original_sky_model_has_a_stand_sequence` (ignored; needs `PCX_SKY_ORIGINAL_MDX`
  pointing at a local `LordaeronSummerSky.mdx`). The same read passed in a private
  Bevy-free probe.

## Unsupported states (recorded, not rendered)

- Lit layer illumination: the source light is not established. The dome is drawn with
  its authored lit flags and no applied light, so it is dark under ambient light. This
  is a fidelity gap, not a faithful PBR result. The diagnostic is set on spawn.
- Map sky with no `SetSkyModel`: no sky is shown. Whether the engine has a default
  sky here is not established.
- `SetTerrainFogEx` and `ResetTerrainFog`: retained only. No fog is rendered, so the
  dome's `Unfogged` flag has nothing to be compared against yet.
- Helper `Dummy01` `KGRT` rotation: evaluated by the joint plan, no vertex uses it.

## Queued original capture

A native or original-runtime observation of the sky's screen placement against the
camera eye, and of the lit layer's brightness at two time-of-day values. It runs after
the existing runtime jobs and needs a root slot. Until then, placement, scale and
lighting stay unverified.

## Build status

- Shared target `/home/maci/Desktop/peacecraft3/target`: the `cargo check` (exit 0) and
  the sky, item-model and actor bin test runs (7, 2 and 15 passed) are **unverified**.
  Another worktree reused a `pcx-play` artifact from an older source identity, so these
  results cannot be tied to this candidate. `.scratch/piece06-check2.log` and
  `.scratch/piece06-test-*.log` are kept only as records.
- `map_loading` `campaign_sky` integration run: not run.
- Private target (`.scratch/target-p06-core`): `pcx-script` `doubled_backslash_in_a_string_literal_decodes_to_one_backslash`
  passes. Confirms `\\` in a script literal decodes to one `\`, the spelling the
  integration tests assume.
- Next: a worktree-private app target, `.scratch/target-p06-app`, for the
  `cargo check`, bin `sky::tests`, `campaign_item_model`, `campaign_actors::` and the
  `map_loading campaign_sky` filters, each run sequentially.

## Empty-unit publication regression (0.5.0)

The production sky lifecycle tests exposed an unconditional unit-sight readiness
query blocking the independent sky pass in sessions with no executed units. The
actor projection now omits that query only for an empty unit universe. Every
nonempty unit universe retains the existing readiness/refusal path, including
hidden units; vision-grid admission and its placement requirement are unchanged.
This is a recreation scheduling correction, not proof of native internals or
rendered parity. The four synthetic production sky tests cover this path.
