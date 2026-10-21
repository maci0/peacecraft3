# CPU hotpath capture

CPU work is measured separately from frame cadence and GPU timestamps. Neither
sample percentages nor instrumented elapsed times are pass/fail performance
thresholds. Deterministic work, allocations and resulting state remain the
regression gates.

## Capture paths

`profile-cpu` now forwards from the application into the core libraries. The
optional spans cover map admission, startup preparation, object tables, asset
resolution and extraction, script parsing/resolution/VM/native dispatch, timer
and event delivery, unit activity, motion, routing, combat, guard/acquisition,
collision metadata, regeneration, inventory, audio decoding, model parsing,
bone matrices, skinning and mesh position submission. Bevy supplies system and
render scheduling spans. Normal builds omit these additional core spans.
The collision-field span measures derivation from metadata, not use of an
already retained radius.

The spans use target `pcx_cpu`. Native calls record their native name, without
recording arguments or asset payloads. A tracing-chrome collector must enable
argument recording to retain that field; Bevy's standard Chrome collector
includes formatted fields in span names. Close the application normally so the
collector flushes the trace.

`tools/profile_trace.py` summarizes threaded B/E traces, reporting both inclusive
elapsed time and time excluding nested child spans. Its tests cover nesting,
interleaved threads, native identities, bounded output and malformed/truncated
input. It refuses async and complete-event formats rather than assigning them
incorrect exclusive times. It reports monotonic wall time, not thread CPU time.

