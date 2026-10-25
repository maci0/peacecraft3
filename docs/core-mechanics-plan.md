# Core mechanics implementation and acceptance

This plan follows the source-derived-data and unchanged-map requirements in
[PLAN.md](../PLAN.md). The target is observable Warcraft III behavior; engine
internals may differ. No stage is complete because its parsers compile or its
unit tests pass. Existing coverage below is a source audit at `15c802f`, not a
new original-client fidelity result.

## Shared rules

Use the canonical ObjectHost units, player state, game clock, orders, pathing,
events and deterministic RNG. Player input, map scripts, AI and replay commands
must reach that same state. Rendering reads accepted simulation results. Parse
rules from the selected installation and map overrides; keep those identities
in traces. Missing rules must be visible errors, not invented defaults.
The [mission loading audit](re-notes/mission-data-loading.md) records the source
chain for campaign rows, map paths, map headers, scripts and initial difficulty.

For each loop, implement source admission, simulation, lifecycle cleanup,
script events, player commands and presentation together. Validate malformed
input before spending resources or consuming RNG. A retry must not duplicate
damage, production, awards or callbacks. Use deterministic work limits, never
machine-speed thresholds, for regression gates.

## Dependency order and concrete gates

| Stage | Current implementation and gaps | Completion gate |
|---|---|---|
| 1. World and orders | Canonical units, ownership, selection, move/stop/hold/patrol/follow, pathing and guards exist. Hover uses the ground route; source-bound flight has bounded coverage. Floating/amphibious placement, routes and source-derived model surface height now have bounded coverage. Original water-mover trajectory/render comparisons, queued commands, crowded movement, transports and complete command presentation remain open. | Same seeded commands reproduce positions, accepted orders and collision outcomes; blocked/occupied destinations, death, transfer and cancellation work. Compare original trajectories, including group orders and repeated commands. |
| 2. Combat and survival | Melee/projectiles, armor/attack types, regeneration, hero attributes, death and some passive bonuses exist. Full weapon families, acquisition/visibility, evasion/criticals, splash/bounce/line attacks, invulnerability/ethereal interactions and corpse lifecycle remain a matrix to close. | Original seeded duels and small battles match hit times, HP, mana, cooldowns, RNG and death/award events. Test armor/type combinations, misses, lethal simultaneous hits, interrupted swings and in-flight missiles. |
| 3. Economy and technology | Human/Orc-style gold/lumber loops, cargo/deposits, resource/food counters and ordinary AI replacement training exist. General player queues, prerequisites, cancellation/refunds, research jobs, construction/repair/upgrades, upkeep and race-specific workers remain incomplete. | A player builds a base, gathers, trains, researches, cancels and rebuilds through the real command card. Compare resource/food changes, production times and event payloads to original runs; interrupt by pause, destruction and owner transfer. No free instant production. |
| 4. Heroes, abilities and items | Hero levels/skills/XP, inventory and selected spells/passives exist. Complete source ability families, target validation, cast phases, channels, dispels, buffs, stacking, auras, autocast, shops, revival and item effects remain incomplete. | Each authored ability family has positive/negative target cases, cost/cooldown and interruption checks against original behavior. Run interactions, not only isolated spells; awarded XP, shared XP and level-up events must match. |
| 5. Information and environment | Time-of-day and some visibility-related state exist. Complete fog/exploration, shared vision, detection/invisibility, elevation, blight, terrain restrictions and minimap feedback need connected verification. | Original matched maps reveal/hide the same units and areas, preserve explored fog, enforce targeting visibility and update allied/shared vision after death and transfer. |
| 6. AI and groups | Campaign AI VM, harvest assignments, guards, captain membership/routes and some production exist. General build/research priorities, attack waves, recruitment, controller timing and unsupported AI natives remain incomplete. | Unchanged mission AIs gather, replace losses, construct, research, defend and attack without scripted stand-ins or inert successful natives. Differential traces explain each remaining discrepancy. |
| 7. Mission lifecycle | Startup, timers/triggers, quests, victory and Continue exist in parts. Human02's 60-second gate uses a victory stimulus; it is not a completed first mission. Profile/progression and complete mission UI remain unfinished. | Play the first authored mission from menu to its real objectives/victory and next chapter without injected victory. Then repeat across authored RoC/TFT/demo chapter lists and custom-map compatibility corpus. |
| 8. Reproducibility and persistence | Logical clocks, RNG and replay infrastructure exist; broad trace/save/network acceptance is open. | A ten-minute combined match produces identical traces across 100 runs; save/reload continues the same state, and multiplayer command transport preserves the same results. Browser execution gets its own runtime gate. |

Stages are dependency order, not permission to postpone a required caller: e.g.
production needs command-card orders and events in stage 3. Later mechanics may
require extending an earlier stage. Shared definitions and algorithms should be
reused rather than duplicated for scripts, AI or UI.

## Implementation queue

1. Close production prerequisites using resolved `Requires`/`Requiresamount`
   and canonical technology state. Start with admitted upgrade prerequisites;
   then exact unit requirements and source equivalence relationships. Check all
   requirements before resource debit or a start callback.
2. Extend the same production lifecycle to player training queues and source
   command-card buttons; implement cancellation/refunds, completion placement,
   producer destruction/transfer and correct food reservation. Then add timed
   research and construction/repair/upgrades, including race-specific behavior.
   The [player queue contract](re-notes/player-production-queues.md) records
   original enqueue, active-food and UI-cancellation behavior and the source
   bindings needed before changing the shared jobs.
3. Run a combined gather → build → train → research → fight scenario. Fill
   combat/ability/visibility gaps exposed by that scenario before adding more
   unsupported content. Carry every failure into a reproducible regression.
4. Replace the first-mission victory stimulus with actual player objectives and
   an unchanged-map playthrough, then advance through the authored mission list.
5. Expand the mechanic matrix across all races, ability families and custom
   maps; complete deterministic, persistence and multiplayer acceptance.

The delivered replacement production path checks admitted upgrade and unit
prerequisites, including source upgrade/alternative relationships, before costs
and callbacks. Player queues and the rest of the economy stage remain open. The mission
loading audit also exposed model-cinematic rows in the source index: they are
retained and typed, and standalone playback remains a stage 7 requirement.
Neither result closes a whole mechanics stage.

## Evidence to retain for every completed mechanic

- Source edition/version, input identities and resolved map overrides.
- A small nonstock synthetic regression proving values are derived, plus
  malformed-input and interrupted-action tests where relevant.
- A real public command/script path exercising state, events and lifecycle.
- A matched original-client observation for the claimed external behavior.
- Strict affected native/WASM checks and a visible gameplay check when the
  mechanic changes UI, animation, sound or world presentation.

Record exact remaining differences in TRACKER.md. Runtime observations on the
local reference platform do not establish Windows, browser, audio or pixel
parity. Broad campaign completion remains open until the actual playthrough
gates pass.
