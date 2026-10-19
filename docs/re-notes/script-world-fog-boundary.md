# World fog boundary on campaign ground

Executed `EnableWorldFogBoundary(boolean)` calls now retain an explicit presentation
switch. The optional non-constant boolean/nothing declaration must share the
`CreateTimer` source; missing or incompatible declarations, overrides and malformed
arguments refuse. The switch begins unknown and allocates no identity or clock.

The campaign ground consumer uses the published script context, matching ground,
binding receipts and retained source reader. With an explicit true switch, and
explicit fog/mask settings both false, it builds a bounded R8 boundary mask from
the admitted W3E and pathing dimensions. False removes the mask contribution and
retires its image. Session changes retire the old material contribution. The
profile color comes from exactly one `FogOfWar/BoundaryTerrain` ARGB field in the
selected `UI\MiscData.txt`; no color or region is fitted to screenshots.
Missing sources, malformed profiles, mismatched contexts and budgets report an
error. Unchanged successful inputs reuse the mask; a failed profile read is not
repeated every frame. Full fog of war requires per-player visibility, which this
ground pass does not supply: enabling the boundary with fog/mask active or unknown
withdraws this pass and reports the unsupported state.

## Evidence and reconstruction

Bounded static reads use executable VERSIONINFO `1.29.2.9231`, SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The functional native `0x48fde0` jumps to `0x703900`, reaches world setter
`0x736b80`, writes `+0x21e8` only on change, and invalidates cached terrain
textures through `0x746400`. The texture update at `0x744982`/`0x744a07` selects
an alternate color-table bank when the switch and raster bit `0x10` are set.
Its color setup names the source `FogOfWar` fields, including `BoundaryTerrain`.
The boundary update `0x746710` tests terrain-record bits `0xc0`, expands each
marked tile on a four-cell grid with a one-cell margin, and clips expansion at
the processed rectangle. Candidate displacement matches were followed through
these readers/writers; unrelated raw matches are not treated as functions.

The W3E v11 serialized boundary and map-edge masks are independently described by
the pinned [Corner loader](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/parsers/w3x/w3e/Corner.java).
The reconstruction combines those two bits and expands the marked lower-left
tile corners, excluding the final corner row/column. Its cell ratio is derived
from both admitted grids rather than an embedded map-specific extent. Source
orientation is retained. Linear mask sampling is a reconstruction choice, not a
qualified native sampler trace. The material blends numeric source channels in
the existing ground material's space; original framebuffer/gamma equivalence,
edge sampling and live pixel fidelity still require comparison.

Static windows and command/artifact receipts remain local under
`.scratch/campaign-fog-boundary-root-20261019/`. No original asset contents or
modified original map are committed. Full fog of war, boundary effects on actors,
initial original boundary state, normal campaign bootstrap and playable first
chapter remain open. Parser or synthetic draw checks do not close those gates.

## Verification and current chapter frontier

The unchanged private Human01 prefix now passes `EnableWorldFogBoundary(false)`
and reaches `GroupEnumUnitsInRect` in Blizzard `198432..198471` (common
`63411..63518`). A later exact source-offset check corrects the earlier caller
label: this is `GetUnitsInRectMatching`, reached through `GetUnitsInRectAll` for
the map's starting-forces rectangle. It cannot be supplied by an empty-group
shortcut or creation-request coordinates. Live unit enumeration, accepted
placement and collision remain part of the playable-world frontier. See the
[full creation-stream replay](script-unit-world.md#full-creation-stream-and-rectangle-caller-2026-10-19).
The actual selected terrain produces a 384×384 boundary mask. Its profile and
map identities remain in the local verification receipt; those derived contents
are not published.

All 1,390 ordinary native test executions pass, plus the actual Vulkan ground
readback on a Radeon RX 7900 XTX. The latter checks source-channel blending and
removal at 64×32, 128×64 and 3840×2160, then checks north/south mask orientation
on the ground plane. Three focused actual WASI tests and thirty-nine executable
Cargo-WASM factory gate cases pass. Strict native/WASM Clippy, affected builds
and formatting pass. The Vulkan test is a synthetic float-target correctness
check, not an original-game pixel comparison or performance threshold. No fresh
original game or browser render was run, and Human01 is not playable yet.
