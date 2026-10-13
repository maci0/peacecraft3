# Sprite-layer rendering: the panel's entrance and its render contract

Scope: the expansion main menu's **hanging panel** — the model
`UI\Glues\SpriteLayers\Expansion\TopRightPanel-Expansion.mdx` — its authored
`MainMenu Birth` → `Stand` entrance, and the contract a renderer needs to draw it
over the original 3D scene. The implementation is
`crates/peacecraft3/src/menu_sprites.rs`, a new module the application root
declares and wires.

This note records what was **read from the shipped bytes** and what remains
**unproven**. Nothing here is a screenshot fit, and no projection scale is
invented.

## 1. Selection is by skin key, like the logo

`UI\war3skins.txt` (see `menu-panel-origin.md` §2) maps the panel by edition:

```text
GlueSpriteLayerTopRight_V0=UI\Glues\SpriteLayers\TopRightPanel.mdl
GlueSpriteLayerTopRight_V1=UI\Glues\SpriteLayers\Expansion\TopRightPanel-Expansion.mdl
GlueSpriteLayerTopRight_V1Beta=UI\Glues\SpriteLayers\TopRightPanel.mdl
```

The `%d` is the edition: `0` Reign of Chaos, `1` The Frozen Throne. Only `1`
names the `-Expansion` art; `V1Beta` resolves to the base panel and the `%d` call
cannot produce that key anyway. The module exposes this as
`SKIN_KEY_TOPRIGHT` + `SKIN_INDEX_ROC`/`SKIN_INDEX_TFT` and
`skin_key(family, edition)` → `GlueSpriteLayerTopRight_V1`.

The user-facing selector — **Edition, RoC on the left, TFT on the right, TFT the
default, 1.28 removed** — is the GUI root's; this module only supplies the
edition integer that picks the art.

## 2. The native coordinate facts the contract carries

From `docs/re-notes/glue-coordinate-runtime.md` (read from shipped machine code,
not run):

- **Authored units are divided by `0.8` in X and `0.6` in Y.** The original
  `FUN_00ab2970` is `out[0]=in[0]/0.8f; out[1]=in[1]/0.6f`. These are the
  constants `0.6`/`0.8` at VA `0x00e84160`.
- **The overlay sprites are built with camera argument `−1`.** The glue
  initializer assigns `GlueSpriteLayerTopRight_V%d` with camera `−1` and adds flag
  bit **1 only** — unlike `SpriteBackground`, which takes camera `0` and bits
  **0 and 1**. So the panel overlay is *not* given the direct full-window
  conversion bypass, and takes the default sprite projection.
- **Default sprite projection origin is `frame.left + modelX`,
  `frame.bottom + modelY`** with an identity model-view; the clip-centre
  subtraction cancels and does not recenter the raw bounds.

The module carries these as data: `GLUE_VIEW_WIDTH = 0.8`,
`GLUE_VIEW_HEIGHT = 0.6`, `OVERLAY_CAMERA_ARG = -1`, and the `ScalingMode::Fixed`
orthographic projection in `sprite_camera()`.

## 3. What the panel model actually contains

Parsed from the shipped `TopRightPanel-Expansion.mdx` (339,976 bytes,
`b07996b34b7ddfd179fca4171f21c91f…`):

```
chunks  VERS · MODL · SEQS 8184 · MTLS 816 · TEXS 2144 ·
        GEOS 135912 · GEOA 23004 · BONE 168044 · PIVT 828 · EVTS 584
CAMS    none  -> the overlay takes the default projection (camera -1)
objects  65 BONE nodes + 4 EVTS event objects = 69 = the PIVT count; no HLPR chunk
geometry  57 geosets, 780 vertices, spanning
          x [0.4243, 0.9734]  y [-0.1072, 1.2526]  z ~ [497.896, 498.621]
tracks    BONE: 17 KGTR + 51 KGRT, no KGSC
          GEOA: 50 alpha tracks (KGAO), no KGAC
TEXS      Textures\GlueScreen-Panel-Rail.blp
          UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-MetalPieces-Expansion.blp
          UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-MetalBorder-Expansion.blp
          Textures\GlueScreen-Panel-BNetChannel.blp
          UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-ExpansionDetail.blp
          Textures\GlueScreen-Panel-GearRail.blp
          Textures\GlueScreen-Panel-MetalPieces.blp   (8 records, some repeated)
```

The relevant `SEQS`:

```
MainMenu Birth   2500..3500  1000 ms
MainMenu Stand   3667..4667  1000 ms
MainMenu Death   4833..5500   667 ms
```

