# Hold Position acquisition and the Human01 intro attacker

## Shipped text evidence

The selected Legacy client's `Units_CommandStrings` tooltip for Hold Position
(`War3Local.mpq`, `[CmdHoldPosition]` `Ubertip`) says: the units "stand where they
are and attack units that are within range", and "will not chase down enemy units
that run away, nor move to engage ranged attackers". The recreation follows that
text. Native code for the selected client has not been traced for this behavior.

## Recreation rule

A held unit (activity `held`, set only by the Hold order) with no valid target:

- acquires the nearest hostile live unit whose centre is within weapon range plus
  its own and the candidate's collision radii. Acquire range is not used.
- never steers toward a target.
- clears a target that is out of reach, including its swing and chase state. A
  later enemy inside reach is acquired again.

Stop, explicit Move/attack-target orders and point orders replace the hold as before.
Idle units keep acquisition by acquire range and chasing.

Code: `combat_host.rs` (`hold_position_target`, held branch in `advance_combat`).
Test: `combat_tests::hold_position_attacks_only_within_reach_and_never_chases_a_departing_target`.
That test is synthetic. It asserts target state, the holder position and the enemy position; it does
not establish the native hit timing.

## Human01 intro: what the source state shows

Evidence is from the unchanged `Human01.j` (`.scratch/campaign-placements/j/`) and
the `Maps/Campaign/Human01.w3m` `war3map.w3i` read through `mapx w3i`. These are
source and file facts, not a run.

- Arthas is `gg_unit_Hart_0023` owned by Player(1). Uther is `gg_unit_Huth_0024`
  owned by Player(9). The Uther footmen (`hfoo` 0001-0006, `hC01` 0032/0033) and
  Scout Towers (`hwtw`) are also Player(9).
- The intro issues `attack` target orders only between Footman03/04/07/08, which
  share owner Player(9). Same-owner pairs are not hostile.
- Startup `SetPlayerAllianceStateBJ` calls (`Human01.j:1682-1694`) cover pairs with
  Player(0), Player(8) and Neutral Aggressive. There is no call between Player(1) and
  Player(9).
- `InitCustomTeams` (`Human01.j:7747`) puts Player(1) and Player(9) in team 0.
  `SetPlayerTeam` stores only the team in this recreation.
- The w3i force rows have flags `0x0`. Force `TRIGSTR_328` (team 0) has mask
  `0xfffffefa`, which includes Players 1 and 9. Player(1) and Player(9) player rows
  both have `ally_priorities = 0x0` and `ally_mask = 0x0`.
- The Scout Tower row `hwtw` in `unitweapons.slk` has `weapsOn = 0`. Scout Towers
  therefore do not attack. They are not the attacker.

Inference: the script and the w3i do not put Player(1) and Player(9) in an alliance,
and this recreation keeps them hostile. The zero force flags and zero ally masks do
not disprove native initial team alliances. The selected client's startup rule for
same-team players is unverified, so the same-team hypothesis stays open. Team
membership is not treated as alliance in the recreation, and `SetPlayerTeam`
semantics are unchanged.

Observed in `.scratch/gui-item-20261020/runs/checkpoint-2/gui.log`: hero life is 650
at clock 0 and first drops at clock 12.375 (610.29). It reaches 0 at clock 34.25
while `control=false`. The authored Defeat appears at clock 36.17. The log does not
record the attacker, owner or rawcode of any hit.

## Open items

- First attacker is not identified. Establishing it needs a run that logs each
  damage event with the source handle, owner, rawcode, target and clock. The
  Arthas position near the Player(8)/Player(9) units should be compared with each
  attacker's acquire range. This requires a root-slot replay.
- Native alliance initialization for team members is unverified. It needs a
  selected-client trace of `SetPlayerTeam`/startup alliance state or an original
  runtime observation of Player(1) against Player(9).
- Invulnerability and pause are not applied in the intro. The prior note's claim
  that invulnerability is not consumed by combat is out of date: `weapon_target`
  filters protected targets. Pausing (`PauseUnit`) is still unsupported.

## Diagnostic trace and replay

`ObjectHost::enable_combat_trace` records acquisitions, explicit target orders and
weapon damage with source and target handle, owner handle, current player number,
rawcode, logical clock and life before the hit. It is off by default and changes no
outcome. Synthetic tests: `combat_tests::combat_trace_*` and
`explicit_target_order_attacks_a_passive_ally_while_acquisition_skips_it`. The
passive-ally test shows that `SetPlayerAlliance` is directed: a passive setting from
one owner to another does not stop the other side from acquiring it.

The headless Human01 driver is `.scratch/first-hit/` (ignored). It is a copy of the
production source-audit driver, with the trace enabled before the runtime loop and
printed afterwards. It runs the unchanged map through the production startup path
with no player input.

- Build: `cargo build -p peacecraft3 --lib --locked --offline` (Bevy; root slot), then
  `python3 .scratch/first-hit/compile_first_hit.py`.
- Run: `python3 .scratch/first-hit/run_first_hit.py`, which writes
  `.scratch/first-hit/run-N/stdout` with `COMBAT TRACE` and `TRACE i` lines.

No replay has been run on this branch yet, so the first attacker is not established.
