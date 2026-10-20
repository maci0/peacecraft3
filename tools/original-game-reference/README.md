# Local original-game reference

This runs the supplied original executable in a private Docker/Wine/Xvfb environment.
It is a Wine runtime reference, not native Windows ground truth. The image contains
only Debian/runtime packages. Game files, license files, profile writes, logs and
screenshots stay in a local ignored run directory. No original files are uploaded.

Prerequisites: Docker, Python 3, and Pillow for `report`. Tests additionally use
`jsonschema`. Build package downloads use ordinary network access; game containers
have no network, host display/audio socket, host GPU, or host Wine prefix.

```sh
docker build -t peacecraft-original-reference:local tools/original-game-reference
python tools/original-game-reference/reference.py schema
python tools/original-game-reference/reference.py prepare \
  --source '/path/to/Warcraft III (Legacy)' --run-dir .scratch/original-reference-1
python tools/original-game-reference/reference.py launch \
  --run-dir .scratch/original-reference-1 --width 1280 --height 960 --dpi 96
python tools/original-game-reference/reference.py status --run-dir .scratch/original-reference-1
python tools/original-game-reference/reference.py capture --run-dir .scratch/original-reference-1 --label startup
python tools/original-game-reference/reference.py key --run-dir .scratch/original-reference-1 --key Escape
python tools/original-game-reference/reference.py click --run-dir .scratch/original-reference-1 --x 300 --y 400
python tools/original-game-reference/reference.py stop --run-dir .scratch/original-reference-1
python tools/original-game-reference/reference.py export-replay --run-dir .scratch/original-reference-1 --label match-1
```

`export-replay` copies the Wine prefix's `Warcraft III/Replay/LastReplay.w3g` into
`replays/<label>.w3g` with a JSON receipt. It refuses a running container, so run
it after `stop`. It copies bytes only and does not parse the file.

Dimensions and DPI are caller-selected experiment conditions, not game defaults.
Use a separate `prepare` directory for each resolution, and stop one experiment
before launching the next. For example, the comparison matrix uses 1024×768,
1280×960, 1920×1080, 2560×1440 and 3840×2160, each at 96 DPI. These are test
conditions, not a claimed list of original presets. Pass each width and height
to `launch`; never reuse a running directory with different dimensions. Verify
both the capture's PNG dimensions and the Warcraft client window in its receipt.
Check the displayed Video Options resolution as well. Compare each original
capture to a recreation capture of exactly the same extent and actual DPR 1.
Do not resize a 4K capture to stand in for a lower-resolution run.

Inspect each capture before supplying click coordinates. Capture receipts retain
actual X display geometry/window tree, game video registry and executable hash;
`renderer.txt`, `display.txt`, `wine-version.txt`, `launch.json`, `source.json` and
`actions.ndjson` retain runtime/source/settings/action provenance. `ready` means
Wine initialized; status does not certify a menu was rendered. Launcher exit status
is separate from the container status. Software timing is not an original hardware
performance measurement. The image provides a private PulseAudio null sink;
PNG capture does not record audio. DirectShow movie playback is explicitly
disabled because the tested Wine movie path did not render reliably. This is a
menu reference, not verification of startup movies or the complete opening sequence.

Use `--game-arg=-opengl` for an explicitly selected game renderer experiment, for
example; switches are passed as separate argv entries. No host registry is imported.
Existing local game license files are copied as part of the installation, never
printed. Missing game/license inputs must be supplied normally; this harness does
not acquire or bypass them. Private runs should not be shared.

```sh
python tools/original-game-reference/reference.py report \
  --reference .scratch/original-reference-1/captures/main.png \
  --recreation /path/to/local-matching-canvas.png \
  --output .scratch/comparison-1 --note 'Record source, edition, scene and clock differences here'
```

The report refuses differing pixel extents rather than rescaling. It copies both
images and hashes them into a standalone local HTML folder. A same-size pair alone
is not evidence of matching source, edition, camera, settings or animation phase.

## Paired input replay

`compare` sends one versioned input journal to the original window and the
recreation canvas, then builds side-by-side reports at named checkpoints. It
does not find different buttons independently on each side: pointer coordinates
are the same physical framebuffer pixels, and keys are the same physical ASCII
keys or explicit modifiers. This makes differences visible instead of steering
around them. The exact input schema is included in offline `schema` output under
`extensions.comparison_scenario`.

