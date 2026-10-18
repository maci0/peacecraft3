# Launcher monitor calibration

The earlier small-headless-display overflow was a test-display mismatch, not
proof of a launcher sizing defect. On this host, Xvfb's requested framebuffer
can differ from its active RandR output. The retained 1024×768 captures had a
larger active output; the launcher fitted to that reported monitor. Winit
0.30.13 derives monitor dimensions from RandR CRTC bounds, as seen in its
[monitor implementation](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/x11/monitor.rs).
Changing the launcher's production fraction would have concealed that mismatch.

The shared original/native comparison boundary now checks both `xdpyinfo` and
`xrandr --current`. It requires one active output with the scenario's exact
extent at the framebuffer origin before it searches for or focuses a window.
Successful receipts include that output's identity and bounds. The native PC3
filter permits only the read-only RandR query; display preparation remains
owned by the caller. The runtime image declares `x11-xserver-utils` explicitly.
No original asset or game executable enters that image.

Six synthetic mismatches reproduce the missing guard before the correction.
All 48 Python harness tests pass afterward, including the command filter and
the existing offline schema/process/output checks. A real PC3 process with an
exact-size client on a mismatched display refuses after the two display reads,
before any focus, input or capture command. Three consistent live displays pass
the adapter and produce exact-size client captures. These checks establish
calibration behavior, not original game behavior or CLI-wide conformance.

The unchanged production launcher also runs at its natural size on six
consistent headless desktops: 640×800, 1024×768, 1280×960, 1920×1080,
2560×1440 and 3840×2160. Each client fits inside its framebuffer without a
manual window resize. Eighteen configuration/keyboard captures were reviewed.
They exercise wrapping source paths and focused controls scrolling into view.
The separate adapter checks resize the client only after these natural-size
captures; those later frames do not prove first-run sizing.

Five startup unit tests and seven live Winit cases pass. The saved-seed case
previously assumed 1× scale and timed out at a reported 1.25×. It now compares
the requested seed at the actual scale and still detects an unwanted monitor
fit. This test uses a synthetic saved-resolution marker, not real preference
restoration. Fresh larger cases also report fractional and 2.5× scale. Strict
native Clippy and formatting pass. The Rust edit is confined to the native
ignored test; production launcher, native rendering and WASM code are unchanged.

The rebuilt runtime image independently passes six framebuffer/output probes
with networking disabled and no game or asset mount. Its Dockerfile and image
identity, failed attempts, terminal process results and visual review remain
local in `.scratch/launcher-monitor-root-20261019/`. Existing comparison images
and receipts are unchanged. Their exact-size RGB differences remain diagnostic;
they do not certify equal monitor-mode inputs or deterministic simulation.

The calibration review retained a separate keyboard-heading clipping defect in
`640x800/native-1/keyboard-return.png`. That subsequent correction is documented
in [launcher keyboard focus](launcher-keyboard-focus.md); these before receipts
remain unchanged.
Decorated desktop work areas, Windows, original/WASM runtime parity and the
original-baseline deterministic gate remain open. No public site was deployed.
