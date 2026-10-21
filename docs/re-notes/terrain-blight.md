# Terrain blight and regeneration

## Initial map state

W3E corner records carry blight in bit `0x20` of their texture/flags byte,
independently of the texture slot. `pcx-map` already preserves that byte as a
slot nibble and four flag bits; `Corner::is_blighted` names the corresponding
flag without changing file serialization.

Format references: the [mdx-m3-viewer corner reader](https://github.com/flowtsohg/mdx-m3-viewer/blob/master/src/parsers/w3x/w3e/corner.ts)
and [Warsmash corner reader](https://github.com/Retera/WarsmashModEngine/blob/main/core/src/com/etheller/warsmash/parsers/w3x/w3e/Corner.java)
both read that bit. These are format implementations, not observations of the
selected game's regeneration or proof of player-affinity semantics. No code or
original terrain contents are copied from them.

`InitialBlight` reads every flag from the admitted terrain, retaining its map
source identity. Its world origin comes from W3E. Grid spacing is the selected
client's already derived placement cell size multiplied by the admitted map's
terrain/pathing dimension ratio. There is no copied measured world-size literal
and no `Terrain.slk` class-letter substitution for missing state.

The nearest corner owns a half-open square footprint centred on that corner.
This footprint convention and the use of map-authored blight for all owners are
recreation choices pending a valid original-runtime eligibility comparison.
Outside those footprints there is no map-authored blight.

## Regeneration projection

The existing activity projection supplies its accepted straight motion spans.
Initial positions are captured before movement. Regeneration integrates standing
gaps and tails at their accepted position and splits each moving span at grid
crossings. Diagonal crossings visit a cell once. All visited cells consume the
caller's deterministic work budget; zero-length intervals do no work. Pools and
regeneration clocks are staged before any pool is written, and failed public
span output is discarded under the existing session-stop-on-error contract.
Motion effects themselves are not rolled back.

Only sessions with moving blight-regeneration units need the internal span
buffer. Callers already requesting spans share that buffer; no global movement
history is retained. A direct clock rebase with unprojected blight-unit motion
refuses rather than integrating the entire interval at its final position.
Relocation must be reconciled at a projection boundary; position writes and
relocating reveals/pathing changes refuse before mutation when that boundary is
missing. Continuous integration
and its existing f32 pool rounding do not establish native regeneration tick or
delay parity.

## Runtime script changes remain open

Selected source: Legacy 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Static primary registrations point to `SetBlight` handler `0x4a6f10`,
`SetBlightLoc` `0x4a6f70`, `SetBlightPoint` `0x4a6fd0` and `SetBlightRect`
`0x4a7030`. Each resolves its player and forwards a bitmask formed from the
player record. Circle/location call `0x6d98d0`; point calls `0x6d9ab0`.
Separate registrations use the known no-op stub family. These static paths do
not establish which registry is active in a runtime call.

The player ownership mask must survive later setter implementation. Painting,
overlap/removal, the initial layer's relationship to player layers and the
regeneration eligibility predicate still need qualification. This increment
does not admit setters or silently discard their player argument. It also does
not render initial or changing blight textures.

The previous phase-enabled native probe is invalid for eligibility: its unit
was misplaced and its records truncated. See [regeneration](unit-regeneration.md#phase-enabled-blight-probe-one-run-outcome-2026-10-20).

## Verification scope

Synthetic tests exercise all flag combinations, source spacing/origin, budgets,
straight and diagonal crossings, reverse travel, chunk composition, standing
and moving life regeneration, independent mana, and refusal without pool writes.
Parser and host tests do not prove native or rendered fidelity. Original-map and
compile results are recorded only after their checks finish.

Private-target validation on this increment: `pcx-map` plus `pcx-play` package
tests pass 1,275 / 0 failed / 52 ignored (including ten focused blight-related
tests, one of which preserves the missing-state refusal). Strict all-target
clippy on both crates, the WASM playback library check, and native
`peacecraft3 --all-targets --features test-fixtures` check pass. The selected
Human01 startup/order/profile and Human03 patrol-initialization gates pass.

The selected unchanged Human03 ROC startup/order gate now reaches tick 155,
where its movement work budget exhausts. The previous first-tick missing-blight
refusal is gone. The broader gate still exits 101, so this is progress through
the recreation, not playable campaign acceptance or native eligibility evidence.
