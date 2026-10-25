# Executed unit models and animation

Whole-mission playability remains in progress. The normal campaign factory
prepares source inputs and installs a session-owned model loader with the selected
reader and logical timer clock. The dated checks below retain their historical
frontiers; the latest controls section records current progress.

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

## Removed animation targets (2026-10-25)

Set/Queue animation now share the removed-unit no-op used for null targets.
The selected `4aafc0` path resolves the target through `483940` and jumps past the
animation work if resolution fails. Canonical removed entries follow that branch;
other handle families still refuse, with the actual animation native as error
context. This lets the unchanged TFT cinematic cleanup retain removed references
without aborting its ResetUnitAnimation helper. Synthetic tests cover removed,
null and wrong-family targets without resurrecting an identity or queue. The
chapter then returns control and advances to its later `CreateCorpse` frontier;
this does not qualify rarity selection or all animation lifecycle edge cases.


## Clip transitions and playback controls (2026-10-25)

`SetUnitBlendTime(unit, real)` and `SetUnitTimeScale(unit, real)` now reach the
retained canonical animation and the campaign mesh consumer. Their optional
nonconstant declarations must match the admitted creation source and exact ABI.
Malformed arguments and nonfinite reals refuse without changing animation;
null and removed units are no-ops, while other handle families refuse.

The model default comes from the final MODL word, in milliseconds. The explicit
blend setter derives its clamp maximum and near-zero disable threshold from the
selected client image at runtime; neither operand is copied into a production
constant. The decoder checks the clamp and disable branch destinations and binds
the image identity to player admission. Truncated MODL data has no guessed default.

A transition retains one flat snapshot of local BONE/HELP poses. An interruption
samples the visible logical mixture before selecting the new clip, so it does not
jump back to the prior sequence. Translation and scale interpolate linearly;
rotation uses a normalized shortest quaternion interpolation. The renderer applies
this mixture before its existing hierarchy, pivot, billboard and look-at work.
Snapshots are shared immutable arrays, not recursively nested animation states.
Globally sequenced channels continue at their existing global phase instead of
blending a stale snapshot. HELP transforms now use the same decoder adapter in
both the logical animation and checked graphics hierarchy.

Playback speed changes preserve the local sequence phase. Zero freezes local
playback and an active transition; positive values resume/scale them and negative
values reverse local playback. Looping intervals wrap in either direction;
nonlooping clips clamp to their authored bounds. Queued clips use the resulting
completion boundaries, including after a freeze. A finished transition cannot
reappear when playback reverses. Global sequences retain the existing independent
elapsed clock. That global-clock policy and the signed-speed behavior are
recreation choices pending original-runtime comparison, not native parity claims.
Existing PauseUnit behavior is unchanged; visible sampling while paused remains
a separate fidelity question. Changing a positive blend duration during an
already active transition currently changes future transitions only; exact native
retiming of an active blend remains unqualified. Particle-emitter simulation
retains its existing scheduling; time-scale equivalence for emitters is open.

Water Elemental presentation now observes actual sequence progress instead of a
fixed unscaled deadline. A frozen Birth clip therefore remains frozen, and a
scaled Spell clip returns to Stand at its visual completion. The ability's actual
cast point, mana, cooldown and summoned-unit lifetime remain on their existing
logical gameplay clock.

### Evidence and remaining work

Local evidence is retained under `.scratch/unit-animation-controls-20261025/`.
The selected image is SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Local static windows join the functional SetUnitBlendTime handler through
`4ab230` to `6c05a0`, its source-pointed bounds and the model blend bridge.
SetUnitTimeScale forwards through `4abbd0` to `69ce00`, which stores the requested
real and conditionally forwards it to the model. The bounded trace does not
identify its `+3f8` condition with PauseUnit. These are local PE/disassembly
observations; REA client approval was unavailable, and no original client was
launched or reconfigured.

Synthetic tests discriminate nonstock MODL duration, decoded bounds, malformed
branches/truncation, interruption, quaternion direction, global channels,
freeze/resume/reverse, queue boundaries and failed-call atomicity. A CPU consumer
test verifies the actual posed mesh after a blended rotation/scale about a pivot.
A real summon regression verifies that changing presentation speed leaves the
gameplay spawn at its cast point. These are not GPU or original pixel comparisons.

The unchanged TFT opening still restores control at logical time four. Its later
corpse setup now selects actual source Decay Bone clips and passes the delayed
SetUnitTimeScale call. The retained post-control horizon next refuses
UnitSuspendDecay at Blizzard.j:3133. RoC passes the whole entry/control/horizon
diagnostic. Automatic decay, decay suspension, resurrection, native transition
qualification and full mission playthrough remain unfinished.

Validation for this slice: 1,830 core tests, 675 application tests, both published
map/audio integration suites, the explicit selected-image blend gate, strict
workspace native and production WASM Clippy, native/WASM application builds and
the standalone graphics all-target check pass. There was no WASM runtime, new GPU
capture or original-client comparison. Earlier failing fixture/compile/lint logs
and the still-failing TFT horizon are preserved separately from successful gates.
