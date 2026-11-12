# Script group lifecycle and membership

`playercolor_host::ObjectHost` reuses the existing session-owned `EnumHost`
arena. `TimerHost` remains a public alias with the same timer-only `checked`
entry point. The implementation lives in `object_host.rs`; it does not introduce
another allocator or Program. Calling `with_groups(max_members)` explicitly
admits group operations from that same selected Program.

Admission checks the exact declared signatures, constant flags, scalar argument
and result types, group/unit ancestry, unique declaration sources and absence of
script-function overrides. The supplied common script declares the supported
`GroupAddUnit` and `GroupRemoveUnit` returns as `nothing`; a different boolean
shape is refused instead of silently being assigned this implementation.
The interface names are API identifiers, not copied authored game data.

Supported operations are `CreateGroup`, `DestroyGroup`, `GroupAddUnit`,
`GroupRemoveUnit`, `GroupClear`, `FirstOfGroup` and `IsUnitInGroup`.
Each group has a live arena identity. Membership stores pairs of existing group
and unit identities, with duplicate additions ignored. Removal and clearing
preserve other groups and never destroy the units. Destruction clears membership,
revokes the group identity, and leaves aliases stale. Empty lookup returns null.
All groups, timers and symbolic values use the same non-recycling identity budget.

The caller bounds total live membership entries across all groups. Vector layout
is checked for the actual platform before admission, and capacity/length checks
precede insertion. Failed additions preserve existing contents. Clearing and
destruction release membership entries for reuse; retained vector capacity and
allocator overhead are not a total-heap-byte promise. No original limit or
member list is embedded in this path.

Traversal currently follows insertion order, including remove/re-add going to
the end. This is a deterministic recreation choice; native ordering parity needs
original gameplay traces. Unit arguments must already be typed identities in the
owning arena. Initial arena admission did not create simulated units or prove
association with a World entity. Later world enumeration, orders, filters and
`ForGroup` integrations reuse these identities; the correction below exercises
the compiled callback path. A null unit passed to `GroupAddUnit` or `GroupRemoveUnit` is no member
and returns without change (the shipped TFT NightElfX06 caravan init adds
`udg_CaravanUnitArray[7]` before a later trigger creates it). Explicit null
units in `IsUnitInGroup` are qualified by the later correction below. Mutation
and empty-lookup APIs still refuse null groups. Unknown, wrong-family and
non-null destroyed-group queries retain contextual errors outside the supported
null short-circuit; native exceptional-input behavior beyond that is unverified.

## Verification

Nine synthetic tests cover shared identity capacity, membership uniqueness and
order, isolation between groups, clear/destruction, stale aliases, zero/overflow
budgets, atomic capacity failure, bad arguments/types/signatures and function
overrides, compiled globals and an actual VM membership-draining function.
All 202 native tests pass with five explicit original-input ignores. Strict
native/WASM all-target/all-feature Clippy and WASM library builds pass. Five
actual 32-bit WASM checks pass (mask 31, zero imports), including the platform
layout limit, same-Program initialization, atomic membership capacity, stale
group rejection and timer/enum family isolation.

The same retained Human01 admission and qualified inputs documented in
[selected-map-neutral-host.md](selected-map-neutral-host.md) now complete all
authored global initialization: 448 observed native calls, four timers, three
groups and 432 symbolic identities. Repeating initialization preserves objects
and makes no native calls. The existing Program's map-owned `config` function
then reaches `SetMapName` at map-script line 7807, which remains unsupported.
Its diagnostic resolves the VM's internal main-source alias to the actual
admitted map-member label. The map, script, executable and profile hashes remain
unchanged; no original payload or profile is published.

Actual commands, source/artifact hashes and output are local in
`.scratch/group-arena-root/`. The successful final original gate is run 3;
the first two runs retained obsolete harness expectations (an initializer
failure and a directly qualified resolver source label). Those failed runs are
preserved separately. This proves global initialization and the next config
boundary, not completed config/main, launcher Host wiring or a playable campaign.