plus the rest of the glue state machine (`RealmSelection`, `SinglePlayer`,
`Options`, `Battlenet…`, with `Morph` pairs). The panel therefore has an authored
entrance, sampled per submenu.

Constant `z` near 498 means the panel is **screen-space art** composited over the
scene, not scenery.

## 4. The data source and the sampled-pose contract

`PanelArt::load(bytes)` parses the model into sequences, bones with their keyed
tracks, geoset-animation records, per-geoset quads, textures and the camera count.
`PanelArt::sample_pose(&seq, t_ms)` evaluates at the absolute frame
`seq.start + t_ms`:

- each bone with a key on or before the sample yields a local
  translation/rotation/scale (`NodePose`), built from the model's own `KGTR`,
  `KGRT`, `KGSC` tracks;
- each geoset-animation record yields an alpha, from its `KGAO` keys or its static
  value.

Interpolation follows the file's own types: `None` steps, `Linear` interpolates,
and the tangent types use their keys' tangents — `Bezier` as the cubic through the
stored **control values** `(a.value, a.out, b.in, b.value)`, `Hermite` as the
Hermite the decoder uses. Two cautions on that:

- The Bézier-as-control-values reading is for **vec3** values. It is the reading
  that reproduces the logo's measured mid-segment `0.65`, but it is not a claim
  that `pcx-mdl`'s own sampler agrees — that sampler conflates Hermite and Bézier
  onto the Hermite basis, so it would give `0.458` for the same keys.
- **Rotation is not covered.** Quaternion interpolation is not established from
  the format, and the panel's 51 rotation tracks are Hermite, so no rotational
  accuracy is claimed until that interpolation is pinned down.

No catalogue constant is treated as the renderer: the sample is driven entirely by
the parsed keys.

## 5. The render contract, and its isolation

`SpriteLayerContract` is data — the `0.8 × 0.6` viewport, the `−1` camera
argument, the render layer, and the edition index — and `sprite_camera()` returns
an **optional** `(Camera2d, Projection, Transform)` bundle whose projection is
`ScalingMode::Fixed { width: 0.8, height: 0.6 }`.

Two deliberate limits:

- **Z and screen position are not set.** No evidence fixes this layer's Z, and no
  screen offset is invented; the transform is left at the default for the root to
  place from the frame's own anchors.
- **No GPU state is created.** The module touches neither `MainMenuScene`, its
  `GlobalAmbientLight` nor `mainmenu_scene`'s cameras. Isolation is a single
  layer number, `SPRITE_RENDER_LAYER = 7`: the root gives that layer to both the
  sprite camera and the sprite entities so the pass does not draw the original 3D
  scene. The `RenderLayers` component itself is the root's line to add, because
  this module owns no rendering state.

## 6. Posed-extent probe, binding evidence, and the exact blocker

`tools/diag/panel_skin.py` (read-only) replicates the released `pcx-mdl` layout and
skins the shipped panel at the `MainMenu Birth`/`Stand` key times. It is
reproducible: `python3 tools/diag/panel_skin.py [model.mdx]`.

### Correction: the `BONE` framing

The panel's `BONE` records are `size` **plus an 8-byte trailer** — walking `size`
alone lands on a node's `0xFFFFFFFF` parent sentinel and desyncs after one node.
That trailer is `pcx-mdl`'s `BONE_TRAILER = 8`, and `Mdlx::bones()` already
applies it, so the decoder walks this model correctly (65 nodes, ending exactly
at 168,044 bytes). The probe's first attempt did not, which is what made an
earlier pass report a walk failure. **No decoder change is needed**; the note's
earlier "no finished mesh" framing was too pessimistic about the decoder.

### Correction: the `GEOA`→geoset association

The `GEOA` record's explicit geoset id is at record **`+24`** (`body + 20`), which
`pcx-mdl` skips. Read directly, the panel has **51** `GEOA` records whose ids are:

```
0, 2, 3, 4, 5, 6, 7, 8, 14, 15, 16, … 56
```

— a **subset** of the 57 geoset indices, **missing `{1, 9, 10, 11, 12, 13}`**.
So binding alpha **by array order would misbind at least those six geosets**.

