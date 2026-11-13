# Native game queue and called campaign pacing

Selected input: local Legacy `Warcraft III.exe`, VERSIONINFO 1.29.2.9231,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
This joins the bounded code observations in
[legacy-simulation-clock](legacy-simulation-clock.md) to a campaign consumer.
It does not establish complete simulation, scheduler, heading or spell parity.

Normal-campaign adoption of this candidate is held until both script and AI wake
families carry source-qualified raw points and callback scopes. The called timer
and timed-event tests below do not qualify those other continuation families.

## Producer and arithmetic

The initializer at `0x406730` converts its two integer immediates through
`0x9a1620` and divides through `0x9a02d0`. The resulting global is the same
operand placed in the scheduler's `0x40090092` event payload. The reader checks
that relationship and derives both the nominal rational interval and its native
real word through the existing selected `NumericPolicy` arithmetic. No measured
step, clamp, queue limit or tolerance is stored as a production balance constant.

The simple-timer policy binds virtual `+8` to `0x7fb700`, its game-queue selector,
source minimum/cycle-cap operands, constructor and scalar initialization. The
queue update at `0x7fac80` adds one source real step, drains through the old limit
on wrap, calls `0x7f9360` to soft-subtract the limit from retained request words,
and drains the new remainder. Its near-zero remainder operand is read from the
image. The implementation repeats the source add; multiplying the interval by a
step count is observably different.

Timed-trigger events use the extended family. The bounded static join is:
`0x4ae060 -> 0x48cee0 -> class getter 0x5093a0`, matching event getter
`0x5091e0`; factory initialization `0x405670` selects the constructor
`0x509300`. That constructor installs the extended timer vtable in its `+0x34`
subobject. Setup `0x5093c0` forwards to `0x7fbf80` with selector zero, and the
vtable's scheduling slot is `0x7fb820`. The reader checks this factory,
constructor, setup and dispatch relationship. This is native-code evidence,
not a new original-client timed-event runtime comparison.

Extended timers retain their **current internal entry**, with the remaining
segment count, separately from the original whole timeout. A future full expiry
is only a legacy projection; it is not a live native queue entry. Internal entries
consume work without delivering a JASS callback. Rebase changes the current
entry and query epoch together. Native queries read the actual entry/count;
legacy queries keep the earlier explicit projection. This distinction matters
when another callback reads a timer at an equal internal-entry deadline.

## Independent original-client observations

Two bounded, private Docker/Wine/Xvfb runs used verified null audio and no host
display, audio device, socket or network access. A PE32 helper used only process
query and memory-read rights. It did not inject, call into, suspend or write the
original process. Map scripts, attributes, helper executables, traces and source
copies remain local.

The first run contains 1,024 attempts: 1,004 readable samples, 20 initial
unavailable samples, 993 stable brackets and 11 changing brackets. After the
preloading queue was replaced, 956 of 957 stable gameplay brackets match repeated
source additions from the fresh origin. Host binary32 nearest-add matches only
44. The remaining stable observation is retained as unexplained.

The independent longer run contains 2,048 attempts: 2,045 readable samples,
three unavailable samples, 2,019 stable brackets and 26 changing brackets. It
observes the gameplay queue counter change from zero to one. In that gameplay
cohort, 2,011 of 2,015 stable brackets match the source add/wrap recurrence; host
nearest-add matches ten. Four stable outliers remain unexplained. A read bracket
is not atomic, and the temporary callback cursor is one possible explanation,
not a proved classification of those outliers. The exact first wrap instant was
not sampled; its model boundary is diagnostic evidence only.

Local receipts:

- `/tmp/pc3-sol09-heal-native-probes-1794518633246501640/queue-runtime-evidence-receipt.json`;
  raw queue trace SHA-256
  `9e372e87b9170478fd4d258dc3795bd6855e9b9ad843b16ccf3aa80f39133fa4`.
- `/tmp/pc3-sol09-heal-native-probes-1794519165737895446/queue-runtime-evidence-receipt.json`;
  raw queue trace SHA-256
  `d26a73551fe6d638d5a52b25601c74cb8e3cf3a5e037b87cb408df658b95b049`.
- `/tmp/pc3-sol02-timed-event-source/receipt.json`: twelve independently bounded
  class/factory/setup range identities, plus its local assertion script.

Both reference containers were independently checked stopped with PID zero after
collection. These observations qualify the bounded queue recurrence under Wine;
they do not establish native Windows or complete campaign tick parity.

## Called path and domain compatibility

