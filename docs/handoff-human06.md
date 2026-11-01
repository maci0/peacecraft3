# Human campaign handoff

Date: 2026-11-01. Branch: `main`. Previous release: `v0.8.0`.

The Reign of Chaos human campaign plays through Human05 and the Human05 interlude. Human06 starts and runs. It does not reach the authored victory.

## What is done

Earned profile caches, in order. Each victory test recopies the previous profile before it runs, because a failed run can touch the profile.

| Step | Result | Cache after |
| --- | --- | --- |
| Human01 | authored victory, then `continue_campaign` can still panic on ability order | `10febfdfbf5234db3495d0cb8e788cc4b8336553380731aeb809138038f7fb7d`, 275 bytes |
| Human02 | tick 53057 / 829.015625 s | `ccaaa88461094a05e02a6f816b541f5a1f3e04e20aab42c324b3c9fbd82d5876`, 488 bytes |
| Human03 | tick 17857 / 279.02 s, next `Human04.w3m` | `2d4f66e281a5588092c4c7bde9a116ce93bea8e22b422ffc6697c2e0f42bf98b`, 886 bytes |
| Human04 | tick 28673 / 448.02 s, next `Human05.w3m` | `9c8885c2a5b3c10f1167d8a18b00eebddd5828f7d1feda5ddb65a913b4283aca`, 1284 bytes |
| Human05 | tick 115969 / 1812.02 s, next `Human05Interlude.w3m` | `375e6541cba8e2d2149813cfc9afb1be1dc00b63f1668f6ca3c545173e2ea468`, 2114 bytes |
| Human05 interlude | tick 64 / 1.00 s, `ChangeLevel` to `Human06.w3m`, score flag clear | cache unchanged |

Human06 ("The Culling") uses that Human05 cache. `udg_ThePlayer` is `Player(1)`. The hero global is `udg_ArthasVariable`. Victory is `udg_ArthasDenials` reaching 100, then `ChangeLevel` to `Maps\Campaign\Human06Interlude.w3m` with the score flag set. Defeat paths set `udg_GameOver` without that level change: all of player 1's structures gone, or `Die Fool` after 100 conversions.

## Human06 route

`crates/peacecraft3/src/campaign_human_route/human06.rs`, test `selected_human06_route_reaches_authored_victory` (ignored). While control is off it fires `EVENT_PLAYER_END_CINEMATIC`. While control is on it attack-moves the local army at the nearest living shown `nzom` within 3000 of the local player's start location. `PCX_HUMAN06_DECISIONS` overrides the default of 2400 decisions (64 ticks each).

Last run, 4800 decisions, from a fresh copy of the Human05 cache:

- Four `nzom` units appear by tick 13312 / 208 s.
- Nearest one stays about 9278 from the start location, so the 3000 cutoff never marches.
- Denials stay 0. Conversions stay 0. Army falls from 13 to 9.
- Tick 69184 / 1081.00 s stops on `no gold-accepting depot at mine exit` for worker 2233, exit `[12014.03, 5872.0]`, previous depot 2117.
- No authored exit.

An earlier policy that attacked every nearby unit scored 4 denials and then lost the town (`udg_GameOver` at tick 81443 / 1272.55 s, army still 12). Holding the army inside 2200 of the building centroid and also fighting nearby armed units killed the army (0 denials, `udg_GameOver` at tick 119936 / 1874 s).

## Host work landed for this stretch

These are recreations where the note says so. The unit tests named below passed in debug `cargo test -p pcx-play --lib --offline`.

- Overlapping live blight: a cell is blighted when any active source covers it. A removed source still refuses. `growth_state_ors_live_overlap_and_refuses_removal`.
- Structure upgrade when no builder lists `Builds` and the request is not `ugol`: an idle unit whose `Upgrade` list names the kind pays the target cost and becomes that unit after `bldtm`. `ai_structure_requests_answer_false_without_a_builder_and_refuse_missing_placement_data`.
- `TriggerRegisterVariableEvent`: polled at the end of the range boundary. Fires when the real or integer changed and the comparison holds. Not synchronous with the JASS `set`. `variable_event_queues_when_the_real_changes_across_the_limit`.
- Unset camera target: local player's assigned start location, else the location indexed by that player, else the lowest index. No start location still refuses. Not a decoded default. `unset_camera_target_reads_the_local_players_assigned_start_location`.
- `LeaderboardSetItemValueColor` and `LeaderboardSetItemValue`. `leaderboard_rows_preserve_color_and_style_history`.
- `CameraSetSourceNoise` and `CameraSetTargetNoise` store magnitude and velocity. No shake samples. Covered by `camera_getters_report_live_target_and_fields_with_angles_in_radians`.
- AI commands are a stack: `GetLast` reads the newest pair, `PopLast` removes it. `ai_commands_are_a_stack_and_an_empty_read_refuses`.
- `TeleportCaptain` relocates current captain members through `SetUnitPosition`. `ClearCaptainTargets` clears their combat target and attack point. `CaptainAttack`, `CaptainVsUnits`, and `CaptainVsPlayer` attack-move the captain. Dispatch covered by `suicide_unit_reaches_the_world`. Bodies not decoded.
- `IsUnitAlly`: own units and passive allies. Neutral Passive is not an ally. `is_unit_enemy_reads_the_players_view_and_never_counts_neutral_passive`.
- Lumber re-seek with an empty waypoint list stops the worker and drops the task unless it is carrying. The initial unreachable harvest order still refuses. `an_unreachable_tree_refusal_keeps_an_existing_move_route`.
- Gold return is not limited by `Ahar.Area`. A missing depot still refuses. `gold_mine_exit_without_a_usable_depot_preserves_hidden_worker_and_mine`.

## How to run the next Human06 attempt

Recopy the Human05 profile first. A failed run can change it.

```bash
src=.scratch/human05/profile
dst=.scratch/human06/profile
rm -rf "$dst" && mkdir -p "$dst"
cp -a "$src"/. "$dst"/
chmod 700 "$dst"
PCX_HUMAN06_DECISIONS=4800 bash .scratch/human-chain/run.sh Human06 human06-next \
  "$(pwd)/.scratch/human06/profile" \
  375e6541cba8e2d2149813cfc9afb1be1dc00b63f1668f6ca3c545173e2ea468
```

Install used by `run.sh`: `/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)`. Addon `PCX_NATIVE_CHAPTER_ADDON=roc`. The test is release, `--features test-fixtures`, locked, offline. Log: `.scratch/human-chain/human06-next.log`.

## Still open

- Human06 authored victory. The four `nzom` units stay about 9300 from the player's start, and denials stay 0. Worker 2233 still has no usable gold depot at the mine exit even after the area limit was removed (previous depot 2117).
- Human06 interlude and Human07 are not routed.
- Human01 `continue_campaign` still asserts ability order after the cache is written (`AInv`, `AHer`, `AHhb` versus restored `AHer`, `AHhb`, `AInv`).
- The same `.w3g` in retail Warcraft III and here, tick for tick, is not done. The structural reader and writer round-trip layout only.
