# Source-bounded session players

`ObjectHost::with_players(context, slot_rule, PlayerBudget)` explicitly admits
player identity and slot-setup operations. The Program must be the exact Arc
retained by that admitted map context. The slot rule must be the independently
approved `GetBJMaxPlayerSlots` rule; another getter is not an interchangeable
count. The existing `SelectedMapNeutralHost` checks the private admission witness,
map/W3I integrity, source receipts, header applicability and native signature.
Its selected result supplies the domain size. No conventional slot count,
neutral-player ID or fallback count is embedded in this path.

That source witness remains owned by the player state. The context and compiled
Program cannot be replaced behind the executing VM. Caller limits bound admitted
slots and aggregate retained player-name bytes; platform layout is checked before
state allocation. Zero source slots means an empty domain; negative or over-budget
counts refuse. Input-profile approval and selected-successful-W3I policy retain
the trust/qualification boundaries in
[selected-map-neutral-host.md](selected-map-neutral-host.md). This does not identify
the active native registration family or reproduce its cache/default paths.

`Player(number)` lazily interns one identity for each number in the admitted
zero-based range. `GetPlayerId` returns that number, not the opaque arena token.
Players, timers, groups and symbolic constants share the existing bounded arena;
no second allocator can collide with them. Duplicate lookups allocate nothing,
and capacity failure preserves existing state. Ordinary reserved objects named
`player` do not acquire player state. Tokens remain session-scoped, not global
cross-session authentication.

Executed setters retain team, start-location reference, color, race preference,
race selectability, controller, name and score-screen visibility. Matching
getters return assigned values; unset fields refuse instead of inventing lobby
names, colors, controllers, races or start points. Symbolic arguments must belong
to their exact admitted arena family. Names keep the VM's existing WTS resolution
and exact UTF-8 contents; replacement checks aggregate retained bytes before
allocation and preserves earlier names on failure. This is not a peak-heap bound.

Start references must already exist in executed map configuration. A force-start
command records the forced status and occupied start, as described by the supplied
common script. Later start assignments retain the force marker; this is a
recreation choice pending native late-override traces, not a claim about native
flag storage. Occupancy queries refuse if the player domain or start record is absent; they
do not silently report unknown data as unoccupied. No unit is moved and no
random-placement algorithm is invented.
Team values remain assigned nonnegative identifiers without inferring their range
from `SetTeams`. Invalid inputs refuse; native exceptional-input behavior is
unverified. Exact declarations/constant flags, type ancestry, declaration-source
ambiguity and function overrides are checked before admission.

## Verification

Fourteen synthetic tests cover both source-rule branches and unsigned metadata,
bounds/zero/negative domains, shared identity interning and family isolation,
real getter/setter state, defined/forced starts, aggregate UTF-8 names, capacity
failure, source/context mutation, missing witness, wrong rule, exact signatures,
source ambiguity and script overrides. The retained owned-VM test initializes
player globals and aliases, executes localized name/team setup, drops input
owners and preserves the context across calls. The synthetic map fixture gained
an optional WTS member with matching input budgets; no production import policy
was weakened.

All 227 native tests pass with five explicit original-input ignores. Strict
native/WASM all-target/all-feature Clippy and WASM library generation pass. Five
actual 32-bit WASM checks pass (mask 31, zero imports): source-derived slot
branches, ownership/localization and layout refusal, typed settings/forced starts,
atomic aggregate-name failure, and cross-context/budget rejection. The first
WASM leaf compile omitted a transitive synthetic fixture helper; restoring the
complete existing helper fixed that harness-only error. Native verification also
caught an initially unused test import before the final run.

The same qualified Human01 context and separately approved rules now execute
player setup in authored `config`. Five player identities and 35 retained field
assignments match captured source calls; the approved rule supplies their domain.
Four timers, three groups and 432 symbolic identities remain intact (444 arena
entries total). The actual run makes 534 native calls including its next refusal:
`SetStartLocPrioCount` at map-script line 7765. Globals remain initialized without
new calls or replacement objects. Map, script, executable and profile hashes
remain unchanged. Actual commands, artifact/input hashes and logs are local in
`.scratch/player-arena-root/`; no original payload, profile, names or coordinates
are published.

This is player configuration state, not complete config/main, lobby membership,
unit ownership/spawning, resources, alliances, slot-state defaults, gameplay or
native handle parity. Start-priority relationships and explicit launcher
profile/Host wiring remain open.


The later [start-priority slice](script-start-priorities.md) executes the remaining
priority statements and completes the same map's authored `config`. The next
actual boundary is `GetCameraMargin` in `main`, at map-script line 7781. Placement
and gameplay remain unverified.
