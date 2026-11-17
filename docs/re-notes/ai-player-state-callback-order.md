# AI writes and map player-state observers

The selected executable is 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
This records a bounded source/runtime qualification. The recreation admits the bounded initialized-map-VM bridge below. Other
cross-program callback families remain unsupported.
The separate [sleep-domain qualification](legacy-timer-start.md#script-and-ai-sleep-clock-domains-2026-11-13)
does not depend on that bridge being complete.

## Native source

The shared `SetPlayerState` registration at `0x49e6f4` selects `0x4a97a0`.
Its helper `0x4a9800` calls `0x7fb380`, which resolves the state property,
stores its new effective value at `+0x78`, then calls `0x8c3a90` with the old
value before returning. For a changed value and no alternate recipient, that
function calls `0x8ae820`. It traverses the listener chain at `+0x64` and calls
each recipient's virtual slot `+0x20` directly. No tick boundary or queued
handoff appears in this inspected chain.

`TriggerRegisterPlayerStateEvent` uses `0x4addf0`, factory `0x48c6e0` and type
getter `0x508050`. Setup `0x508070` attaches its property listener through
`0x4bfcb0`, `0x7fb2e0` and `0x8097a0`. These source joins and the observation
below support delivery before the setter returns to its AI script. They do not
establish the machine-level behavior of every possible virtual receiver.
Retained REA evidence is local at
`/tmp/pc3-sol02-ai-observer-source-evidence.json`; the session was closed to
`/tmp/pc3-sol02-ai-observer.snap`.

## Controlled observation

The earlier 744 sleep records showed observers preceding the next observed AI
signal, but every signal used another native. They alone did not establish
setter-return ordering. A smaller follow-up uses two controlled arms with the
same AI program, once with the watched observer enabled and once disabled:

1. AI writes lumber 11, then watched gold 7001.
2. The enabled map observer reads lumber 11 and writes 12.
3. AI immediately reads lumber into a local, writes 22, then reports the local
   through a distinct gold registration.
4. After a diagnostic non-dyadic Sleep, the sequence repeats with 31, 32 and 42.

The disabled control reports retained locals 11 and 31. The enabled observer
reports retained locals 12 and 32, while the report callbacks see the later
banks 22 and 42. Thus the observer's mutation is visible to the AI's immediately
following read, before its later write or yield. Delivery after an AI yield
cannot reproduce these retained locals and banks. Distinct trigger identities
retain each phase even if actual state has changed before delivery.

Both independent elapsed-clock channels, using integer sign/exponent/mantissa
encodings with verified controls, are bit-identical at observer entry, observer
exit and AI report within each phase. They also match the corresponding disabled
control. The callback boundary preserves the current game-queue readpoint.
These query observations do not establish a universal timer quantum.

The control has five records and the enabled arm nine; strict schema, identity,
counts, bank ordering, both query channels and raw hashes were checked. The raw
SHA-256 values are
`5f5c9830a13141afc3b10261ea4dd68a7f94ad85610d3a57f35ba0cc51f01418`
and `bc695266e4f856335445a3d28b6aa358072cb2417b03fdc4b7a713e8db1457a7`.
Witness, verifier and cleanup receipt are local:
`/tmp/pc3-sol02-ai-observer-witness.json`,
`/tmp/pc3-sol02-verify-ai-observer.py` and
`/tmp/pc3-sol02-ai-observer-cleanup.json`.

Both runs used private Docker Xvfb, a verified reference null sink, no host audio
devices and no network. Both containers stopped with PID zero and no OOM.
This is Wine/software Mesa evidence, not native Windows or a replicated
observer cohort. No original map, asset, generated copy or raw capture is
committed. Sleeping/reentrant observers, callback failure, AI re-entry and saves
inside this boundary were not tested.

## Minimum caller bridge

The two-program route must preserve the following ownership requirements.
Map `ScriptCall::Call` and `Spawn` function IDs belong to the map program;
forwarding those IDs into the AI VM would execute the wrong program.

Retain the pending AI native return and canonical ordered callback batch, return
the single world loan, run callbacks in the actual map VM, then return null and
continue the AI exactly once. Reuse the existing synchronous player-state
mutation/delivery machinery. A refused bridge must precede mutation; callback
failure must recover the world without replaying a committed setter or AI main.
Handle both an initial `StartCampaignAI` inside an active map call and AI wake
resume from the session.

The selected game point must remain active through map delivery and the AI's
following work or re-park. The handoff needs no fabricated Sleep deadline, game
tick or producer-segment completion. An initial AI call inherits the enclosing
callback's current game point, including a real callback's independent game
cursor. Preserve the map's existing synchronous event response and context
restoration rules. Full implementation planning is retained locally at
`/tmp/pc3-sol02-ai-observer-order-plan.md`; it adds no production API or policy.

Acceptance needs the actual map and AI VMs sharing one world with these banks,
initially and after a non-dyadic Sleep, plus scope/world recovery and a callback
failure proving no duplicate mutation. Separate arithmetic tests, a manually
recreated intended loop or a copied passing fixture do not close this frontier.

## Failure ownership

The diagnostic bridge reserves its capture marker before calling the setter.
A setter failure can therefore retain the owning callback fault even when no
map action starts. Both native and legacy wake entry return that same retained
fault identity; subsequent frame retries refuse before changing the world or
clock. An ordinary AI error without a capture remains an AI execution error,
allowing unrelated players and previously parked map threads to continue.

Initial AI entry and legacy wake preserve the original script error when player
retirement also fails, appending the retirement error rather than replacing the
cause. The canonical world returns to the map owner while the failed AI VM stays
retained in pending retirement without a world loan. Cleanup must not replay the
committed setter or its remaining script tail.

These are recreation failure policies, not original-client error observations.
That diagnostic validation kept the production bridge disabled. Capture from AI
global initializers, direct-host observers, sleeping/reentrant observers and
active-bridge checkpoint publication
remain refused; full Save/Load and general native tick parity remain open.

The diagnostic validation on 2026-11-17 executes all ten bridge cases against
the selected executable above, with both legacy and native counter branches.
They cover immediate rereads and nested callbacks, condition failures at initial
entry and wake, first-error/retry fault identity, pure-map entry gates, ordinary
no-capture errors, unrelated queued events and older parked threads. All ten
pass under verified private Xvfb and null audio. The current core AI filter
passes 73 tests with four explicit input-dependent skips; the expanded callback
boundary case passes separately. Four focused bridge core cases overlap that
AI filter. Strict native all-target and WASM library Clippy, native/WASM library
builds, formatting and the frozen eleven-file Rust hash check pass. Local logs
and the source freeze are under `/tmp/pc3-continue-ai-bridge-*`. Those runs
claimed no fresh original-client run, browser execution or production admission.


## Initialized map VM admission (2026-11-18)

Ordinary `MapScriptSession` construction now admits `StartCampaignAI` observer
handoffs only through a native dispatch issued after the map VM completes its
globals. The VM mints a private-constructor, non-copyable token borrowing its
resolved program. The host requires pointer identity with the retained map
program before arming the bridge. A separately resolved or cloned equivalent
program cannot authorize the map's callback IDs. Existing host implementations
retain their default dispatch; supplied decorators forward this context while
preserving neutral-native exclusions and once-only native recording.

The token identifies a program and initialized dispatch, not a published session
or global store. The ordinary session keeps its exclusive host and globals in
one retained VM. Embedders transferring execution must preserve that pairing;
this mechanism does not establish full-root restore or arbitrary VM interchange.
The existing session entry barriers and sticky/active bridge checks remain.

An initialized owner retains bridge capability even when no observers exist at
AI startup, allowing later registrations before a wake. No-observer starts from
direct hosts or map global initializers retain their previous native path and do
not acquire that capability. If observers are subsequently registered, those
unowned continuations refuse before their next player-state write. AI global
initializer writes with observers, reentrant starts, sleeping map observers and
active-bridge checkpoint publication remain unsupported.

Fresh checked extraction of the unchanged selected Human02 container and script
matches the identities in [the runtime notes](campaign-ai-runtime.md). Its
`main` registers the introduction and introduction-cancel action functions
through `InitCustomTriggers`, as well as the gold/lumber player-state observers.
Both introduction actions contain `StartCampaignAI` and map sleeps. These are
source caller/registration paths, not a new execution of either cinematic or a
passing mission horizon. The selected executable also matches the retained
14-record original-client ordering witness above; its raw backing traces are
not available in the restarted scratch environment.

The final program-bound source passes 3,637 package tests with 193 explicit or
input-dependent skips, strict native all-target/WASM library Clippy and actual
native/WASM library builds. Fourteen ordinary-constructor application cases and
the existing generic campaign caller pass under verified private Xvfb and null
audio. The equivalent-program rejection case also passes separately. Cases
cover resumed map-thread startup, later registrations with all three ownership
origins, primary fault identity, scope recovery and preservation of unrelated
work. The first matrix run passed 13 cases and failed before reaching the foreign
program gate because its second global store exceeded the synthetic trigger
budget. The corrected fixture capacity and all final results are recorded
separately under `/tmp/pc3-ai-admission-*`. No new original-client run, browser
execution, full Save/Load or general native tick parity is claimed.