Normal selected-source campaign preparation attaches the general queue policy to
`NumericPolicy`, independently of map ability declarations. `map_startup_host`
admits its active state after numeric admission and before VM globals execute.
No new map-input flag or user option selects it.

The application calls
`CampaignRuntime::advance_one_tick -> campaign_host::advance_paced_tick`.
Application `Duration` contributes integer nanoseconds times the source rational
denominator. Whole source intervals are drained one by one; fractional and whole
unconsumed credit remains in the host. A paused callback retains its unfinished
outer step, including either side of a wrap. Resuming does not discard or add a
replacement interval for that partial step.

Driver choice is one-way. A fresh session can choose native paced execution or
the existing caller-time API. A legacy call that advances time or consumes a due
legacy request chooses Legacy; a genuine zero-work same-time call leaves it
fresh. An explicit finite initial caller time whose bits differ from the source
origin, including negative zero, selects Legacy during admission. Its value is
preserved; no native point is inferred from it. Missing or invalid initial clocks
and malformed source policies still refuse. A native session rejects an arbitrary
legacy horizon before mutation, and
a legacy session rejects later native pacing before mutation. Startup requests
are retained in both cases. In particular, timed-event native metadata is
prepared atomically at the first paced call, so choosing Legacy preserves the
old timed-event path. Existing unqualified hosts continue to use caller time.
Legacy horizon receipts are recreation observations, not native producer proof.

Before a source callback the queue selects its stored raw point. Timer starts,
queries and post-callback resume rearm read that point. The outer point is
restored only after expiry settlement. Public `f64` seconds remain a view; they
are never cast back into the native cursor. Unit activity keeps its existing
phase ordering. This change does not silently install a native physics phase.

## Persistent state and checks

All active state belongs to the existing host or arena:

- `ObjectHost.native_game_clock: Option<State>` retains immutable source rules,
  driver domain, completed `Point { cycle, local }`, integer pacing credit,
  pending drain/remainder and selected callback point.
- `Run.native` retains the current native entry point; the old deadline remains
  separately available to Legacy execution. `QueryRun.epoch` retains original
  timeout data, current entry and remaining segment count.
- `autocast_ticks` retains the existing legacy deadline and optional native raw
  point. Default creation and ON preflight the native point before publication.
  Due acquisition remains an explicit refusal in this clock slice.
- The published VM owns its existing timer delivery cursor. No new clock state
  lives in an application singleton or outside `ObjectHost`/the arena.

Deterministic limits charge source steps, internal timer entries, retained timer
admission/rebase visits and existing mandatory activity work. Wrap preflights all
retained timer and autocast words before publishing a rebase. Failed preparation
preserves their prior words. Normal errors retain earlier completed effects,
consistent with the existing session API.

Synthetic regressions cover nonstock ratio/clamps/cycles, fractional carry,
callback-created raw deadlines beyond a cycle, two-phase wrap, domain rejection
and budget refusal. Actual published-session/application `advance_ticks` cases
cover two nonstock wraps, selected-source recurrence across a cycle with a long
extended timer, exact timed-event callback scheduling, pause carry, retry without
extra time, and session replacement. Parser checks and these synthetic effects
are not original-client ability or heading parity.

## Independent frame and sleep queues

The published session now accepts `advance_native_frame(Some(FrameInput), budget)`
with independent granted game duration and raw real elapsed milliseconds. `None`
resumes retained work. The owned FIFO finishes a frame's game work before its real
work; a later frame cannot overtake either phase. Both queues retain their own
completed point, pending wrap drains and selected callback point. The real queue
uses the same source add/subtract and wrap implementation as the game queue.

Native-code bindings identify the real event `0x40090094` at `0x5af4a3`, its
unsigned-counter conversion table, binary32 conversion and SSE multiplication
operand, forwarding through `0x5ea690` to `0x7f4820`, and the integer chunk
initializer at `0x40d060`. The manager constructs its game and real queues through
the same constructor and cycle operand. These operands are read from the selected
image. The observed GetTickCount fallback supplies milliseconds; the alternative
QPC initialization remains unqualified.

Map `TriggerSleepAction` binds through `0x4ae850`, its VM frame flag and
`0x4fbdd0` to an extended timer on the real queue. AI `Sleep` binds through
`0xab7770`, `0xac3e20`, `0xab5640` and `0xad3560` to an extended timer on the
game queue. They reuse the admitted extended-timer policy and current-entry
epoch. Admission computes only the current entry; internal entries consume work
when drained and do not resume a VM. Each queue rebases only its own requests.