`menu_sprites.rs` therefore reads the id itself, from the raw `GEOA` chunk via the
decoder's public `Mdlx::payload`, and carries it on `PanelGeosetAnim::geoset_id`.
Lookup is `PanelArt::geoanim_for_geoset(id)` / `alpha_for_geoset(id, frame)`, which
match on the id — **never** on position — so a geoset with no record returns
`None` rather than picking up a neighbour's alpha. A record whose id cannot be read
yields `None`, which is reported, not defaulted. The unit test
`geoset_anims_bind_by_id_not_by_position` pins this, and the shipped-model fixture
asserts the real ids (`0..56` minus `{1, 9..13}`) and that `alpha_for_geoset(1, …)`
is `None`.

Two further facts fall out of the probe:

- **Bone-id mapping verified.** The 65 bones have **sequential object ids
  `0..64`**, and every `MATS` skin index lies in `0..64` and resolves — 0 bad
  entries across all 57 geosets. So index == object id here, and the skinning
  table is well-formed.
- **Geoset→material is available.** Each geoset's `MATS` trailer carries its
  `material_id` (`0..16`), so geoset→material→texture can be bound exactly.

### Correction: the hierarchy is complete, and there are no helper nodes

`PIVT` holds **69** pivots while `BONE` holds **65** nodes, which invites the
guess that four helper (`HLPR`) nodes parent some bones. Measured, that guess is
wrong on both counts:

- **There is no `HLPR` chunk.** The panel's chunks are exactly `VERS MODL SEQS
  MTLS TEXS GEOS GEOA BONE PIVT EVTS`.
- **The four extra pivots are `EVTS` event objects.** The `EVTS` chunk (584 bytes)
  holds **4** objects — the first is `SNDXARPD`, object id `65` — and
  `BONE(65) + EVTS(4) = 69`, which is `PIVT` exactly. Their tracks are `KEVT`
  trigger-frame lists (the first has 9 keys at `2500, 8833, 12000, 14000, 20333,
  25500, 46500, 51667, 56833`, the sequence starts), **not** translation or
  scale. So there are **no helper scale/translation keys to report** — none exist.
- **No bone parents to a node outside `BONE`.** Census of every `parent_id`:
  **0** bones reference an id outside the `BONE` id set. So the parent chain used
  for the posed extents is complete, and `pcx-mdl`'s "bones-only skeleton may drop
  helper parents" limitation **does not bite** for this model. `MATS` resolving in
  `0..64` was not sufficient to prove this on its own; the `parent_id` census is.

### The measured extents (linear-sampled — an estimate, not a proof)

```
MainMenu Birth 2500..3500
  frame 2500  posed x[0.4243,0.9734]  y[-0.1072,1.2526]  z[497.896,498.621]
  frame 3000  posed x[0.4243,0.9734]  y[-0.0494,1.2526]  z[497.896,498.621]
  frame 3500  posed x[0.4243,0.9734]  y[-0.0524,1.2526]  z[497.896,498.621]
MainMenu Stand 3667..4667   posed y holds at [-0.0524, 1.2526]
```

The panel's lower edge **rises from −0.1072 to −0.0494** during `Birth` and holds
through `Stand` — real authored motion. `x` is constant and `z` stays near 498.
The alpha-visible extent matches the posed extent, because every `GEOA` alpha
track is interpolation **0** (step) with static alpha 1.0, so the alpha is exact.

### What this does **not** establish

- **No projection inference.** The posed extent reaching `y 1.2526` and `x 0.9734`
  against the authored `0.8 × 0.6` does **not** prove the glue applies a further
  scale or mapping. Geometry extending past the viewport is ordinary, and a
  transform must never be inferred from fitting an extent. The earlier draft of
  this note drew that inference; it is withdrawn.
- **No native-projection diagnosis.** The probe samples the translations and
  rotations **linearly**, while the panel's own tracks are `KGTR` at
  interpolation 3 (Bézier, 17 tracks) and `KGRT` at interpolation 2 (Hermite, 51
  tracks). A linear approximation cannot diagnose the native projection, and
  `pcx-mdl`'s `sample_vec3` conflates Hermite and Bézier onto the Hermite basis,
  so using it would not recover the authored Bézier either. The extents above are
  an estimate of the motion's *shape*, not its exact curve.
- **No rotation claim.** Quaternion interpolation is not established from the
  format alone, so no rotational accuracy is asserted. Computing full node world
  matrices exactly would require resolving that interpolation — not choosing a
  basis.

So the honest state is: the panel **has** an authored entrance with real motion,
the alpha is exact, and the hierarchy is verified complete; the **exact** posed
curve and any projection remain open, and closing them needs the
Hermite/Bézier split and the rotation interpolation established first.

## 7. Also corrected: `MainMenuButton` is genuinely absent

The earlier note called `MainMenuButton.mdl` "dangling" from a nominal `.mdl`
check. Re-checked across **all five archives** (`War3`, `War3x`, `War3Local`,
`War3xLocal`, `Deprecated`) and **both extensions** (`.mdl`, `.mdx`), the path
`UI\Glues\MainMenu\MainMenuButton\MainMenuButton.*` appears in **none** of them.
So the `MainMenuButton` skin key really has no file behind it in this install —
the claim holds under effective archive priority, not just the nominal name.

## 8. State, and the handoff

### The module compiles and its tests pass — but that is *parser* evidence

`crates/peacecraft3/src/menu_sprites.rs` compiles (checked standalone against the
built `bevy`/`pcx_mdl` rlibs, edition 2024) and its 8 tests pass, including the
shipped-model fixture, which runs against the real
`TopRightPanel-Expansion.mdx` extracted from `War3x.mpq`.

That fixture is **parser evidence**: it proves the decoder and this module read the
shipped bytes correctly — the sequences, the by-id `GEOA` binding, the bones, the
quads, the zero camera count. It is **not** browser, runtime or render proof. No
pixels were produced, no projection was exercised, and nothing here shows the panel
drawing in the menu. Rendering parity remains open.

### Render data API (root wires it)

```rust
// paths + selection
menu_sprites::PANEL_MODEL / PANEL_MODEL_SERVED
menu_sprites::skin_key(menu_sprites::SKIN_INDEX_TOPRIGHT, edition)   // "GlueSpriteLayerTopRight_V1"
menu_sprites::SKIN_INDEX_ROC / SKIN_INDEX_TFT

