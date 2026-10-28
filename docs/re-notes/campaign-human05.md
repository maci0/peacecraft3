# Human05 (March of the Scourge) route

Status: the recreation runs Human05 for 1223 s of game time under public player commands
and then stops on the orange AI's first structure order: `SetProduce` for a Necropolis, which one
of the four acolytes the map gives that AI could build. Placement remains only partly decoded, so the
request refuses. The authored victory path (the 1800 s timer) is not reached. The stop is identical
across two runs. This is a recreation result under a player policy, not original-client evidence.

## Authored objectives

Read from the selected `Maps\Campaign\Human05.w3m` `war3map.j` (extracted to
`.scratch/human05/`, not committed).

- Survive until `udg_Timer30Minute` (1800 s, `StartTimerBJ`, `war3map.j:4643`) expires:
  `Trig_Ending_Start` sets `udg_GAMEOVER` and queues `Trig_Ending_Cinematic_A`. `Trig_Victory`
  (`SetNextLevelBJ("Maps\\Campaign\\Human05Interlude.w3m")`, `CustomVictoryBJ(Player(1), true, true)`)
  is the other end: `Trig_Victory2` runs it when players 3, 5 and 6 own no structure. Which
  ending trigger reaches `Trig_Victory` after the cinematic is not traced here.
- Defeat: `Trig_Defeat` when the local player owns no structure, with `udg_GAMEOVER` false.
- The intro (Cinematic A to F) is guarded by `udg_IntroCancelled`; the harness's cinematic
  skip turns it true at 2 s.
- The undead player runs `h05_green.ai` (`CampaignAI`, ghoul, necromancer, abomination and
  meat wagon waves; `SetBuildUpgrEx` after wave 3 at about 10 minutes of script time).
- Villages are corrupted by Mal'Ganis (`UC02`, hidden, hero level 6, skill `SNdc`) through the
  `darkconversion` target order; the `Dark_Conversion_*` triggers listen for
  `EVENT_PLAYER_UNIT_SUMMON` of Neutral Passive.

## Harness

`crates/peacecraft3/src/campaign_human_route/human05.rs`, ignored test
`selected_human05_route_reaches_authored_victory`. Runner `.scratch/human05/run.py <label>
[decisions]` (`PCX_RELEASE=1` for the release build; headless, null ALSA sink, no `DISPLAY`,
a fresh private clone of the earned Human04 profile `.scratch/human04/profile-v1` with the
cache SHA-256 pinned to `0ff12705...`). Human04's score-screen Continue into Human05 is
`PCX_HUMAN04_CONTINUE=1`; the Human05 run itself starts from the cloned profile.

Policy (a player policy, not an authored rule), once per simulated second: skip a cinematic
when control is off; train a footman at the barracks; keep the party (heroes and the authored
footman, rifleman and priest kinds) at the keep `gg_unit_hkee_0043`. It builds no farm and
casts no spell. Checkpoints every 16 s print party, footman and gold counts,
`udg_IntroCancelled` and `GAMEOVER`.

## Refusals fixed

Each has a synthetic nonstock test. Refusals are listed in the order they appeared.

1. `Sin` and `Cos` in script math: a bit-exact port qualified through the fingerprinted
   source window (`script_math.rs`, `table_trig`).
2. `IsUnitEnemy` (`combat_host.rs`, `is_unit_enemy_native`); Neutral Passive owners answer
   false because the shipped trigger asks it of every unit.
3. `attackground` friendly fire: the area filter now follows the weapon row's `enemy` and
   `notself` tokens (see [Human03](campaign-human03.md)).
4. `SetItemPosition` (`item_host.rs`).
5. `SetUnitPathing` on a removed unit is a no-op (`unit_activity_host.rs`).
6. Timer dialogs (`CreateTimerDialog`, title, display): state only, no rendering
   (`timer_dialog_host.rs`, `Entry::TimerDialog`).
7. `RemoveAllGuardPositions` (`unit_guard_host.rs`).
8. A training building with no replacement choice defaults to 3, read from exe 0xabf960 and
   0xabfa00 (`campaign_mode_policy.rs`, `unit_training_host.rs`).