A map sleep captures the current real point even inside a game callback. Its
real wake keeps the completed game point visible to timer creation and queries.
AI wakes enter their exact selected game point. New sleeps capture that selected
point, and scope cleanup runs after normal return, re-park and failure. Canonical
thread contexts, AI player identities, VM locals and parked tickets remain owned.
Fresh requests retain their separate legacy deadlines until driver selection;
failed native preparation commits no frame, timer or wake row. Existing legacy
AI zero/negative-duration refusals remain synchronous. Once native mode is
selected, qualified zero and fractional-negative AI sleeps use the source minimum.

Public regressions exercise both real map and game AI wakes, two independent AI
locals, timer creation at the completed game cursor during a real wake, budget
retry without replay, native refusal and world recovery, real and game wraps,
segmented real sleeps, re-park origin, and real wake/unpause without game progress.
These are actual published VM callers with synthetic scripts and nonstock queue
parameters; they do not establish complete original-client campaign parity.

The explicit frame transport does not infer native turn input from elapsed time.
Normal application adoption still needs the source 25-unit quantum, admitted
game-speed/N and carry adapter, including completion of an in-flight quantum when
script pause is requested. The application accelerator is separate from script
game speed. Native mode must be selected before globals/config/main in that
platform factory. The generic Fresh/Legacy APIs and the earlier game-only pacing
prototype keep their existing contracts; neither is a substitute for this adapter.

## Fresh local counter grants

`advance_native_counter_frame` feeds the same FIFO with counter records. The
selected speed setter and scheduler identify the speed-count table; its length,
quantum, credit cap, constructor scale, elapsed ratio and fresh modifier are read
from the selected image. The bound fractional-carry word lies in a validated PE
zero-filled tail. Grant arithmetic uses the scheduler's unsigned conversion,
binary32 multiply/add, finite nonnegative truncation and binary32 carry subtraction.
It does not substitute queue soft arithmetic for the native SSE operations.

Counter records retain checked sub-millisecond carry and DWORD elapsed wrap.
Each quantum captures the current admitted speed. A pause or speed change during
its game work preserves the remainder of that grant; later quanta read the new
state. All quanta precede the record's real advance. This fresh local model
recreates the observable counter phases without implementing network packet pulls,
replay overrides or external controller time-scale changes.

Native speed requests validate their selected table index before replacing the
current identity. Locked requests and legacy different-identity refusals retain
their existing contracts. The counter stores its original duration with the FIFO
record and reports committed duration only after the real phase completes, so
application tick accounting need not count a partly delivered record.

Synthetic kernel and published-session cases cover counter wrap, fractional carry,
in-flight pause/speed changes, pending grants, locked/invalid speed requests,
real-wake unpause and partial-record accounting. Normal application routing is
still held on the coordinated physics, pre-startup factory and resumable selected
callback integration; this counter transport alone does not activate it.

## Remaining fidelity work

The normal application adapter and pre-startup mode selection remain open. The
new called queue transport does not reconstruct source points from `f64`.

The older game-only prototype refuses an unbound map wake before changing its public
clock or consuming its thread. An unbound AI wake refuses before the no-expiry
path could advance the clock to its legacy deadline. Earlier qualified timers
still run. Published regressions exercise both refusals, retained parked work,
and the unchanged Legacy map sleep path. This guard is temporary; it does not
implement the paired queues or enable normal application adoption.

The generic equal-expiry heap comparator, source queue sequence rollover and
ordering across timer, automatic-cast and physics consumers remain open. The
current tie policy is the documented recreation policy. Native heading needs its
own source periodic event and callback/visual phases; existing activity projection
is not proof of that physics producer. Common automatic-cast retry/readiness,
positive acquisition, Heal channel ownership and class lifecycle interruption
remain active work. The omitted `Asps` and `Afak` default-class frontiers are
already separately admitted; acquisition and effects still must execute rather
than skip due requests.

The legacy and game-only prototype session stops future delivery after a
`PauseGame` callback. The independent frame transport completes already granted
game work. Native observations show completion of an in-flight 25-unit quantum,
then suppression of later game quanta while real sleep continues. The normal
adapter must reproduce that grant boundary; preserving either generic session
contract by itself is not native pause parity.

The outer native scheduler's speed/input/QPC calibration and wall-clock pacing
are still only partially qualified. The new caller supplies nominal game-duration
pacing; it does not reproduce those unqualified outer scheduler inputs. Full save
continuation must include the active queue and pending caller budget/phase;
this change does not enable a partial save format.
