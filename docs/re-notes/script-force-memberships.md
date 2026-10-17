# Explicit force memberships

`ObjectHost::with_forces(ForceBudget)` admits the loaded non-constant interfaces
`CreateForce() -> force` and `ForceAddPlayer(force, player) -> nothing`.
Players must already be admitted. Both natives share the selected source of
`Player` and `CreateTimer`. The force ancestry is walked from the loaded return
type to `handle`; its declarations must share that source and the family must
be an agent. Parent edges are not copied from a measured table.

Each force owns its explicit member vector in the existing `EnumHost` arena.
Creation starts with no members. Add validates a real force entry and an existing
admitted player row, then retains the player identity. A generic reserved object
named `force` or `player` cannot be adopted. Reported force kind comes from the
loaded constructor's return declaration. The read-only `force_members` accessor
returns these explicit memberships, not an inferred player domain.

The caller bounds force records and aggregate distinct memberships separately
from shared arena identities. New records and memberships refuse before growth
at their corresponding limits. Failed calls leave prior state unchanged.
Duplicate insertion succeeds without allocation, including at capacity; this is
an explicit recreation container policy, not native duplicate-input proof.
Force identities and memberships live until Host/session drop. Tokens are
session-scoped and unrecycled under the existing arena policy.

The unchanged selected Human01 completes globals and `config`. In
`InitBlizzardGlobals`, the initialized `bj_MAX_PLAYER_SLOTS` is 16 and matches the
independently admitted player domain. The VM creates 17 forces: each of the 16
`bj_FORCE_PLAYER` array entries contains exactly its corresponding executed
player, and `bj_FORCE_ALL_PLAYERS` has no explicit members. The same Host retains
868 identities, 155 units and the preceding filter, alliance, player-state,
rectangle, trigger and sound records. These are execution observations for this
selected input, not constants in the implementation.

The next refusal is `ForceEnumPlayers` in selected `Blizzard.j` at
`437759..437808`, declared in `common.j` at `66202..66289`. Its arguments are the
final force and null. Null does not establish its enumeration candidate domain;
no controller, occupancy, team or automatic membership is inferred. The local
stable-source commands and receipts are in `.scratch/force-root-20261017/`.
No original client, rendered chapter or application Host factory was exercised.

Owned-VM tests cover aliases, independent containers, shared timer/filter
identities, duplicate insertion at capacity and unchanged callback side effects.
Additional checks cover zero/full caller budgets, actual shared-arena exhaustion,
malformed/wrong-family/unknown/reserved identities, missing player admission,
source/type/interface overrides and an extra authored ancestry edge. Native and
actual WASI execute the same tests.

`ForForce`/`GetEnumPlayer`, destruction, removal, clear and membership-query
natives remain unsupported. `ForceEnumPlayers` requires the separately admitted
startup participation described below; callback iteration must execute on the same VM with an explicit
candidate context. Neither operation follows from container retention.

## Original enumeration controls