9. An unknown tech id in a player research setter stores the level only (`player_host.rs`).
10. Dark Conversion (`ANdc` and the map's `SNdc`, `unit_curse_host.rs`). The class is
    bound by the parent row's `code`; the metadata ordinals of `Ndc1` (race) and `Ndc2`
    (`UnitID`) choose the columns. After the cast the buff (`BNdc`) and its `Targetart` stay on
    the target for `Dur`; then the target is removed and `UnitID` (`nzom`) is created at its
    position and facing for the caster's owner, and `EVENT_UNIT_SUMMON` fires with the caster as
    summoner. Targets must match the `UnitData` race (`commoner`). A caster hidden by the script
    can cast (`ShowUnitHide` precedes the order in the shipped trigger).
    Not established: that `Dur` is the delay before the conversion; the new unit's owner (read
    from the triggers, which listen on Neutral Passive and then reassign it); that a `targs`
    list without an alliance token admits every relation; the new unit's lifetime (none).
    A caster removed before the conversion ends refuses loudly.
    Tests: `unit_curse_tests.rs` (`dark_conversion_*`).
11. `GetUnitX` and `GetUnitY` of a removed unit. The shipped `Dark_Conversion_*` conditions ask
    for the coordinates of a unit that an earlier trigger of the same summon event has already
    replaced. The tombstone answers its removal-time point during the removal tick, as
    `GetUnitLoc` does, and 0.0 later. Exe evidence: 0x497d70 (`GetUnitX`) and 0x497db0
    (`GetUnitY`) return the static float at 0x112d88c when the handle resolves to no unit
    object; no instruction in the image writes that address, so it reads as 0.0. That a removed
    handle resolves to none after the removal tick is an inference. Test:
    `removed_unit_coordinates_survive_the_removal_tick_then_read_zero`.

12. Campaign AI research: `GetUpgradeGoldCost`, `GetUpgradeWoodCost`, `SetUpgrade` and an
    in-progress-aware `GetUpgradeLevel`, on the shared training queue. See
    [AI research](ai-research.md). The undead AI's `StartUpgrade` calls now run.
13. Research completion ordering. The first completed research (a level change on a moving
    undead unit whose life regenerates on blight) stopped with `blight regeneration needs
    motion projected before a clock rebase`. The level change now runs after the tick's
    regeneration (`complete_research` in `unit_training_host.rs`, called from
    `unit_activity_host.rs` after `advance_unit_regen_motion`), so no rebase integrates
    unprojected motion.

