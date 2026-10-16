# Local original-game reference harness

The current harness uses Docker, Wine, a private Xvfb display and software Mesa.
It copies the supplied game into an ignored private run directory. The runtime
image contains no game files; original files, profile writes, screenshots and
audio recordings stay local. This is a Wine reference, not native Windows
ground truth.

The user also supplied Windows installation media. Image metadata was inspected,
but no Windows VM disk or guest installation was created before selecting the
container alternative. A native Windows VM remains a separate future cross-check.

## Actual observations

The selected executable reports version 1.29.2.9231 and has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The private copy contains 564 source files; independent root verification of
every retained manifest size and hash passes after the game run. The current
host installation was not modified.

The successful menu run uses Debian Wine 10.0, wined3d and software llvmpipe,
without a game renderer override. Its caller-selected experiment is 1280 by
960 at 96 DPI. A Win32 probe records the actual display mode, locale and 39
virtual display modes. These are properties of this Wine/Xvfb experiment, not
a universal original-game resolution list or physical monitor capability.

Retained captures show a clean TFT Main menu and Video Options. Two untouched
Options landing captures, separated by about a minute, show the selector and
confirmation controls without a settings body. The recreation currently enters
Gameplay settings immediately; that navigation difference requires correction.
The images and action timestamps establish these observations; parser tests do
not establish them.

The first attempts exposed missing 32-bit GL libraries and a sound-initialization
dialog. Later runs add the required distro libraries and an isolated PulseAudio
null sink. Retained monitor recordings contain nonzero samples, but this does
not prove audible output, native mixing, or sound fidelity. The movie-capable
attempt rendered black video and exited after Escape; the successful menu
session explicitly disables Wine's unsupported DirectShow movie decoder.
Startup movies and the complete split-gate sequence remain unverified.

## Commands and isolation

See [the harness instructions](../../tools/original-game-reference/README.md)
for prepare, launch, status, capture, key, click, stop and comparison commands.
The runtime is non-root, has no network or host display/audio/GPU mounts, drops
capabilities and uses an owned private directory and container label. A matching
running experiment can be reused; changed display/DPI/arguments and prior failed
launches refuse reuse. Stop preserves evidence. No command removes the host
installation or existing virtual machines.

The CLI has offline schema output and explicit JSON data/errors. Its v0.3
candidate declaration is checked against pinned published schema bytes. Process
and synthetic tests check parsing, stream separation, copying, ownership,
calibration-preserving retries and report extent refusal. These checks do not
constitute a full CLI Spec conformance claim or a rendered-game test.

## Capture acceptance and current limits

Comparison reports require equal pixel extents and preserve the input images.
They do not resize either side. The first current-code recreation screenshot
was 1280 by 961 despite its 1280 by 960 canvas bitmap: the browser rounded a
fractional element origin outward. The report correctly refused that pair.
The private capture driver now checks integer device-pixel alignment, an exact
measured clip, actual DPR and the resulting PNG dimensions. Its offline geometry
checks pass; the corrected browser capture still needs a runtime rerun.

The selected recreation fixture also has explicit archive/variant choices and
unsynchronized animation clocks. Equal extents alone cannot prove matching
sources, edition, settings, timing or appearance.

Runtime files are under `.scratch/original-game-vm/`; root verification receipts
are under `.scratch/original-game-reference-root-20261016/`. Earlier failures are
preserved. An intermediate workspace permission profile blocked Docker and
loopback rechecks. After access was restored, the existing owned container was
revalidated and controlled successfully; no duplicate was launched during the
restriction. The retained Video → Cancel → Main → Options action sequence again
shows an unselected Options landing screen.

## Multiple-resolution reference runs

Each requested framebuffer is a separate private installation/profile run at
96 DPI. The experiment matrix covers 1024×768, 1280×960, 1920×1080, 2560×1440 and
3840×2160. These are caller-selected comparison conditions, not game defaults or
an original preset table. Runs are sequential to avoid competing software-render
loads. Existing runs and captures are preserved when stopped.

The fresh 3840×2160 run has a matching PNG, X display and Warcraft client window;
its Video Options control also reads 3840×2160. The 1024×768 run independently
renders Main, unselected Options landing and Video Options with the matching
resolution. All five sizes now have reviewed Main, landing and Video captures,
matching PNG dimensions, X DPI and Warcraft client-window extents. The Video
control reads the selected resolution in every run. In the 4K run the observed
menu has side bars around its 4:3 content;
these remain part of the captured output. No authored layout or game setting is
changed to fit a screenshot.

Actual captures, display/window receipts, action timestamps and command logs stay
in ignored local directories. Equal-size screenshots are necessary for comparison
but do not synchronize animation clocks or establish complete visual fidelity.
