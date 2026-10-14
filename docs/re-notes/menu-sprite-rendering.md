# Sprite-layer rendering: the menu panels' entrance and sounds

Scope: the main menu's glue sprite layers `GlueSpriteLayerTopLeft` and
`GlueSpriteLayerTopRight` (the hanging panel), their `MainMenu Birth` →
`MainMenu Stand` entrance, and the sound events that entrance fires. The
implementation is `crates/peacecraft3/src/menu_sprites.rs`.

## 1. Selection is by skin key

`UI\war3skins.txt` `[Default]` maps each layer by edition:

```text
GlueSpriteLayerTopLeft_V0=UI\Glues\SpriteLayers\TopLeftPanel.mdl
GlueSpriteLayerTopLeft_V1=UI\Glues\SpriteLayers\Expansion\TopLeftPanel-Expansion.mdl
GlueSpriteLayerTopRight_V0=UI\Glues\SpriteLayers\TopRightPanel.mdl
GlueSpriteLayerTopRight_V1=UI\Glues\SpriteLayers\Expansion\TopRightPanel-Expansion.mdl
```

`_V%d` is the edition: `0` Reign of Chaos, `1` The Frozen Throne. Both layer
names are strings in `game.dll`. The table names `.mdl`; the archives ship only
`.mdx` for these keys, so the loader requests the `.mdx`. The native extension
rewrite has not been traced.

## 2. Which sequences the main menu plays

`game.dll`'s animation-tag string table holds the glue screen tags (`mainmenu`,
`realmselection`, `singleplayer`, `maincancelpanel`, `battlenet…`) next to
`Death`, `Birth` and `Stand`. The panels' `SEQS` use those tags as prefixes:

```text
MainMenu Birth   2500..3500  1000 ms
MainMenu Stand   3667..4667  1000 ms   (SEQS non-looping flag set)
MainMenu Death   4833..5500   667 ms
```

Both panels carry the same 62-sequence state machine. The loader matches a
sequence whose two whitespace-separated tokens are the screen tag and the
animation, case-insensitively. It plays `MainMenu Birth` once and then
`MainMenu Stand`, holding Stand's last frame because its `SEQS` flag is
non-looping. The native glue dispatcher that selects these names is not traced;
the tag table is the evidence.

Posed through `pcx_gfx` skinning at the Birth key times, the TFT right panel's
upper section drops from `y 1.227` to `0.631` while its lower piece rises from
`y −0.107` to `−0.052`: the two parts close on the button stack. The left
panel does not move during `MainMenu Birth`.

## 3. Sound events

Each panel's `EVTS` objects are named `SNDX` + a four-character code; their
`KEVT` frames are absolute model frames:

```text
TopRightPanel(-Expansion)  SNDXARPD  2500, 8833, 12000, …   (MainMenu Birth starts at 2500)
                           SNDXARPU  4833, 11167, …         (MainMenu Death starts at 4833)
                           SNDXABPU / SNDXABPD              (other screens)
TopLeftPanel(-Expansion)   SNDXALPD  5667, 60000
                           SNDXALPU  8000, 62333
```

The code resolves through `UI\SoundInfo\AnimLookups.slk`
(`AnimSoundEvent` → `SoundLabel`) and `UI\SoundInfo\AnimSounds.slk`
(`SoundName` → `DirectoryBase`, `FileNames`, `Volume`):

```text
ARPD → RightGlueScreenPopDown → Sound\Interface\RightGlueScreenPopDown.wav, Volume 64
```

The WAV is PCM, 16-bit stereo 22,050 Hz, 146,866 bytes, and extracts with the
Rust MPQ reader. Volume is applied as `Volume / 127`. Pitch (`1`), pitch
variance (`0`), channel, priority and `CHANNELFULLPREEMPT` are not applied. A
label that lists several files is refused: the engine picks one at random, and
no panel sound needs that yet. Only events whose frames fall inside the played
Birth or Stand window are loaded; a missing table or file is logged and the
entrance plays without that sound.

## 4. What the panel models contain

`TopRightPanel-Expansion.mdx` (`b07996b34b7ddfd179fca4171f21c91f…`):

```text
chunks   VERS MODL SEQS MTLS TEXS GEOS GEOA BONE PIVT EVTS   (no CAMS, no HLPR)
objects  65 BONE + 4 EVTS = 69 = PIVT count
geometry 57 geosets; z ≈ 497.9 .. 498.6 (screen-space art)
tracks   BONE: 17 KGTR (Bézier), 51 KGRT (Hermite), no KGSC
         GEOA: 50 KGAO tracks, all step-interpolated
```

`TopLeftPanel-Expansion.mdx` has 79 geosets and 2 `EVTS` objects. Every
material layer of both panels is unshaded (`shadingFlags & 0x1`); most use
`filterMode 2` (blend), one per panel uses `1` (alpha test), and one carries
`0xC1`.

Measured corrections that still hold:

- `BONE` records are `size` plus an 8-byte trailer; `Mdlx::bones()` applies it.
- The `GEOA` record's geoset id sits at record `+24`. The panel's ids skip
  `{1, 9..13}`, so binding by position would misbind. `pcx_gfx::mdx_opacity`
  binds by the authored id.
- Every bone `parent_id` resolves inside the `BONE` set, and every `MATS` index
  lies in `0..64`.

## 5. Rendering

`menu_sprites::spawn` builds each panel through `mainmenu_scene::spawn_geosets`
(per-geoset material from its own layer flags and `TEXS` wrap flags) and poses
it each frame through `mainmenu_scene::skin_geosets`, the background's
skinning path. Opacity uses one `OpacityPlan` per sequence window.

The panels draw through their own camera:

- Orthographic, `ScalingMode::Fixed` at the authored `0.8 × 0.6`, centred on
  the frame. The default sprite projection adds model X/Y to the frame's
  left/bottom (`glue-coordinate-runtime.md`), so model units are glue units.
- The UI camera's viewport, so the panels share the authored frame with the
  buttons at every window shape.
- Order `0`: after the background (`−1`), before the UI (`1`); no clear.
- `RenderLayers` layer 7 on the camera and the geosets, so neither camera draws
  the other's meshes.
- Looking down −Z with near/far from the models' own Z extents. Transparent
  geosets sort by depth, so larger Z draws in front. The native sprite depth
  convention is not traced; the headless capture shows chains behind the metal
  border, as expected.
- `Tonemapping::None`: the art is texture colour, not scene radiance.
- `NoFrustumCulling`: skinning moves geosets without refreshing their bounds.

The clock starts when the panels spawn, after the background, and advances by
frame delta. A long first frame (asset upload) is counted, so a slow machine
shows less of the 1-second entrance.

## 6. Evidence and gaps

- Unit tests: timeline (Birth once, Stand hold or loop), event firing once per
  pass, sequence tag matching, `.mdl` → `.mdx`, sound resolution through both
  tables (synthetic SYLK).
- `original_panels_load_with_entrance_and_pop_down_sound` (ignored; needs
  `PEACECRAFT3_INSTALL`): both TFT panels load, and the right panel fires
  `RightGlueScreenPopDown.wav` on Birth's first frame.
- Native render captured on Xvfb with llvmpipe: the panel closes on the button
  stack with chains, metal border and the lower Quit housing in place. No audio
  device was present, so playback was not heard.
- Not verified: browser rendering (panels and textures resolve through imported
  archives by game path; sounds go through `peacecraft3Audio.event`), native
  depth order, the alpha-test threshold, `0x40`/`0x80` depth flags, Hermite
  quaternion parity, and the other glue screens' Birth/Death transitions.
