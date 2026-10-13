# Authored menu texture-animation bindings

This is shipped-byte and reconstruction-source evidence. It identifies a
missing texture-animation path; it does not establish native UV matrix
conventions or explain the screenshot's streaks or lower black region.

## Inputs and reproduction

The local version-800 model files have these identities:

| Model | Bytes | SHA-256 |
| --- | ---: | --- |
| RoC `MainMenu3d.mdx` | 2,142,419 | `cc2f5bc76ce3d0d456bf193338ed02ffdbebbf6271fbbefe630d7f954e89bf6a` |
| TFT `MainMenu3D_Exp.mdx` | 798,240 | `15586ffd9dc25b82a4f8ce2e679581f6e14cbcdc0916c7422d89308f010f6293` |

The TFT digest matches the explicitly selected expansion member in the local
115-member browser fixture. Archive adjacency and filenames do not identify
the archive's game version or native lookup priority.

The local probe uses the existing checked `pcx-mdl` readers for materials,
textures, geosets, TXAN tracks and raw GLBS durations. Its source and output
are `.scratch/menu-uv-contract/src/main.rs` and
`.scratch/menu-uv-contract/observations.log`. It was linked against an existing
private-target library and run with warnings denied; no renderer source or
shared build artifacts changed. Original bytes stay local.

## Stored relationships

All indices below are zero-based and describe these inputs, not runtime constants.

| Input | Geoset/material/texture relationship | Texture animation |
| --- | --- | --- |
| TFT | geoset 14, 25 vertices → material 12, layer 0 → TEXS 16 `SKY2.blp` | TXAN 0, `KTAT`, linear, global sequence 20 |
| RoC | material 3, layer 0 → TEXS 3 `cloudstile2.blp` | TXAN 0, `KTAT`, Bézier, global sequence 0 |
| RoC | material 27, layer 0 → TEXS 3 `cloudstile2.blp` | TXAN 1, `KTAT`, linear, global sequence 20 |
| RoC | geoset 29 → material 29 → TEXS 30 `white32.blp` | `NO_ID`; this layer has no TXAN |

The TFT sky uses coordinate set 0, TEXS wrap flags 3, filter mode 0 and
shading flags 1. Global sequence 20 has duration **999,033 ms**, independent
of the menu's shorter Stand loop. Its translation track has two authored keys:
frame 0 at `(0,0,0)`, and frame 999,033 at approximately
`(-16,0.00700456,-5.00007)`. These are three stored components, not a chosen
two-dimensional UV offset.

RoC's global sequence 0 is **144,967 ms** and has two Bézier translation keys;
global sequence 20 is **999,033 ms** and has two linear translation keys.
The probe preserves values and tangents through the decoder rather than
replacing either clock with the scene-loop duration.

## Reconstruction gap and closing check

`mainmenu_scene::spawn` applies authored texture wrapping, while its material
creation and advance systems do not consume the layer's TXAN reference into a
UV transform. GEOA/KMTA opacity is already applied; it is a separate path.

Before wiring texture motion, establish the native track-to-UV transformation:
which stored components affect U/V, their signs, rotation/scale composition and
any pivot. The nontrivial third component makes an assumed first-two-component
projection unsuitable as evidence. Then bind by the layer's actual TXAN index,
sample its independent GLBS clock, and verify the material update on the
identified geoset with an independent synthetic translation and an original
runtime comparison. Do not tune offsets against a screenshot or claim that
texture motion alone repairs camera, geometry, lighting or framing.
