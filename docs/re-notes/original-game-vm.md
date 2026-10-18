# Local original-game reference harness

The current harness uses Docker, Wine, a private Xvfb display and software Mesa.
It copies the supplied game into an ignored private run directory. The runtime
image contains no game files; original files, profile writes, screenshots and
audio recordings stay local. This is a Wine reference, not native Windows
ground truth.

The user also supplied Windows installation media. Image metadata was inspected,
but no Windows VM disk or guest installation was created before selecting the
container alternative. A native Windows VM remains a separate future cross-check.

## Initial observations (2026-10-16)

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
confirmation controls without a settings body. The recreation then entered
Gameplay settings immediately. The later source-derived landing correction
removes that initial navigation difference; current observations follow below.
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

## Early capture acceptance and limits

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

## Paired input replay (2026-10-16)

The comparison command now sends one bounded, versioned physical-input journal
to both the private original window and a fresh local browser canvas. It checks
the original client extent/DPI/focus and disables X keyboard auto-repeat for
explicit down/up replay. The browser uses actual DPR1, identical physical pixels,
physical key codes and a hash-checked code-only loopback page. Original imports
remain local File/Blob data; service workers and WebSockets are blocked.

A live 1024×768 diagnostic replays one pointer move and six key transitions,
with four paired checkpoints: Main, Options entry, Video and return to Main.
All eight images were visually reviewed. Both sides receive the same event
objects; the receipt retains actual operation times and requested delays after
both preceding operations finish. There were 15 admitted code GETs and no
browser errors, uploads or sockets. The CLI returns the documented difference
outcome 1 with JSON data on stdout and empty stderr. Images differ; this is not
a parity pass. In particular, the original Options landing has no selected
category body while the recreation immediately displays Gameplay settings.

Two harness defects were found through actual checks and corrected. Viewport
coordinates passed directly to CDP capture produced a correctly sized but wrong
crop containing demo HTML; the retained 4K attempt is rejected. A synthetic
red-header/green-canvas browser test verifies the document offset and exact
canvas coverage. Another early diagnostic included capture cost in absolute
deadlines, compressing later input waits; the current scheduler waits relative
to completion of both preceding operations. Those early artifacts stay local
and are not accepted as timing evidence. The real browser adapter test also
checks explicit pointer events and shifted physical letter/digit keys.

Randomness and animation phase are still uncontrolled. The synthetic simulation
RNG, JASS native RNG and menu-particle stream are separate; current gameplay
Hosts do not implement the supplied random natives. No original launcher seed
hook has been proved. A non-null script seed therefore refuses before input
effects; a recorded number never becomes an applied-seed claim. The report
always retains `deterministic_parity_verified: false`. A private, intentionally
modified test-map probe has produced matching seeded observations in two fresh
original runs; see `original-rng-control.md`. This is not acceptance of
unmodified campaign behavior. Recreation RNG and paired seed integration remain
open.

The extended diagnostic completed four paired checkpoints at 1280×960,
1920×1080 and 2560×1440. A 3840×2160 browser screenshot exceeded the adapter's
30-second operation bound; that run is retained as failed, not a 4K paired pass.
Original-only 4K reference captures remain independently valid. Earlier browser
receipts sampled menu state before the screenshot; slow software rendering can
advance the screen while capture waits. Current receipts retain before/after
snapshots and flag changes instead of treating a pre-image snapshot as proof of
the rendered state. Animation/game tick synchronization remains open.

## Three-application resolution diagnostic (2026-10-18)

The fresh diagnostic runs original WC3/Wine, the actual PC3 native application
and the actual PC3 browser WASM application separately at 1024×768, 1280×960,
1920×1080, 2560×1440 and 3840×2160. Each receives the same eleven-event journal,
including four capture checkpoints: Main, Options entry, Video and return.
The sixty retained PNGs have the requested physical dimensions; none is resized
or normalized for comparison. All three pairwise exact RGB comparisons are
recorded at every checkpoint. Pixel fidelity fails. No version-string mask is
applied because its exact glyph footprint remains unqualified.

