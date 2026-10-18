# Source-selected menu additive blending

The retained original TFT lens-flare draw uses `SrcAlpha / One` RGB addition.
The recreation now keeps Bevy `AlphaMode::Add`, which weights the evaluated
texture/material alpha once before addition. The previous custom `ONE/ONE`
override bypassed that weight and made transparent texels contribute light.
It has been removed from the shared background/panel geometry material.
Authored alpha, textures, tracks and colors are unchanged. Mode 4 keeps its
existing Add mapping; this observation does not newly qualify that mode.
PRE2 particles and the UI logo have independent material paths.

## Original runtime evidence

The bounded reader joins the retained model's unique flare topology and index
range to the original Wine/D3D9 draw. Its directly preceding applied stateblock
selects source blend `SrcAlpha`, destination blend `One`, blending enabled, and
stage-zero alpha modulation of texture and current alpha. The reader checks
thread, context, stateblock, draw tuple and adjacency. Synthetic wrong-context,
wrong-draw and changed-factor cases exercise the join. [Wine 10.0's enum header](https://github.com/wine-mirror/wine/blob/wine-10.0/include/wine/wined3d.h)
confirms the recorded factor and stage identifiers.

The selected source model is the locally supplied Legacy TFT background with
SHA-256 `15586ffd9dc25b82a4f8ce2e679581f6e14cbcdc0916c7422d89308f010f6293`.
Receipts are `.scratch/menu-alpha-root-20261019/native-blend-1.json` and
`wine-enum-qualification-1.json`; the trace remains local. This is retained
application API state joined by topology, not a fresh native file-open receipt
or proof of every fragment input, alpha-test threshold or framebuffer alpha.
No screenshot measurement supplies an opacity value.

## Current material verification

A real Bevy render-graph regression draws a synthetic texture over a nonzero
background. Four cases cover zero texture alpha, zero material alpha and two
nonzero products. Twelve RGB checks enforce
`background + texture_rgb * tint_rgb * texture_alpha * material_alpha` in an
RGBA16Float target. The old override fails the zero-texture-alpha case; the
corrected path passes all four cases on Radeon Vulkan. All eight synthetic
menu render-graph cases pass, including the retained color/fog/depth checks.
This bounded readback tolerance tests shader arithmetic, not visual parity.

The current application suite passes 651 tests with 30 explicit ignores.
Strict native/WASM Clippy, formatting and both application builds pass. Two
explicit source-model gates cover 101 poses each; five fixed-pose source
background readbacks reach 3840×2160. Full-billboard posing is documented in
[menu-billboards.md](menu-billboards.md).

Fresh native and local packaged WASM applications each complete Main → Options
landing → Video → Main at 1024×768, 1280×960, 1920×1080, 2560×1440 and
3840×2160. Forty new journal captures were visually reviewed against twenty
retained original WC3/Wine captures. All sixty unscaled pairwise RGB comparisons
differ; no pixels are excluded, including the unqualified version-text footprint.
Navigation acceptance passes; exact output acceptance fails. The browser records
85 admitted local code GETs and no uploads, sockets or page errors. All owned
capture runtimes stopped. The public site was not deployed. The artifacts use
one frozen current working tree, including preserved foreign gameplay edits;
these are not clean-checkout build receipts. The report is
`.scratch/menu-alpha-root-20261019/comparison-1/index.html`.


The material extension still applies authored depth flags, uses the shared
fog shader, and retains ordered submission with weighted OIT disabled.
Unsupported equations and inconsistent base modes refuse before scene writes.
Full original fragment inputs, cutout/culling, Modulate2x and destination-alpha
behavior remain separate gates. The current cutout convention is not changed
by copying a measured API threshold into a constant.

## Superseded additive claim

The earlier ONE/ONE interpretation and its 72-channel synthetic readback proved
an explicitly chosen recreation equation, not the original game's equation.
Its old-Add negative control likewise tested that choice. The leading claim in
the previous version of this note and the corresponding tracker item are
withdrawn. Historical receipts in `.scratch/menu-additive-root-20261016/` and
`.scratch/menu-additive-bevy-gpu-20261016/` remain unchanged; they must not be
used as original blend-fidelity evidence. Earlier browser/package checks did
not establish full visual parity.

## Earlier authored depth verification (retained)

The source layer's shading word now reaches the shared background/panel material
specialization. The disabled-test profile also disables writes: replacing an
OpenGL disabled test with wgpu `Always` alone would incorrectly keep opaque
depth writes. Descriptor tests cover opaque, mask, blend and additive modes,
including unrelated state preservation and mode 6's existing clear-bit fallback.

The actual Vulkan overlap readback passes 108 RGB checks over four cases:
ordinary occlusion, a farther disabled-test layer, a nearer no-write layer, and
a nearer disabled-test opaque layer. The last case distinguishes disabling the
write from merely changing the comparison. The existing geoset-colour readback
also passes through the shared harness. Both ran on Radeon RX 7900 XTX/RADV.
The application suite passes 346 tests with 18 explicit ignores. Strict native
and browser-WASM Clippy and warning-denied application documentation pass with
stable source hashes. Formatting checks pass. Receipts are in
`.scratch/menu-depth-root-20261017/verification-1/`.

The matching local browser package also renders reviewed wide and compact Main,
Options landing and Video Options captures. The five-capture run reaches its
480-second collection deadline before completing the resolution-list click;
it exits 124 and is not a complete interaction gate. Both owned reference
clients were resumed and their lobbies visually rechecked afterward. The package
contains no original assets and was not published.

The captures retain visible gaps: the Video Options panel background/border,
popup text placement, unsupported settings, fog/lighting and font edges need
separate work. No screenshot fit or replacement content was introduced. Receipts
and visual reviews are in `.scratch/menu-depth-root-20261017/`.
These checks establish the recreation's material pipeline and the four recorded
browser states; original Warcraft III execution of the bits remains unobserved.
