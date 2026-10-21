# Five optimization rounds and accelerated simulation

Comparisons fix the workload, inputs and compiler. Allocation counts, route work
and asset modification events are regression gates. Time, FPS, hardware cycles
and GPU timestamps are observations only. Original content and receipts remain
local in `.scratch/profiles/five-rounds/`.

| Round | Workload | Before | After | Preserved check |
| --- | --- | ---: | ---: | --- |
| Sorted commands | 512 units, 2,000 ticks | 2,000 allocations / 64,000 bytes | 0 / 0 | digest `867f3f6327d60281` |
| Disconnected goal pocket | synthetic 32×32 grid | 4,647 route work | 648 | endpoint `[66,54]`, clear segments, occupancy |
| Identical skinned positions | 100 repeated frames | 100 mesh modification events | 0 | exact position bits; a changed bit still emits one event |
| Asset path identity | 1,000 Unicode paths | 2,000 allocations | 1,000 | normalization and unsafe-path rejection |
| Skeleton composition | 100 independent bones | 103 allocations | 4 | exact hierarchy/cycle/pose outputs |

Sorted input bypasses stable sorting; out-of-order and duplicate-player handling
remain stable. Pocket pruning requires a closed component unreachable from the
seeded start, including diagonal corner rules. An incomplete proof retains the
old lower bound. The 128-query proof ceiling bounds optional algorithm work; it
is not an original game constant. Proof queries consume the existing work budget;
route limits are unchanged. Path normalization lowercases its owned string in
place. Skeleton composition reuses its traversal stack without changing matrices.
Connected blocked goals can incur bounded proof overhead; the pocket result is
not a claim that every route is faster.

Position comparison happens before mutable asset access. Unchanged geometry
therefore avoids Bevy's mesh-modified event and subsequent preparation/upload
when no other system modifies that mesh. Skinning, lighting, colour, textures and
animation still run. This does not claim a universal GPU frame-time reduction.

```sh
python3 tools/perf/run.py
cargo test -p pcx-play --lib unit_placement --locked --offline
cargo test -p pcx-gfx --test mesh_upload --test pose_composition --locked --offline
cargo test -p pcx-assets --locked --offline
cargo test -p pcx-mdl --test pose_sampling --locked --offline
```

The simulation gate now requires zero allocations for its unchanged sorted-input
workload and state digest. Other delivery orders retain their sorting fallback.

## Clock control

**Ctrl+Shift+F9** cycles campaign pacing **1× → 2× → 4× → 8× → 1×** and logs
the rate. Plain F9 does not change it. Embedders can use
`CampaignRuntime::set_speed(NonZeroU32)` and `speed()`. This application control
does not rewrite authored `SetGameSpeed` or time-of-day settings.

Only new wall time is multiplied. Existing debt is unchanged, loading remains
excluded, pauses do not accrue ticks, and overflow stops the session. The update
budget still bounds ticks per frame; excess debt drains later. Every fixed tick
uses the existing host and tick-derived deadline. No timers, random calls or
unit projections are skipped or aggregated. Volume-group fades retain their
wall clock and sound is not pitch-shifted.

Tests compare timer results and debt across 1×/2×/4×/8× and different partitions.
The ignored source-RNG gate requires `PEACECRAFT3_RANDOM_REAL_IMAGE` and
`PEACECRAFT3_RANDOM_BINDING`. It compares callbacks, random results, complete
source-bound random state and 64 subsequent draws.

The local unchanged Human03 comparison also passes through the application's
`CampaignRuntime`: 1,024 ticks (16 game seconds), with all 131 complete executed-unit
records, RNG state and the next 64 draws equal across all four rates. It takes
1,024/512/256/128 application updates respectively. This is a headless comparison,
without injected player orders or a renderer. A separate 4,096-tick run reaches
64 seconds without the previous movement-budget failure at tick 2,570; it does
not establish chapter completion.

These establish internal equivalence for those workloads, not whole-game native
lockstep. The [native scheduler](../re-notes/legacy-simulation-clock.md) and
[RNG startup/consumer gaps](../re-notes/original-rng-control.md) remain. Replay
inputs must be matched by logical tick rather than wall time.

## Hardware profiling

The native baseline uses the selected Legacy Footman in the model viewer,
RX 7900 XTX, RADV 26.2.4 and a symbol-preserving development build with
`profile-gpu`. Bevy reports actual GPU timestamps and shader invocations. RADV
produced an approximately 10 MB RGP capture through
[Mesa's frame capture](https://docs.mesa3d.org/envvars.html#envvar-MESA_VK_TRACE_FRAME).
No global driver, permission or clock settings changed.

This is a model-viewer capture, not a full rendered campaign. Live frame numbers
do not fix animation phase; timing and shader counts are report-only. The fixed
mesh-event test isolates redundant uploads independently of graphics load.
Capture collection alone does not establish shader occupancy/cache bottlenecks.

`perf stat` user instructions/cycles work. Hardware and software stack sampling
were denied, so no flamegraph or sampled hotspot percentages are claimed.
Instruction counts depend on compiler, architecture and allocator. Deterministic
work counters, not timing thresholds, decide regressions.

Before/after captures and 29 timestamp reports per run retain 588 vertex-shader
invocations and 372 clipped triangles for the native model view. Fragment work
and timings vary with animation phase and machine load. The baseline opaque-pass
GPU median is 0.007674 ms; its 0.039105 ms clustering median is higher. These tiny
scene timings identify where this scene spends work, not a full-campaign bottleneck
or an optimization speedup. CPU stack-sampling denial remains a profiling limit.

## Verification

The private tree passes 1,560 affected core tests and 809 app tests (73 and 49
ignored respectively), strict all-target Clippy with `test-fixtures,profile-gpu`,
the app WASM check and the standalone performance tool's strict Clippy. Three
runs with an owned CPU-busy process match the unloaded allocation/state gate.
The retained 2,883 selected-code/native-backed RNG checkpoints also pass; they
retain their original evidence limits. Scoped formatting and frozen input hashes
are checked. Raw logs, library manifests, input identities, timing summaries and
the two RGP captures remain local.
