# Campaign escape menu

The source menu key and the HUD Menu button now open a modal over the retained
campaign scene. They no longer reveal the chapter-selection screen. The launcher
remains setup; the game main menu is the screen after the intro; this is the
separate in-game menu.

## Source and layout

The selected Legacy 1.29.2.9231 namespace supplies
`UI\FrameDef\UI\EscMenuMainPanel.fdf` (SHA-256
`867070f699d3612e14d60b8dcfafccf3fdc9811b07f60e4223e96f27d44f6754`)
and `EscMenuTemplates.fdf` (SHA-256
`89467e8c1905e0894e381195225c5b0d7c7258f900d7018582694ec8193a4ea7`).
The selected resolver, published receipts and active parsed context must agree
before loading. Race sections come from the canonical local player, followed by
the existing skin fallback. Source receipts remain attached to the menu assets.
No original files or generated copies are committed.

Panel dimensions, anchor chains, fonts, strings, justification offsets, button
states, pressed-label displacement, focus links and texture/sampler settings come
from those files and the selected skin/global-string tables. The shared glue and
script-dialog text renderers draw them. Intrinsic title extents come from shaped
text. The selected panel supplies the backdrop extent. Its placement follows the
selected executable's modal-root and backdrop anchor chain, as described below. Input and decoded payload limits precede
asset publication; a refused load registers no partial font set.

## State and actions

`CampaignCamera.menu_open` keeps scene rendering active while the existing runtime
excludes menu time from gameplay pacing. It does not overwrite an explicit script
or embedding pause. The VM, canonical units, pending callbacks and logical clock
remain the same. World orders, inventory use, camera/minimap input and cinematic
Escape delivery are blocked while the modal owns input. Escape backs out of the
current submenu before closing; it cannot also dismiss an underlying quest log.
Blank pointer presses retain an enabled control's focus within the modal, using
the same focus boundary as the frontend; Tab navigation remains available.

Return closes the modal. End Game and Previous switch source panels. Restart
requests the currently admitted map through the existing transition pipeline;
it retains the current session until replacement succeeds. Mission Quit requests
the existing no-score exit path, then uses normal session/camera/HUD retirement.
Confirmed program exit uses the application's exit event on native targets; in a
browser it clears the session and frontend layer to return to setup. Cancellation
emits no exit. Buttons carry the originating session witness and panel identity,
so stale widget actions cannot act on a replacement session or another panel.

