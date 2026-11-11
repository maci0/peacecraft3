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

Indexed `war3map.w3d` edits carry the variation as the first selector with data
`0`. Version-1 tables have no selectors: the variation is the last character of
the 4-character id, so `dvr1` through `dvr9` name variations 1 to 9. The reading
that `0` names variation 10 (`dvr0`) is inferred, not measured in native code:
the selected `UndeadX07c.w3x` ZZcd edits contain exactly those ten ids, and the
metadata row `dvr1` has `repeat` 10. The ZZcd tint is `80,180,220` for variation
1 in that map, measured with `objx dump`.

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

Random playback and loop-sound control remain contextual refusals. The radius/nearest native is still unsupported.
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

## SetDoodadAnimationRect selection (native decode)

Reviewed exe 1.29.2.9231. Name at VA `0xe77600`, registered at `0x4a0f94` with handler `0x4a7cc0`.
The handler reads the rectangle's four floats (rect `+0x24`, `+0x28`, `+0x2c`, `+0x30`) and calls
`0x7056e0`, which calls selector `0x751d90` and applies `0x75d370` to each hit in array order.

Observed selection:

- Records have stride `0x188` (array at `+0x10`, count at `+0xc`). The record type at `+4` must equal
  the requested kind; `-1` selects every kind. Visibility and life are not tested, so hidden doodads
  are selected.
- Footprint cell count at `+0xa0` at or below zero (no pathing footprint): the point `(+0xc, +0x10)`
  is selected when it lies inside the rectangle with inclusive bounds.
- Count above zero: the box is the position ± half-extents, each half-extent is cells × 16
  (`0x10dd5b8` is 32.0, halved), and the facing at `+0x18` rounded to the nearest quarter turn, an odd quarter
  swapping the two extents. The doodad is selected only when the box overlaps the rectangle
  strictly on both axes (`hi > lo`), so touching edges do not select.
- The rectangle floats at `+0x24`/`+0x2c` pair with record `+0x10`, and `+0x28`/`+0x30` with `+0xc`.
  `GetRectMinX` (`0x495080`) reads rect `+0x28`, `GetRectMinY` (`0x4950a0`) `+0x24` and
  `GetRectMaxX` (`0x495040`) `+0x30`, so `+0x24`/`+0x2c` are min and max Y, `+0x28`/`+0x30` min and
  max X, and record `+0xc` is world X, `+0x10` world Y (observed).
- The box helper `0x753040` returns an empty box unless `[+0xa0] > 0`. The facing is raised by
  `0xe92324` (2 pi) while negative, divided by `0xe9231c` (pi/2) with `0xe6957c` (0.5) added and
  truncated (`cvttss2si`); an odd result swaps the half extents. The intersection is
  `max(low, rect low)` against `min(high, rect high)` per axis (`0x751e1f`..`0x751e51`).

### Footprint cell counts `+0xa0`/`+0xa4` (observed, 2026-11-11)

- `0x754450` builds the instance record with both fields zero (`0x754806`, `0x75480c`), then
  `0x753240(record)` fills them when the type id at `+4` is nonzero.
- `0x753240` calls CDoodads vtable slot `0xa4` (`0x75b150`, vtable `0xeaf308`, RTTI `CDoodads`).
  For a type in the map's object edits it uses `0x76aba0`, else the stock database `0x769810`; both
  read the type's `pathTex` string (string field index `0xa` in the stock table, `7` or `8` for an
  edited alive or dead variant) and return
  false when it is empty, `none` (`0xe9dda4`) or `_` (`0xe92074`). With no path `0x753339` stores
  zero in both fields.
- With a path, `0x71abc0(path, &+0xa0, &+0xa4, &pixels)` strips the extension after the last
  separator, appends each of `.tga`, `.blp` (`0xea9710`, 5-byte stride) and calls `0x71a7c0`. The
  TGA branch (`0x71a8d7`..`0x71a8f7`) stores header bytes 12 (width) in `+0xa0` and 14 (height) in
  `+0xa4`; the descriptor byte 17 bit `0x20` and bits per pixel (byte 16) are read at
  `0x71a90d`/`0x71a8d7`. A failed read leaves the fields zero and logs through the error sink
  (`0x7532b2`), which selects the point branch.
- Native half extents are width * 16 and height * 16 world units (`0x10dd5b8` 32.0 times 0.5), not
  the texel order the unit footprint loader (`0x4deb67`) uses: the box here is not transposed.

Recreation: `RectangleSelection` (`placed_doodads.rs`) carries `Point`, `Footprint { width, height }`
read from `<stem>.tga` of the resolved `pathTex`, or `Unresolved(reason)`. A missing, unreadable or
non-TGA texture (the `.blp` branch is not decoded) refuses only when the doodad's type is selected
by the call. The loop bound on the negative-facing normalisation is a recreation guard: the native
loop does not terminate for a huge negative angle. Tests use synthetic doodads and rectangles.

Not decoded: the meaning of the `+0x150` value (5 in this path); the `0x1000000` flag and `+0x14c`
test that choose the alive/dead path in `0x75b150` (the recreation never selects the dead path for
a doodad, since `Doodads.slk` has no dead texture column); BLP size reading.

Effect (`0x75d370`): `soundon`/`soundoff` control a loop sound; `show`/`hide` set or clear flag bit
`0x08000000` at `+0x84`; any other name requests an animation stored once on the selected doodad.