The current startup adapter handles Main-menu comparisons; map/gameplay startup
and replay loading remain to be connected. Prepare and launch a fresh reference
run, then inspect its Main-menu capture.
`compare` requires the Xvfb `-r` setting used by the current session script, so
OS auto-repeat cannot insert extra keys. Stopped or older experiments remain
preserved. It requires exactly one default X screen and a full-frame client on
that screen, with exact dimensions and DPI; another screen cannot supply the
calibration. Supply a local, explicitly prepared ISO source closure containing one
import candidate, an asset-free loopback demo page and its code build receipt:

```sh
python tools/original-game-reference/reference.py compare \
  --run-dir .scratch/original-reference-1 \
  --scenario /path/to/local-inputs.json \
  --url http://127.0.0.1:8957/index.html \
  --manifest /path/to/code-build-receipt.json \
  --iso /path/to/local-source-closure.iso \
  --playwright-module /path/to/playwright-core/index.mjs \
  --output .scratch/paired-comparison-1
```

The source closure stays a local File/Blob import. Browser requests are restricted
to hash-checked code entries in the supplied receipt; uploads, redirects outside
that receipt, service workers and WebSockets are blocked. The browser uses a fresh
profile. The original private profile and visible window must already be at the
declared starting state; the first checkpoint retains that state for visual review.
No screenshot or title alone verifies every hidden game setting or archive choice.

A scenario contains `version: 1`, `edition`, `extent: [width,height]`, `dpi: 96`,
the exact observed original `window_title`, `seed`, and an ordered `events` array.
Each event has `after_ms`, the wait after both preceding operations finish:

```json
{"kind":"key","phase":"down","key":"Tab","after_ms":500}
{"kind":"key","phase":"up","key":"Tab","after_ms":150}
{"kind":"pointer","phase":"move","x":17,"y":23,"after_ms":0}
{"kind":"pointer","phase":"down","x":17,"y":23,"button":"left","after_ms":0}
{"kind":"pointer","phase":"up","x":17,"y":23,"button":"left","after_ms":150}
{"kind":"checkpoint","name":"after-input","after_ms":2000}
```

These are independent example event objects, to place inside `events`, not game
coordinates or hotkey defaults. Use explicit moves before button presses; down/up
must balance, and checkpoints cannot extend held inputs. ASCII keys are named in
lowercase plus explicit `Shift`, `Control` or `Alt`, mapped to physical browser
key codes and original X keysyms. Other keyboard layouts and unsupported keys are
not inferred. The host page is placed at the framebuffer origin; the game UI is
not moved or fitted. Capture validation checks actual DPR1, CSS/bitmap size,
viewport and document offsets, and resulting PNG dimensions. An exact-size image
with the wrong page crop is not a valid canvas capture.

The browser captures the visible canvas through CDP without preparing DOM fonts
or changing page styles. Before capture, the CDP session receives the measured,
unchanged viewport, screen and DPR: a fresh session otherwise resets screen bounds
to Chromium defaults, changing subsequent source-derived mode choices. The PNG
crop uses document coordinates, with capture beyond the viewport disabled. Actual
screen and canvas geometry are checked again afterward. The real-browser adapter
test checks unchanged screen dimensions, exact scrolled crop pixels and input
delivery. Geometry and input operations remain bounded to 30 seconds. Capture
has a separate 90-second bound for software-rendered 4K readback; the protocol
wait is bounded to 150 seconds including the preceding geometry check. These
are observation limits, not performance pass/fail thresholds. Timed-out requests
still poison the adapter, stop the replay and retain the failure.
The host page's Open menu preparation has a 120-second click bound, separate
from replay input. Startup protocol waiting is bounded to 300 seconds to include
import, preparation and the existing Main-menu readiness check. Failed startup
is retained and closed; a new attempt requires a fresh output directory.

The journal records both targets' actual operation times and scheduling lateness.
Browser capture receipts retain snapshots before and after the screenshot and
flag changes during capture. Slow rendering can advance the UI while a screenshot
is pending; neither snapshot independently attests the image's simulation tick.
Screenshot work does not consume the next requested input hold/wait. This is
barrier-relative wall-clock replay, **not simulation lockstep**. Captures retain
the two real renderers' outputs; clocks and animation phases are not synchronized.
Acceptance requires identical rendered output across Original WC3, PC3 native
and PC3 WASM, except for the bottom-right version string. This is exact decoded
RGB equality: zero channel tolerance, no color normalization or image rescaling.
Only a verified version-text footprint may be excluded; a bottom-right rectangle
or a whole text frame would also hide unrelated background pixels. That footprint
is not qualified yet, so **no pixels are currently excluded**, including the
footer. Raw captures and differences remain intact. This stricter interim check
cannot certify the requested version-text exception or full visual parity.

