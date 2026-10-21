# Hold Position acquisition and the Human01 intro attacker

## Shipped text evidence

The selected Legacy client's `Units_CommandStrings` tooltip for Hold Position
(`War3Local.mpq`, `[CmdHoldPos]` `Ubertip`) says: the units "stand where they
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

Historical inference, superseded by the original-client probe below: the script
and the w3i do not put Player(1) and Player(9) in an alliance,
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

- The first attacker is identified below. Its initial hostility is contradicted
  by the later original-client startup probe; see the final section.
- General team-based alliance initialization remains outside this probe's scope.
- Invulnerability and pause are not applied in the intro. The prior note's claim
  that invulnerability is not consumed by combat is out of date: `weapon_target`
  filters protected targets. Explicit `PauseUnit` now suspends acquisition and
  attack progress; see [unit-pause.md](unit-pause.md). The intro does not issue it.

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

## First hit: production replay on the isolated build

Run `.scratch/first-hit/run-1` (driver `source-audit-diag`, built from
`.scratch/first-hit/source-audit-diag.rs` against
`.scratch/first-hit/build-isolated-resume/cargo-lib.json`, exit 0 in 68m 47s).
The `pcx_play` artifact embeds this worktree's `combat_host.rs`, and `peacecraft3`
embeds this worktree's paths. The earlier shared-target artifact embedded another
worktree's path and was rejected. Unchanged `Human01.w3m`, no player orders,
`prepare_new_game` inputs only. The run reached game time 36.17 (authored Defeat).
The driver then returned its own ledger-route error because no route was given.
The 60-second window was therefore not completed.

- Arthas is handle 777 (player 1, `Hart`). Uther is handle 833 (player 9,
  `Huth`).
- First damage to Arthas: 12.375 s, source Uther (833), 39.71 damage, life 650
  to 610.29. Uther's `Acquired` event on Arthas came earlier, at 10.344 s. It was
  automatic acquisition (no order).
- Uther's Acquired event at 10.344 s is the first hostile relation to Arthas in
  the trace. The first Arthas damage happens at 0.51 s, when Player(1) footman
  779 hits horse unit 781 (`hhdl`, owner Player 9).
- Arthas acquires horse 781 at 0.010 s. The horse is at (-3939, 3127), about 387
  units from Arthas's start (-3859, 2748), inside acquire range 500. Horse owner
  transfer to Player 9 is at `Human01.j:1794-1797`.

The recreation's former default hostility, with no passive alliance, explains each
`Acquired` event. Whether the selected client makes Player(1) and Player(9)
allies at start was unverified at this checkpoint, so this replay did not confirm
the team hypothesis. The later original-client probe supersedes that uncertainty.

## Original-client startup alliances (2026-10-22)

The local `PCXA` diagnostic keeps the selected Human01 template's `config` and
`main` statements, inserting additive query functions before `main`, an entry
query as its first statement, a post-main query as its last statement, and a
two-second timer query. All other original archive members round-trip unchanged.
Executable SHA-256 is `3f2ed012…570eed`; template SHA-256 is `f5b15561…40ee9`.
The diagnostic map is not unchanged-campaign acceptance evidence. It runs under
the private Wine/software-rendered reference, not native Windows hardware.

Run `908fa8da6710` retained 250 alliance records at each entry, post-main and
timer checkpoint (five configured players, ten kinds, every directed pair).
Before `main`, every configured pair has passive alliance. Player 1 is USER;
players 0, 2, 8 and 9 are NEUTRAL. In particular, players 1 and 9 report ally true,
enemy false and passive true in both directions. The neutral players' current
team is -1, unlike their executed configuration assignments. Do not use equal
configuration teams as the cause of these initial alliances.

Post-main and timer records preserve the player 1/9 passive alliance. The
template's explicit setters change its intended enemy pairs, including 1/8,
to passive false. These changing results provide a positive control for the
queries: this is not a constant stub result. The optional team-setter stimuli
were skipped by their configuration-readback guard, so no runtime SetPlayerTeam
effect is claimed. One harness Space dismissed the observed chapter card; the
following capture shows the original Arthas/Uther cinematic. All raw markers,
hashes, captures and generated maps remain local in `.scratch/campaign-opening/`
and the private reference run directory.

Static evidence in the same executable explains the controller policy:
`0x47E7B0` collects controller classes using comparisons at `0x47E82C` and
`0x47E831`, then checks active controller classes at `0x47E8DA`. It sets a
directed alliance in both directions for active/neutral pairs (`0x47E925`,
`0x47E937`) and neutral/neutral pairs (`0x47E9D6`, `0x47E9E9`), through the
same set-bit helper `0x4C1F70` used by the primary `SetPlayerAlliance` handler.
The classes correspond to USER/COMPUTER and RESCUABLE/NEUTRAL in the supplied
common script. The alliance kind is read from the call-site operands.

`player_alliance_rules` reads those branches and call targets
from the reviewed image, retaining its source identity. New-game startup builds
an initial relation matrix from its current controller snapshot after `config`
and before `main`; it does not fabricate executed setter records. Explicit
script assignments take precedence, including false. Combat and Holy Light
share that effective relation. Record-only hosts retain their previous policy.
Synthetic tests vary controller and alliance operands, refuse missing/unknown
windows, and check that initial defaults are not setter records, failed admission
preserves state, and explicit false overrides only its direction. The explicit
selected-image gate passes. The unchanged-map no-input replay now reaches all
60 requested game seconds twice with Arthas alive at 650, rather than zero life
and an unintended Defeat pause at 37.625. Its survival/clock assertions reject
that observed baseline. No alliance, control, pause or player order is injected
by this replay gate; normal startup supplies the source-derived policy.

Final private-target verification: `pcx-play` package 1,247 passed / 51 ignored;
app fixture suite 590 passed / 52 ignored; three decoder gates and the explicit
Human01 replay pass; strict native all-target Clippy, affected core/app WASM
checks and touched-file formatting pass. Receipts and frozen input hashes are
in `.scratch/campaign-opening/`. The initial package attempt omitted the four
required original-drop input paths; supplying the retained matching originals
made those existing tests pass without changing them.

Not established: full chapter completion, full native/rendered opening parity,
general team/force alliances, all initial alliance kinds, or the current-team
getter view. In particular, the native neutral-player team -1 observation is
retained as a gap rather than replacing executed configuration assignments.
