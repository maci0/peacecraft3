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
work. No new identities, rows or variable-size storage are allocated. Duplicate
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
