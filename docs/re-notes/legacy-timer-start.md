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
identity and clears the run. Fresh-timer elapsed/remaining/timeout queries return
zero, following the retained original-client observation above. They neither
start a run nor advance game time. Invalid/retired identities still refuse.
`GetExpiredTimer` outside an active live-timer callback remains refused.
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

The scheduler prepares bounded trigger-delivery snapshots before consuming a deadline.
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
Edits to retained subscription, condition and action lists follow the snapshot
policy. This does not apply to the direct timer handler: the qualified follow-up
below resolves its current record after all subscription actions. Other
mid-firing edits, arbitrary yielding/nested delivery, complete native timing,
equal-deadline fidelity and an automatic application clock remain unqualified. Other event families remain registrations without world delivery.

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


## Zero one-shot cinematic timers (2026-10-20)

The selected BJ cinematic-fade helper supplies a zero-duration one-shot timer on
Human01's cancellation path. The timer consumer now accepts that request at its
current logical deadline, delivering it at the next scheduler boundary. A zero
periodic timeout, negative/nonfinite timeout or overflowing later deadline still
refuses before replacing the existing run. Existing expiry and VM work budgets
also bound callbacks that repeatedly rearm zero timers.

This is an explicit recreation policy motivated by shipped script calls, not a
new measurement of native zero-delay scheduling. The earlier positive-only
scope above describes the preceding implementation. Synthetic tests exercise
zero expiry and preserve the old run on invalid starts; native rounding, exact
same-boundary order and edge-delay fidelity remain open.

## Mandatory activity boundaries during timer catch-up (2026-11-13)

Source inspection found that the published session retained the next construction
or morph boundary only as a shortened timer target. When no timer expired there,
it recognized training and mirror boundaries but could return at construction or
morph work without reaching the requested game time. If an AI wake was retained
at a later deadline, that same branch could resume the AI at the earlier boundary.

The timer host now selects the earliest mandatory training, construction, mirror
or morph boundary together. The session preserves that boundary identity and
continues after mandatory work before considering a later AI wake or completing
the requested timer advance. Existing expiry ordering, pause policy, callbacks,
work limits and timer timeout admission are unchanged. Boundary deadlines still
come from the canonical units and admitted rules; no source value is embedded.

The nonstock morph fixture now supplies its prepared world and admitted context
to the actual `MapScriptSession<MapStartupHost>`. A test-only startup constructor
retains the existing neutral-host stack without duplicating asset preparation.
The published VM executes the form order, then `advance_timers` must cross its
earlier phase/flight boundaries, consume a later null-handler timer and reach
every requested target. One catch-up and chunked advances must produce the same
final clock, form and height. The former host-loop-only test was replaced because
it could not catch a session early return. This regression does not run an AI VM.

Evidence class: reconstruction source inspection and synthetic execution, not a
new original-client observation. The selected native reference remains version
1.29.2.9231, image SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
No additional native queue/tie-order, pause/resume, zero-delay or tick-parity
qualification is claimed by this correction.

Isolated validation uses `/tmp/pc3-fidelity-sol61-02-target` and source hashes for
this worktree. The named
`published_session_reaches_target_after_morph_boundaries_and_later_timer`
regression runs once and passes; the timer-runtime group passes nine tests with
one explicit local-common-source gate ignored, and the morph group passes 27.
Native and `wasm32-unknown-unknown` library builds pass with the workspace's
warnings-denied policy. Formatting and source-hash checks pass. Test launches
verify private Xvfb and null audio. Receipts are retained locally in
`/tmp/pc3-timer-session-validation/`. A preceding shared-target command that
selected zero tests was discarded, and the earlier host-loop-only test was not
accepted as session coverage. No browser runtime or original-client comparison
was run for this correction.

## Query precision and retained pause state (2026-11-13)

This follow-up uses the same selected image and version identified above.
It qualifies timer **queries**, independently of the recreation's double-precision
queue deadlines. The earlier statement that native query rounding is absent now
applies only to explicit partial timer hosts without admitted timer-query rules.
The campaign client attaches the reviewed rules to its existing numeric policy.
No frame rate, universal callback quantum or deadline rounding follows from this
change.

### Source and runtime evidence

