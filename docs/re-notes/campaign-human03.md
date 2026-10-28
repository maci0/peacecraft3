# Human03 (Ravages of the Plague) route

Status: under public player commands the recreation wins Human03 through the map's
own victory path in 279.02 s of game time, reproducibly across two runs. This is a
recreation result under a player policy; it is not original-client evidence and says
nothing about fidelity of timing, combat or the cinematics.

## Authored objectives

Read from the selected `Maps\Campaign\Human03.w3m` `war3map.j` and `war3map.wts`
(extracted to `.scratch/human03/`, not committed).

- Main quest "Investigate Villages": three items (explore the countryside, Arthas
  must survive, Jaina must survive).
- Main quest "Grain Warehouse": destroy `gg_unit_ngwr_0074` at (-6240, 1184).
  `Trig_Ending01` fires on that unit's death, sets `udg_GameOver`, runs the ending
  scene and `Trig_Victory`, which reaches `Trig_Next_Level_Run`
  (`SetNextLevelBJ("Maps\\Campaign\\Human04.w3m")`, `CustomVictoryBJ(Player(1), true, true)`).
- Optional "Fountain of Health" side quest.
- Defeat: death of `udg_Arthas` or `udg_Jaina`.
- The map has no economy. Player 1 starts with two footmen, a captain and Arthas
  restored from the `Human03` cache entry; Jaina joins after the opening.

Entering `gg_rct_Granary03Area` runs the Kel'Thuzad scene. Damaging the warehouse
spawns ghouls (`Trig_Spawn_Ghouls`).

## Harness

`crates/peacecraft3/src/campaign_human_route/human03.rs`, child module of
`campaign_human_route.rs`, ignored test
`selected_human03_route_reaches_authored_victory`. Runner:
`.scratch/human03/run.py <label> [decisions]` (headless, null ALSA sink, no
`DISPLAY`, fresh private profile cloned from the earned
`profile-human02-victory-12-outro`, cache SHA-256 pinned to `767df015...`).

Policy (a player policy, not an authored rule): once per simulated second, skip a
cinematic when control is off, otherwise attack-move the whole party to the next
authored rectangle in `ROUTE`, cast Holy Light on a party member below half life,
and after the last rectangle attack the warehouse. A rectangle counts as reached
when Arthas' collision circle overlaps it. Every command goes through
`campaign_input::standard_target` and is journaled.

`ROUTE` is the order of authored trigger rectangles along the walkable route:
Concerned villagers, Bandit village, Fountain quest zone, Murloc area, Defend
zone, Holy Light hint, Grain area, Skeleton start, Priests start, Mortar team
area, Granary03Area. The detour east and north to the Defend zone is forced by the
map: the placed destructable `YSdc` ("destroyed bridge", path texture
`CitybridgeDestroyed.tga`) blocks the crossing in the live collision raster. A
flood fill over that raster gives a 376-cell route.

The route search has a per-order work budget (`route_work` 65536 at admission).
A single order across the 4000-unit detour ends at the nearest reachable point
(the unit stops with no path). Hops between authored rectangles stay inside the
budget. Whether the original client finds the whole detour from one order is not
established.

## Refusals fixed

1. `SetSoundPosition` on a 2D sound (`Blizzard.j:2221`, via
   `SetSoundPositionLocBJ(gg_snd_H01VillagerC38 ...)`, `war3map.j:4570`). The
   recreation refused every `SetSoundPosition` on a sound not created with
   `is3D=true`. Native wrapper `0x4a9d70` has no flag check, so the call is now
   accepted and stored. See [sound properties](script-sound-properties.md).
   Tests: `file_sound_tests.rs`, `label_constructor_tests.rs` now assert
   acceptance for position and keep the refusal for the other four spatial setters.
2. `GetCameraField`, `GetCameraTargetPositionX` and `GetCameraTargetPositionY`
   (`Blizzard.j:1285`, `GetCurrentCameraSetup`, used by `Trig_Grain01_Queue`).
   Implemented in `camera_view_host.rs` over the live camera view.
   Evidence: the Legacy executable (SHA-256 `3f2ed012...`) registers
   `GetCameraField` at `0x4914f0`. It calls `0x5e0550`, a ten-entry switch over the
   field index; fields 2, 3, 4 and 5 (angle of attack, field of view, roll,
   rotation) go through `0x803960`, the others through `0x800d80`. The selected
   `Blizzard.j` scales exactly those four by `bj_RADTODEG`, so they are returned in
   radians; stored values stay in degrees. Roll has no `[Camera]` key; its value
   comes from the script's own `bj_CAMERA_DEFAULT_ROLL`. An unset camera target
   refuses, as `GetCameraTargetPositionLoc` already does. Not established: the
   native's value for a field the script never set beyond these defaults.
   Test: `state_host_tests.rs::camera_getters_report_live_target_and_fields_with_angles_in_radians`.

