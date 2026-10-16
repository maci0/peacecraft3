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
```

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

The journal records both targets' actual operation times and scheduling lateness.
Browser capture receipts retain snapshots before and after the screenshot and
flag changes during capture. Slow rendering can advance the UI while a screenshot
is pending; neither snapshot independently attests the image's simulation tick.
Screenshot work does not consume the next requested input hold/wait. This is
barrier-relative wall-clock replay, **not simulation lockstep**. Captures retain
the two real renderers' outputs; clocks and animation phases are not synchronized.
The report compares decoded RGB pixels, gives changed bounds and channel errors,
and preserves the unchanged source PNGs. Checkpoints retain the browser's change
flag: `frame_time` means state changed during capture and the image is unqualified
for stable comparison; otherwise `match` and `pixel_mismatch` describe the pixels.
These labels do not establish which simulation tick was rendered.
Exit 0 means the observed checkpoint
images match, exit 1 means they differ, and exit 2 means input/runtime refusal.
None of these outcomes establishes complete game parity.

**Seed control is not implemented yet.** `seed: null` explicitly requests an
uncontrolled visual diagnostic. A signed script seed is recognized but refuses
before replay rather than claiming an arbitrary launcher argument was applied.
The synthetic simulation RNG, JASS RNG and menu-particle RNG are distinct; no
verified original seed hook currently joins them. Deterministic comparison needs
both setters observed, matching map/settings, controlled draw ordering and clock
checkpoints, and repeated matching outputs. The receipt always keeps
`deterministic_parity_verified: false` until those controls are real.

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
