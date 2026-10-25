# Placed doodad animation

Decorative instances come from the selected map's `war3map.doo`; model paths,
variation counts and vertex colours come from the selected `Doodads.slk`, its
metadata and `war3map.w3d` edits. The source creation number identifies an
instance inside the session, independently of script handle tokens. A shared
canonical instance carries its animation and visibility; the renderer reads
that state. Replacing the session retires the corresponding scene roots.

The loader resolves the requested variation against the selected asset inventory.
Missing or ambiguous models remain explicit per-instance errors. It does not
substitute a stock prop or infer one from the mission name. Placement preserves
the authored Z, angle and three scale components rather than grounding every
model at terrain height or applying an extra default model scale.
Repeated colour overrides select the authored variation through their metadata
selector. Missing, out-of-range or unsupported selectors refuse instead of
silently falling back to the default table colour.

## Native evidence

REA/Ghidra inspected the selected executable with SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
These are native-code observations, not a rendered comparison:

- The primary `SetDoodadAnimationRect` registration selects `4a7cc0`. It resolves
  the rectangle and forwards a copy of its bounds, the requested type, animation
  string and boolean to `7056e0`.
- `751d90` filters by type, with `-1` selecting all types. Its point-selection
  branch includes rectangle edges. A separate footprint/bounds-overlap branch
  exists; point tests alone do not establish that branch.
- `75d370` stores an animation request on each selected instance. It also handles
  `show`/`hide` and loop-sound requests separately from animation names.

| Role | VA | Bytes | SHA-256 |
|---|---:|---:|---|
| Registration | `4a0f8a` | 20 | `416a95f51b1b49f988ece10f0ba96253a2d74af955f09e649e27d23f581014dc` |
| Rectangle entry | `4a7cc0` | 86 | `ddfae5731a372ccabe1bfaf64868588c592ab90a84c4293aca8428383acc9d66` |
| Enumeration adapter | `7056e0` | 186 | `dfca5fee4e3590e1dd5fef483303fab53c8c5430316ca8e59728e51ccb880f86` |
| Selection | `751d90` | 384 | `4a76091e1771f0a4358a08125f4424fe609612e66754b5290e28e6d5d22f70f2` |
| Instance request | `75d370` | 426 | `4bf88f7bb9febc1e6be2569a513cf598b52858d612339abf995afab7b3fa3790` |

The unchanged prologue calls the Blizzard helper with `stand` and `death`;
that helper passes `false` for random playback. Both lightning and tutorial
marker models have authored geometry, sequences and unparented PRE2 emitters.
No model path, clip interval, colour or placement was copied into production
constants to accommodate those calls.

## Implementation and remaining gates

The host validates the owned declaration and preflights the entire matching
animation batch before publishing changes. An unsupported matching model leaves
earlier matches unchanged. Clip sampling uses the existing logical animation
clock, including nonlooping completion. Null rectangles return normally.

Drawing reuses the existing source geometry/material/animation pipeline and PRE2
simulation. Source-flagged particle quads sort within their emitter. Executed
clip changes retain live particles, the random stream, fractional emission and
global-sequence time. Scene placement and simulation are separate from the
native's internal object layout.

Random playback, loop-sound control and footprint-overlap rectangle selection
remain contextual refusals. The radius/nearest native is still unsupported.
Dynamic doodad lights, model-event sounds, fog visibility policy and a complete
original-rendered comparison remain open. Do not count this slice as complete
doodad presentation or complete scripting support.

The offscreen unchanged RoC prologue run now captures its menu, chapter selection
and post-skip world. Decorative rocks, logs, grass and tutorial markers reach the
scene. The capture also exposes remaining world/presentation work: destructable
trees are separate from these decorative instances, world fog is incomplete,
and empty bottom-console regions still expose the scene. Console viewport and
content masking need a source/rendered comparison; this observation does not
establish their original dimensions. The strict run retains its sky-illumination
and sight-occlusion refusals rather than treating the entry image as parity.

## Deferred particle publication (2026-10-25)

The campaign-entry replay exposed a host lifetime error: decorative projection
queued its particle entities through Bevy `Commands`, then advanced those same
particles before the entities existed in the query world. The particle ownership
check correctly rejected that premature call. The former unconditional sky error
had masked this diagnostic in the final renderer report.

Actor projection and doodad-particle advance now run as chained systems with the
standard deferred-command publication boundary between them. The second phase
rechecks the script-session witness and uses the canonical rows and game clock.
It catches up to the authored animation's current time on that first published
frame; it does not skip a visible frame or discard emission. Later missing
entities, meshes or materials still use the existing ownership refusal. Hidden
doodad roots also receive their canonical visibility in the spawn bundle, so
publication cannot expose them for one frame before the first query update.

A synthetic production-projection regression first reproduced the same ownership
error. It now verifies nonempty particle geometry on the first published frame,
clock catch-up, a later real emitter deletion refusing without advancing the
clock, and hidden-to-shown root visibility. All model, texture, clock and source
profile values in the fixture are synthetic. The underlying particle simulator,
source animation records, random state, colour conversion and ownership checks
are unchanged. Final local gates and the explicit offscreen campaign replay are
retained in `.scratch/campaign-particle-publication-20261025/`.

This fixes publication ordering. Parented PRE2 emitters are now admitted from
the joint plan that already owns their pivots. Ordinary particles keep the
world pose sampled at birth. Model-space particles follow the current ancestor
pose and are not rewritten in the simulation. A hidden ground item still
publishes no root, and unknown item visibility still refuses. Graphics tests
passed 62 with 3 ignored. The selected `FireRockSmall` model passed its
separate admission gate. Item birth passed 1 and doodad publication passed 2.
`pcx-gfx` Clippy passed. Native app Clippy stopped in unrelated `pcx-play`
ability-tooltip code, so WASM Clippy did not run. The earlier offscreen replay
remains exit 101 and still records the old parented refusal plus sight
occlusion. This does not establish complete original-client particle parity,
dynamic lights, sound events or world visibility fidelity.
