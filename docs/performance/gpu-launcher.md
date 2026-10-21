# Native launcher GPU capture

The `965279c` tree builds the native application with `profile-gpu` in its private
target. Its binary SHA-256 is
`4b044f14841701d3785afbd1fada9721457b0a130dff9cc5cdc7f71910e4c947`.
The normal-user application selects the RX 7900 XTX, RADV NAVI31, Mesa
26.2.4-arch3.1 and Vulkan. Only its own window was captured. No driver settings,
clock policy, sysctls or other applications were changed.

The visible scene is the native launcher using the selected Legacy install,
after switching to Reign of Chaos. Attempts to select Human03 did not establish
a chapter start. This is **not** campaign rendering, gameplay acceptance or a
campaign GPU bottleneck measurement. The cause of the unobserved chapter start
is unresolved; input delivery and application behavior have not been separated.

## Measurements

Between 14:09:00 and 14:11:00 UTC on 2026-10-21, 121 diagnostic reports give the UI
pass GPU median as 0.044834 ms (range 0.041723–0.047772 ms). The empty 2D opaque
pass diagnostic median is 0.000211 ms. These are development-build observations
under current host load, not regression gates or optimization speedups.

RADV saved a separate 9.7 MiB RGP capture at its frame 3864. RGP 2.7.1.13 opens
the file and displays Vulkan queue submissions, user markers, pipeline events,
wavefront/cache-counter plots and the NAVI31 device information. The event-timing
view labels `ui` as 49.794 microseconds and `main_opaque_pass_2d` as 38.345
microseconds in this captured frame. Keep these RGP event intervals separate
from the diagnostic-query intervals and their multi-report medians. Overlapping
draw durations must not be summed into a pass time.

The capture was visually inspected in the actual RGP viewer on an owned virtual
display. This establishes that the capture can be analyzed, beyond merely
creating a nonempty file. It does not establish complete instruction-trace
coverage, an ISA-level bottleneck, or a shader optimization opportunity. The
viewer needed termination after its quit shortcut did not exit; the application
itself closed normally, with no panic or error in its retained log.

## Failed route and local evidence

Launching the application on Xvfb selected the real AMD adapter but failed at
presentation because that server has no DRI3 support. A render-thread panic is
present despite a zero process exit code. That run is explicitly invalid; the
real-display capture does not use its output. Xvfb was sufficient for RGP's
ordinary viewer UI, which does not validate Vulkan presentation there.

The capture, owned-window screenshots, RGP analysis screenshots, raw diagnostic
log, bounded launch scripts, build log and SHA-256 manifest remain local under
`.scratch/profiles/gpu-campaign/`. `diagnostics.json` records each reported
metric's unit, sample count, median and range. `invalid-xvfb.json` preserves the
failed route. All owned application, profiler and virtual-display processes have
stopped. No original assets or captures were committed or uploaded.

Full rendered campaign coverage, GPU measurements during combat and movement,
and native visual/lockstep parity remain open. See the [profiling routes](../profiling.md)
and [CPU capture evidence](cpu-hotpaths.md) for the distinct CPU and compute paths.