The retained REA source join covers constructor `0x4fe380`, start `0x4fe6f0` /
`0x7fb820`, schedule `0x7fb940`, extended remaining `0x7fc0b0`, full timeout
`0x7fbad0`, active elapsed `0x7fbb90`, object elapsed `0x4fe470`, pause
`0x4fe640`, resume `0x4fe6b0` and dispatch `0x7fc390`. The segment initializer
at `0x40d210` calls the same admitted integer-to-real helper as the numeric
policy. The schedule routine loads its minimum delay from the selected image.
Neither operand is copied into a simulation constant. The remainder routine
`0x9a1a90` uses the fractional part of an approximate reciprocal product, then
multiplies by the segment; host floating-point remainder is a different operation.

Static evidence IDs include
`ev_db21eb4449ca53eceae21ce942cfd23013cd20dcd368f5ca355efc48063343c4`
(pause),
`ev_dfc0abf7d2b63817c094c2f0871f6236d3aaa4b5ac9a0e48e2af40433e4c8ff0`
(resume), and
`ev_e260a2eb9c21676859e6cb99c46a2e471ac6047892ccdd3f134a7a08b13a2340`
(extended dispatch). The local complete 18-record bundle is
`/tmp/pc3-sol02-clock-audit/query-lifecycle-rea-evidence.json`; the analysis
session was closed after retaining its snapshot. This is static evidence,
not an observation that those instructions executed in a particular capture.

Three diagnostic scenarios were each repeated twice in a private, networkless,
read-only Wine/software-Mesa container with verified private Xvfb and Pulse null
sink. Original map terrain and main were retained; bounded diagnostic JASS and
its corrected member CRC were substituted locally. These are instrumented
reference maps, not unchanged campaign or native Windows acceptance runs.
Every owned probe container was stopped and independently verified exited/PID 0.
No map bytes, generated copies or captured original contents enter this repository.

The paired-clock scenario queries independent 8-second and 2000-second timers
from the **same** callbacks. Requested non-dyadic durations include `0.02`,
`0.47`, `1.03`, `0.0317` and `0.0031`; dyadic controls include `0.5` and `1.25`.
The short/long readings differ, with independent signed `I2S(R2I(delta*scale))`
controls establishing that formatting alone does not cause the discrepancy.
The 2000-second readings cannot establish an 8192 Hz callback quantum. The
source query's cancellation and arithmetic order account for their apparent grid.

The lifecycle scenario records 72 snapshots / 216 query values. An independent
bounded JASS encoder normalizes each real by exact power-of-two multiplications
and reports its sign, exponent and 24-bit mantissa with integer conversion.
It avoids formatted-real strings as the bit oracle. The two complete 88-record
traces agree after removing the run nonce; raw SHA-256 identities are
`695a650024019c0a9b06874e8b09a1e6a17d80f4dc8ddf71087067cef51a7874` and
`ed19dde14304acbc75c253633e2e92a6e92b68c41bdb74cd76437296e36e992c`.
The local witness and cleanup receipts are
`/tmp/pc3-sol02-clock-audit/query-lifecycle-witness.json` and
`query-lifecycle-cleanup.json` in that directory. It exercises short and long
starts from a callback, segment crossings, pause/resume, restart and expiry.

### Implemented readpoints and bounded acceptance

`NumericPolicy::with_timer_queries` validates same-image source windows and
operand/call relationships and reuses the existing software arithmetic and
reciprocal table. Each started timer retains a separate query epoch and cached
remaining value. Full segments use the source's 16-bit count; construction and
projection are bounded by that count without allocating per-segment entries.
Unsupported real classes, invalid operands, counts outside that representation
and changed image/policy identities refuse before replacing an armed run.

Active elapsed is reconstructed virtual timeout minus active remaining in
source arithmetic order. Pause stores remaining. Inactive elapsed is original
raw timeout minus that cached value. Resume creates a partial query epoch from
the cache; `TimerGetTimeout` still returns the original raw timeout. Restart
retains the cache. Consequently inactive elapsed can legitimately be negative
after a shorter restart. Neither elapsed nor remaining is artificially clamped.
Fresh unstarted timer queries retain their previously qualified zero behavior.

