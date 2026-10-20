# Legacy TimerStart clock and callback binding

Bounded static evidence for Legacy PE32 `Warcraft III.exe`, VERSIONINFO
`1.29.2.9231`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
This resolves the previously unread JASS selector in
[legacy-simulation-clock.md](legacy-simulation-clock.md). The static chain is not
a runtime test or scheduler implementation; a bounded Wine follow-up is below.

The session has no connected REA tools. A read-only CLI diagnostic reported
missing Codex registration; no configuration or provider was changed. The
investigation used the existing bounded `objdump` method, checked PE raw-section
bounds, direct-call decoding and vtable reads. Commands, disassembly, fourteen
range digests and a static verifier remain local under
`.scratch/timer-start-source-root-20261017/`. That static step launched no
executable. No original bytes or generated original contents are committed.

## Observed native path

1. Registration at `0x49be2c` passes signature `(Htimer;RBC)V` at `0xe6fc00`,
   name `TimerStart` at `0xe6fc10`, and function `0x4ad0f0` to registrar
   `0x8ccc80`. A second table at `0xcb7114` registers that name with the common
   zero-return stub `0xcbda30`. Its runtime selection was not investigated.
2. `0x4ad0f0` resolves the timer through `0x483220`. A non-null result forwards
   timeout, periodic and handler to `0x4fe6f0`; null returns without that call.
   Invalid-handle behavior inside the resolver remains unqualified.
3. `0x4fe6f0` obtains or creates the callback record at timer `+0x40`, then
   passes the supplied handler to `0x4e4a80`. That function replaces record
   `+0x24` with the argument, including zero. The timeout is copied through the
   timer's `+0x44` value object.
4. The start call at `0x4fe724..0x4fe733` passes timeout, message `0x80204`, this
   timer, periodic, and **zero** to `0x7fbf80`. That wrapper forwards the last
   argument unchanged to virtual slot `+0x8` on the timer subobject at `+0x24`.
   Its constructor installs vtable `0xe7baf8`; that slot contains `0x7fb820`.
5. `0x7fb820` tests the last argument at `[ebp+0x18]`. Nonzero sets subobject
   flag `0x4` and selects `AGILE_DATA+0x48`; zero selects `AGILE_DATA+0x44`.
   The separate `[ebp+0x14]` argument controls periodic flag `0x2`. Construction
   clears the flags. Restart with an existing queue entry marks that entry and
   clears both selector/periodic bits before applying the new inputs.

**Derived binding:** this functional JASS path selects the earlier note's
game-time queue `+0x44`, independently of periodicity. It does not select the
raw-real-time queue. Application clock production, speed mapping, QPC calibration
and actual game-time advancement during startup or pause remain open.

## Callback boundary

Timer object vtable `0xe7cc34` has dispatch slot `+0xc` at `0x4fdf90`. For message
`0x80204`, it calls `0x4fe540`. That function sets timer context through
`0x4ebf00`, invokes object virtual slot `+0x10`, then passes the callback record
to `0x4e4720`, before restoring context through `0x4ebce0`.

`0x4e4720` reads that same record `+0x24`. A zero handler returns without calling
the executor. A nonzero handler is forwarded to `0x474ff0`, whose observed path
obtains execution state through `0x8cc130` when needed and calls `0x8cc230`.
Executor flags/statuses and continuation ownership were not established. The
object virtual call precedes the callback wrapper; this alone does not prove
when subscribed trigger actions complete relative to the handler.

The queue-fire path `0x7fc390` prepares one-shot removal or periodic rescheduling
before invoking the retained object's dispatch slot. It also has a segmented
timeout path using runtime data at `0x115ea08`. Callback restart/destroy/pause,
long-timeout rounding and general equal-expiry ordering remain open. The limited
runtime cases below do not settle those complete behavior spaces.

## Verification and implementation limits

| Raw range, end exclusive | SHA-256 |
| --- | --- |
| `0x4ad0f0..0x4ad114`, native entry | `50db69b2438f138dd59df8e1242ae9e5f164b106e30a4fa8cd3dbf9f565cb9f6` |
| `0x4fe6f0..0x4fe73e`, object start | `17aa2d28f701880872230d698cc973cb4917c838f8125c2f2ebc3f1ef9c6ee1a` |
| `0x7fb820..0x7fb938`, virtual start | `c495bce69f60ef23f70dbe46a40d915db1451924a02c536e541d051c271c8a95` |
| `0x4fe540..0x4fe5cd`, callback delivery | `dfb1a6469e92f8892fd61beae9d6cd3c0fdb0483d8aa6e75a1eb1131e162c36f` |