The report compares decoded RGB pixels, gives changed pixel counts, bounds and channel errors,
and preserves the unchanged source PNGs. Checkpoints retain the browser's change
flag: `frame_time` means state changed during capture and the image is unqualified
for stable comparison; otherwise `match` and `pixel_mismatch` describe the pixels.
These labels do not establish which simulation tick was rendered.
Exit 0 means the observed checkpoint
images match, exit 1 means they differ, and exit 2 means input/runtime refusal.
None of these outcomes establishes complete game parity.

**Seed control in visual replay is not implemented yet.** The serial RNG fixture
below controls the selected JASS stream separately. `seed: null` explicitly requests an
uncontrolled visual diagnostic. A signed script seed is recognized but refuses
before replay rather than claiming an arbitrary launcher argument was applied.
The synthetic simulation RNG, JASS RNG and menu-particle RNG are distinct; no
verified original seed hook currently joins them. Deterministic comparison needs
both setters observed, matching map/settings, controlled draw ordering and clock
checkpoints, and repeated matching outputs. The receipt always keeps
`deterministic_parity_verified: false` until those controls are real.

## Original WC3 | PC3 native | PC3 WASM

Add `--native-config /path/to/local-native-config.json` to `compare` to replay the
same journal against all three actual applications. The native PC3 process must
already be running on its dedicated local X11 display at the scenario's exact
framebuffer extent and 96 DPI. Its executable, process start identity, window
owner, build receipt and content manifest are verified. The adapter never launches
or terminates that caller-owned process. It refuses changed identities, wrong
geometry and focus failures; it does not resize a target to fit a comparison.
Launch that dedicated display with `Xvfb :N -screen 0 WIDTHxHEIGHTx24 -dpi 96 -r`
and local authentication before starting PC3. The adapter checks the server's
`-r` startup policy to prevent held keys from repeating with host timing.

Before starting either native target, also inspect `xrandr --current`. Its single
active output must have the scenario's exact extent at `+0+0`, matching
`xdpyinfo`. Some Xvfb versions leave an active output with different bounds even
when `-screen` creates the requested framebuffer. Winit reads that output for
monitor sizing and available modes. A matching PNG alone does not qualify these
inputs. Both native adapters now refuse this mismatch before focus or input;
successful calibration records the output name, extent and origin. The PC3
command filter only permits the read-only `xrandr --current` query. Preparing
the display remains the caller's responsibility; the adapters do not change
modes or resize images. Rebuild the reference image to include the declared
`x11-xserver-utils` dependency. See
[the calibration evidence](../../docs/re-notes/launcher-monitor-calibration.md).

The native configuration is versioned and available offline through `schema`:

```json
{"version":1,"target":"pc3-native","pid":1234,"start_ticks":5678,
 "display":":177","window_title":"peacecraft3 launcher",
 "executable":{"path":"/absolute/path/peacecraft3","sha256":"<file SHA-256>"},
 "build_receipt":{"path":"/absolute/path/build.json","sha256":"<file SHA-256>"},
 "content_manifest":{"path":"/absolute/path/content.json","sha256":"<file SHA-256>"}}
```

Replace every example value with the actual process identity and fingerprints;
`start_ticks` is Linux `/proc/PID/stat` field 22. Build and content documents use
`{"version":1,"kind":"build-receipt","files":[{"path":"/absolute/file","sha256":"<file SHA-256>"}]}`
and the same shape with `kind: "content-manifest"`, respectively. Every listed
file is hashed, with at most 256 files and 16 GiB per document. The build list must
include the actual executable. The build receipt also records `source_head`
(40 lowercase hex characters) and `source_tree_sha256` (64 lowercase hex
characters); both must match the WASM code manifest. Record the exact tracked
source tree used by both builds, and keep compiler commands/results with it.
Keep the content document explicit about the selected archives and provenance;
the adapter verifies fingerprints, not the meaning or completeness of a closure.

The same event is dispatched to all three targets, and the next event's delay
starts after all three operations finish. Every checkpoint preserves raw PNGs in
this order: **original WC3 / Wine | PC3 native app | PC3 WASM app**. The receipt
contains WC3/native, WC3/WASM and native/WASM pixel differences. Any differing
pair makes `images_equal` false. Equal dimensions are required; there is no image
rescaling. Input failures release potentially held input and retain the failure
receipt. Existing outputs are never reused.

Both X11 targets verify the actual pointer position after every move. A large
framebuffer can still have a smaller XRandR monitor area that clamps the pointer;
that refuses instead of recording the requested coordinates as delivered. Verify
the actual menu after launcher preparation before starting the shared journal.

