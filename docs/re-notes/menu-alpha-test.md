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

At that qualification stage the panel caller still lacked the join from the MDX
layer to the runtime packet. Compatible draw counts/FVF in the retained D3D9
trace did not uniquely identify a panel material. The default code path is now
joined as described below; a uniquely attributed original panel draw and
arbitrary instance overrides remain open.

Private receipts for that stage are under
`.scratch/menu-panel-input-root-20261019/`.

## Default cutout binding (2026-10-19)

The game layer reader stores serialized FilterMode at raw-layer offset zero.
The material builder at `0x924360` maps modes zero through six to backend
selectors through a jump table at `0x9245c4`. Filter one selects backend one.
The builder writes that selector into shared-layer offset zero, then copies it
to per-instance layer offset `+0x18`. Default submission at `0x91a6e7` passes
layer `+4` to `0x7d0d60`; the virtual setter at `0x7d3980` copies the 24-byte
packet into renderer `+0x398`. Its selector therefore reaches `+0x3ac`, joining
the alpha-test/blend branch above. Fifty-five instruction checks and seven
bounded switch/copy cases qualify this default path. The game/editor reader
fingerprint is checked against both complete image digests. Packet-wide SSE
copies are checked statically, not interpreted with the scalar SSE harness.

The supplied, digest-qualified client now supplies the byte alpha reference to
menu preparation. It is normalized by the byte format range, never copied as
a measured constant. Both background and panel geometry bind that reference
for filter one. Their shared shader discards combined texture/material alpha
below the cutoff, preserves surviving alpha and uses SrcAlpha/InvSrcAlpha RGB
blending. The cutout retains its mask submission phase and depth writes;
authored depth flags retain their existing separate handling. Other filter
modes and PRE2 keep their own paths. Without a supported client profile, panels
report that limitation and retain the existing unbound material convention.

The GPU regression fails on the previous opaque-survivor path, then passes on
the corrected path with independent synthetic texture/material inputs, including
a value exactly at the cutoff. Material and geoset preparation checks cover
binding, source-value preservation and invalid-reference refusal. Private
receipts are under `.scratch/menu-native-cutout-root-20261019/`.

Validation passes 162 graphics, 576 playback and 657 application tests,
strict native/WASM Clippy, both builds, five executed WASI parameter tests,
nine synthetic GPU composition tests and an explicit original-input offscreen
background gate. Five calibrated native and five trusted browser navigation
journals pass at 1024×768, 1280×960, 1920×1080, 2560×1440 and 3840×2160. Both
live targets load the qualified client profile without a cutout fallback.

Forty new native/WASM captures are compared with twenty retained WC3/Wine
captures. All sixty full-frame RGB comparisons differ, with no rescaling or
pixel exclusions. Visual review still finds a darker Video left panel, different
particle rays/density/phase, settings values/availability and pointer/focus cues.
The version pixels remain included because no exclusion footprint is qualified.
These runs are diagnostic comparisons, not identical-input deterministic
simulation or native Windows acceptance. Builds include the preserved foreign
gameplay edits, rather than a clean-checkout build; no public site was deployed.

This is the qualified default material behavior, not proof of every original
panel instance or complete rendered parity. Arbitrary instance overrides,
framebuffer alpha, the separate disabled-depth-test convention, animation/RNG
phase, settings consumers and native Windows remain open.

Package and browser receipts are in `.scratch/menu-cutout-release/`.
