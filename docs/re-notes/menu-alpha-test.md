# Classic MDX Transparent reconstruction convention

Classic LAYS filter mode 1 (Transparent) maps to Bevy `AlphaMode::Mask(0.75)`.
The cutoff is a selected renderer convention, independently specified by primary
MDX renderer code; it is not copied from a measured game asset or screenshot.
Static layer alpha and authored KMTA/GEOA sampling remain unchanged. This mapping
applies to model material layers, not PRE2's separate filter-mode enumeration.

## Primary evidence and implementation choice

The pinned [war3-model layer renderer](https://github.com/4eb0da/war3-model/blob/52d7f421a5505a0434d764294057f2009ce3987b/renderer/modelRenderer.ts#L3648-L3670)
sets Transparent's discard threshold to 0.75. Its WebGPU SD path independently
sets the same value at line 1187. These are renderer literals selected from the
layer's mode, rather than an alpha-cutoff value read from LAYS.

The pinned [mdx-m3-viewer layer implementation](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/viewer/handlers/mdx/layer.ts#L62-L72)
classifies Transparent with the opaque modes for depth writes and leaves blending
disabled; only modes above Transparent enable blending. Its separate authored
layer alpha and KMTA sampler remain present in the same class.

The same pinned [vertex shader](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/viewer/handlers/mdx/shaders/sd.vert.ts#L45)
composes vertex, geoset and layer alpha. Its [fragment shader](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/viewer/handlers/mdx/shaders/sd.frag.ts#L32-L45)
multiplies sampled texture color by that varying before testing combined alpha
below 0.75; equality survives. Thus one renderer supports the selected cutoff,
composed-alpha order and unblended classification together. The war3-model
WebGL implementation differs by enabling source-alpha blending. These are
published renderer conventions, not original-native proof.

The installed Bevy 0.20 source establishes the target behavior: StandardMaterial
multiplies material base color and sampled texture color in `pbr_fragment.wesl`,
then `pbr.wesl` invokes `alpha_discard`. In `pbr_functions.wesl`, Mask discards
combined alpha below the cutoff and sets surviving alpha to one. Authored sampled
opacity remains in material base color before that test. PRE2 is unaffected.

## Limits and acceptance

The primary viewer retains surviving combined alpha; Bevy Mask sets surviving
alpha to one. Admitted coverage and unblended source RGB agree on these points,
while framebuffer alpha and subsequent use require separate evaluation. Authored
NoDepthSet/NoDepthTest overrides are not implemented by this mapping. No native
execution or screenshots selected the cutoff. Native cutout parity remains open.

Synthetic ECS tests cover actual source-layer material construction, mode
separation, preservation of authored material alpha
and the other blend gaps. Installed-target source assertions cover combined-alpha
ordering, below/at/above-cutoff admission and a composed-alpha case that differs
from testing texture alpha alone. These are source/component checks, not GPU or
original-runtime tests. Compare original cutout edges and animated layer/geoset
opacity in the selected menu at multiple resolutions before accepting rendered
fidelity. ONE/ONE additive and Modulate2x gaps remain separate.

The combined application gate passes 273 native tests with eight explicit
original-input ignores, strict all-target/all-feature native/WASM Clippy, and the
paired original-menu scene gate. Commands and actual logs are retained in
`.scratch/menu-cutout-root/`. The isolated release passes actual web WASM import/refusal checks. A fresh local
original-fixture Main/Video Options/Cancel/Main run passes four captures and
source-label/control checks, with no uploads, sockets or page errors. The
captures show the scene and source panels but do not isolate animated cutout
coverage or depth order; those native/rendered parity questions remain open.
Package and browser receipts are in `.scratch/menu-cutout-release/`.