14. Lumber depot distance (the 1171 s stop). Two original-client probes settled it: a loaded
    lumber worker walks to the only town hall 2400 and 4400 away, deposits and returns to the
    tree, so lumber depot choice is no longer bounded by `Ahar.Area`
    (`harvest_runtime.rs`, `select_depot`). Gold keeps the bound; a worker with no owned depot
    still refuses. Evidence and receipts: [worker harvest orders](worker-harvest-orders.md#distant-depot).
    Tests: `a_loaded_lumber_worker_returns_to_a_depot_beyond_the_search_radius`,
    `a_lumber_worker_without_any_owned_depot_refuses_loudly`.

15. `SetProduce` for a structure with no builder. The executable's builder search finds none and
    the request answers false with nothing ordered (exe 0xac3070, 0xac2b00), so that case no longer
    refuses; a unit that lists the kind in `Builds` or `Upgrade` still does. See
    [AI construction](ai-construction.md). Test:
    `ai_structure_requests_answer_false_without_a_builder_and_refuse_placement_with_one`.

## Furthest verified state

`r14` and `r15`, run from the same inputs, have identical logs: 77 checkpoints and RNG
checkpoint `fnv64=d42949ae424eb960`. At 1218 s the party holds 26 units (18 footmen), both heroes are alive,
`udg_IntroCancelled` is true, `udg_GAMEOVER` is false, and 1130 gold is unspent.

Two fresh release runs, `placement-evidence-1` and `placement-evidence-2`, reached the same
1223 s refusal. Their Human05 checkpoints, RNG checkpoints and refusal text match each other
and `r14`; both tests exit 101 because the victory path is not reached. No gameplay code was
changed by those runs. At that time the snapshot diagnostic reproduced both Necropolis sites
with the recorded town coordinate and source-derived exclusion evidence while bypassing native
reach. Subsequent snapshot checks include reach, but the placement search remains disconnected
from orders. The recreation still lacks the live town list, live reach inputs and undead construction path. See
[AI construction](ai-construction.md) for the evidence and limitations. No later Human05 state
has been verified.

Release runs `snapshotproof-1` and `snapshotproof-2` also stop at 1223 s with exit 101.
Their Human05 and RNG checkpoint lines match each other and `r14`, including the final RNG
checkpoint `d42949ae424eb960`. Their refusal includes the remaining placement inputs.

Release runs `town-order-1` and `town-order-2` again stop at 1223 s with exit 101.
All 98 Human05 and RNG checkpoint lines match each other and `r14`, including the final RNG
checkpoint `d42949ae424eb960`. The refusal now identifies live town registration and eligibility,
reach occupancy and links, effect-source evidence and undead construction as missing inputs.

The diagnostic applies the per-unit exclusion predicate and source-derived enumeration
rectangle with snapshot-specific effect evidence. This does not change the 1223 s production
refusal or establish a later run.

The live host records ownership re-registrations and selects a visible peon using registration
order before considering the structure fallback. The fallback refuses when needed because its
native root-ability state is not represented. Release runs `undead-lifecycle-1` and
`undead-lifecycle-2` both stop at 1223 s, and their 98 Human05 and RNG checkpoint lines match
each other and `town-order-1`. Their first refusal identifies the missing live town association,
reach occupancy and links, and effect-source evidence. The selector and unattended construction
path are not connected to AI placement.

The reach-chain cleanup helper was subsequently checked against the selected executable and
the baseline and farm snapshot sites. Live chains and their cleanup schedule remain unconnected,
so no later Human05 state was tested in that change.

Release runs `bounded-set-20261029-a` and `bounded-set-20261029-b` both reach the same
1223 s placement refusal. Their 98 Human05 and RNG checkpoint lines match each other and
`r14`. The bounded set evaluator is not connected to production placement, so these runs
do not validate live reach inputs.

Stop at 1223 s (tick 78272): the orange AI (player 5, `h05_orange.ai`, started by
`Trig_OrangeTown_Actions`, `war3map.j:8681`) asks `SetProduce` for `'unpl'` and refuses with
`AI construction placement is not admitted: ... by "uaco" at [6329.0, 2320.2]`. The builder is one
of the four acolytes `Trig_Corruption04` creates for player 5 (`war3map.j:7899`).

An original-client probe of exactly this scenario (the four acolytes, `Trig_ResourcesOrange`,
`Trig_OrangeTown` run three seconds after start on a patched Human05) shows the original building
the whole base: Necropolis at (6208, 2752) by 12 s, ziggurats from 15 s, a haunted mine on the gold
mine at 24 s, altar, graveyard and crypts by 47 s. It is the target an implementation must match;
details in [AI construction](ai-construction.md).

## Open

- The snapshot diagnostic now selects the baseline, farm and two worker-layout Necropolis
  sites at their recorded dequeue counts with the hierarchical reach check enabled. Its
  sampled workers and supplied town origins do not replay movement or timer callbacks;
  general live reach inputs, effect-source qualification and town selection remain open.
  The 1223 s Human05 refusal remains in both `birth-arena-20261029` runs, whose checkpoint and
  RNG lines match each other and the prior `undead-lifecycle` run. A region registered by
  the script overlaps the baseline candidate square. The entry tests and bounded chain query
  are implemented for supplied links, but live cell-chain ordering remains unrepresented. See
  [AI construction](ai-construction.md).
- AI structure placement. The grid, visitation order, terrain classification and nearby-object
  score caps have a separately tested implementation, as do town-coordinate refresh and
  native snap arithmetic. The AI
  search and player orders share a validator, for which the existing player-placement code is
  the current implementation, with the sampled-state discrepancy above. Native town list and
  state mapping, texture dimensions, the additional reach check and
  complete texture-rectangle enumeration remain open; AI cost timing, worker choice and
  script retry timing are also open ([AI construction](ai-construction.md)). The shared
  construction path now supports an unattended Undead site, but AI placement does not call it.
- The 1800 s survival timer and the ending are not reached; refusals after 1223 s are unknown.
- Gold return distance (no probe), the choice among several lumber depots, and path versus
  straight-line "nearest" are the recreation's, not measured.
- The Human04 open item (the AI target scorer refuses a unit that ever carried an effect) did
  not fire in Human05: the policy casts no spell. Evidence that would settle it is listed in
  [Human04](campaign-human04.md).
- Cinematic F also orders `darkconversion`. The intro is cancelled at 2 s, and no stop in
  these runs came from that chain; whether it never runs after the cancel is not traced.
- The unestablished choices under Dark Conversion and [AI research](ai-research.md), and the
  state-only timer dialogs.
