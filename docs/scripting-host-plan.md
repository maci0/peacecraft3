# Complete scripting support

The scope is the complete scripting surface selected from the installed game,
including `common.j`, `common.ai`, the callable `Blizzard.j` library and each
unchanged map's script or editor trigger graph. Campaign startup is the first
acceptance gate, not the end of the scope. A declaration, source-code mention,
successful no-op or parser test does not establish implemented behavior.

Current unchanged campaign boundary (2026-10-25): RoC Prologue01 and TFT
NightElfX01 publish their sessions, restore player control after the opening skip
and pass a five-minute idle script horizon. The TFT permanent-corpse helper now
executes through real decay suspension, retaining canonical bone-phase corpses.
This verifies that script path, not full mission objectives or native timing parity.
The offscreen post-intro main-menu → campaign run reaches gameplay but refuses
rendered completion for unqualified sky illumination and missing terrain/tree
sight occlusion. The launcher is a separate setup surface. Resurrection and the
complete scripting surface remain unfinished.

## Coverage accounting

Generate the native inventory from the exact selected script bytes, retaining
their edition/version, source digest, signatures and spans. Never maintain a
copied stock native list. Report these separately for each API:

1. Declaration resolves with the selected source/type hierarchy.
2. Arguments and return values have tested behavior, including invalid inputs.
3. State and identity are shared with the actual world, UI, audio and AI.
4. Callbacks, timing and lifecycle work through public script calls.
5. Original-runtime and rendered/audible comparisons support the fidelity claim.

Unknown coverage stays unknown. An API can be partially implemented: supported
cases and contextual refusals must remain visible. Do not count an API as complete
because one campaign does not exercise its missing cases. Runtime-only generated
reports and original scripts remain local; only the generic audit code and
synthetic fixtures belong in the repository.

Native application setup reserves larger compute-worker stacks before Bevy's
default task pools are installed. This prevents valid campaign preparation from
overflowing an unoptimized development worker. Thread counts still follow Bevy's
configured default assignment policy; the stack reservation is a local resource
policy, not game data. Host assembly also runs synchronously on a dedicated
bounded native stack: Bevy can lend a smaller render-thread stack to compute
work, so changing compute-worker sizes alone was insufficient. A two-MiB caller
regression covers that boundary; the unoptimized assembly frame observed in a
local crash was about six MiB. Spawn failures remain contextual admission errors
and panics propagate, with no publication of a half-created host. The selected
prologue gate also runs without a
`RUST_MIN_STACK` environment override. Some standalone synthetic test apps create
their pools earlier, so their complete fixture suite retains an explicit test
worker stack setting. WASM uses its platform execution stack unchanged.

`pcx-play/tests/native_inventory.rs` parses all declarations, including unused
ones, detects conflicting signatures and records library-function counts. Its
explicit local-original test takes `PCX_SCRIPT_COMMON_J`, `PCX_SCRIPT_COMMON_AI`,
`PCX_SCRIPT_BLIZZARD_J`, `PCX_SCRIPT_EDITION`, `PCX_SCRIPT_VERSION` and a fresh
`PCX_SCRIPT_INVENTORY_OUTPUT` path. Output is created exclusively, preserving
previous reports. A local Legacy archive observation found 1,383 distinct native
declarations and 1,044 library functions; these are measured inventory counts,
not thresholds or implementation counts. Every behavior status starts unreviewed.

## Implementation order and acceptance gates

| Area | Required behavior and gate |
|---|---|
| Language and VM | Types, nullable values, locals/globals/arrays, expressions, functions/code values, limits and source errors. Execute unchanged raw JASS and the editor-trigger output; test reentrancy and continuation state. |
| Math, strings and identifiers | Source-compatible arithmetic, conversions/formatting, string operations, hashes, rawcodes and order/ability identifiers. Compare edge cases and random streams with the selected original version. |
| Handles and containers | Shared canonical identity, null/deleted/wrong-family behavior, locations/rects/regions, groups/forces, enumeration, boolexprs and filters. Verify mutation during callbacks and nested enumeration. |
| Triggers, events and scheduling | All selected event registrations/responses, condition/action execution, timers, waits, pause and logical clocks. Verify event payloads, order, synchronous callbacks and retry atomicity. |
| Players and world | Slots, alliances, resources, technology, units/items/destructables, ownership, removal, queries and state changes. Script, player input and AI must observe the same objects and rules. |
| Orders, combat and abilities | Immediate/point/target/queued orders, movement families, acquisition, weapons, damage, abilities/buffs, hero skills, inventory and production. Public script calls must drive the existing mechanic, never a parallel approximation. |
| Environment and information | Weather, terrain/pathing/deformation, fog/exploration, visibility, blight, time, lighting and camera. Test world state plus the visible result, with values from the selected assets. |
| Presentation | Effects, animation, sounds/music, text, quests, dialogs, multiboards/leaderboards, cinematic controls and interface changes. Connect host state to live consumers and test cleanup after mission replacement. |
| Campaign and AI | All selected AI natives, build/attack priorities, guards/captains, caches, save/load, progression and game exit. Run unchanged campaign AIs and actual objectives through the menu-to-next-chapter loop. |
| Persistence and multiplayer | Caches/hashtables where declared, synchronization, save/reload, local-player semantics and deterministic commands. Repeat the same logical trace across supported targets without duplicate side effects. |

For every added native, inspect its existing callers and connect admission,
dispatch, shared state, renderer/audio/UI consumers, fixtures and lifecycle tests
in the same slice. Use synthetic nonstock data to discriminate derivations, then
explicit local-original gates. All automated runtime tests are headless and muted.

## Current campaign-entry priorities

1. RoC `Chasing Visions`: the unchanged application gate publishes the mission,
   returns the opening to player control with Escape events and passes a
   300-second idle script horizon. The offscreen post-intro menu run reaches
   control and mounts the selected-unit console, inventory, portrait and standard
   commands. Its strict renderer check still refuses sky illumination and
   terrain/tree sight occlusion; full hero-command behavior is not established.
2. TFT opening: source-driven corpse construction, blend/playback and decay
   suspension let the complete delayed permanent-corpse helper run. Its unchanged
   300-second horizon retains canonical suspended Bone/Decay Bone corpses.
   Resurrection, exceptional deaths/effects, native timing comparisons,
   asynchronous preload cancellation, special minimap layers and water
   presentation remain separate fidelity work.
3. Complete the in-game menu's unavailable controls and original-behavior
   comparisons. Its source panels now overlay the paused mission and connect
   return/end/back, restart and exit handlers; Save/Load/Options/Help/Tips are
   disabled rather than pretending support. Script exit and the mounted Quit path
   retire owned UI before a later launch. Complete source-driven save/load and
   paused settings/help content remain work; native quit/score and dynamic
   visibility/timing parity still need qualification.
4. Continue each unchanged first mission through player control and its real
   objectives; turn each uncovered API/behavior gap into a regression.
5. Run the complete source inventory and the custom-map compatibility corpus.
   Empty startup diagnostics alone do not close the matrix above.

The host is not yet feature-complete. Weather presentation, the listed acceptance
gaps and the wider mechanic gaps in [the core plan](core-mechanics-plan.md)
remain acceptance work, not supported-feature claims.
