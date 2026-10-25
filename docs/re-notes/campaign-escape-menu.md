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
text. The selected panel supplies the root extent; centering that root and sizing
the common backdrop to it are explicit composition choices, not screenshot fits
or a claim about native CFrame objects. Input and decoded payload limits precede
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

Save, Load, Options, Help and Tips are still unavailable. The separate Pause
button is disabled; opening this single-player menu already pauses its clock.
Their source disabled art/text are shown, and they do not report placeholder
success. This does not qualify the original client's dynamic visibility rules,
multiplayer menu behavior, quit/score choices or exact transition timing.

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
