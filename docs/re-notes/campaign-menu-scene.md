# Campaign menu scene and outgoing panels

The menu-switching review uses a fresh, isolated 1280×960 TFT run of the local
1.29.2.9231 client (image SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`).
Local-only inputs, screenshots and monotonic capture times are retained under
`.scratch/campaign-menu-scene/`; no original data is published with this note.

## Observed behavior

After creating and selecting a diagnostic profile, entering Campaign retires the
Single Player panels and fades the whole scene through black. The stable fresh
TFT selector has a Night Elf scene, the expansion wordmark at the top right,
campaign rows on the right, difficulty at the bottom left and Back at the bottom
right. Back also fades through black before the ordinary glue scene and incoming
Single Player panels appear. An early capture made while the profile modal was
still open did not enter Campaign and is retained separately; it is not
transition evidence.

## Source binding and current implementation

The campaign index retains each section's `Background` and complete
`BackgroundFogStyle/Color/Density/Start/End` values. The background key resolves
through the selected edition's skin binding; its model supplies geometry,
textures, camera and animation data. Fog comes from that campaign section rather
than the main-menu fog table. Partial, duplicate or invalid fog values report an
error instead of borrowing another section's settings.

Single Player → Campaign now plays the source `SinglePlayer Death` clips before
retiring the panel scene. Campaign selection loads its own backdrop and omits
the hanging panel housings. Returning rebuilds the ordinary glue scene with
`SinglePlayer Birth`. The logo uses the current FDF's top-left or top-right
anchor, preserving the model-derived quad and existing projection convention.
The observed TFT expansion wordmark is retained; the generic skin `CampaignLogo`
model from the selected base archive contains older artwork and is not silently
substituted for it.

Scene return is admitted through the navigation request before rebuilding the
incoming panels. Their initial textures, opacity and posed geometry use the
incoming clip immediately, including a zero-length layer's Stand mapping.
Control visibility synchronizes after those scene commands are applied, and
Birth keeps the existing transition input lock until it settles.

These are recreation choices using shared rendering and navigation systems,
not a copy of the native screen-manager architecture. The initial background
uses the first indexed campaign when no campaign is selected; the fresh TFT run
qualifies that case, not every persisted-profile selection rule.

The first rendered attempt exposed a particle-validation failure rather than a
missing background file. One emitter references a zero-duration global sequence
from a single-key rotation track. The particle sampler now admits that constant
case without modulo arithmetic or an invented duration; empty or multi-key
zero-duration tracks remain unsupported. A synthetic nonstock quaternion test
checks both clock extremes and rejects the multi-key case. The failed black
background capture and its diagnostic log are preserved separately.

The next run exposed animated billboard parents with reflected or sheared
matrices. Extracting a proper rotation from those matrices had refused later
poses. The renderer now tracks the authored quaternion hierarchy separately and
cancels its parent rotation while retaining the full affine matrix's scale,
shear and pivot. Synthetic tests cover mirrored and collapsed scale, shear,
nested billboards, local spin, unchanged ordinary poses and invalid quaternions.
This is a rendering implementation choice; it does not claim native internal
matrix-layout parity.

The campaign camera also carries eye/target animation keys, including signed
negative preroll frames outside the selected clip. The existing checked camera
sampler now accepts that local preroll and filters it by the selected interval;
negative global-clock keys still refuse. The background renderer samples its
camera at the clip's start and then updates the owned camera, roll and billboard
facing on the same frame/elapsed clocks as the scene. A production-system fixture
checks eye, target, roll, billboard direction and preservation of another camera.

## Full-screen transition follow-up

The retained original entry frames show the housings leaving before the whole
picture darkens. Back darkens the campaign picture before revealing the glue
scene. The Campaign FDF's full-frame `SlidingDoors` sprite names the fade model;
its mesh, texture, material and `Birth`/`Death` opacity tracks now drive a separate
overlay above the menu UI. No fade duration or opacity ramp is copied into code.

Single Player entry waits for the longest outgoing panel clip before starting
the cover. Navigation changes at the opaque endpoint. That endpoint holds while
the destination FDF and background are admitted, and loading time does not skip
the reveal. Returning panels keep their initial pose until the overlay retires.
The shared transition input lock also covers dynamic campaign selection and
mission activation. Source replacement cancels the overlay; a reported backdrop
failure is revealed as an unavailable state rather than trapping the menu at
black. The loading handoff and incoming-pose hold are recreation scheduling
choices, not claims of native screen-manager internals or frame-exact timing.

The follow-up passes 704 application tests, three explicit original-input gates
(fade import/spawn, incoming panel poses and destination controls), strict native
all-target and WASM library/binary Clippy, the desktop build and formatting.
Receipts are under `.scratch/campaign-menu-scene/checks-fade-first/`. A new
isolated 1280×960 TFT run records 32 timed frames for each of Single Player,
Campaign, Back and return to Main under `native-fade-final/`. The entry and Back
sequences show darkening, the hidden owner replacement and a completed reveal;
Main remains reachable afterward. There are no rendering errors in that run.
The original and recreated capture clocks are retained, not treated as a
frame-exact timing or performance gate. Earlier unfocused setup/intro captures
are separate from these four navigation sequences. The owned runtime is stopped.

## Remaining differences

This slice does not complete the transition or screen rendering. The shared
background renderer currently starts the looping Stand
instead of playing the campaign model's Birth and then Stand. The campaign row
layout, unlock/profile rules, cursor, ambient sound and exact control fades also
remain distinct work. In particular, a successful scene import or parser test
does not establish matching pixels, sound or timing.

The rendered TFT check exposes further concrete limits. The Night Elf scene
draws and animates, but foliage has visible dark cards/fringes absent from the
reference. Its PRE2 batch reports `SortFarZNeedsParticleSort` and is not drawn.
The Bonus campaign background reports unsupported parented/animated point
lights and shows a background-unavailable notice. No substitute lighting or
particle behavior is invented. These are remaining renderer frontiers, not
successful visual-parity gates.

## Validation

The final source passes 700 application tests (55 library and 645 binary) and
69 graphics tests. Five explicit original-input gates pass: logo parsing,
edition-switch ownership, destination FDF admission, initial Single Player
panel poses, and all supplied campaign-camera clips. Strict native all-target
and WASM library/binary Clippy, the desktop build, formatting and diff checks
pass. The source freeze and binary fingerprint are retained in
`.scratch/campaign-menu-scene/checks-incoming-final/`.

A fresh 1280×960 TFT desktop run under isolated llvmpipe was visually reviewed
through Main → Single Player → Campaign → Single Player → Main. The target
background and logo anchor switch, source motion continues, and return shows
the Single Player panels without retaining the campaign logo. An earlier
candidate with the same background-cache key also verifies that activating the
already displayed first campaign opens its chapters without a background
rebuild; the observed load count stays unchanged. All owned runtimes are
stopped. Earlier failed or partial probes remain separately labelled.

These captures diagnose composition and transition ownership. They are not a
pixel-parity or live-performance pass, and the browser campaign path was not
rendered. The original reference uses isolated Wine/OpenGL; the recreated
desktop uses software Vulkan. No elapsed-time/FPS threshold is used as a
performance gate, and no original asset content is committed or uploaded.