The actual public host regression
`selected_native_timer_query_bits_match_public_lifecycle` admits the selected
numeric/rule image and drives `TimerStart`, queries, pause/resume/restart and
`next_timer_expiry`; it compares all 216 bit fields against the explicit local
witness. It passed with both replicated traces supplying that witness. Run it
with `PEACECRAFT3_QUERY_IMAGE`, `PEACECRAFT3_QUERY_WITNESS` and `--ignored`.
The ordinary synthetic tests derive an alternate segment and minimum delay,
exercise retained cache/negative elapsed and preserve armed state on refusal.
They establish reconstruction invariants, not additional native observations.

### Remaining limits

Callback query time follows the retained query epoch's source arithmetic;
outside timer callbacks it still comes from the caller-supplied logical clock.
The application's complete native game-clock producer and wrap policy remain
separate work. Timer-register timed-event callbacks retain their existing clock
path; their native queue implementation is not inferred from `TimerStart`.
Delivery continues to use the existing double-precision deadlines and recreation
tie policy. Exact segment-end residual scheduling, minimum-delay firing and
arbitrary equal-boundary order have not been changed or claimed equivalent.
Regular periodic callbacks retain the old epoch readpoint, while the segmented
periodic source path restarts before dispatch; the new bit witness does not
qualify every periodic or yielding/reentrant combination. Resume with a non-null
handler has a separate retained-callback rearm path at `0x4fe5d0`; its complete
schedule remains unqualified here. No application-wide tick parity is claimed.

## Resume dispatch and callback readpoints (2026-11-13)

A second source-bound lifecycle slice extends the query policy above. Two fresh
private Wine runs produced identical normalized 58-record traces with 18 script
callbacks. Their raw digests are
`766b83522653e177255ca908c4da8812bdfc02e3543bfca7f0e28934ab7e76f2` and
`60d3ffa00132375330bdae1a4bcf9b2408caff55b67b4c54f2e431d8193e75c0`.
Both clients were independently verified stopped/PID 0. The local collector,
bit witness and cleanup receipts are `/tmp/pc3-sol02-callback-run.py`,
`/tmp/pc3-sol02-callback-witness.json` and
`/tmp/pc3-sol02-callback-cleanup.json`. The same controlled-map, source-image,
headless/null-sink and native-Windows limitations apply.

The source join adds pause, resume, stop, object dispatch, post-resume rearm and
extended expiry dispatch windows to the numeric policy's same-image admission.
REA evidence `ev_04aa4a440eb83c02a7bca20f7a2eef7344428db96a2565f9fc87e6b31d5dc253`
shows `0x4fe5d0` rearming the current raw timeout only when the retained callback
is non-null. Object dispatch `0x4fdf90` calls the handler first for message
`0x80272`, then performs that rearm. Stop `0x7fbbd0` clears the periodic bit;
resume `0x4fe6b0` reads the cleared flag. Extended dispatch evidence
`ev_07a42ad7adb72e67d200581a6b7e5798f3da2b619995b4b220a5b4a1f55a73f6`
and stop evidence
`ev_d1584a885a001ea3110397383847ed041041e87fffce40657f6f538d344eb1d1`
bind the corresponding source branches. These static instructions explain,
but do not independently substitute for, the runtime observations below.

- Both an originally one-shot and an originally periodic 2-second timer,
  paused at 0.5 and resumed at 1, invoke their non-null handler at reference
  elapsed 2.5 and 4.5, then remain silent through the 123-second observation.
  Pause therefore loses periodicity. The first partial expiry's handler sees
  inactive raw-timeout-minus-cache queries; its return rearms the full timeout
  once. The subsequent ordinary one-shot does not rearm again.
- A regular periodic callback observes the old epoch's period and zero
  remaining. Pausing and resuming from that handler invalidates the old query
  snapshot. The zero cached remainder receives the source minimum delay;
  its partial expiry then rearms one full-timeout one-shot.
- Restarting with a null handler from the partial-resume callback suppresses
  the post-handler rearm. The restarted timer retains its earlier pause cache.
  Ordinary callback restart and destruction retain their earlier qualified
  behavior.
- An exact segment timeout goes through a residual request even when that
  residual is zero. Its callback occurs after the image-derived minimum delay.
  Segmented periodic callbacks observe the next epoch prepared before the
  handler, including the source arithmetic's small nonzero elapsed readpoint.