The local verifier rechecks all fourteen ranges and the registration, forwarding,
selector and callback relationships. These are static checks, not runtime parity.
At the static-evidence step, `ObjectHost` still refused `TimerStart`; the
default Human01 startup refusal remains unchanged without explicit timer inputs.
No measured operand was copied into a simulation constant; `TICK_MS` is unchanged.
A recreation may use a different queue architecture while preserving observable
behavior. Delivery must share the existing session's VM and identities and
preserve callback time and lifetime changes, with timing/order differences explicit.

## One bounded Wine lifecycle probe

A fresh isolated run used that same executable under Wine 10, Xvfb at 1280×960
and Mesa llvmpipe. It ran a retained Human01 map with reversible instrumentation
around its authored `InitBlizzard` call. The original `main` and `config` remain;
only `war3map.j` changes in the unsigned experiment, whose omitted attributes and
signature do not claim authenticity. This is an instrumented reference, not an
unmodified-map acceptance run or native Windows ground truth.

The probe starts a clock timer and schedules synthetic positive-timeout cases.
Those durations and operations are experiment conditions, not original defaults.
It records timer queries and `GetExpiredTimer` from callbacks, then closes the
log from its completion timer. The reviewed final capture shows Human01's opening
cinematic; it is not a recreation rendering comparison.

| Probe condition | Observation in this run |
| --- | --- |
| Query a fresh, unstarted timer | Elapsed, remaining and timeout each print zero. |
| Start A, start B, restart A at the same nominal delay | B fires before A. This does not establish a universal equal-expiry rule. |
| Replace an armed handler | Only the replacement handler appears before completion. |
| Replace an armed handler with null | The old handler does not appear before completion. A separate null-handler timer still invokes its expiry-trigger action. |
| Destroy an armed timer | Its handler does not appear before completion. |
| Restart a periodic timer from its first callback as a shorter one-shot | The replacement fires once; no further old periodic callback appears before completion. |
| Destroy a periodic timer from its third callback | Three callbacks appear, with no fourth before completion. |
| Register an expiry-trigger action and a direct handler on one timer | The non-yielding action logs before the handler. Both observe the same expired timer identity. |

The log has sixteen records, including fourteen timer snapshots/callbacks and
explicit begin/completion markers. Periodic callbacks print elapsed equal to
their configured period and remaining zero at the callback. Clock readings are
formatted through JASS `R2SW` at six decimal places; they do not establish exact
floating-point bits, wall-clock conversion, or equal deadlines below that precision.

Local receipts in `.scratch/timer-runtime-root-20261017/attempt-1/` bind the
probe script, reversible member checks, selected installation, staged map, launch,
generated log and both inspected captures. The verifier reparses the log, checks
all callback identities and suppression/count cases, and rejects missing completion,
wrong-identity and wrong-handler mutations. The probe's container was stopped and
preserved. Both retained LAN containers were already stopped and remain unchanged.

This is one bounded run. Pause/resume, zero/negative/non-finite delays, long-timeout
segmentation, yielding trigger actions, arbitrary reentrancy and repeated-run
stability remain unmeasured. That probe changed no recreation timer implementation or callback scheduler.
No native/recreation timing parity is claimed.


## Bounded recreation timer consumer

`ObjectHost::with_timer_runtime` admits the selected program's timer contracts
and a caller-supplied finite game time. `MapStartupInputs::timer_time` opts the
existing factory into that path before globals and entrypoints execute. `None`
keeps timer scheduling unsupported. There is no implicit application clock,
frame rate, speed mapping or connection to the separate startup-clock input.

`TimerStart` retains one run on the existing timer identity. Only a finite
positive timeout that produces a strictly later finite deadline is supported.
Restart replaces that run and its handler, including null. Destroy revokes the
identity and clears the run. Fresh-timer queries and `GetExpiredTimer` outside
an active live-timer callback refuse; no measured initial values are copied.
Elapsed/remaining/timeout queries use the current run, including callback restart.
The recreation uses double-precision deadlines and converts query results to
JASS reals; native rounding and long-timeout segmentation are not reproduced.

