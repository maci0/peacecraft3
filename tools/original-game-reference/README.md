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

Every command is noninteractive and returns JSON by default. `--format json|text`
precedes the command; `report --output` names its destination. Errors are JSON on
stderr with exit 2; success data uses stdout with exit 0. Each command returns one
bounded document. Metadata is capped at 2 MiB. Prepare/capture/report refuse existing
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
  -s tools/original-game-reference -p test_reference.py -v
```

See [runtime evidence](../../docs/re-notes/original-game-vm.md) for the actual local
source, media discovery, observations and outstanding fidelity questions.