3. `attackground` (`IssuePointOrderLocBJ(udg_MortarTeam01, "attackground", ...)`,
   `war3map.j:6334`, run by `Trig_MortarTeamStart`). New
   `crates/pcx-play/src/attack_ground_host.rs`; the order name resolves through the
   selected order registry (`attackground`) like the other point orders and
   `GetUnitCurrentOrder` reports it while the order stands.
   Read from the selected tables and map edits (`UnitWeapons` row of the caster):
   range, cooldown, damage point, dice, attack type, `minRange`, the missile speed
   profile, the radii `Farea1`/`Harea1`/`Qarea1`, the factors `Hfact1`/`Qfact1` and the
   `splashTargs1` classes. For the stock Mortar Team these are 1150, 3.5, 1.0, 1d13+51,
   siege, 250, artillery, 25/150/250, 0.4/0.1 and `ground,structure,debris,tree,wall`
   (read, not compiled in).
   Recreation choices, unqualified against the original: radii measured from the impact
   point to the victim's center; full damage inside `Farea1`, the `Hfact1` share inside
   `Harea1`, the `Qfact1` share inside `Qarea1`; one dice roll per shot with each
   victim applying its own armor and defense type; reach is weapon range plus the
   caster's collision radius; the shot lands after distance over missile speed with no
   arc (the rule ordinary missiles already use); the caster repeats each cooldown until
   another order replaces the order.
   Refused loudly: a weapon without all five splash numbers; a `splashTargs1` token other
   than `ground`, `air`, `structure`, `enemy`, `notself` and the inert object names
   `debris`, `tree`, `wall`, `ward`, `item`; a target inside `minRange` (the original's
   reaction is not established). Changed during Human05: the earlier refusal of a shot whose
   area holds a non-hostile unit is gone. The splash filter follows the row's tokens: a
   row with `enemy` hits hostile units only, a row without it (the stock mortar, catapult
   and meat wagon rows) hits every unit of a listed class in the area, allies included,
   and `notself` excludes the shooter. That a siege row lacking `enemy` hurts allies is the
   reading of the table, not an observation of the original.
   Not modelled: damage to destructables, trees and items in the area, splash on ordinary
   attacks (the same columns), the attack-ground command-card button, a held unit.
   Tests: `attack_ground_tests.rs` (nonstock radii, factors, speeds and ranges: ring
   factors on one roll, missile flight time, walking into range and order replacement,
   the `enemy` and `notself` tokens, missing columns, an unsupported token and `minRange`).

## Victory run

Two runs from identical inputs (`d1`, `d2`, logs under `.scratch/human03/`) have
identical checkpoint lines (same ticks, positions and life) and the same RNG checkpoint
(`fnv64=4e896439e80dab1c`):

- Game time 279.02 s (tick 17857), 103 journaled commands, 2 party units alive at the
  end, both heroes alive.
- Quests at victory: "Investigate Villages" complete, "Grain Warehouse" complete,
  "Fountain of Health" not complete.
- `udg_GameOver` true, `gg_unit_ngwr_0074` dead, the local player's game result is
  `PLAYER_GAME_RESULT_VICTORY` and the game exit request carries
  `Maps\Campaign\Human04.w3m` with the score screen (`Trig_Next_Level_Run`:
  `SetNextLevelBJ`, `CustomVictoryBJ(Player(1), true, true)`).
- Route: Arthas reached the rectangles in `ROUTE` order; the Kel'Thuzad Encounter
  scene and the ending scene were skipped with the local end-cinematic event, as a
  player pressing Esc would.

The run reaches the warehouse with no Fountain side quest, no Defend-zone clearing
beyond what the policy's attack-move meets, and no Holy Light training. These are
policy limits, not game requirements.

Continuing past the score screen (`PCX_HUMAN03_CONTINUE=1`) admits Human04, publishes it
with Arthas restored from the earned carryover and completes its startup; see
[Human04](campaign-human04.md).

## Open

- Not established against the original: every recreation choice listed under
  `attackground`, and the unit-versus-ground behavior of a splash weapon's ordinary
  attacks.
- The route search's per-order work budget makes a single order across the eastern
  detour stop short (hops between rectangles avoid it); whether the original finds that
  route from one order is unknown.
- Cinematic content (Grain, Priests, Encounter, ending) executes but its look and timing
  are not compared with the original.
- The shared route helpers print a `Human01` prefix in diagnostics.