`MapScriptSession<MapStartupHost>::advance_timers(until, budget)` enters callbacks
through the existing session's callback tickets and VM. It prepares the next
periodic run or one-shot removal before dispatch, then clears expiry context on
success or error without overwriting a callback's restart or destruction. Failed
session replacement retains the prior VM and timers; successful replacement and
clear retire them together. Null handlers consume an expiry without a script call.

Delivery visits deadlines in order, using start/reschedule sequence to break
exact ties. This is an explicit recreation policy, not proof of native tie order.
The caller bounds the number of expiries. Work is bounded by two arena scans per
expiry and a final scan, with arena size already bounded by the caller. A refused
next expiry stays pending, earlier effects remain, and the host's game time records
progress. A matching timer-expiry subscription refuses before consuming that
expiry, so it also blocks later deadlines. Trigger enabled state, conditions and
actions are not guessed or silently skipped.

The tests exercise the actual factory, admitted-map delivery and published VM,
including shared globals/aliases, restart, destroy, bounded catch-up, failed
replacement, callback failure and trigger refusal. A callback may destroy the
expired timer and start another with a null handler. That start keeps its own
run. After its expiry, the published host queries that timer directly; the
callback globals are not rewritten. The active expiry stays until finish.
Synthetic success does not prove original timing parity. This API adds no automatic application advance,
yielding callback continuation, pause/resume, zero/negative-delay behavior or
complete Human01 gameplay.


The callback-lifetime follow-up (2026-10-18) executes all six timer-session cases
natively and from the actual Cargo WebAssembly integration artifact under Node.
All 96 native map-loading tests, strict native/browser Clippy and formatting pass.
The new regression covers destroying the expired timer and starting another during
the same callback, then querying that new timer after its null-handler expiry.
This adds test coverage only; production timer and session code are unchanged.
No new original-client run or native/recreation timing comparison was performed.
Evidence: `.scratch/timer-lifecycle-root-20261018/review-1/acceptance.json`.

## Timed-trigger lifecycle reference (2026-10-18)

The actual owned-factory Human01 RNG experiment now stops at
`TriggerRegisterTimerEvent`; its required four-result gate fails after one exact
original result. Registration alone would hide the missing scheduler and trigger
consumer. This section qualifies additional behavior before implementing that
consumer. Production timer and trigger code were unchanged at that reference step;
the implementation follow-up is below.

Static decoding of the same fingerprinted executable links the functional
registration to an event-owned scheduling subobject. Its configuration forwards
the supplied timeout and periodicity through the same timer wrapper and zero
clock selector qualified above. This supports sharing the existing recreation
game-time scheduler; it does not establish all native queue behavior or require
an extra script-visible timer identity.

One fresh isolated Wine run instruments the unchanged Human01 `main` immediately
around its authored `InitBlizzard` call. Only the script member changes, reversibly,
in a private unsigned map. Positive timeout values and operation sequences are
self-authored experiment inputs. The probe completes with 22 records:

| Probe condition | Observation in this run |
| --- | --- |
| Query a newly created trigger | It is enabled before any explicit enable/disable assignment. |
| Register two conditions, the first false and the second true | Both run in registration order; no action runs. Short-circuiting between condition registrations would lose the second condition's effects. |
| Register two true conditions and two actions | Both conditions run, followed by both actions in registration order. |
| Query context in automatic timed-trigger conditions/actions | The triggering trigger matches its owner, the event matches the loaded `EVENT_GAME_TIMER_EXPIRED`, and `GetExpiredTimer()` is null. |
| Subscribe a trigger and a direct handler to an existing timer | The trigger action runs first with that expired timer. The direct handler sees the same timer, with null trigger and event context. |
| Disable a periodic timed trigger, then enable it between deadlines | No disabled callback appears; delivery resumes on its original periodic phase. |
| Destroy an armed periodic trigger before its first deadline | No action for it appears before probe completion. |
| Destroy a periodic trigger in its first action at its third firing | Its second action still runs for that firing; no later firing appears before completion. |

