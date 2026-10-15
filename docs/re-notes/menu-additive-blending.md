# Source-selected menu additive blending

Classic layer filter mode 3 requests ONE/ONE RGB addition. The previous menu
binding used Bevy Add, whose shader multiplies RGB by alpha first. The shared
menu geometry material now selects the existing StandardMaterial Premultiplied
shader branch for mode 3 and specializes only the forward color blend to
ONE/ONE/Add. Mode 4 retains ordinary Add. No shader fork, texture substitution,
color adjustment or measured asset value is introduced. PRE2 and the UI logo
have independent material paths.

The extension key derives from the checked source filter word, not from Bevy's
base alpha approximation. All source materials are prepared before any model
asset/entity writes; an unsupported or inconsistent later layer refuses the
whole batch. The same store serves background and panel geometry. Opacity,
KMTF and TXAN updates mutate the base material while retaining its handle and
specialization key; source sampler and ordering admission remain unchanged.

The RGB equation does not establish original framebuffer-alpha behavior. The
explicit compatibility policy preserves destination alpha, like the old Add
path. Ordered rendering is retained rather than silently selecting weighted
OIT. Existing zero-opacity visibility, first-layer-only rendering, cutout
convention, Modulate2x and depth/culling gaps remain separate fidelity limits.

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
