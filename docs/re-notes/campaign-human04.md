# Human04 (Andorhal) route

Status: under public player commands the recreation wins Human04 through the map's own
victory path in 449.02 s of game time, reproducibly across two runs. This is a recreation
result under a player policy; it is not original-client evidence and says nothing about the
fidelity of combat, AI strategy, timing or the cinematics.

## Authored objectives

Read from the selected `Maps\Campaign\Human04.w3m` `war3map.j` and `war3map.wts` (extracted
to `.scratch/human04/`, not committed).

- Quest "Investigate Andorhal" (`udg_QuestAndorhal`) and "Confront Kel'Thuzad"
  (`udg_QuestNecromancer`).
- Kel'Thuzad (`udg_KelThuzad`, created at (-2270, 2540) for the undead player) dying runs
  `Trig_Ending`, which sets `udg_GAMEOVER` and reaches `Trig_Victory` and
  `Trig_Next_Level_Run` (`SetNextLevelBJ("Maps\\Campaign\\Human05.w3m")`,
  `CustomVictoryBJ(Player(1), true, true)`).
- Entering `gg_rct_GranaryKelThuzadArea01/02` runs the granary scene.
- Defeat: `Trig_TownIsDestroyed`.
- The undead player runs `h04_green.ai` (`CampaignAI`, ghoul and necromancer waves, started
  by `CommandAI(Player(6), 0, 0)`).

## Harness

`crates/peacecraft3/src/campaign_human_route/human04.rs`, ignored test
`selected_human04_route_reaches_authored_victory`. Runner `.scratch/human04/run.py <label>
[decisions]` (headless, null ALSA sink, no `DISPLAY`, a fresh private clone of the earned
Human03 profile `.scratch/human03/profile-d1`, cache SHA-256 pinned to `bb74bdee...`). It
shares `controlled`, `march_with` and the rectangle helpers with the Human03 harness.

Policy (a player policy, not an authored rule), once per simulated second: skip a cinematic
when control is off; press the barracks' Train button for a footman; when gold is on hand
and nothing queued (the supply cap), ask a worker to build the farm kind the authored
`Trig_PeasantFarm_Actions` names, through the Build menu and `place_build`; keep the party
(heroes and the authored footman, priest and mortar kinds, never workers) at the rectangle
`gg_rct_ArthasMove`; once 8 footmen exist, attack-move the party onto Kel'Thuzad.
The party never casts Holy Light: see the open refusal below.

## Refusals fixed

