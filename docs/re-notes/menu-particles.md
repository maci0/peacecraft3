# Source-driven menu particles

The menu connects classic MDX PRE2 records to a persistent CPU simulation and
Bevy meshes. Every emission property, selected clip, node track, pivot, atlas,
color/alpha/scale segment, texture edge, sampler flag and priority comes from the
same parsed model transaction. No snow texture, measured rate, emitter position
or quality multiplier is substituted into source constants.

`pcx-gfx::mdx_particles` validates the supported root-only profile and owns an
immutable `Arc<Plan>`. Its injected deterministic seed, fixed substep and work
budgets are engine policies. Head, tail and Both retain their authored meanings;
Both produces two pieces of one simulated particle. Signed tracks use the
selected local clip or their own authored global duration. Geometry and particles
share one monotonic submitted clock; local clip wrapping does not reset global
time. Pausing or changing menus uses the existing scene lifetime.

`mdx_particle_render` admits a union of geometry/KMTF and particle TEXS edges,
then checks complete canonical image identity, pixels, dimensions and sampler
ownership before GPU writes. Matching path and sampler reuse a backing; distinct
samplers retain distinct images. Bounded image decoding checks dimensions before
pixel allocation, including JPEG frame metadata through the existing decoder.
Each emitter retains one mesh/material/entity; updates reuse those handles.
Lost entity/mesh/material ownership or a runtime work/capacity failure hides and
disables the whole particle batch once, while preserving geometry.

Unshaded and Unfogged independently control lighting and fog. PRE2 filter 0 uses
source-alpha Blend and filter 1 uses Additive; these are PRE2 enums, not classic
LAYS filter numbers. A particle-only extended material retains the source fog
flag and the camera's actual fog falloff/opacity. Additive RGB attenuates toward
black without colored or directional inscattering; source-alpha particles retain
the standard colored fog path. This is a recreation convention, not proof of
native fog internals. The shader runs fog once before standard tone mapping and
Add premultiplication. Black RGB with nonzero alpha therefore adds no fog-colored
rectangle; alpha-zero pixels were already neutral. Geometry keeps its existing
material path. Camera-facing quads use the actual background camera's
checked source-space rotation. XY pieces and velocity tails follow the selected
primary conventions; no second coordinate fit or depth bias is introduced.

The background camera alone receives the checked geometry/particle source order.
Transparent items are ordered after Bevy's distance sort and before batching,
preserving unrelated items' slots. The declared convention is ascending signed
priority, then geometry/particle class and source ordinal. Earlier depth-writing
passes must share one source plane that precedes the transparent planes. Mixed
depth planes, cross-pass inversions and SortFarZ profiles refuse instead of
silently ignoring source ordering.

Browser candidate hydration checks the same particle/image/profile path before
replacing canonical paths, aliases or revision. Missing/malformed dependencies
reject the candidate. Unsupported particle profiles report a contextual gap and
keep the geometry path; supported emitters are not silently selected from a
partly unsupported batch. Clear, replacement and edition switches use existing
scene/store ownership without a separate particle cache. Original inputs remain
local and are absent from the public code/WASM package.

## Evidence and remaining fidelity

The primary conventions are pinned to
[mdx-m3-viewer](https://github.com/flowtsohg/mdx-m3-viewer/tree/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc)
and the separately retained war3-model reader/renderer source. Their qualified
files and hashes are recorded in `.scratch/menu-particle-source-review/` and
the CPU/render proposal receipts. Static PRE2 width/length use the documented
primary reader convention; the earlier native destination-name disagreement
remains open in `format-mdx-particles.md`.

The integrated application passes 443 affected native tests (18 explicitly
ignored local-input/device cases), strict native/WASM Clippy and formatting.
The separately executed supplied Legacy RoC/TFT ECS gate checks source emitter
counts, material flags, priority/texture bindings and populated TFT particle
mesh buffers against their parsed inputs. Six synthetic fragment draws/readbacks
on software Vulkan preserve black/additive/source-alpha/Unfogged cases; this
small arithmetic gate does not execute the complete PBR fragment.

The corrected code-only release passes actual WASM particle rollback, worker
and fog cases. A local original-repack browser Main/Video Options/Main workflow
admits every source emitter and produces four reviewed captures without uploads,
sockets or page errors. The additive fog rectangles seen in the first integration
capture are removed while particles remain visible. The browser uses software
WebGL rendering, not a physical-GPU or native comparison. Existing material and
capability gaps remain logged, including temporarily empty particle mesh buffers;
there is no claim of a warning-free runtime or original visual parity.

Receipts remain local in `.scratch/menu-particle-root-integration/`,
`.scratch/menu-particle-black-fog/`, `.scratch/menu-particle-black-fog-release/`
and `.scratch/menu-particle-final-release/`. The adapted Bevy fragment's MIT
license accompanies its source and the public code package. No original game
payload is part of either publication.

RNG/distribution, fixed-tick integration, raw emission rate without a guessed
quality factor, head corner order, piece submission order and signed priority
ties are explicit recreation choices. Particle lighting/normals, source
color transfer, radial fog, additive target-alpha behavior and density/spread
still require comparison to the original presentation. Bevy Add provides the
selected additive RGB equation through premultiplication; its target-alpha
semantics are not claimed identical. SortFarZ, parented/model-space emitters,
squirt and other unsupported channels remain reported gaps. Tests do not close
rendered/native fidelity.