Each code-manifest file must declare its SHA-256 and a byte size no larger than
72 MiB. Optimize an oversized WASM build before replay. Responses are base64
encoded through Chromium's DevTools pipe; the raw file limit leaves room under
its observed 100 MiB message capacity. Oversized files refuse before browser
launch instead of disconnecting the browser during transfer.

Native captures come from the actual application window, not a Rust VM test.
The browser captures come from the actual browser WASM application. Both build
and local content identities must be retained separately from the original game's
executable/archive identity. A source-selected local closure does not establish
the original game's archive priority or release identity.

Browser capture samples canvas geometry and menu state together in one renderer
evaluation before the screenshot and one afterwards. This avoids extra renderer
round trips under load while retaining the geometry and state-change checks and
the existing operation deadline. These observations still do not synchronize
simulation ticks or animation clocks.

A static three-column report can also be generated from existing captures:

```sh
python tools/original-game-reference/reference.py report \
  --reference /path/to/wc3.png --native /path/to/pc3-native.png \
  --recreation /path/to/pc3-wasm.png --output .scratch/three-way-report-1 \
  --note "Observed application captures; clocks and RNG are uncontrolled"
```

A static report only labels caller-supplied PNGs; its execution provenance is in
the replay receipt. The native application's frame state is currently unobserved,
and the browser's before/after state does not control animation phase. Three-way
visual replay still refuses a non-null seed. It is a visual diagnostic until
application seed acknowledgments, shared logical ticks, all consumers and repeated
state/pose checkpoints are available. The serial RNG and timer DST gates below
remain separate checks; they do not certify three-application lockstep.

## Repeatable original RNG fixture

`rng-plan` generates one self-authored JASS fixture from explicit signed-i32 seed
calls and an ordered list of integer/real calls. It never chooses a game seed or
substitutes the synthetic simulation RNG. The same `probe.j` runs in the original
and in the selected-policy recreation VM. Results are stored before export;
real results are normalized into their binary32 bits rather than rounded text.
The finite real fixture domain uses ordered bounds drawn from `-1`, `0`, `1`;
unsupported classes refuse. Include at least one nondegenerate integer and real
call. Up to 256 steps, including reseeding and degenerate controls, are accepted.

```json
{"version":1,"nonce":"local-check","steps":[
  {"op":"seed","value":17},
  {"op":"integer","low":-9,"high":13},
  {"op":"real","low":-1,"high":1}
]}
```

These numbers are synthetic test inputs, not original defaults.

```sh
python tools/original-game-reference/reference.py rng-plan \
  --scenario /path/to/local-scenario.json --output .scratch/rng-fixture-1
```

The bundle contains `probe.j`, its globals/functions separately for reversible
local map instrumentation, and a fingerprinted manifest. Call `PCXRngFixture`
once in a serial callback after the original map has initialized its native
environment. Preserve the map's original globals/config/main, verify the exact
insertion and all retained members, and use fresh private runs. The harness does
not automatically rewrite or launch a map. Keep original maps, modified copies
and exported results local. This fixture is an experiment, not unchanged-map
acceptance or a seed for all engine consumers.

The explicit recreation gate uses supplied common declarations, the reviewed
RNG/numeric bindings and that identical generated script. Set
`PEACECRAFT3_RANDOM_REAL_IMAGE`, `PEACECRAFT3_RANDOM_BINDING`,
`PEACECRAFT3_NUMERIC_BINDING`, `PEACECRAFT3_RANDOM_COMMON`,
`PEACECRAFT3_RNG_PROBE` and `PEACECRAFT3_RNG_OUTPUT`, then run:

```sh
cargo test -p pcx-play --test rng_fixture --locked --offline --jobs 1 \
  -- --ignored --nocapture
```

The output directory must exist and contain neither `run-0.txt` nor `run-1.txt`.
Two fresh owned VMs must emit byte-identical observations. Only fixture export
and integer-to-decimal reporting are adapted; RNG calls and numeric operations
use the production VM/host and selected image policies. This does not admit a
production `I2S` implementation. The same gate also runs on actual WASI with
explicit preopened local input/output directories.

Retain a version-1 JSON receipt for each independent run with `target` (`original`
or `recreation`), unique `run_id`, explicit `runtime`, and a `files` object. Its
five entries `image`, `common`, `probe`, `observation`, `execution` each contain
an explicit local `path` and its lowercase `sha256`. `common` identifies the
declarations used to qualify the fixture; its archive/version remains separate
from executable identity. An original execution JSON requires `complete: true`,
`records` and `observation_sha256`; a recreation execution JSON requires
`exit_code: 0`. Retain launch, staging, source and collection provenance with the
original execution. Fingerprints detect changed inputs; receipts alone are not
cryptographic proof that someone launched independent processes.

