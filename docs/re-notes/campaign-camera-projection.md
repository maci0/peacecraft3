# Gameplay camera projection

The live campaign field of view is a source angle, not a vertical field of view.
`SourceProjection` retains that angle and recalculates the perspective when Bevy
updates the world viewport:

```text
r = viewport_width / viewport_height
vertical_angle = source_angle / sqrt(1 + r*r)
```

Bevy's camera system passes the camera viewport size to `update`; the HUD-derived
viewport bounds are an application input and no test here drives that path. No
screenshot zoom factor or fixed resolution is part of the rule.
Near/far clipping, target, attack angle, rotation, roll and distance retain their
existing source paths. Bevy supplies its own depth, subview and frustum conventions.
Recalculating from the retained source angle avoids accumulating the conversion
when a window is resized or the HUD appears.

## Runtime evidence

An isolated Wine run used the selected 1.29.2.9231 executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
and unchanged `Maps\Campaign\Prologue02.w3m`. Public Video Options were saved at
1280×960, gamma 1.0, all quality controls High, shadows On and occlusion On.
The saved registry and capture metadata preserve those settings. No host display
or audio device was exposed.

A bounded Wine D3D trace retains unique projection/view matrices and viewports.
The gameplay perspective has aspect 1.7778 (1280×720 world frame) and a vertical
angle of 34.318 degrees (`.scratch/shadow-fix-20261027/projection-summary.txt`).
Multiplying by `sqrt(1 + r*r)` gives 69.99 degrees, so the matrix agrees with a
source angle of about 70 degrees normalized by the aspect, not used directly. The
map's source angle was not read back in this check. A second run used public intro skipping;
the final view's horizontal camera target agrees with the recreation's skip path.
The normal-intro and skip-path views differ and must not be mixed as one scene.

This is original-client execution through Wine, not native Windows GPU evidence.
The trace's first attempts exhausted their diagnostic bounds before the useful
scene; they remain preserved and are not used as gameplay evidence. The corrected
filter deduplicates numeric records before applying its bound. All raw logs,
settings, captures and model snapshots remain local under
`.scratch/shadow-fix-20261027/`.

## Verification limits

`campaign_camera::tests::source_angle_tracks_viewport_aspect_without_accumulating_conversion`
uses a nonstock angle (0.93 rad) and sizes (137x83, 83x137, back to 137x83), and
checks the vertical angle, aspect ratio, near and far after each `update`.
Rerun 2026-10-27: `cargo test -p peacecraft3 --lib --features test-fixtures
campaign_camera` passed 5 of 5, headless (`.scratch/verify/p3-camera.log`). The
pose test in the same module, which expects the unconverted stored angle, also
passed. No rendered capture was rerun.

Rendered acceptance still requires the combined world/HUD capture; parser or
matrix agreement alone does not establish matching pixels. Bilinear terrain
height targeting, native smoothing, effective graphics controls, dynamic shadows,
terrain lighting and exact scene-clock matching remain separate fidelity work.