The selected timer policy now retains a pending resume-dispatch marker, finishes
its conditional rearm after the handler, and clears callback context even when
finishing fails. It clears periodicity on pause and gives resumed requests a new
sequence so queries cannot reuse an earlier periodic snapshot. The independent
legacy partial mode retains its old reconstruction behavior. Only qualified
source minimum/residual edges adjust the double-precision delivery delay;
ordinary nonzero deadlines keep their previous arithmetic. No universal quantum,
application producer, queue wrap or arbitrary equal-expiry policy is inferred.

`selected_native_timer_callback_matrix_matches_public_lifecycle` drives the
public runtime API through the complete witness, checking every emitted query
and both independent clock channels, callback counts and order. Its original
216-field query witness still passes. Actual VM regressions
`vm_resumed_callbacks_rearm_once_and_respect_handler_replacement` and
`vm_pause_resume_inside_periodic_callback_uses_minimum_then_full_oneshot`
execute the script handlers. A nonstock segment/minimum boundary regression
rejects early delivery and preserves a budget-refused expiry. The synthetic
numeric fixture deliberately changes decimal parsing, so those scripts express
dyadic inputs using power-of-two division rather than assuming decimal values.
These synthetic tests prove the recreation's called lifecycle, not new native
cases.

Fresh/inactive `ResumeTimer`, arbitrary nested/yielding callbacks, destroyed
identities retained across resumed callbacks and application/save-load delivery
state still require their own qualification. This closes the listed bounded
callback cases, not the full timer feature or tick-parity goal. The separate
canonical queue bridge must preserve current extended-segment entries and
counts across rebasing instead of treating a projected unwrapped final expiry
as a native queue entry.


## Direct handler after expiry-subscription mutation (2026-11-13)

The blanket next-firing callback claim above is superseded for the direct timer
handler. Static source `0x4fe540` invokes the object's expiry-subscription virtual
slot first, then reads the callback record at object `+0x40` and invokes
`0x4e4720`, which reads the record's current function. Retained REA evidence
`ev_79d949712349b524138f071d935c93d159ab5a35b12049db7f03b8687d281298`
records that ordering. The local 22-record bundle is
`/tmp/pc3-sol02-subscription-rea-evidence.json`; the analysis session was closed.
This is a static source join, independently corroborated by the runtime cases.

Four fresh private, muted, networkless Wine runs use the same selected executable
and controlled-map recipe identified above. The primary pair has 103 records per
capture; a second pair has 101. Each pair agrees after removing its nonce/prefix.
All four containers were independently verified exited with PID 0, and their raw
hashes rechecked. The local witnesses and cleanup receipt are
`/tmp/pc3-sol02-subscription-witness.json`,
`/tmp/pc3-sol02-subscription-destroy-witness.json` and
`/tmp/pc3-sol02-subscription-cleanup.json`. The primary raw digests are
`dac2036fa62ca7dbfdc074a2a0e40ba8210d47e9838ee0b2f5e16c7afd051253` and
`8f670023c50d7617350e8112346670dd7d387f76b643dadd0fdb97d3662c9674`;
the destruction-variant digests are
`7859825e5a7d8a4d1b056c42269cf85cf96c18336fe60d899ae418fd8a6a1d48` and
`aebf5100c7d5ccb3670c6e5a1cc49c37a39cf1c447302c95ed37615e38ae46d7`.
No original map bytes or captured contents are committed.

The first collector expected the earlier experiment's END-with-suffix form,
while this shorter experiment emitted an exact END marker. Its admission timed
out after the complete capture existed. The corrected strict collector validated
that retained capture offline, including identity, bit controls, record counters
and END; the independently launched replica then passed normal admission. The
original diagnostic remains explicit in
`/tmp/pc3-sol02-subscription-a-corrected-admission.json`. No timeout was counted
as a native pass.

The observed cases distinguish subscription snapshots from the retained direct
handler record:

- Replacing A with B using `TimerStart` in an expiry subscription invokes B in
  that same firing, with the new timer run already visible. Replacing A with a
  null handler suppresses A in that firing.
- Two subscriptions that replace A with B, then B with C, invoke C after both
  subscriptions. The later scheduled firing also invokes C.
