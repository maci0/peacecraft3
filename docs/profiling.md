# FPS and bottleneck profiling

Press **F8** to show or hide FPS and smoothed frame time. The counter starts
hidden, uses a full-window UI camera, stays at the game screen's top right and
does not intercept input. It is separate from the original menu's authored
viewport. Its camera does no rendering while hidden. It reports Bevy frame
cadence, not cinematic video frame rate or simulation ticks.

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
CLI probe did not establish viewer behavior; opening and inspecting a real
capture remains a separate verification step.

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
