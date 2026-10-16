# Source-selected menu additive blending

Classic layer filter mode 3 requests ONE/ONE RGB addition. The previous menu
binding used Bevy Add, whose shader multiplies RGB by alpha first. The shared
menu geometry material now selects the existing StandardMaterial Premultiplied
shader branch for mode 3 and specializes the forward color blend to
ONE/ONE/Add. The same specialization applies shading bits `0x40` and `0x80`.
`0x40` selects `Always` and forces the depth write off. `0x80` forces the
write off and leaves the compare. Mode 4 retains ordinary Add. No shader fork,
texture substitution, color adjustment or measured asset value is introduced.
PRE2 and the UI logo have independent material paths.

The extension key carries the checked source filter word and shading bits
`0x40` and `0x80`. All source materials are prepared before any model
asset/entity writes; an unsupported or inconsistent later layer refuses the
whole batch. The same store serves background and panel geometry. Opacity,
KMTF and TXAN updates mutate the base material while retaining its handle and
specialization key; source sampler and ordering admission remain unchanged.

The RGB equation does not establish original framebuffer-alpha behavior. The
explicit compatibility policy preserves destination alpha, like the old Add
path. Ordered rendering is retained rather than silently selecting weighted
OIT. Existing zero-opacity visibility, first-layer-only rendering, the cutout
convention, and Modulate2x remain separate fidelity limits. Culling is
unchanged. Filter mode 6 keeps the opaque depth write when both depth bits
are clear; either bit forces that write off. The pinned viewers suppress the
Modulate2x write even with both bits clear. A blend layer with `0x40` compares
always and keeps its depth write off. Native depth order and original Warcraft
III execution of these bits remain unobserved.

## Verification boundary

The actual Bevy material pipeline draws synthetic linear textures into a
blendable RGBA16Float target, with HDR, unlit materials, fog/tonemapping/debanding
off and MSAA off. Eighteen cells cover texture alpha zero/interior/one, composed
base alpha, overlapping additive layers and an alpha-blended interloper. All
72 channel checks pass on both llvmpipe and Radeon RX 7900 XTX/Vulkan, maximum
absolute error 0.0004022 within a declared 0.002 half-float tolerance. An actual
old-Add negative draw fails 27 channels across all nine mode-3 cells while
mode-4 cells remain correct. This is a bounded synthetic material-pipeline test,
not original-game or complete menu visual parity.

The native app suite passes 245 tests with 11 explicit local-input ignores;
strict native/WASM checks and builds pass. The qualified Legacy RoC/TFT scene
import passes. The explicitly selected compatibility-tree panel spawn gate
passes separately. The Legacy panel gate still refuses its sound compression
mask 0x81 before geometry construction; this is not a passing panel gate.
Evidence, including failures, is retained in `.scratch/menu-additive-root-20261016/`
and `.scratch/menu-additive-bevy-gpu-20261016/`.

The fresh asset-free package also passes the unchanged local browser gate:
Main at two animation times, Video Options, then Cancel back to Main. All four
captures were reviewed. The source scene admits its geosets and textures with
no missing dependencies or old mode-3 warning. Fifteen local code GETs, no
uploads, sockets or page errors were recorded. Software-adapter capability
warnings remain explicit; there was no new pipeline error. This is local
packaged execution, not a public deployment or original-game comparison.

The retained test ISO predates mandatory panel sound-scale admission. Its two
earlier runs stopped before rendering on missing metadata and remain failures.
The refreshed 150-member local fixture preserves all retained member hashes,
adds explicitly selected source metadata, and retains the original packed
unsupported sound bytes and their checked archive flags/sector framing. Fresh
Rust WASM hydration passes before the browser run. No source skin was edited,
codec refusal bypassed or sound transcoded. The Legacy panel sound still
refuses mask 0x81 and its panel remains unavailable, visibly separate from the
working background and Options controls. The fixture and captures stay local.

The UI logo looks gold/blue in these captures, but its full material composition
and comparison against a running original remain independent questions; this
geometry change does not claim a separate logo-color fix or full menu fidelity.

## Authored depth flags: current verification

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
