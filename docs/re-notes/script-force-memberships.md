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

`ForceEnumPlayers`, `ForForce`/`GetEnumPlayer`, destruction, removal, clear and
membership-query natives remain unsupported. Enumeration needs a qualified
candidate domain; callback iteration must execute on the same VM with an explicit
candidate context. Neither operation follows from container retention.