The later [executed map-config slice](script-map-config.md) passes those metadata
calls using session-owned state and advances the same `config` to `Player` at
map-script line 7706. Group ordering/world integration and completed gameplay
remain unverified.


## Undead02 membership null frontier

The unchanged opening replay at `/tmp/pc3-ai-horizon-main/Undead02.log` published
123 units before refusing `IsUnitInGroup` in the cinematic-skip callback. Its
reported map bytes `134602..134642`, line 2311, match the local extracted call
with `udg_Meat1` and `udg_Meatwagons`. The former is initialized null and assigned
by the later introduction action; the latter is created during globals setup.
The call therefore must be able to query a missing unit without terminating the
script. The map/script were read locally from the supplied Legacy archives;
no original script or map is committed.

Selected local input identities:

- Executable: Legacy 1.29.2.9231,
  SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
- Undead02 map SHA-256:
  `572783d19efec2be7899fd32899ac22c6aa6c6708ad34bbdde25d72d2a72abd6`.
- Extracted script SHA-256:
  `2bf19b49176fadfb00dd3418ca0bc24556d6cc7ce60802f7accbf1849761af46`.

Bounded selected-code disassembly binds the primary name/signature registration
at `0x49db2b` to `IsUnitInGroup` handler `0x499d70`. The handler resolves the unit
through `0x483940`, tests it at `0x499d81`, and returns zero immediately if absent.
Only then does it resolve the group through `0x481f20`; its test at `0x499d95`
branches to that same false return. Both resolvers explicitly return zero for
zero input. Valid resolutions reach membership helper `0x502d50`, which forwards
to the existing association-container lookup `0x4f28d0`. This is native-code
evidence of query branches, not an original-client runtime observation.

The host now accepts null unit/group queries with that unit-first order. An
explicit null unit returns false before consulting a group token, including a
retained destroyed-group token; a validated typed unit and null group also
return false. Malformed scalar/arity calls remain boundary errors. Unknown or
wrong-family units still error before a null group can mask them. With a
non-null unit, unknown, wrong-family, unowned and destroyed groups still refuse.
`DestroyGroup` reaches virtual slot `+0x5c`; its complete later resolution
lifecycle was not qualified here, so it does not authorize suppressing arbitrary
stale-handle failures.

Removal uses existing canonical behavior: `RemoveUnit` retires memberships in
all groups and retains a typed unit tombstone in the same arena. The new real
script regression creates a nonstock unit, adds it to two groups, removes it and
queries both its retained handle and a null unit. It proves cleanup and false
membership without replacing unit identity or allocating query objects. Direct
boundary and compiled `ForGroup` callback regressions cover null unit/group,
short-circuit ordering, strict failures, unchanged membership and handle counts.
These checks do not qualify general group traversal order or native removal
retention timing. The unchanged Undead02 replay still requires integration and
rerunning after this correction.


Validation of this correction uses the isolated worktree target
`/tmp/pc3-fidelity-sol61-07-target`. All 20 group host/iteration tests pass,
including the two named null-query regressions; the separate real removal
lifecycle test also passes. Strict native and WASM Clippy with all targets and
warnings denied, both library builds, formatting and diff checks pass. All runs
use a verified private offscreen display and null audio sink. Exact commands,
source worktree path and exit codes are retained in
`/tmp/pc3-lane07-group-validation/1/results.json`; the test executable omitted
debug symbols for this crate while retaining assertions. This is branch
validation, not a new original-client or unchanged-mission completion result.

Independent main integration executes all three new membership regressions
within the complete gameplay package: 3,169 passed, zero failed, 144 explicit
ignores. Strict native/WASM all-target Clippy, both libraries, formatting and
the application test-binary rebuild pass with source hashes unchanged.
The unchanged RoC Undead02 replay then publishes 123 units, restores control
at game time 1.5 and passes the 300-second post-control idle-script horizon.
It previously refused the null `udg_Meat1` membership query during the opening.
Receipts are retained in `/tmp/pc3-group-range-main-validation/` and
`/tmp/pc3-group-range-horizon-main/`. This is recreation execution, not an
original-client comparison or a completed mission.