```sh
python tools/original-game-reference/reference.py rng-compare \
  --fixture .scratch/rng-fixture-1 \
  --original /path/to/original-run-1.json --original /path/to/original-run-2.json \
  --recreation /path/to/recreation-run-1.json --recreation /path/to/recreation-run-2.json \
  --output .scratch/rng-comparison-1
```

Each target needs 2..8 independent receipts and observation files. Selected
image, declarations and probe must agree. The comparison validates executed seed
acknowledgments, every call's order/arguments, complete exports, fingerprints and
successful collection/execution. It writes a local side-by-side HTML table with
real bits shown in hexadecimal and reports the earliest differing operation.
Exit 0 means all ordered results match, exit 1 means they differ, and exit 2
means refusal. Existing outputs refuse reuse; replaying game input is never an
automatic retry. Malformed, over-budget or duplicate-field inputs refuse with
JSON diagnostics on stderr and no stdout data.

A separate DST gate runs the selected RNG inside the production VM/timer queue:

```sh
cargo test -p pcx-play --test rng_fixture --locked --offline --jobs 1 \
  -- --ignored selected_rng_timer --nocapture
```

It needs the same image, RNG/numeric bindings and common declarations. Synthetic
periodic callbacks use explicit logical time. Fine polling, delayed catch-up and
one large catch-up must retain every callback in the same order. Different work
budgets and bounded injected CPU work must preserve exact callback times, integer
and real bits, script counters and the complete retained RNG state. The gate also
requires a changed seed to produce a different trace. No elapsed-time or FPS
threshold decides success. Native and actual WASI runs must emit the same trace
digest.
This verifies the admitted scheduler under those conditions, not the original
engine's timer tie policy or a complete gameplay clock/render integration.

This controls **one serial JASS RNG experiment**, without world tick advancement.
It does not synchronize engine startup, secondary consumers, world state,
animation or rendering. Visual `compare` still refuses non-null seeds, and
`deterministic_parity_verified` stays false. Matching this fixture must never be
reported as a complete deterministic campaign or pixel comparison.

Comparison outputs are restricted to local `.scratch` or `/tmp`. Existing output
directories refuse reuse. Interrupted or failed replays close their browser and
attempt to release potentially held original inputs; inspect the retained receipt
before preparing another run. Interrupts emit a structured error with exit 130.

Every command is noninteractive and returns JSON by default. `--format json|text`
precedes the command; `report --output` names its destination. Errors are JSON on
stderr with exit 2; completed data uses stdout with exit 0 or compare's difference
outcome 1. Each command returns one
bounded document. Subprocess/protocol metadata is capped at 2 MiB; replay retains
at most 256 events and 32 checkpoints. Prepare/capture/report refuse existing
destinations. Launch returns an already-running owned container only when display
size, DPI and game arguments match. It refuses a stopped container or a prior
failed-launch receipt: choose a new private run to preserve evidence. Stop is safe to
repeat and retains the container. Key/click are not safe to repeat blindly. All
container inspection/control validates its path-derived name and ownership label.
No cleanup command deletes containers, files, host installs or existing VMs.

Replay failures inside the browser worker report the failing stage and latest eight error,
socket, request and console observations per collection, with omitted counts.
Primary error messages/codes are capped at 4000/128 characters with explicit
truncation flags. These tails keep the caught failure response within the
metadata ceiling.
This diagnostic limit does not change input delivery, readiness or click timeouts.

The offline schema declares the v0.3 candidate, pinned to published schema bytes
whose SHA-256 is checked against the accompanying published CHECKSUMS.txt. The
candidate can change upstream. Schema validation and subprocess tests are distinct
from full CLI Spec conformance; full conformance is not claimed.

```sh
python -m venv .scratch/reference-test-env
.scratch/reference-test-env/bin/pip install jsonschema pillow
.scratch/reference-test-env/bin/python -m unittest discover \
  -s tools/original-game-reference -p 'test_*.py' -v
node --test tools/original-game-reference/test-browser-adapter.mjs
PCX_PLAYWRIGHT_MODULE=/path/to/playwright-core/index.mjs \
  node --test tools/original-game-reference/test-browser-adapter.mjs
```

The explicitly gated browser test uses only a synthetic colored canvas. It checks
scrolled-document crop, exact pointer delivery and shifted physical key behavior.
It is not a rendered-game or original-input-consumption test.

See [runtime evidence](../../docs/re-notes/original-game-vm.md) for the actual local
source, media discovery, observations and outstanding fidelity questions.
