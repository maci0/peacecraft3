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

Local evidence is retained under `.scratch/profiles/cpu-hotpaths/`: raw traces,
perf data, flat and call-chain reports, build logs, repeated-run receipts, and
the bounded driver. Nothing there is uploaded.

Affected-target validation passed: 1,430 core library tests (43 explicit gates
ignored), strict application Clippy with `profile-cpu` on all targets, application
WASM checks, core WASM checks with profiling enabled, four trace-summary tests,
and formatting. The object-file census requires the explicit local install;
its initial run without those inputs failed and is retained separately.

Two renderer capture attempts are invalid. The X11 window stayed unmapped;
forcing a map request did not establish visibility. The first trace also grew
with repeated audio-backend timestamp errors. The second capture reported lost
sample chunks and was terminated without a complete trace. Animation spans
alone do not prove visible rendering. These runs contribute no frame or GPU
performance claim. Campaign HUD, audio mixing, GPU driver submission and all-map
coverage remain open.