Two fresh private runs of the supplied executable, VERSIONINFO `1.29.2.9231`
(SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`),
loaded instrumented copies of the selected Human01 map at 1280×960 and 96 DPI.
These are Wine/software-renderer observations, not native Windows ground truth.
Map/archive release identity is separate from executable identity. The selected
compile-prelude receipts do not independently establish runtime archive winners.

All 17 retained map members were independently re-extracted; only the script
changed. Reversing the edits restores the original script. New unsigned experiment
archives omit attributes/signature metadata; no authenticity is claimed. The
first probe preserves original configuration. The second changes one explicit
configuration assignment from neutral to computer and adds a full source-slot
controller/state census. Both record immediately before and after the original
`InitBlizzard` call. Generated preload records are read as data, never executed.

In the original-configuration run, null enumeration contains only the active
user. With the computer stimulus, it contains that user and the computer.
Null, repeated-null and always-true-filter membership agree within each run,
before and after initialization, including the authored all-player force.
False enumeration empties a populated destination, including a manually added
neutral. The selective predicate retains only its chosen eligible player while
still visiting the same candidates. Independent `IsPlayerInForce` checks agree
with the recorded `ForForce` members. Callback visit order and final member order
are retained separately; no general ordering rule is inferred.

The second census excludes empty NONE/NEUTRAL slots and playing CREEP-controller
neutral slots. Thus neither every admitted `Player()` identity nor every playing
slot is a valid enumeration domain. The computer stimulus also changes EMPTY to
PLAYING, so it does not isolate controller from slot-state effects. Reserved-id
rules versus controller tests, nonplaying USER/COMPUTER, LEFT, RESCUABLE, runtime
controller changes, join/leave and reentrant predicates remain unmeasured.
Absent Host controller assignments must not become fabricated NONE defaults.

The next useful consumer is null-filter replacement from an explicitly qualified
current participation set. A captured startup set must stay bound to its exact
map, executable, launch/configuration and initialization context; it cannot stand
in for a changing lobby. Non-null predicates require same-VM candidate context.
Those controls alone do not admit a production participation policy. No list of
measured ids is embedded in production. These two different experiments do not meet the 100-run
baseline gate or prove a general eligibility algorithm. Local receipts remain in
`.scratch/force-enumeration-probe-20261017/` and
`.scratch/force-enumeration-computer-probe-20261017/`; both containers were stopped
and preserved. No original assets, modified maps or generated records are published.

## Explicit startup participation

`admit_startup_force_players` opts the current ObjectHost into null-filter
`ForceEnumPlayers` after its caller supplies qualified current participants.
The caller passes the exact admitted `Arc<ParsedMapContext>` and existing player
identities. A copied context or a generic reserved player token refuses. Every
identity is validated, duplicates refuse, and the supplied count is bounded by
`ForceBudget.max_memberships` before copying. Successful admission replaces the
previous input; a failure leaves that input and all force state intact.
Exact Arc identity binds the map, not capture provenance or lobby eligibility.
Those remain the caller's responsibility; no controller defaults or discovery
algorithm is inferred by this API.

With this admission, a null filter replaces the destination with exactly those
players in supplied order. The final aggregate count subtracts the old members
before adding replacements. Capacity and allocation checks precede clearing;
failed calls preserve every force and its accounting. Repeated calls do not
accumulate duplicates. An admitted empty set clears a force, including manually
added players. It is distinct from unavailable/invalidated participation.
Non-null filters still refuse without evaluation or mutation.

`invalidate_force_enumeration` makes subsequent enumeration unsupported while
retaining the last force memberships. Accepted `SetPlayerController` writes
invalidate automatically; rejected writes do not. External departures, lobby
changes or other participation changes require explicit caller invalidation and
fresh qualified input. This is a startup snapshot, not a live-session service.
A stale external snapshot is a caller risk; immutable map identity alone cannot
attest a changing lobby. No application participation provider was added.

The local unchanged Human01 gate loads its startup players from the preserved
original-configuration observation, checking map, selected compile-prelude,
executable and observation hashes first. It does not compile captured ids into
Rust or infer them from the Host's lazily interned player rows. After `config`,
the same Host admits those existing identities and runs unchanged `main`.
The all-player force now contains the one supplied participant; the 16 explicit
singleton memberships, seven wrappers and earlier state still pass, with 868
shared identities and 155 units. The next refusal is `GetGameSpeed` in selected
`Blizzard.j` at `437880..437894`, common declaration `59055..59114`.
This is controlled source execution with explicit captured startup input, not a
playable chapter or general native eligibility policy. Evidence remains local in
`.scratch/force-null-enumeration-root-20261017/`.

Owned-VM tests exercise independent forces and aliases, exact aggregate limits,
empty versus invalidated input, transactional re-admission, context/interface/
identity refusal, controller invalidation and recovery. Non-null filters leave
callback side effects untouched. Native and actual WASI execute the same tests.
