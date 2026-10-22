# Executed unit models and animation

This is first-mission work in progress. Human01 is not playable. The normal
campaign factory now installs a session-owned model loader when its selected
reader and logical timer clock are available. Ordinary Host input preparation
still has no production producer, and later mission orders remain unsupported.

The model path comes from the admitted definition's UnitUI/file metadata,
including a scalar custom-object override. Model bytes, source identity and
SEQS records are retained on the existing executed unit. Shared paths reuse the
same immutable model. A failed read precedes unit allocation and does not consume
creation capacity. Extensionless model paths receive the binary-model extension;
missing or malformed sources and unsupported variation lists refuse.

`SetUnitAnimation` selects an authored sequence on that unit, with no replacement
unit or handle. Local clips use the logical game clock, wrap or clamp according
to SEQS, and preserve the actor's global elapsed clock across clip changes.
Exact names take precedence; token-prefix variants currently use source order.
That variant choice is a recreation convention, not established original rarity
selection or RNG call order. The original engine's implicit seed remains a
separate open question.

`QueueUnitAnimation` appends a resolved sequence index to a bounded per-unit
FIFO. Name resolution and a fallible queue reservation happen before any logical
mutation; an absent name is a no-op and overflow refuses. The current clip plays
to its authored completion boundary, then each queued clip starts exactly at the
previous boundary. A single large simulation step drains every boundary it
crosses. One queued clip covers one authored interval even when its SEQS record
loops; after the queue drains, Stand is selected at the final boundary. These
one-interval looping and Stand-fallback rules are recreation choices: queued
capacity, loop behavior and terminal selection still need a native runtime
comparison. The reviewed client's Queue handler at `0x4a4ae0` forwards flag `2`
to `0x4aafc0`; Set uses flag `0`. Model admission now source-checks both
`unit, string -> nothing` declarations. A successful explicit Set or engine
animation selection clears queued intent; a missing sequence selection leaves
the queue untouched. Renderer sampling remains immutable, and activity ticks
advance the queue in place rather than cloning it. Pending queues keep an idle
unit on the activity advance list, paid for by that list's existing per-unit
work unit.

The actor consumer reads published VM units and accepted positions, not imported
placement ordinals or the original CreateUnit coordinates. Terrain height, class
scale, movement height, tint and creation color remain source-derived. The
selected skin supplies palette prefixes and palette count; inventory lookup
retains the complete prefix, so an unrelated cinematic texture with the same
leaf name cannot become a unit's team texture. The metadata's team-color enum
keeps its string-wire map override. Both source/reader lifetime and actor identity
are scoped to the published session.

Every material layer is projected into a separate draw, preserving the authored
material/layer relation, UV set, opacity, texture-selection tracks and GEOA
association. The shared renderer retains its existing inconsistent-blend refusal
for ordinary callers. Transparent draws retain per-actor source ordering within
that actor's existing phase slots. This does not yet qualify opaque/cutout
multipass scheduling, model culling, billboard orientation or rendered fidelity.
Source DNC unit lighting remains unimplemented in this actor consumer.

## Retained checks

Local evidence is under `.scratch/campaign-unit-animation-root-20261020/`.
The unchanged original map completes all five authored animation calls. All
155 executed units retain source models. Handle/rawcode, requested-coordinate
and accepted-position bit records match the prior original-qualified replay;
this is a regression comparison, not a new reference-game run. The next script
refusal is `IssueImmediateOrder("holdposition")`.

The isolated CPU ECS audit runs the production actor spawn/update functions on
those 155 executed records: all actors pass, producing 694 layer draws and
84,818 layer vertices; 109 actors change posed positions during the sampled
logical interval. No model or material source failed. The audit uses local
copies of the selected textures and never publishes the partially executed VM.
It is not a GPU capture or a normal-app playability result.

The host suite passed 583 tests, with eight explicit local-input ignores. The
application suite passes 418 tests (33 explicit ignores), and map integration
passes 148 (one explicit ignore). Four animation/model cases pass in the actual
WASI runtime, including failure-before-allocation. Strict native/browser checks,
both full builds, formatting and diff checks pass. The unchanged host sources
and final actor sources are hashed in the verification receipt.

These checks do not close the original/native/WASM deterministic or rendered
comparison gates. Models without SEQS are not present in this mission's audited
model set; the current static fallback does not advance their global animations.
Shader lighting, posed normals, source culling and billboard behavior need actual
render acceptance before this consumer is faithful.
