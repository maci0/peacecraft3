# Script timer creation in the shared session arena

`playercolor_host::TimerHost` is an explicit opt-in wrapper around the existing
`EnumHost`, not an independent handle allocator. It admits the selected Program's
non-constant `CreateTimer` and `DestroyTimer` declarations, validates their exact
argument/result types and the declared timer ancestry, and refuses source
ambiguity or script-function overrides. The same Program is retained into the VM;
selected-map scalar adapters and localization can wrap it without recompilation.

Creation allocates a live timer entry in the existing bounded arena. A timer is
an object identity, not a constant, null result or symbolic integer conversion.
Multiple creations return distinct identities and cannot collide with enum
values. Destruction marks that exact timer entry dead; arena type lookup and
further destruction reject the dead identity. Aliases do not revive it. Ordinary
reserved objects merely named `timer` do not acquire timer lifecycle state.

The arena's existing `max_handles` policy bounds total identities ever issued,
including destroyed timers. Tokens are not recycled, so an old alias cannot
address a new timer. Capacity failure preserves existing entries; destruction
does not restore capacity. This is an explicit recreation/resource policy, not
a claim about native allocation or recycling. Tokens are scoped to the owning
session; a raw `HandleId` is not cross-session authentication.

This slice implements only creation and destruction. It does not assign timeout,
clock, period, elapsed values or callbacks. Scheduling and time-query natives
remain unsupported, and no timer can expire through this wrapper yet. Invalid,
null, wrong-family and already-destroyed timer arguments refuse contextually;
that refusal policy is not native exceptional-input parity. The native scheduler
findings in [legacy-simulation-clock.md](legacy-simulation-clock.md) are unchanged;
`TICK_MS` is untouched.

## Source and verification

The qualified common/BJ/map inputs are the same locally retained inputs described
in [selected-map-neutral-host.md](selected-map-neutral-host.md). The common
script declares timer as an agent subtype and provides both lifecycle signatures.
Blizzard's global initializers invoke creation; their names, arguments and source
locations are read from the compiled Program, not a copied initializer list.
No original bytes or generated original contents are committed.

Nine synthetic tests cover shared identity allocation, destruction and aliases,
capacity failure, argument/family checks, signature and source ambiguity,
script overrides, actual compiled global initialization, sticky next-native
failure, and a VM create/alias/destroy sequence. All 193 affected native tests
pass, with five explicit local-input ignores; strict native/WASM all-target and
all-feature Clippy and WASM library generation pass. The first scoped Clippy run
caught two redundant test clones; those were fixed using borrowed slices before
the complete final check run.

An actual retained Human01 gate composes this wrapper with the six separately
approved scalar rules and source-selected enum contracts. It creates two live
timers, retains 432 symbolic identities (434 total arena entries), and then
refuses `CreateGroup` at Blizzard line 584. Its 444 native calls are observations,
not constants. Retry yields the identical contextual failure without more calls
or identities. The original map/common/BJ/executable/profile hashes remain
unchanged. Real commands, artifact hashes and captured output are local in
`.scratch/timer-arena-root/`.

This does not establish timer expiration, config/main execution, a playable
campaign, native handle semantics or default launcher Host wiring.


The later [group slice](script-group-arena.md) names the shared wrapper
`ObjectHost`, retaining `TimerHost` as a compatible alias and its timer-only
constructor behavior. Explicit group admission completes the same map's global
initialization; timer scheduling remains unsupported.

The later [timer-expiry subscription slice](script-trigger-admission.md#timer-expiry-subscriptions)
links an already live timer and an already created trigger through an event
record on that same arena. It does not add running state or expiration. Timer
start, scheduler integration and callback delivery remain open; the chapter
runner's caller-supplied tick callback interface is not a JASS timer scheduler.


A later [positive-timeout runtime](legacy-timer-start.md#bounded-recreation-timer-consumer)
adds one optional run to these same timer entries. Caller-supplied game time and
an explicit published-session advance enable bounded non-yielding callbacks;
the timer-only constructor above still admits creation/destruction only.
Expiry-trigger execution and an automatic application clock remain open.