On the current host unprivileged counters work but stack sampling returns
`EACCES`. A one-run privileged perf observer successfully records a workload
launched through `sudo -u maci`; the game remains UID 1000. No executable
capabilities, sysctls, clocks, or power policy were changed. Linux also documents
`CAP_PERFMON` for profiling access; see the
[kernel security guidance](https://www.kernel.org/doc/html/latest/admin-guide/perf-security.html).

## Human03: startup and simulation

The bounded workload starts the unchanged selected Human03 map in RoC mode with
seed 1 and runs 4,096 ticks (64 game seconds) through production headless startup
and clock advancement. It sends no player order. The runner can send the normal
end-cinematic event if a game pause is observed; no such event was needed in
these captures (`skipped_intro=false`). This is a profiling workload, not a
whole-map acceptance test.

The selected executable SHA-256 is
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`;
the map SHA-256 is
`a62e36a1825a2edb0e2259d36db0c44fb50cb877a457014ed568168045559b82`.
Inputs are the selected local Legacy install, not uploaded or embedded assets.

The optimized profiling build uses O3, thin LTO and symbols. A stack capture of
five runs with the additional core spans compiled out recorded 1,897 cycle
samples and zero lost samples. The leading **self** sample proportions in the
combined capture (startup, gameplay and a small wrapper) were:

| Sampled work | Approximate share |
| --- | ---: |
| Combat acquisition iteration/minimum selection | 18.44% |
| Huffman decompression | 13.95% |
| Unit collision-radius metadata lookup | 13.84% |
| SHA-256 compression | 5.32% |
| Weapon-target validation | 3.61% |
| ADPCM decompression | 3.46% |
| Region event polling | 2.99% |
| SLK table insertion | 2.85% |
| MP3 layer decoding | 2.25% |

These percentages prioritize investigations; they are not speedup claims. User
Rust symbols resolve. Kernel symbols remain restricted, so kernel samples cannot
be attributed by symbol. The capture also reports unsuccessful BPF metadata
synthesis; it does not report lost cycle samples.

A separate fine-grained trace recorded 1,061,505 completed spans. It counted
324,473 combat acquisition attempts, 336,781 guard steps and 295,290 collision
metadata queries. Its elapsed times include instrumentation effects and must
not be compared directly with the uninstrumented stack run as a speed benchmark.
The five uninstrumented runs and the instrumented run have identical unit handle,
rawcode, creation-owner and position projections plus complete source RNG state.
That comparison does not cover every VM global or rendered state.

## Retain derived collision geometry

Each created unit now retains the result of validating its own resolved
`UnitBalance.collision` field. Combat, movement, guards and range events read
that result instead of walking the definition again. No rawcode, field id or
radius is copied into a constant. Missing or malformed geometry remains a
deferred query error with the current caller's context; creation does not newly
refuse it. There is no admitted runtime collision setter. A future setter must
update the retained value.

The same Human03 workload now records 148 derivations, matching its 148 observed
creation native calls, instead of 295,290 queries that each walked metadata.
Its unit projection and complete source RNG state remain byte-identical to the
baseline, both with and without core tracing. A deliberate repeated-scan mutant
still produces the same projection but fails the work-count gate (295,438
derivations for 148 creations). The exact candidate source was restored afterward.

One uninstrumented counter comparison recorded 34,413,117,816 retired user
instructions before and 32,679,071,407 after. This combined startup/gameplay
measurement is report-only; it is not a cross-machine or elapsed-time gate.
The work gate compares derivation and creation counts, independent of host load.
Core library tests (1,071), actor tests (15, two explicit gates ignored), strict
application Clippy and application WASM checks pass on the candidate.

## Combat acquisition

Combat now gathers unit handles once after spell and summon advancement, in the
same arena order. The list also retains each unit's initial attacker eligibility,
as the previous attacker list did. Each acquisition scans these handles rather
than all non-unit arena entries. Target liveness, protection, current position
and hostility are still checked at selection time, including after an earlier
unit kills a target in the same step. Combat queues callbacks for later delivery;
it does not execute script-created units inside this loop. This list is local to
one step, not a cross-step cache.

Nearest-target comparisons retain `total_cmp` and the existing first-handle tie
rule, while computing each candidate's distance once. Hold Position retains its
separate reach rule. The per-step vector now includes all units and an attacker
eligibility flag, so its storage can be larger than the previous attacker-only
vector. No spatial approximation, target reordering or RNG draw is introduced.
This preserves the recreation's acquisition policy; native policy remains
unqualified.

The two new synthetic regressions cover equal-distance selection with non-unit
handle gaps (idle and held), and acquisition after a target dies earlier in the
same combat step. All 1,073 core tests pass (29 explicit gates ignored), along
with strict application Clippy and application WASM checks.

The unchanged Human03 workload preserves the same unit projection and complete
source RNG state, all 249 span call counts, and the ordered sequence of 12,366
native-call names. This does not compare native arguments, every VM global or
rendered state. One uninstrumented comparison recorded 32,678,945,389 retired
user instructions before and 27,761,127,778 after, about 15% fewer. This combined
startup/gameplay counter measurement is report-only, not an FPS, elapsed-time
or portable instruction threshold. Receipts are in `acquisition-validation/`
under the local evidence directory.

Local evidence is retained under `.scratch/profiles/cpu-hotpaths/`: raw traces,
perf data, flat and call-chain reports, build logs, repeated-run receipts, and
the bounded driver. Nothing there is uploaded.

The instrumentation checkpoint passed 1,430 core library tests (43 explicit gates
ignored), strict application Clippy with `profile-cpu` on all targets, application
WASM checks, core WASM checks with profiling enabled, four trace-summary tests,
and formatting. The object-file census requires the explicit local install;
its initial run without those inputs failed and is retained separately.

Four X11 renderer capture attempts are invalid. The X11 window stayed unmapped;
forcing a map request did not establish visibility. The first trace also grew
with repeated audio-backend timestamp errors. The second capture reported lost
sample chunks and was terminated without a complete trace. Animation spans
alone do not prove visible rendering. A third bounded probe preserved the normal
desktop-session environment and still did not establish visibility. An activation
request in the fourth did not remove its hidden state. These runs
contribute no frame or GPU performance claim. Campaign HUD, audio mixing,
GPU driver submission and all-map
coverage remain open.

## Privileged observers and BPF flame graphs

A normal-user Wayland viewer now closes normally through its own window's close
request, producing a complete trace. KWin reports that owned window as active,
not hidden and not minimized. The earlier X11 diagnostic used `windowclose`,
which destroys the X11 window; its subsequent swapchain-loss messages are not
evidence of an initial rendering failure. Use a close request (`windowquit` on
X11), not destruction, when a tracing collector needs to flush.

A `sudo perf` observer attached to the normal-user Wayland viewer recorded 4,494
samples over twelve seconds, with no lost-sample warning. No capabilities,
sysctls or power settings were changed. This uses a development build and a
single model viewer; it is not rendered campaign acceptance.

The installed bpftrace 0.27 has DWARF unwinding support. A process-filtered
`profile:hz:99` probe with `dw_ustack` produced a zoomable, Rust-demangled CPU
flame graph from 1,171 samples of repeated unchanged Human03 startup and 4,096
simulation ticks. The process completed all 24 repetitions with the same unit
projection and complete source RNG state. The probe samples twelve seconds of
this repeated workload, not every tick of every repetition.

Splitting by observed `headless_startup::start` and `Startup::step_clock` stack
branches assigns 930 samples to startup, 217 to simulation, and 24 to other work.
Separate SVGs retain those weights. Startup's leading self samples are Huffman
decompression (212), SHA-256 compression (187) and ADPCM (67); simulation's are
acquisition iteration (55), weapon-target checks (30) and region polling (25).
These sample counts prioritize investigation; they are not deterministic work
budgets or timing thresholds.

Stack depths are 10–31, below the configured 64-frame limit. All samples contain
an unresolved runtime address, although the Rust hotpath frames resolve. The
external DWARF helper generated nine kernel-header declaration warnings, retained
with the raw output. Folded weights match the collected map count; the SVGs
parse and the combined graph was visually checked. This does not prove complete
symbol resolution or full unwind coverage.

The corresponding model-viewer BPF capture has only thirteen samples, too few for
hotspot conclusions. All raw data and graphs remain local under
`campaign-bpf-dwarf/` and `viewer-bpf-dwarf/` in the evidence directory. FlameGraph
tools are pinned at `41fee1f99f9276008b7cd112fca19dc3ea84ac32`; no original assets
or captures were uploaded. Full campaign rendering, GPU analysis and native
lockstep remain open.

## Huffman input buffering

The loading flame graph led to a bounded byte reservoir in the Huffman bit
reader. Tree traversal now consumes buffered bits instead of indexing the
compressed input for every bit. The reservoir holds at most fifteen bits and
loads one byte only when needed; it never reads past the input slice. Tree
weights still come from the imported codec image, and tree construction,
adaptive updates, output limits and the public decoder API are unchanged.

A synthetic differential test compares mixed-width reads with the former
bitwise method across every byte value, byte offset, read width and truncated
prefix of its four-byte fixtures. The codec suite passes 76 tests, strict
profiling-enabled Clippy and WASM checks pass, and all 2,716 retained native WAV
dumps still match decoded bytes. The dump SHA-256 manifest also verifies. These
dumps are from compatibility Storm under Wine, not Legacy-runtime PCM or audible
output evidence.

Two alternating baseline/candidate Human03 runs preserve the unit/RNG projection.
User-mode instruction counts are 27,761,163,231 / 27,761,109,383 before and
27,336,885,834 / 27,337,009,656 after, about 1.5% fewer across startup and 4,096
ticks. These hardware counters remain report-only. Frozen input hashes, commands,
logs and state comparisons are local in `huffman-validation/`; no timing gate or
new claim about rendering, GPU work or native lockstep follows from this change.
