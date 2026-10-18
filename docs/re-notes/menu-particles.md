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
checked source-space rotation. XY pieces retain the selected primary convention;
tail width follows the selected client's camera-plane normalization described
below. No second coordinate fit or depth bias is introduced.

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

The integrated application passes 446 affected native tests (18 explicitly
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
WebGL rendering, not a physical-GPU or native comparison. Empty emitters now keep
their CPU mesh without requesting GPU extraction. Populated meshes re-extract
under the same handle; retirement stays hidden and retains scene ownership.
Actual Bevy extraction and live/empty/live scene tests cover that lifecycle.
A fresh release/browser run preserves the four views and reports zero empty-mesh
allocator warnings, uploads, sockets or page errors. Other material/capability
gaps remain explicit; this is not a warning-free-runtime or original-parity claim.

Receipts remain local in `.scratch/menu-particle-root-integration/`,
`.scratch/menu-particle-black-fog/`, `.scratch/menu-particle-black-fog-release/`
and `.scratch/menu-particle-final-release/`. The empty-mesh correction has new
native/original/WASM receipts in `.scratch/particle-empty-root/` and release,
actual WASM and browser receipts in `.scratch/particle-empty-release/`.
The adapted Bevy fragment's MIT
license accompanies its source and the public code package. No original game
payload is part of either publication.

RNG/initial ages, fixed-tick integration, raw emission rate without a guessed
quality factor, head corner order, piece submission order and signed priority
ties are explicit recreation choices. Particle lighting/normals, source
color transfer, radial fog, additive target-alpha behavior and density/spread
still require comparison to the original presentation. Bevy Add provides the
selected additive RGB equation through premultiplication; its target-alpha
semantics are not claimed identical. SortFarZ, parented/model-space emitters,
squirt and other unsupported channels remain reported gaps. Tests do not close
rendered/native fidelity.

## Circular emission cone (2026-10-19)

Non-line emitters now sample one inclination within the authored latitude and
a separate full-circle azimuth. Previously, two independent latitude-limited
tilts could place particles outside that cone. The source-facing line plane,
node rotation, authored speed/variation, extents, opacity and emission rate keep
their existing paths. The unused quaternion-product helper is removed.

Fresh bounded disassembly uses the selected Legacy client SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`:

- PRE2 dispatch `0x8f3b21` reaches reader `0x909930`, whose third scalar is stored
  at record `+0xdc`. Binding `0x9251f0` converts it from degrees to radians and
  passes it to the plane emitter's inclination setter `0x97f5a0`.
- When source flag `0x20000` is clear, the binding separately supplies a full
  turn to azimuth setter `0x97f5c0`. The line branch skips that setter.
- RTTI associates `CPlaneParticleEmitter` with spawn method `0x97f1c0`. Its
  velocity combines sine/cosine of those two angles. The math helper fallbacks
  contain `FSIN` and `FCOS`; they are not named from a screenshot inference.

The local `native-cone-2.json` receipt checks thirty instructions and rejects an
azimuth mutation. A first receipt attempt refused an incomplete instruction at
its window boundary; that failure remains retained. The historical analysis
snapshot was refused for an analysis-profile mismatch and is not presented as
a fresh successful analysis. No original-game runtime was launched for this
increment. The complete bone-to-plane matrix and extent-axis equivalence remain
unqualified; this correction establishes the circular angular support, not an
identical native particle trajectory.

The synthetic cone regression fails before the correction and passes afterward.
All 156 graphics tests and 654 application tests pass, with nine and thirty
explicit ignores respectively. A private wrapper compiles the same production
CPU module and integration-test source as actual WASI code: 21 tests pass and
the explicit original-file test remains ignored there. Two native tests read
the selected TFT model in place and exercise all three clips and all particle
profiles. Strict native/browser-WASM Clippy, both application builds and
formatting pass.

Fresh native and browser-WASM journals cover Main, the blank Options landing,
Video and return at five extents through 3840×2160. Native capture now calibrates
the active RandR output before application startup. Native platform scale and
browser DPR remain separate inputs. Forty new captures are
compared with twenty retained WC3/Wine references; all sixty exact RGB
comparisons still differ. The private evidence and report are under
`.scratch/menu-particle-policy-root-20261019/`. Animation phase, particle PRNG and
floating-point equivalence, quality scaling, native Windows hardware and full
rendered parity remain open. No pixel exclusions, opacity fit, quality multiplier
or public-site update is part of this correction.

## Ballistic gravity (2026-10-19)

Live particles now advance from the previous velocity using
`z += vz * dt - gravity * dt * dt / 2`, then update `vz -= gravity * dt`.
The previous semi-implicit update used the new velocity for the entire step,
adding downward drift. Authored gravity and the existing coordinate, lifetime,
fixed-step, work-budget and RNG policies retain their existing paths.

Fresh selected-client disassembly joins PRE2 reader `0x909930`'s gravity field
`+0xf8` through binding `0x9251f0` and setter `0x97e4e0` to emitter field `+0x160`.
The live branch in `0x97d170` explicitly subtracts the half-acceleration position
term before updating velocity. Thirty-one instruction checks and a rejected
missing-half-term mutation are retained in the private `native-gravity-3.json`
receipt under `.scratch/menu-particle-gravity-root-20261019/`.

The synthetic trajectory regression fails before the fix at height 16.5 instead
of 17.0 after one second. It passes at both 125 ms and 250 ms integration steps
afterward. Those binary-exact synthetic inputs check the individual trajectory;
they do not prove identical emission timing or complete state across different
step policies. All 157 graphics tests, 654 application tests and 22 actual WASI
particle tests pass, with nine, thirty and one explicit ignores respectively.
Both selected-TFT CPU/quad gates, strict native/WASM Clippy, application builds
and formatting pass. The selected-model birth/update/retained work counters
match the preceding cone candidate without elapsed-time or FPS thresholds.

The rebuilt native application completes the four-view 4K journal. The first
browser run times out awaiting the trusted Open menu click acknowledgment;
its failure and clean Chromium shutdown remain retained. One isolated diagnostic
uses scheduled DOM entry and completes Main, Options landing, Video and return
with the same compiled WASM. That verifies rendering, not the trusted entry
workflow. Multiple changed inputs prevent attributing the original timeout to
a specific cause. The diagnostic report keeps those acceptance results separate.
No fresh original-game run or public deployment is part of this increment.

The density investigation also traces a separate shared-emitter rate factor,
its file initializer and finite-value setter checks. Its option-name/current
saved-state join remains unqualified, so no quality multiplier is applied.
Native floating-point identity, initial ages, the native clamped-time lifecycle,
particle PRNG and full three-target rendered/simulation parity remain open.

## Trusted 4K entry follow-up (2026-10-19)

The same sealed application completes the unchanged trusted-click workflow in
an isolated run and alongside the native 4K renderer. Read-only event observations
retain trusted pointer/mouse/click delivery and the handler's canvas focus handoff.
Both full eleven-event journals reach Main, the blank Options landing, Video and
Main return. A third browser journal runs the tracked worker without those event
observers and also passes. The native journal passes after the concurrent browser
run. One earlier native attempt missed its operator visual-review deadline and
is retained as a failed experiment, not evidence about browser entry.

This does not identify the earlier click timeout's cause. Concurrent menu readiness
took longer, but the observed click itself returned; no click timeout or input
sequence was changed. Error-path process tests independently exposed a harness
bug: a retained console log could exceed the worker's failure-response ceiling,
discarding its structured error. The worker now bounds diagnostic tails, declares
omissions/truncation and includes the failing stage. The regression fails before
and passes after, including Unicode logs. All 51 Python harness tests and five
browser-adapter tests pass, including the explicit real-browser canvas case.

Sixteen new 4K checkpoints are visually reviewed. The three-target report uses
the tracked worker's four frames, four fresh native frames and four retained
WC3/Wine frames. All twelve exact RGB comparisons differ; no version pixels are
excluded. Source application inputs and artifacts are unchanged from the gravity
candidate; only harness diagnostics changed. No Rust rebuild, fresh original-game
run or public deployment is claimed. Receipts and raw comparison reports remain
local under `.scratch/browser-entry-ack-root-20261019/`. Reliable entry, original
RNG/phase, native Windows and full rendered/simulation parity remain open.

## Camera-plane tail width (2026-10-19)

The shared quad builder previously divided a tail's perpendicular by its full
three-dimensional length. That narrowed oblique tails as their depth component
increased. The selected client instead forms the perpendicular from camera-space
displacement and divides by `sqrt(max(x*x + y*y, 1))`. Depth still determines the
tail endpoint, but is absent from the width denominator. The numerical unity floor
tapers subunit projected tails without dividing by zero. Authored tail length,
scale, color, opacity, UVs and emission rates retain their parsed asset paths.

Fresh offline evidence from the same qualified Legacy executable joins the
PRE2 tail-length reader through binding `0x925414` and setter `0x97e6c0` to
the tail branch in `0x97d9a0`. The branch transforms negative velocity, multiplies
by authored tail length, sums only the two view-plane squares, clamps that sum
to unity, takes its square root and divides the sampled scale by it. Forty-one
instruction checks include the square-root helper and reject a mutation that
substitutes depth into the denominator. This establishes the width calculation;
it does not re-prove the complete vertex winding, UV/color path or native floats.

The selected model has nineteen tail-bearing emitters, including Both-mode
splashes and fog as well as the two identified ray emitters. All profiles pass
the explicit source CPU/quad gates. The oblique-tail regression fails before the
fix at 18.585787 instead of 18. It passes afterward for two camera orientations
and multiple depth velocities. A separate subunit regression and the existing
degenerate/eye-parallel checks preserve finite tapered/collapsed geometry.
All 158 graphics tests, 654 application tests, strict native/WASM checks and
both application builds pass. Actual WASI execution of the production CPU and
quad modules passes 37 tests, with two explicit local-input ignores. Source-model
CPU birth/update/retained counters remain unchanged; no FPS threshold is used.

Five fresh native and five trusted browser journals complete the same four views
at 1024×768, 1280×960, 1920×1080, 2560×1440 and 3840×2160. All forty new captures
are visually reviewed with twenty retained WC3/Wine references. All sixty exact
RGB comparisons still differ, with no pixel exclusions or rescaling. Native
monitor modes are calibrated; browser capture uses actual DPR 1. The original
reference, mixed-source fixture, platform scale inputs and uncontrolled animation
phase remain explicit. No fresh original-game run or public deployment occurred.

Evidence, the retained failed inventory assertion and the corrected source
inventory remain local under `.scratch/menu-plane-transform-root-20261019/`.
Original RNG/initial ages, effective quality settings, animation phase, native
Windows and exact original/native/WASM output remain open.
