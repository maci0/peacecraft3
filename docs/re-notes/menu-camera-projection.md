# Authored menu background projection

The background camera now uses its checked CAMS angle, near/far planes, eye and
look-at target. Missing cameras, malformed source values and degenerate Z-up
poses refuse before scene writes; no fallback angle or clip clamps replace the
input. Camera zero follows the original glue background factory's selection.
Logo and panel orthographic cameras are unchanged.

The logical rectangle comes from the existing engine glue coordinate convention,
not a copied FDF dimension. MainMenuFrame and ControlLayer use SetAllPoints;
[the coordinate evidence](glue-coordinate-runtime.md) establishes their logical
coordinate space and the distinct full-window background path. For logical
width/height ratio `r` and stored angle `F`, the equivalent projection uses:

```text
vertical_angle = F / sqrt(1 + r*r)
aspect = r
```

Uniformly scaling the logical coordinates leaves this rule unchanged. A literal
multiplier copied from one observed rectangle would not. Bevy's ordinary
perspective projection overwrites aspect with physical target dimensions on
resize; the background therefore uses a small custom projection that retains
its logical frustum. Projection, subview cropping and culling use Bevy's depth
conventions rather than copied D3D matrix entries. Full-surface presentation
preserves the existing background/UI separation.

## Evidence and scope

Retained authored-camera constructor/matrix evidence supplies the logical-ratio
normalization hypothesis. A controlled offline run of the supplied executable
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
VERSIONINFO 1.29.2.9231, reached the TFT menu after one observed intro-skip input.
The selected reference image contains an audio-unavailable modal; its brightness
and occluded pixels are not an undimmed appearance reference.

The unique perspective in that run agrees with the checked retained TFT camera's
pose, clip pair and normalized source-angle prediction. The first perspective
cohort's 18 draw index counts and cumulative vertex/index offsets match the
retained model's topology. Two identical small geosets are individually
ambiguous. This qualifies a background-camera cohort without selecting a matrix
by screenshot appearance or claiming every native-opened model byte is known.

The trace explicitly sets a 1024-by-576 context viewport before the first matching
draw. The X client is 1280-by-1024; render-target switches occur before presentation.
Resource extents and final composite/stretch mapping were not established by the
bounded trace query. The equivalent logical-frustum implementation does not
reproduce those internal render targets, and cross-mode presentation parity
requires rendered comparison. The background factory's full-window behavior is
independent retained static evidence, not inferred from those two dimensions.

Raw traces, captures, checked-reader input hashes, the complete topology join and
command receipts stay in ignored local evidence directories. No original assets,
model arrays, images or generated content are included in this change.

## Remaining fidelity work

Six synthetic tests cover logical scaling, dynamic resize, subview cropping,
frustum/culling and malformed pose refusal. The initial camera application gate passed
266 tests with eight explicit original-input ignores. Separate required RoC/TFT
scene import and retained material-binding gates pass, as do strict all-target/
all-feature native/WASM checks and an isolated release WASM/bindgen/package build.
The 23-file package contains only code, WASM and decoder corresponding source.
A subsequent local original-fixture Main/Video Options/Cancel/Main browser gate
passes four captures, with no uploads, sockets or page errors. The scene fills
the background surface and retains readable source UI; this is an application
workflow check, not native visual acceptance. Its fog appearance prompted the
separate material correction described in `menu-authored-fog.md`. The combined corrected release separately passes the same workflow, 273 native
tests, strict native/WASM gates and actual WASM import cases. Publication is
tracked separately; these checks do not establish native presentation parity.

Camera position/target/roll tracks retain the current static-camera limitation.
Fog, lighting, particles, layered materials, sky/water appearance and screen
transitions remain separate work. Bevy's infinite reverse-depth projection plus
far-frustum culling is not proof of bit-identical finite native GPU clipping.
Mathematical and ECS tests do not establish final rendered/native parity.

## Widescreen presentation

The logical frustum above is unchanged. Retained original runtime main-menu captures at
1920 by 1080 and 2560 by 1440 are a centered 4:3 picture with black side bars.
Their content boxes match the UI viewport already computed for those sizes:
origin (240, 0) and 1440 by 1080, and origin (320, 0) and 1920 by 1440. The
1024 by 768 capture fills the window. Square and 5:4 clients keep the existing
full-window branch; those shapes were not observed in this capture set.

The scene camera now uses that same viewport; a separate camera clears the
bars to black while the menu exists. This reproduces the observed final
presentation without claiming the same internal render-target composition.
A local browser build of this viewport change has produced reviewed main-menu
captures at 1920 by 1080 and 1024 by 768. The wide capture has the centered
1440 by 1080 picture; the compact capture fills its canvas. Both pass the
capture adapter's exact canvas/viewport geometry checks. The remaining Options
sequence hit its operation deadline, so this is not full workflow acceptance.
The integrated native application suite passes 340 tests against stable inputs.
The actual-window startup and render-size transaction checks also pass; these
checks do not draw the menu or close the Options capture gap. Logs are retained
under `.scratch/menu-ui-native-root-20261017/verification-5/`.

The actual imported ISO candidate matches its retained MPQ byte-for-byte.
Fresh reads of the background, skin, font, fog configuration and two expansion
panels match their selected Legacy-install members; all four source-archive
fingerprints in the fixture metadata match the original capture's recorded
installation. This narrows input uncertainty without establishing the original
runtime's winning archive or matching animation phase. Background contrast,
panel appearance and font fringes still differ. Evidence stays local under
`.scratch/startup-current-baseline-20261017/attempt-4/` and
`.scratch/startup-current-asset-qualification-20261017/`.