1. `CreateBlightedGoldmine` (`war3map.j:624`, the undead player's start). New
   `blighted_goldmine_host.rs`. The unit type is read from the selected script:
   `Blizzard.j`'s `CreateUnitAtLocSaveLast` routes exactly one rawcode (`'ugol'`) to the
   native, and `BlightGoldMineForPlayerBJ` calls it to make a "Haunted Gold Mine". The unit is
   then created through `CreateUnit`, so model, resource ability and footprint come from the
   tables and the caller's `SetResourceAmount` sets the gold. Refused loudly when the script
   does not name the type. Not established: whether the native itself paints blight (the shipped
   melee start code paints a 768-radius patch with a separate `SetBlightLoc`, which suggests
   not). No blight is painted and the `SetBlight*` natives stay unimplemented.
   Tests: `unit_runtime_tests.rs` (a nonstock mapped type and the refusal).
2. `GetUnitDefaultAcquireRange` (`OpeningCancelled`). New `unit_acquire_range_host.rs`: the
   unit row's `UnitWeapons` `acquire` cell, so map edits apply; an empty cell refuses.
   Tests: `unit_acquire_range_tests.rs`.
3. `CommandAI` and the AI-side `CommandsWaiting`, `GetLastCommand`, `GetLastData`,
   `PopLastCommand` (`h04_green.ai` waits for the map's signal in `WaitForSignal`). New
   `campaign_ai_commands_host.rs`: a per-player queue. Which pair "last" names when several are
   pending is not established, so a read with more than one pending pair refuses; a backlog over
   64 refuses. Test: `state_host_tests.rs`.
4. A `harvest` target order on a tree (`IssueTargetDestructableOrder`, `OpeningCancelled`).
   `harvest_runtime.rs` starts lumber work on that tree, not limited by the search radius; a tree
   that is dead, hidden or invulnerable returns false. Test:
   `an_explicit_harvest_order_on_a_tree_is_not_limited_by_the_search_radius`.
5. An explicit gold `harvest` order with the only town hall 1012 from the worker (800 from the
   mine; `Ahar` `Area1` is 900). The depot is now also searched within the search radius of the
   mine, after the worker's own area, as the mine-exit reselection already does. A recreation
   choice: the shipped opening orders this harvest and the original must accept it. Test:
   `an_explicit_gold_order_accepts_a_depot_near_the_mine_but_far_from_the_worker`.
6. `IssueBuildOrderById` (`IssueBuildOrderByIdLocBJ`, the tutorial farm, barracks and town
   hall orders). The construction host makes the same request as the build-id point order and
   reports whether it was accepted. Test:
   `the_build_order_native_makes_the_same_request_and_reports_acceptance`.
7. A non-attack target order at nothing or at an already removed unit (the farm-done trigger
   orders `repair` on the removed opening town hall) returns false instead of refusing. A live
   target with an unimplemented order still refuses. Test:
   `a_non_attack_target_order_at_nothing_or_a_removed_unit_issues_nothing`.
8. Revealing a hidden structure whose model had turned (`ShowUnit` after `Trig_Granary01`
   hid the undead player's units; the Necropolis was at 41.19 degrees from combat turning). A
   standing footprint now keeps the facing it was oriented with at placement; a new footprint at
   a non-quarter facing still refuses. Test:
   `a_standing_footprint_keeps_its_creation_facing_across_hide_and_reveal`.

Also: the score-screen Continue check no longer assumes `bj_lastLoadedUnit` is Arthas (Human04
loads Jaina last). With `PCX_HUMAN03_CONTINUE=1` the Human03 run now passes the score screen
into Human04, publishes it with Arthas restored from the carryover and completes startup.

## Victory run

Two runs from identical inputs (`v1`, `v2`, logs under `.scratch/human04/`) have identical
checkpoint lines and the same RNG checkpoint (`fnv64=4114b672250360a2`):

- Game time 449.02 s (tick 28737). Eight footmen trained, both heroes alive, 2 party units
  alive at the end (the rest are in the ending scene).
- `udg_GAMEOVER` true, Kel'Thuzad dead, the local player's result is
  `PLAYER_GAME_RESULT_VICTORY`, and the game exit request carries
  `Maps\Campaign\Human05.w3m` with the score screen.
- Quests at victory: "Investigate Andorhal" complete, "Confront Kel'Thuzad" not complete.
- The earned Human05 input cache is `0ff12705...` (`profile-v1` and `profile-v2` agree).
- The granary and ending scenes were skipped with the local end-cinematic event.

The blight-growth change was checked with `blight-growth-v1-20261030`. Its
checkpoints and RNG agree with `g1` through 313.00 s, then it refuses at
313.484375 s when structure handle 2838 completes with a blight-removal
effect. The effect's removal and overlap behavior is not yet admitted, so this
run does not reach the earlier victory at 449.02 s. See
[terrain blight](terrain-blight.md#growth-and-placement-investigation-2026-10-30).

## Open

- Holy Light and the AI target scorer. An earlier policy that cast Holy Light stopped at 435 s
  with `campaign targeting for unit ...: candidate has effect history`. The scorer
  (`campaign_targeting_host.rs`, `score_campaign_candidates`) refuses any candidate for which
  `unit_has_effect_history` finds an effect handle ever attached, destroyed or expired
  included; spell art attaches one to the healed unit, and a deliberate test
  (`campaign_targeting_refuses_destroyed_and_expired_effect_history`) pins it. What would settle
  it: a native reading of the Legacy executable's target-scoring routine (the one
  `SetTargetHeroes` feeds) showing whether it reads any per-unit attachment or only life, class,
  distance and the hero flag, or one matched original-runtime run in which an AI attacker chooses
  between a unit that carried a destroyed effect and an identical one that did not. Until then the
  harness avoids spells; revisit if a chapter cannot.
- The AI waves' arrival and strength are the recreation's, not compared with the original;
  the base held without defenders in this policy, which may differ from the original.
- Not established, as above: the blighted mine's full native behavior, the order of several pending AI
  commands, depot search for explicit gold orders, building footprints after model turns, and
  whether buildings should turn at all when attacking (combat sets `face` for any attacker).
- The Human05 chapter is not started here.
