# Performance tests without background-load failures

Performance regression gates must use fixed work and deterministic counters, not "finishes in N milliseconds". FPS and elapsed time remain useful measurements, but they vary with other processes, scheduling, GPU contention, clock/thermal state and profiling overhead. They are report-only, never pass/fail thresholds.

## Current audit

The existing `crates` and `tools` tree had no performance benchmark harness or wall-clock speed assertions. The following timing checks are functional rather than performance budgets and remain separate:

- `intro.rs`: a locally installed decoder must eventually produce a frame, and skipping must cancel/reap it promptly. These explicitly ignored media/runtime tests use liveness deadlines, not a throughput baseline.
- `pcx-net/transport.rs` and `src/host.rs` (run by the `session_server` binary): discovery, handshake and connection timeouts are protocol/liveness behavior.
- `menu_panel.rs`, `mainmenu_scene.rs`, and `menu_sprites.rs`: animation tests advance an injected simulation clock or inspect authored clip lengths. They do not measure host execution time.
- Browser checks use timeouts to wait for observable state. These are functional failure bounds, not FPS gates.

A saturated machine can still hit a functional liveness timeout; passing the performance gates below does not remove that practical distinction. No existing timeout has been relabeled as load-independent.

## Deterministic simulation gate

```sh
python3 tools/perf/run.py
```

This standalone tool calls the existing `pcx_sim::step`; it does not copy or replace the simulation. The workload is versioned `sim-movement-v1`: seed `0x574333`, 512 units, four players, 2,000 fixed ticks, and one deterministic move/stop/nop intent per player per tick. World and command setup happen before measurement. The gate checks the final world digest and actual allocation calls/requested bytes while executing those ticks. Its sorted input now requires zero allocations, reallocations and requested bytes. Unordered input retains the stable sorting fallback. See the [five optimization rounds](performance/five-rounds.md) for the unchanged-digest comparison and other work gates.

The counters come from a forwarding process allocator, with no wall clock in gate mode. This catches allocation regressions in real simulation work even when background processes slow execution. The fixed tick/command/unit counts identify the workload; they are not claims about the simulator's internal instruction count. A CPU regression that adds work without allocating can pass this gate and needs instruction profiling/review. Changes to gameplay semantics require reviewing the expected digest, not automatically accepting a new baseline.

The small tool has its own locked manifest and release profile; it adds no dependency to the game or workspace. Its generated binary and reports live in ignored `.scratch/perf/`. It exercises the current movement simulation only: combat, economy, map loading, asset formats, scripted campaigns, menu rendering and the GPU need their own fixed-work counters as those paths become complete.

The counting allocators here and in `crates/pcx-mdl/tests/pose_sampling.rs` need the unsafe `GlobalAlloc` interface to observe actual allocation calls. Both forward unchanged layouts and pointers to `System`; their counters do not allocate or unwind. The pose test uses saturating increments so counter overflow cannot unwind from an allocator callback. Each implementation carries its safety contract and the only local exception to the `unsafe_code = "deny"` policy. The game libraries and application inherit that policy from the workspace.

The retained `vendor/naga` 26.0.0 snapshot now forbids unsafe code, uses checked handles/nonzero alignment arithmetic and the standard UTF-8 boundary method, and requires Rust 1.99 / edition 2024. It is separate from the registry Naga 30.0.1 selected by the application's lockfile on 2026-10-14. Standalone checks use a byte-identical copy in `.scratch/unsafe-naga/`, with only a `[workspace]` table appended for isolation: 115 native tests pass in both debug and release; minimal/no_std and all-feature WASM checks deny warnings and pass. These are synthetic code checks, not original-game runtime evidence.

## Optional reports

```sh
python3 tools/perf/run.py --timing
python3 tools/perf/run.py --instructions
```

The timing path emits `elapsed_ns_report_only`. That number never affects exit status. It is a diagnostic for the actual machine and current load; do not compare it as a CI speed limit.

The instruction path uses an installed Valgrind Callgrind and collects only entry/exit of `pcx_perf_measure`. The `*pcx_perf_measure*` matcher includes the ordinary Rust function name, preserving the collection boundary without an unsafe unmangled symbol export. Setup, world digest, JSON output and process startup are outside that collection region. It still includes the allocation-counter instrumentation and allocator code invoked by the workload, so this is the instrumented workload's instruction count. Callgrind's `Ir` counts are deterministic executed work rather than wall time; its [primary documentation](https://valgrind.org/docs/manual/cl-manual.html) describes entry/exit collection and instruction events.

Instruction reports record compiler, target architecture, operating system, build profile, Rust flags and Valgrind version. Compare only matching conditions; compiler changes, code generation, library/allocator versions and architecture can alter instruction counts. These counts do not measure GPU work, memory/cache stalls, frame pacing, or real elapsed latency. The tool deliberately reports instruction counts without inventing a universal cross-machine instruction budget. Allocation/state gates still run during instruction profiling.

Outputs are `.scratch/perf/results/gate.json`, `timing.json`, `instructions.json`, and `simulation.callgrind`. A failed/empty Callgrind collection reports an error rather than presenting zero work as a success. No additional profiler installation or paid service is required for the default gate.

## Verified local result and profiling limitation

The initial fixed workload passed with 2,000 allocations, zero reallocations, 64,000 requested bytes on x86_64, and state digest `867f3f6327d60281`. Five additional executions with a concurrent CPU-busy process produced identical gate metrics and passed. The optional timing path also passed; its elapsed value was reported only.

The installed Valgrind 3.25.1 could not profile this host: its decoder rejected instruction bytes beginning `62 F1 7F 48` inside the system dynamic loader's `_dl_start`, before the workload started, and terminated with SIGILL. The resulting zero collected instructions is **not** a benchmark result. The instruction command correctly failed; no baseline or speed claim was derived from it. Use a compatible Valgrind/system runtime combination for instruction collection. The allocation/state gate works independently of that profiler. Profiler subprocess core dumps are disabled so a failed engine does not leave binary dumps in the source tree.
