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
owning arena. This slice does not create simulated units or prove association
with a World entity. World enumeration, orders, filters and `ForGroup` callbacks
remain explicit refusals; no candidate units or successful callback stubs are
invented. Null, invalid, wrong-family and destroyed inputs refuse contextually;
native exceptional-input behavior is unverified.

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