The application builds share tracked source `6a3c2fc`. Their native executable,
WASM package and local imports have independent fingerprints. The original is
the retained 1.29.2.9231 Wine reference. The recreation uses a locally selected
152-member menu closure; that does not establish retail archive priority or
identical source editions. Missing client-derived settings remain explicitly
unavailable, and four packed popup sound codecs remain unsupported.

Original and native Main, Options landing and Video render at all five sizes.
At the 4K WASM Video checkpoint, Options landing is still visible: the recorded
Video key arrives while the transition is active, and the category does not open.
That is a failed navigation comparison, not a successful Video-screen capture.
The widescreen native
and original captures both retain black side regions around menu content.
Panel bodies are visibly paler in PC3; fog, lighting and effects also differ.
WASM and native additionally differ in notices and captured transition state.
Some WASM return checkpoints occur while panels are moving, so this is not a
steady-frame or load-independent timing test. Cached before/after browser
snapshots do not acknowledge the simulation tick that produced a PNG. RNG and
animation phase remain uncontrolled.

The capture adapter now uses the visible canvas's checked document clip directly
through CDP, avoiding Playwright's DOM font/style preparation. The CDP session is
bound to the already measured viewport, screen and DPR, then those dimensions
are checked again after capture. Capture has a finite 90-second bound, input and
geometry 30 seconds, and capture protocol waiting 150 seconds. Host Open menu
preparation has a separate 120-second click bound and startup protocol waiting
300 seconds. These are observation limits, not performance acceptance gates.
Both X11 adapters verify actual pointer position after a move: an XRandR monitor
can clamp input even when the root framebuffer and captured client are larger.

Earlier failed attempts remain retained. Two native launcher-only attempts were
rejected through visual review and replaced by existing keyboard navigation,
with Main visually checked before replay. Original 4K controller interruption
was recovered in the same original process from a reviewed Main state. The first
current 4K WASM attempt stopped at host-page preparation; a fresh bounded run
uses the corrected preparation limit. No failed adapter is reused.

Evidence and the local three-column report are in
`.scratch/menu-resolution-transparency-root-20261018/`; the final verification is
`verification-final-1.json` and report `comparison-1/index.html`. All original
files and generated captures remain private. Forty-six Python harness checks and
four Node checks, including real Chromium crop/input validation, pass. This
harness change does not rebuild the public site or close native Windows, full
visual fidelity, simulation lockstep or the original baseline gate.

The transparency investigation separately follows production opacity decoding
on the selected panel models and texture alpha planes. No measured alpha value
was copied into production. Linear versus encoded-space blending is a plausible
rendering difference, but the effective original state is not yet established;
adding Bevy's Camera2d-only compositing component to these Camera3d materials
would fall back to linear and is not a fix. Transparency correction remains open.


## Follow-up 4K panel-clock correction

The five-resolution matrix above is retained as the baseline. A subsequent
`c90730a` native/WASM source build corrects panel time lost to Bevy's virtual
clock cap and advances existing transitions before new navigation requests.
Fresh 3840×2160 native and browser runs now show all four requested screens,
including the Video body that the baseline WASM run missed. Each browser
after-capture state is settled. These eight new raw captures are compared with
the earlier original 4K captures; no new original session was launched.

The first native attempt stopped before replay because a bounded pointer query
was missing from its command filter. That harness omission is corrected and
covered through the actual filter; the successful rerun delivers the full
journal. The failed attempt remains retained. Forty-seven Python harness checks,
four Node checks, 645 application tests and strict native/WASM Clippy pass.

See `.scratch/menu-panel-clock-root-20261018/verification-final-1.json` and
`comparison-1/index.html`. Navigation acceptance passes at this tested size;
exact RGB equality still fails. Transparency, fog, background/particle timing,
unsupported settings and sound codecs, simulation/animation lockstep and full
original fidelity remain open. No pixels were excluded or rescaled, and all
original data and captures remain private.
