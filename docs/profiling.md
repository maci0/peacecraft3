# FPS and bottleneck profiling

Press **F8** to show or hide FPS, smoothed frame time in milliseconds and a
rolling graph of the last 120 frame times. The graph uses individual frame
samples, oldest at the left and newest at the right; its labelled vertical range
adapts to the observed samples. The display refreshes four times per second.
These are real frame timings, independent of accelerated simulation ticks.

Press **F7** for display/backend details, entity and image counts, CPU mesh-asset
vertex counts, device allocator usage and available campaign diagnostics (tick,
speed, seed, clock, selected units and errors). Mesh-asset vertices are not a
drawn-vertex count: instances may reuse assets, and render-only assets may lack a
CPU copy. Allocated/reserved device bytes are distinct from whole-GPU VRAM.
When the overlay is installed, F7 is reserved for diagnostics; use the Quests
button if its authored shortcut is F7.
On Linux, the [AMDGPU driver counters](https://www.kernel.org/doc/html/latest/gpu/amdgpu/driver-misc.html)
expose whole-GPU VRAM used/total through read-only sysfs for the selected PCI
adapter; those totals include other apps.
Unavailable memory reports are labelled rather than replaced with zero.

Both panels start hidden, stack at the top right, and do not intercept input.
They share a full-window UI camera separate from the original menu's authored
viewport. When both are hidden the camera does no rendering, and panel sampling
and formatting stop. Focus the game window or browser canvas before using the
keys. These diagnostics report Bevy frame cadence, not cinematic video frame
rate, and do not change simulation state or consume random draws.

The native diagnostic component is visually checked at 1280×800 and 480×800.
Its wide/narrow captures show actual RADV allocator and whole-GPU VRAM values;
the synthetic fixture runs no campaign. Unit tests cover independent toggles,
bounded history, invalid samples, mesh counts and unavailable memory reports.
Campaign text is bounded to three error excerpts; complete errors remain in
the existing diagnostic log. Browser compilation passes; these native captures
do not establish browser rendering or full-campaign behavior.

FPS and all CPU/GPU elapsed times are observational reports affected by competing
work, clocks and presentation. They are not performance regression thresholds.
Use [deterministic performance gates](performance-testing.md) for pass/fail.

## Available profiling routes

```sh
python3 tools/profile.py doctor
cargo build --locked --profile profiling -p peacecraft3 --features profile-cpu
cargo build --locked --profile profiling -p peacecraft3 --features profile-gpu
TRACE_CHROME=.scratch/profiles/bevy-trace.json target/profiling/peacecraft3
```

The profiling build preserves symbols. `profile-cpu` enables Bevy's system spans
and Chrome trace output, which can be opened in a local Perfetto viewer. Close the
app normally so the tracing guard flushes the file. `profile-gpu` enables Bevy's
render diagnostics and diagnostic logging. GPU timestamps/statistics depend on
backend capabilities; unsupported backends still expose CPU diagnostics. See
[Bevy's profiling guide](https://github.com/bevyengine/bevy/blob/v0.20.0/docs/profiling.md).

For native Linux CPU stacks:

```sh
python3 tools/profile.py cpu -- target/profiling/peacecraft3
perf report -i .scratch/profiles/cpu.data
```

The helper uses user-space cycles and DWARF stacks. Profiling overhead, scheduling
and clocks affect these samples; they diagnose hotspots rather than assert speed.
The local kernel permits self-process performance counters. Permission failures
on another host are reported, not bypassed by changing global kernel settings.

## bpftrace CPU flame graphs

Launch the game normally, then attach the observer with `sudo`; the game keeps
its user and desktop-session environment. On this host bpftrace 0.27 supports
[`dw_ustack`](https://bpftrace.org/docs/release_027/stdlib#dw_ustack), which uses
DWARF unwind data rather than requiring a frame-pointer rebuild. That helper is
experimental and x86-64-only. An ordinary `ustack` capture can be incomplete
when the binary omits frame pointers.

```sh
sudo bpftrace -p "$game_pid" -e '
config = { unstable_dw_ustack=enable; max_map_keys=8192; }
profile:hz:99 /pid == $1/ {
  @[dw_ustack(bpftrace, 64)] = count();
  @sample_count = count();
}
interval:s:12 { exit(); }
' "$game_pid" > .scratch/profiles/cpu-bpftrace.txt

perl /path/to/FlameGraph/stackcollapse-bpftrace.pl \
  .scratch/profiles/cpu-bpftrace.txt > .scratch/profiles/cpu.folded
perl /path/to/FlameGraph/flamegraph.pl \
  .scratch/profiles/cpu.folded > .scratch/profiles/cpu.svg
```

The conversion tools are from [FlameGraph](https://github.com/brendangregg/FlameGraph).
Keep raw stacks and warnings, verify folded weights match the collected map
count, and inspect stack depths and unresolved frames. Rust frames may require
individual symbol demangling before rendering. These are sampled on-CPU
proportions, not elapsed-time gates or GPU measurements. The
[CPU report](performance/cpu-hotpaths.md) records the verified local captures.

## AMD graphics and ROCm compute are different routes

The linked [rocprofv3 documentation](https://rocm.docs.amd.com/projects/rocprofiler-sdk/en/latest/how-to/using-rocprofv3.html)
describes HIP/HSA tracing and compute kernels. The game uses wgpu graphics, so
running its Vulkan renderer under rocprofv3 does not establish GPU draw profiling.
The installed rocprofv3 and rocpd are usable for an actual HIP/HSA workload:

```sh
python3 tools/profile.py rocprof -- /path/to/hip-workload
```

The helper collects `rocpd`, not deprecated direct CSV/PFTrace output. Empty GPU
data is not evidence that the renderer did no work. Hardware-counter collection
on Radeon gfx11/gfx12 also requires the documented stable performance state;
the helper does not alter GPU clocks, power policy or system permissions.

This host has an RX 7900 XTX with Mesa RADV. Mesa provides its own graphics
capture route, analyzed by [Radeon GPU Profiler](https://gpuopen.com/rgp/):

```sh
python3 tools/profile.py gpu -- target/profiling/peacecraft3
# In another terminal, once the desired scene is running:
python3 tools/profile.py trigger
```

The helper sets only child-process `WGPU_BACKEND`, `MESA_VK_TRACE` and its trigger
path. RADV writes captures to `/tmp`; use a local RGP viewer to inspect the result.
These controls are documented by [Mesa](https://docs.mesa3d.org/envvars.html#envvar-MESA_VK_TRACE).
This route needs real RADV/Vulkan execution; browser SwiftShader/WebGL cannot
validate AMD hardware bottlenecks. RGP viewer availability is listed by `doctor`. The local RGP 2.7.1
package is under `.tools/profiling/` with its bundled libraries. A headless
CLI probe did not establish viewer behavior. A newer native launcher capture has
now been opened and visually inspected in RGP; the [GPU report](performance/gpu-launcher.md)
records that evidence and its remaining campaign-coverage limits.

For browser frames, use browser Performance tooling and the actual WebGL/WebGPU
backend. On macOS use Xcode's Metal profiling tools; Windows Vulkan/DX12 uses the
applicable graphics profiler. These platforms are not validated by Linux traces.

## Current validation limits

The FPS toggle/camera test and both profiling-feature compile checks pass. The deterministic movement allocation/state
gate passes under concurrent CPU load. Native `perf` instructions and a CPU stack-capture smoke check pass
on this host. Installed Valgrind cannot decode an instruction in the system loader
and fails before the workload; no Callgrind baseline is claimed. RADV produced a 9,079,804-byte RGP capture from the cached native Vulkan
launcher at frame 61. This verifies hardware capture infrastructure, not the
latest gameplay performance or viewer analysis. F8 was visually verified in the rendered browser app at wide and portrait
viewport sizes, before and after resize. Changed pixels stay at the canvas
top right, and hiding the counter restores the prior image exactly. No FPS
threshold is used. Native profiling executables remain a separate check.

Generated traces, captures and original asset content stay local and untracked.

The [five-round optimization record](performance/five-rounds.md) adds deterministic
CPU/allocation/upload checks, native GPU reports and accelerated fixed-tick pacing.
On the latest host check `perf stat` works but both hardware and software stack
sampling are denied; the earlier stack-capture smoke result above is historical.

The [CPU hotpath capture](performance/cpu-hotpaths.md) records a newer successful
one-run privileged observer with the game still running as the normal user. It
also documents optional core phase spans, sampled hotspots, and the remaining
rendering and validation coverage. Unprivileged sampling remains denied; no
host profiling permissions were changed.
