# Explicit startup player slot states

`ObjectHost::admit_startup_player_slots` accepts caller-qualified
`(player, playerslotstate)` identities for the exact admitted player-context Arc.
It updates existing rows only. It does not create players, derive occupancy from
controllers or apply a default to omitted or subsequently created rows.

The loaded nonconstant `GetPlayerSlotState(player) -> playerslotstate` and
constant `ConvertPlayerSlotState(integer) -> playerslotstate` contracts must be
unambiguous, have no function overrides and share the admitted Player/CreateTimer
source. Admission walks the loaded state-type ancestry to `handle`, checking
source provenance. Every supplied player must be a real admitted row, and every
state must be an actual symbolic arena entry inside the explicitly selected
conversion's domain. Ordinary reserved objects and same-family values outside
that domain refuse.

The admitted source slot count bounds input size and the nested validation/search
work. No new identities or player rows are allocated. Duplicate
player entries refuse. All entries are validated before replacement; a failed
call preserves every previous state. A successful replacement clears omitted
rows. An explicitly empty input leaves getters supported but values unset;
missing values refuse instead of returning an invented EMPTY state.

`PlayerSettings::slot_state` exposes the retained optional handle. The getter
returns that exact identity. `invalidate_startup_player_slots` retires admission
and clears these values without changing executed player setup. Accepted
controller writes also invalidate conservatively; rejected writes preserve the
snapshot. This is an explicit recreation freshness policy, not proof that a
native controller write changes occupancy. External join, leave or lobby changes
require caller refresh/invalidation. Context identity does not authenticate an
observation or supply a live provider.

## Numbered input before player creation (2026-10-20)

`admit_startup_player_slot_numbers` accepts explicit source slot numbers with the
same context, contract and state-domain checks. It retains at most one pair per
source slot. A later real `Player(number)` copies only its explicitly supplied
state into the new row. Missing entries remain unset. The identity-based API
still requires existing rows and clears omitted entries, including future ones.
Both APIs validate and finish fallible copying before replacing the snapshot.
Accepted controller writes and explicit invalidation retire both existing and
future entries. No controller-to-occupancy rule or default state is inferred.

The production startup factory now passes its already-numbered post-config input
directly to this API. It previously required every number to have an existing
handle, refusing an explicit roster that included a player first created in
`main`. The application regression checks that path and the intervening timer
handle order, rather than pre-creating players to satisfy admission.

The source-prepared Human01 Host accepts all sixteen entries from the retained,
qualified original census after its authored `config`. Five players already
exist; the other eleven are created only by subsequent explicit `Player` calls.
Admission allocates no handles, and every getter returns the exact selected
global identity. This is an explicit comparison input, not a new-game default
provider or a new original runtime observation. No seed is injected and this
check does not execute `main`.

All 586 Host tests and 172 application library/map tests pass. Five slot tests
also execute in actual WASI, including replacement, invalidation of future
entries and rejected-input atomicity. Strict native/browser checks, both full
application builds and formatting pass. The first WASI filter selected zero
tests; that run is excluded and retained beside the corrected five-test run.
Evidence remains local under
`.scratch/campaign-bootstrap-root-20261020/numbered-slots-2/`. Ordinary runtime
state preparation, a live lobby and first-mission playability remain open.

## Qualified original input and continuation

This slice reuses the one original-client Wine/software-renderer startup census
qualified by [startup settings](script-startup-settings.md). No new game run was
needed. That probe preserved the original configuration, recorded every source
slot before and after `InitBlizzard`, and reports identical controller/state
pairs at those two points. Its executable, map, compile-prelude and observation
identities remain separate and hash checked. It is not a general slot-default,
controller/occupancy causality or live multiplayer proof. Generated preload text
is read as data and remains local.

The private unchanged Human01 verifier loads that receipt and resolves the named
state globals through its same initialized VM. Only five player rows exist after
`config`; their observed states are admitted without minting any other player.
The remaining census rows are not used as defaults for later identities.
`GetPlayerSlotState` then succeeds for the actual source call. Prior state remains
checked: globals and `config` complete, all three startup-setting globals match,
and the arena retains 868 identities, 155 units, seven filter wrappers, 17 forces
and 17 explicit memberships.

The next refusal is the already-implemented `GetPlayerController`, whose later
player row has no executed controller assignment. The verifier identifies player
number 3; both its controller and slot state remain unset. The statement is in
selected `Blizzard.j` at `438445..438479`, declared in common at `61313..61388`.
The single-player counting loop has not completed. No NONE controller, slot-state
inheritance or participation rule is invented to pass it.

Three synthetic owned-VM/Host tests cover aliases and exact returned identities,
replacement/omission, explicit empty input, invalidation, later rows, real shared
arena exhaustion, duplicate/over-domain inputs, malformed calls, unknown and
wrong-family handles, selected conversion scope, declaration/type/source changes,
function overrides and controller-write atomicity. Sparse synthetic enumerands
are fixture data, not copied original values. Native and actual WASI execute the
same tests. Commands, source-stability checks and unchanged-map receipts are
local under `.scratch/player-slots-root-20261017/`.

The later [current-controller input](script-player-controllers.md) resolves that
controller refusal from the same census while keeping setup assignments separate.
A production current-player provider, application Host factory, live lobby and
join/leave behavior, completed chapter initialization and rendered gameplay remain
open. No original assets or generated payloads are committed or uploaded.