Save, Load, Options, Help and Tips are still unavailable. Their source disabled
art/text are shown, and they do not report placeholder success. The separate
Pause control is omitted in this single-player modal, matching the isolated RoC
reference capture; its FDF frame remains available to resolve other controls'
anchors. Opening the menu already pauses the single-player clock. Multiplayer
visibility/pause behavior, quit/score choices and exact transition timing remain
unqualified. See [the live menu evidence](escape-help-tips.md#isolated-live-font-and-navigation-check-2026-11-05).

## Verification

Synthetic tests discriminate nonstock panel geometry, skin paths and shortcuts;
reject malformed justification offsets; retain an explicit pause while toggling
the menu; consume submenu Escape correctly; and reject stale panel/session input.
Explicit original-data gates load all three owned panels for RoC and TFT and
verify same-map restart requests and confirmed-exit events on an actual prepared
session. These are source/factory checks, not rendered parity.

The isolated offscreen application run reaches the campaign through the post-intro
main menu, captures the source main/end/confirmation panels and verifies clock
pause/resume, Return, HUD Menu reopening, exit cancellation, mission quit and a
later chapter launch. Its final strengthened run also exercises the actual Restart
Mission widget and verifies replacement of the session before quitting and
launching again. A subsequent focus-only regression reproduced loss of keyboard
focus after a blank press and passes with the shared focus boundary. The
observation and source hashes are retained separately for
each run under the ignored `.scratch/campaign-escape-menu-20261025/` directory.
Winit and Audio plugins are disabled; display variables are absent and a private
runtime/null audio configuration is checked before launch.

The overall rendered gate still fails for sky illumination and terrain/tree sight
occlusion. No original-client paired pixels, audible mixing comparison, browser
menu device run or full mission-objective completion is claimed. The unavailable
controls and broader scripting/game-mechanics plan remain acceptance work.

## Single-player Pause visibility (2026-11-05)

The fresh isolated RoC reference's Game Menu omits Pause while retaining the
space/anchor relationships used by the remaining controls. The recreation now
omits that painted control in its single-player escape-menu plan. Its FDF node
remains intact, so controls anchored to it continue resolving through the source
graph. No source dimensions or offsets are replaced or fitted to the capture.

The synthetic regression uses nonstock geometry and anchors Save below Pause,
then checks that Save retains its source-derived position after Pause is omitted.
The targeted escape-menu suite passes five tests; its explicit original-input
gate is ignored in that run. The headless mounted-menu capture also checks that
Pause has no live button entity. That rendered run and strict native/WASM checks
are still pending under `.scratch/menu-pause-visibility-20261105/`.
The original reference is verified stopped. Full-frame and multiplayer parity
remain unqualified.


## Native modal anchor chain (2026-11-05)

Static evidence in the reviewed Legacy 1.29.2.9231 image identifies the modal
container constructor at `0x61d630`. The anchor call at `0x61d706` places its
root; calls at `0x61d71c` and `0x61d732` supply its extent. The named backdrop
created at `0x61d750` receives a second anchor at `0x61d788`, then becomes the
parent of the named escape panel created at `0x61d799`. The backdrop offset is
therefore part of authored placement; centering each panel directly in the
window discarded it. Logical viewport factors come independently from the axis
conversion instructions at `0xab2a9f` and `0xab2ab7`.

`pcx_play::escape_layout` qualifies the selected executable fingerprint, checks
operand opcodes and call targets, and reads those extents, anchors, offsets and
frame names at runtime. Original bytes remain local. Both initial projection
and shaped-text remeasurement compose the same anchor chain. The shared control
spawner accepts that parsed viewport. Unsupported images or invalid records
refuse admission rather than substitute guessed placement.

Four synthetic tests cover nonstock anchor composition, operand decoding,
malformed instructions, invalid anchors and non-finite inputs. The explicit
selected-original executable gate also passes. These checks establish decoding
and composition, not rendered pixel parity.

The fresh isolated rendered run reaches and captures the main/end/confirmation
panels but fails during mission-exit capture. The receipt has no session before
capture and a newly published session at game time zero afterward, while all
Bevy clocks remain fixed. This repeats the baseline failure. A redundant check
for absent host did not resolve it and was removed. The quit/loading publication
path needs investigation; the capture clock assertion is preserved. No passing
full rendered run is claimed.

The baseline full-frame comparison against the fresh isolated original capture
reports 1,015,639 differing pixels out of 1,228,800, with bounds spanning the
entire frame. Side-by-side, overlay, exact difference, mask and receipt stay in
`.scratch/menu-pause-visibility-20261105/full-frame-comparison/`. World state,
camera and animation ticks are not synchronized, so these numbers are diagnostic
only. The pair exposes the placement error, missing world/HUD dimming and disabled
controls. No full-menu pixel acceptance is closed by this change.

The new full-frame comparison is retained locally under
`.scratch/menu-pause-visibility-20261105/full-frame-native-anchors/`. It reports
1,001,490 differing pixels of 1,228,800 and mean absolute channel error
22.320694715711806, compared with 25.771567925347224 for the baseline. The
side-by-side confirms the vertical placement correction. Border tiling, dimming,
text and disabled-control differences remain visible; world state and ticks are
still unsynchronized. The five synthetic app tests, explicit selected-asset
admission gate, native/WASM builds and native/WASM Clippy pass with warnings
as errors. These results do not close full-menu functional or pixel acceptance.