- Destroying the timer in its subscription still invokes the retained direct
  handler, while `GetExpiredTimer()` returns null. Installing B before destruction
  invokes B with that null identity. Installing null before destruction invokes
  no direct handler. The original preparation-time handler is therefore not a
  sufficient fallback, and skipping every retired timer is also incorrect.
- Periodic restart, pause, and pause/resume from subscriptions retain the
  separately qualified timer lifecycle and query readpoints. The captures include
  both short and long clock channels and bounded integer bit encodings; they do
  not infer an application clock from formatted time strings.

The joint called fix retains the latest handler in active expiry metadata and
updates it only after successful `TimerStart` preparation, arena replacement and
sequence commit. Destruction clears the active timer identity while preserving
that retained handler, source and delivery sequence. The session resolves
`current_timer_callback` after all subscription actions and invokes it once.
`TimerExpiry.callback` remains historical preparation data for compatibility.
Raw queries on retired timer aliases still refuse; previously parked thread
identity rules are not broadly relaxed by this active-delivery correction.

The runtime components have 17 passing cases, including handler/null replacement,
replacement followed by destruction, unrelated timer isolation and rejected-start
atomicity. The joint session change has 14 passing published-VM cases, including
six new native-backed mutation variants, plus strict native/WASM checks and the
WASM library/application checks recorded in
`/tmp/pc3-sol10-current-handler-validation/`. This validates the called recreation
path; the instrumented Wine observations remain bounded evidence rather than
native Windows or complete tick parity. Yielding subscription mutation, arbitrary
subscription-list edits, save-point projection and destroyed identities retained
by earlier parked threads remain separate qualification work.

## Script and AI sleep clock domains (2026-11-13)

This qualification uses the selected 1.29.2.9231 executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
It does not replace a real-time deadline with a game-time point. The source
joins below are distinct from the bounded Wine observations and from the
pending recreation integration.

### Native code

`TriggerSleepAction` registration `0x49cb25` selects `0x4ae850`, which calls
`0x8cd210` with the requested duration and zero userdata. That setter marks the
current VM frame at `+0x34` and stores the raw duration at `+0x40`; it does not
schedule a timer. The ordinary zero-userdata return path reaches
`0x4fbdd0` through `0x4fb026`. It retrieves that duration with `0x8cc3d0` and
calls `0x7fbf80` at `0x4fbe31`, selecting real time with the final argument one.
The enclosing trigger-execution constructor `0x4f9610` installs the extended
timer vtable `0xe7baf8` at object `+0x34`. This is an extended real-queue sleep,
not a simple game timer or a measured minimum wait.

AI `Sleep` registration `0xab60d7` selects `0xab7770`. Its `0xac3e20` helper
marks a VM sleep with zero duration, then stores the actual requested duration
in the active AI thread event's FloatMini at `+0x24`, whose value is at `+0x28`.
After the VM returns status three, `0xab5640` schedules that value through
`0xad3560`, event `0xd01bf`, final selector zero. Constructor `0xad33a0`
installs the same extended timer vtable at event `+0x44`. AI Sleep therefore
uses the extended game queue. Status two and AI-paused wake retries use the
separate source retry period; they must preserve their retained VM ticket.

The extended start routine `0x7fb820` selects `AGILE_DATA+0x44` for zero and
`+0x48` otherwise. It decomposes long waits into its source-derived segments.
`0x7fb940` clamps each current entry using its image operand and soft-adds it
to the selected queue's current word. Its policy must not inherit the simple
timer's whole-period cap. Each pending entry and remaining segment count belongs
to its own queue; `0x7f9360` rebases that queue's retained requests on cycle wrap.

The producer `0x5aefd0` dispatches game substeps (`0x40090092`) first, then the
idle event (`0x40090093`), then the asynchronous event (`0x40090094`) when raw
counter elapsed is nonzero. The latter converts the unsigned counter difference
to nearest binary32 and multiplies it with the image operand referenced by
`MULSS` at `0x5af4ff`. This is hardware arithmetic, separate from the queue's
soft addition/subtraction. `0x5ea690` forwards that payload to `0x7f4820` when
`CGameUI+0x298` is clear. That routine checks its independent real-queue pause
counter, splits updates using the initializer at `0x40d060`, soft-subtracts
consumed chunks, and advances the real queue. Neither its input nor its phase
can be inferred from a nominal game Duration.