// parse + authored entrance
let art = menu_sprites::PanelArt::load(&bytes)?;        // PanelProblem on a gap
art.sequence("MainMenu Birth") / ("MainMenu Stand") / ("MainMenu Death")
let pose = art.sample_pose(birth, t_ms);                // bones + per-geoset alpha
art.alpha_for_geoset(geoset_id, frame)                  // bound by id
art.geoanim_for_geoset(geoset_id)
art.model_bounds() / art.textures / art.camera_count

// optional, isolated camera (root adds RenderLayers + viewport)
menu_sprites::sprite_camera()                            // Fixed 0.8 x 0.6
menu_sprites::SPRITE_RENDER_LAYER
```

### Authored `MainMenu Birth` / `Stand` mapping

The panel's own `SEQS`, and how the module drives them:

| Sequence | Frames (ms) | Length | Role |
| --- | --- | --- | --- |
| `MainMenu Birth` | `2500..3500` | 1000 ms | entrance, played on menu appear |
| `MainMenu Stand` | `3667..4667` | 1000 ms | held state |
| `MainMenu Death` | `4833..5500` | 667 ms | dismissal |

`sample_pose` evaluates at the **absolute** frame `seq.start + t_ms`, because the
records key absolute model frames. Alpha is step-interpolated (all 50 `KGAO` tracks
are interpolation 0), so the alpha is exact. Translation is Bézier and rotation
Hermite — see the caveats above; the sampled translation/rotation curve is an
estimate until those are pinned.

### The blue plate behind each menu entry is a button backdrop, not the panel

For the record, since it comes up: the blue backing behind a menu entry is the
**button's own** `StandardButtonBackdropTemplate` background —
`UI\Widgets\Glues\GlueScreen-Button1-BackdropBackground.blp`, tiled
(`BackdropTileBackground`, size `0.032`) inside the edge
`GlueScreen-Button1-BackdropBorder.blp`, drawn with the state's edge swapped on
push. That is per-entry and separate from the hanging panel model here; it is
documented with exact paths in `docs/re-notes/menu-button-states.md`. The FDFs give
no tint, so the blue itself lives in that `.blp` and is not verified by these notes.

### Constraints respected

- Only `menu_sprites.rs`, `menu-sprite-rendering.md` and `tools/diag/panel_skin.py`
  were touched. No `Cargo.toml`, `main.rs`, shared decoder, UI, loader or git edits.
- No projection is inferred from an extent fit; offscreen geometry is treated as
  ordinary. No Hermite-quaternion native behaviour is claimed — it would need
  machine-code evidence, which is not yet in hand.
- **Next** (bounded): establish the Hermite/Bézier split in sampling and how the
  original interpolates rotation, with machine-code evidence; then the posed curve
  can be compared against the native `CSpriteFrame` projection. Same for
  `TopLeftPanel-Expansion`.

