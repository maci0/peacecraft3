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
NoDepthTest (`0x40`) and NoDepthSet (`0x80`) are applied by the menu material,
independently of this cutoff. Khronos `glDepthMask` leaves the depth buffer
unchanged when the depth test is disabled, even when the mask is true
(<https://registry.khronos.org/OpenGL-Refpages/gl4/html/glDepthMask.xhtml>,
<https://wikis.khronos.org/opengl/GLAPI/glDepthMask>). The pinned viewer's
`Layer.bind` reaches that state by disabling `DEPTH_TEST` before it sets the
mask, so `0x40` selects `CompareFunction::Always` and forces the write off.
`0x80` forces the write off and leaves the compare. Clear bits leave
`GreaterEqual` and the blend-coupled write. The mapping is the pinned
mdx-m3-viewer, war3-model, and Warsmash profile. Original Warcraft III depth
execution remains unobserved. No native execution or screenshots selected the
cutoff. Native cutout parity remains open.

Synthetic ECS tests cover actual source-layer material construction, mode
separation, preservation of authored material alpha
and the other blend gaps. Installed-target source assertions cover combined-alpha
ordering, below/at/above-cutoff admission and a composed-alpha case that differs
from testing texture alpha alone. These are source/component checks, not GPU or
original-runtime tests. Compare original cutout edges and animated layer/geoset
opacity in the selected menu at multiple resolutions before accepting rendered
fidelity. [ONE/ONE additive](menu-additive-blending.md) now has a separate menu
material; Modulate2x remains an explicit gap.

The combined application gate passes 273 native tests with eight explicit
original-input ignores, strict all-target/all-feature native/WASM Clippy, and the
paired original-menu scene gate. Commands and actual logs are retained in
`.scratch/menu-cutout-root/`. The isolated release passes actual web WASM import/refusal checks. A fresh local
original-fixture Main/Video Options/Cancel/Main run passes four captures and
source-label/control checks, with no uploads, sockets or page errors. The
captures show the scene and source panels but do not isolate animated cutout
coverage or depth order; those native/rendered parity questions remain open.

## Native backend qualification (2026-10-19)

The reviewed Legacy client has a different backend contract from the current
Mask convention. Its render-state block copies the blend selector into
renderer field `+0x3ac`. The nonzero-selector branch at `0x7df01f` enables both
alpha blending and alpha testing, reads source/destination factors from image
tables and selects GreaterEqual for the alpha comparison. Helper `0x7d3380`
reads the byte-reference word from the supplied image table at `0x10df96c`.
Backend selector one uses SrcAlpha/InvSrcAlpha. This state does not turn surviving
source alpha into one. The installed Bevy Mask helper does, and its current
project cutoff is below the decoded backend-selector-one cutoff.

Thirty-one instruction checks, six bounded executions of the reference helper
and a rejected shifted-table mutation qualify those backend facts. A synthetic
alpha between the two cutoffs distinguishes their admission decisions. These
are source/instruction checks, not GPU or pixel acceptance. No measured threshold
has been copied into production, and no opacity or rendering behavior changed.

The panel caller still needs a complete join from the parsed MDX layer through
its runtime state packet, including instance overrides, to this backend selector.
The game layer reader at `0x8fc830` matches the established editor reader after
masking relative call operands, but that alone does not establish the later
selector mapping. The retained full D3D9 trace has compatible draw counts and
states; count/FVF compatibility alone does not uniquely identify a panel layer.
Do not apply selector-one behavior to MDX filter one on that evidence alone.

Private receipts are under `.scratch/menu-panel-input-root-20261019/`.
Source-bound cutoff/blend transport, a uniquely associated panel draw, actual GPU
composition and original/native/WASM rendered parity remain open.
Package and browser receipts are in `.scratch/menu-cutout-release/`.