The final capture shows the original opening cinematic, not recreation rendering
or chapter completion. Probe-member re-extraction, reversible edits, script
resolution, complete callback identities, context/order checks and seven rejected
log mutations pass. A self-authored formatter initially referenced an absent
helper; the failed resolution is retained and corrected before the only runtime
launch. An initial capture command's argument refusal is also retained. The
successful run is stopped and preserved, and all original payloads remain local.
Commands, range hashes, script/log fingerprints and verification are in
`.scratch/timed-trigger-root-20261018/`.

These are bounded observations from one instrumented Wine run, not native Windows
truth, an unmodified-map baseline or production delivery acceptance. They do not
settle yielding actions, arbitrary nesting, callback-list mutation, condition
destruction, edge delays, native rounding or general equal-deadline ordering.
The implementation must preserve one owned arena, clock and VM; evaluate all
registered conditions before deciding whether to run actions; retain the active
action tail across self-destruction; and restore event context before a direct
handler. Admission, work bounds, failure cleanup and replacement lifetime still
need actual native/WASM consumer tests. The RNG factory gate remains failed.

## Owned timed-trigger consumer (2026-10-18)

`MapStartupInputs::trigger_runtime` now separately opts the existing object host
into `TriggerRegisterTimerEvent`, trigger destruction and timer-trigger callback
delivery. It requires the existing timer runtime. The caller supplies an explicit
initial enabled state, a reviewed constant `gameevent` global name and a retained
timed-event limit. The host validates the native signatures, declaration sources,
selected conversion and global initializer, then derives the event identity from
that initializer. No measured event number or enabled default is embedded in
production code. Omission preserves the previous unsupported paths.

Normal fresh-game preparation now selects the source-decoded enabled operation
and derives the timed event identity and corresponding common-source global.
This uses the existing runtime described here; see
[fresh trigger input](campaign-source-preparation.md#fresh-timed-trigger-input-2026-10-20)
for its reconstruction policy, verification and remaining range-event refusal.

Timed events retain their runs on event records in the same bounded arena and
use the existing game-time clock and sequence ordering. They do not allocate a
script timer. Finite positive deadlines are required, and registration/capacity
refusals preserve prior runs and identities. Disabled periodic events consume
their scheduled deadlines without callbacks, retaining their phase. Destroying a
trigger prevents future delivery without recycling its token.

The scheduler prepares bounded callback snapshots before consuming a deadline.
`MapScriptSession::advance_timers` enters every registered condition in order
through the current session's callback tickets and VM. It combines their results
after evaluating them all, then executes the ordered action list when accepted.
The active action tail survives trigger self-destruction. Timer-expiry
subscriptions run before a direct timer handler, and trigger/event context is
cleared before that handler. Automatic timed events have no expired timer.
Callback failure clears context while preserving prior script effects; failed
replacement retains the previous published VM, arena and clock.

The caller bounds expiries; snapshot preparation is bounded by arena size and
retained subscriptions, and execution retains the VM's cumulative step limit.
Callback-list edits take effect on the next firing under the current snapshot
policy. Mid-firing edits, yielding/nested delivery, native precision/long-timeout
segmentation, equal-deadline fidelity and an automatic application clock remain
unqualified. Other event families remain registrations without world delivery.

Validation passes 446 Host tests on native and actual WASI, with six explicit
local-input gates ignored; 19 application-library and 107 map-loading tests,
with one explicit local-input gate ignored; four new timed-trigger cases plus
the six existing timer cases executed from the actual Cargo browser-WASM artifact;
strict native/browser all-target Clippy and formatting. Cases cover refusal
atomicity, declaration/default inputs, condition ordering, active action lifetime,
disabled phase, context cleanup and successful/failed session replacement.

The unchanged Human01 factory prefix now passes timed-event registration and
stops later at unsupported `TriggerRegisterUnitInRange`, Blizzard span
`82885..82941`. The required four-result RNG gate still fails after one match on
native and actual WASI (exits 101 and 134). Its private primary-only warmup adapter
and explicit compatibility catalog remain experiment limitations; production
engine RNG initialization is unchanged. No placeholder range-event registration
was added to conceal the missing world consumer.

Commands, failed checks and final validation remain local in
`.scratch/timed-trigger-root-20261018/implementation/`. Original payloads are not
published. No additional original-client run was needed for this implementation,
and no rendered gameplay, public-site or 100-consecutive baseline gate is closed.