Script `PauseGame` calls `0x5f1460` with argument three zero. The UI real-time
pause field consequently remains clear. The network state changes through
`0x5ac440` to pausing; the producer completes its current quantum before entering
paused state. Subsequent game quanta stop while asynchronous real events can
continue. The UI/menu pause branch uses different arguments and is not covered
by the script-pause observation.

The local REA bundle is `/tmp/pc3-sol02-sleep-source-evidence.json`. The raw
initializer window is checked as a virtual-address byte range: a decompiler
request for an unrecognized small initializer may fall back to a file offset
and must not be treated as that initializer's source.

### Bounded original-client observations

Two fresh normal runs each passed 147 strict records: 24 map sleeps across two
start phases, 12 sequential AI sleeps, and 36 timers started at the resulting
wake readpoints. Requests include zero, a small negative, small positive values,
non-dyadic periods and exact dyadic controls. Every query uses integer-encoded
binary32 sign/exponent/mantissa with independently verified controls, alongside
short and long elapsed clocks. AI reports through distinct player-state EQUAL
events whose map actions read the clocks and start the diagnostic timers; these
are event observations, not direct AI timer-query calls.

The two normal raw trace hashes are
`7a8866bd8a3f514f19e5a4b98d07935b0d72f6ffac42a40fba8f3d529d4a930b`
and `10906d9dae4ad37b2dcddf8d77125c9cc8628e89a9b126614a2f100b84be1222`.
Both map start phases can wake at the same game-queue readpoint: their different
game-time starts occur while the real-queue origin remains unchanged. This does
not establish a universal wait quantum, or a minimum from the displayed elapsed
values. AI's small/zero/negative requests and non-dyadic requests independently
exercise the game-queue minimum and addition readpoints.

Two further runs each passed 150 records including script pause, real sleep and
script unpause. Each also completed 2,048 read-only memory sampling attempts;
all 6,146 JSON records per sampler passed ordinal/schema/bound checks. In one
stable bracket sequence the game queue held `1.2249988317489624` for 595 host
milliseconds while the real queue advanced from `1.2729973793029785` to
`1.8639944791793823`. The sleeping map thread woke and unpaused the game; the AI
signal stayed unchanged over that pause. The game queue's final in-flight
quantum is observable, so pause must not retroactively erase that work.

One supplemental run passed the same 150 script records and all 8,194 sampler
records. All 2,048 counter observations read the QPC selection flag as zero;
`0x796ca0` therefore used its GetTickCount fallback in this run. The alternative
QPC scale initialization remains unqualified. Corrected passive physics-context
reads are separate observations and establish no general callback cadence here.
The earlier physics-context cohort failed its identity check and remains unknown.

The local witness is `/tmp/pc3-sol02-sleep-witness.json`; independent container
cleanup receipts are `/tmp/pc3-sol02-sleep-v2-cleanup.json`,
`/tmp/pc3-sol02-sleep-v3-cleanup.json` and
`/tmp/pc3-sol02-sleep-v4-cleanup.json`. All admitted runs used a private Docker
Xvfb, a verified null sink, no host audio devices and no network, then exited
with PID zero and no OOM. An earlier AI diagnostic using direct map timer/preload
natives crashed before a trace and is explicitly excluded, with its failure
retained at `/tmp/pc3-sol02-sleep-v1-failed-status.json`. Original assets, private
maps, raw captures and generated copies remain local.

### Required called integration

A map sleep must retain a raw real-queue epoch, even when it is requested inside
a selected game callback. At its real wake, the real callback selects its real
entry; `TimerStart` and other game-clock consumers still see the current game
queue, after the producer's game work for that iteration. An AI sleep retains a
game-queue epoch and resumes at its selected game entry. Re-parking, errors,
wraps and continuation capture must preserve and unwind those distinct scopes.

The existing extended-entry arithmetic can be reused. A coherent producer must
carry independently qualified raw real elapsed, game pacing/speed state and
source event ordering. Treating two equal Durations as evidence for both clocks,
casting a legacy deadline to a raw point, or introducing a fitted wait quantum
would lose the observed behavior. This section qualifies the domains and bounded
readpoints; it does not claim that the recreation's parked map/AI wake integration,
UI pause, QPC path, large negative waits, arbitrary yielding or full tick parity
is complete.
