# Original Options workflow

The target remains the original game's observable appearance, sound and feel;
the Bevy implementation may use different internals. `menu_screens.rs`,
`menu_settings.rs` and `launch_gui.rs` build Options from locally supplied FDFs,
their explicit include edges and inherited templates. Frame anchors, dimensions,
text, font properties, backdrop paths, slider ranges/steps/initials and referenced
widget art are read from those definitions; font faces resolve through the
supplied skin table. Layout remains relative. Missing or unsupported definitions
are reported instead of receiving invented content. See [font binding](menu-font-binding.md),
[backdrop reconstruction](menu-reconstruction-renderer.md) and
[menu art](menu-blue-art.md). Those shipped-byte observations establish inputs;
the documented atlas, widget placement and rendering conventions still need
original-client comparison. [Native coordinate evidence](glue-coordinate-runtime.md)
supports the frame conversion, but is not a recording of native execution.
Keep the selected asset source and edition explicit: the target Legacy executable
identity does not certify every archive in another local tree, and an installer
label does not establish its payload's release.

## First-run settings and mode-list boundary

The current source inventory finds 30 slider/checkbox/popup definitions; only
five connect to a runtime setting. This is not every editable field:
`GLUEEDITBOX GamePortEditBox` is separately unsupported. Other controls remain
disabled until their input/render/audio/gameplay owners can consume a checked
value. Qualified native inputs can provide read-only source fallback previews;
archive-only browser inputs report unavailable values. Neither presentation
admits an applied preference. Enabling a widget or persisting a value alone is
not implementation.

The inspected Options FDF and include graph supply widget ranges, steps and
initials, but no resolution MenuItems or admitted checkbox/popup selections.
Those slider initials are not evidence of the game's first-run preferences.
Named config/MiscData inspection did not establish those preferences either.
The exact client's lookup-null [fallback data path](original-options-defaults.md)
now has a checked source importer. Subsequent native inspection connects the
control reads, mode provider, source-table augmentation and local matching.
The enumeration-family selector is wired when that profile is admitted.
Augmentation, exclusive physical modes and effective first-run selection remain
open. A measured resolution or slider value must not become a source constant.
Current window-derived selection remains a recreation convention. Native audio
consumes qualified client fallbacks when no saved preferences win; unsupported
or MPQ-only sources retain explicitly unqualified
neutral multipliers. See [source defaults](original-options-defaults.md).

Native monitor mode records include refresh and bit depth. When the selected
client admits an enumeration-family profile and the primary monitor has video
modes, the menu offers those grouped source labels in provider order. Otherwise
it lists measured render sizes. Original labels, filtering, grouping and
matching are checked at their [source boundary](original-display-modes.md).
Selector-3 work-area families and exclusive physical modes are not offered. Browser
choices stay render extents. An admitted frameless popup profile can list its
source rows against one physical screen observation. That observation is not a
monitor, and the rows are not exclusive or work-area modes. The
[qualified native enumeration](original-display-modes.md) retains device/mode
identity, depth and refresh; its complete provider also adds source-table and
derived choices that this selector does not invent. No fixed preset list
replaces that policy.

Supported settings are actual render resolution, music/effects gains and their
enable toggles. Qualified native choices come from the primary monitor's video
modes after the admitted client's enumeration filter and refresh grouping.
Without that profile, native choices are the current window, the current
monitor size and that monitor's video-mode sizes, reduced to render sizes.
Without an admitted frameless popup profile, or without a positive physical
screen bound, browser choices are the current window plus the preview
container and screen at the device pixel ratio. The container stays measurable
after a selection; temporary canvas bitmap sizes are excluded. Those
`displayModes` pairs are not monitors. The popup bound is the separate
physical screen observation. A missing or unsupported client image keeps the
measured choices. An empty admitted popup list stays empty. Apply changes the
render extent. This is not an enumerated OS mode list. Browser preview fitting centers and scales
the canvas with a CSS transform, preserving its backing resolution and layout box;
it does not independently resize authored game controls. Fit view can make a high-resolution
menu too small to read in a narrow window. The host Actual size toggle instead
keeps identity scale and scrolls within the preview; switching it off restores
the contained view. Wheel input pans only in Actual size mode. This host viewing
choice changes neither game preferences nor authored geometry. The bottom-left cog on Main
opens separate PeaceCraft configuration and is hidden on original submenus. All cameras sharing the primary window
receive its changed dimensions before Bevy updates camera targets, so programmatic
resolution changes update their cached render extents consistently. These
requested-dimension updates are not a Winit/compositor acknowledgement.
Other game settings remain disabled. Their optional
[source fallback previews](original-options-defaults.md#read-only-fallback-previews)
retain checked raw units, source item order and localization without changing
the draft or installing input behavior. Missing or unsupported projections are
marked Unavailable; unavailable sliders hide an arbitrary thumb. This does not
make those settings effective or establish original first-run state.

Entering Options shows the category controls with no settings body selected.
The selected FDF still supplies keyboard focus; `TabFocusDefault` does not choose
a category. Explicit Gameplay, Video and Sound clicks show their own body while
retaining draft edits. Returning to Main and reopening Options resets category
selection. This follows the isolated original-client entry and reentry
observations. The empty landing's OK enablement remains a comparison question;
this correction does not infer a dirty-state rule from its appearance.

Entering Options copies applied preferences into a draft; page changes retain
that draft. Sound sliders reflect effective gains through the parsed authored
range, preserving qualified raw source values when available. Widget initials
do not override active audio; saved gains remain authoritative. Slider edits,
toggle edits and resolution selection affect only
the draft; Cancel discards it. Escape first dismisses an open resolution popup,
otherwise it cancels the page. The source OK button performs Apply:
validate the supported values and currently available render size. A changed
size enters a pending request; preferences and audio are published only after
Winit reports that client extent following submission. Cancel or pre-save
failure requests restoration, which is also checked before claiming success.
Dirty edits remain retryable after a save failure. The source Options screen
stays open while pending, with duplicate Apply and edits held. See the
[portable resolution transaction](original-options-resolution-apply.md).
User gain multiplies
the source-derived sound gain; disabling playback retains its gain for re-enabling.
Native preferences live in the platform user configuration directory at
`peacecraft3/settings.json`; browser preferences use localStorage. Invalid or
unavailable saved values are reported. Validation failure makes no request or
write. A storage failure preserves saved preferences/audio and checks best-effort
size restoration; unconfirmed restoration reports the actual available client
size instead of promising unchanged presentation. A browser audio failure after
commit is explicitly reported as saved/client-size-reported, with no false
rollback. This transaction and
storage design are recreation conventions, not proven native preference internals.
The version-1 resolution field remains a window/browser render size. Unknown
nested settings fields refuse instead of silently dropping an unsupported
exclusive-mode request. Existing version-1 preferences remain readable.

Optional source-derived submenu shells cover Single Player, Campaign, Custom
Campaign, saved games, replays, skirmish and LAN; the original LAN button routes
to the multiplayer shell. Back/Cancel returns through the implemented navigation.
These are navigation and rendering foundations: campaign composition needs more
work, gameplay actions and other asset support remain incomplete. Main-menu panel
timelines are currently reused on other pages; screen-specific transitions remain
open. The sky's radial distortion and black lower scene, plus water texture
selection/material work, remain separate rendering gaps. No complete recreation
or exact original-client visual match is claimed.

Verification status on 2026-10-15:

- Strict native Clippy run 11 and WASM Clippy run 10 pass for all targets/features
  with warnings denied. Native suite run 8 passes 2 library, 170 application
  (7 ignored) and 74 integration tests; the six JavaScript unit cases pass.
  Four camera-system regressions exercise the actual shared-window target refresh,
  including the old mismatch. These checks establish implementation behavior,
  not native or rendered fidelity. The only Rust change after that full test run
  is the separate configuration subtitle, now edition-neutral Game configuration.
- Combined original-fixture browser run 12 passed import, draft/Cancel/Apply,
  sound gains and persistence, resolution changes including the formerly failing
  reload/shrink case, wide and portrait views, readable Actual size Main/Sound
  controls, wheel panning, Fit restoration, Single Player, Campaign and LAN.
  It remains an automated failed run: its final whole-canvas OCR missed part of
  the separate cog caption. Root and independent visual review found the exact
  PeaceCraft settings caption readable and unclipped in the retained image.
  No screenshot or log was rewritten to turn that failure into a pass.
- Targeted cog run 1 separately passed actual imported-menu hover/click through
  to project configuration. Targeted keyboard run 2 verifies Arrow keys, real
  mouse wheel and Fit restoration without changing backing resolution; this
  latter host-only case uses synthetic saved preferences, not original assets.
  Earlier run 11 genuinely failed portrait-fit OCR. Actual size provides the
  readable source-sized alternative rather than claiming the tiny Fit overview
  is readable. Complete original-client visual fidelity remains open.
- Final release 8 / package 12 includes the edition-neutral configuration title
  and focusable preview viewport. The package allowlist contains 23 code, WASM
  and GPL source files. Original game bytes, imported ISOs and generated copies
  of asset contents stay local and must not be committed or uploaded. Local
  browser requests are restricted to code GETs; no asset uploads or sockets are
  permitted.
- Final targeted cog run 3 passes both the synthetic base-only edition fixture
  and the original TFT local repack: actual pointer opens configuration, and OCR
  verifies PeaceCraft / Game configuration. The base-only case also rejects TFT;
  it is an edition-policy test, not a rendered original RoC acceptance. Four
  captures, 30 code GETs, zero uploads, sockets, page or renderer errors. Cog run 2
  was invalid for that semantic-label assertion: its synthetic base fixture
  deliberately contains a synthetic label, not original Single Player text.
  The failed run remains retained, and run 3 corrects only that test assumption.
  Final WASM SHA-256 is
  `c3c4cf523988ddae2fd47cca1565872b54a58307a8d54046880ffa8a166e7780`.
  The acceptance is this explicitly scoped set of checks and visual reviews;
  combined browser run 12 remains failed at its final caption OCR assertion.

Publication: source commit `12eb299` and the asset-free demo commit `1380001`
are pushed. GitHub Pages reports that demo build complete. All 23 served files
match the final package hashes, including its WASM. The public demo is
<https://maci0.github.io/peacecraft3-demo/>. Original-import interaction checks
above ran against the identical packaged code on loopback; public-file equality
is distinct from repeating those local-file interaction checks on the hosted URL.

## Blank category landing validation

The application suite passes 4 library, 319 application and 84 integration tests
with 15 explicit application ignores. All-target, all-feature scoped Clippy with
warnings denied, the WASM compile check and workspace formatting pass. The
explicit original Options popup-cardinality test passes separately for the
caller-selected compat-tree and Legacy installs; this does not assign either
archive a release through directory adjacency. Source-leaf WebAssembly tests
also exercise the blank body and retained source focus. These checks do not
replace a fresh browser visual comparison or certify complete Options parity.

## OK follows the selected category

Two private Wine/Xvfb reference runs of the pinned Legacy executable
(`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
VERSIONINFO `1.29.2.9231`) at 1024×768 and 96 DPI show gray, inactive OK
on the blank category landing. Clicking it leaves that landing open. Settled
Gameplay, Video and Sound pages each enable OK before any edit. It remains
enabled after an actual music-slider change and a category switch. Cancel
returns to Main; reopening Options restores the blank landing and inactive OK.
Immediate transition captures were excluded from the settled-state comparison.
This establishes the observed category rule, not a dirty-draft rule or full
visual parity.

The control rebuild applies `InteractionDisabled` before assigning default
focus. Pointer collection, shortcut handling and disabled artwork use that
component; the action consumer also refuses an action queued before its button
became disabled. Cancel still restores the applied draft. No sampled pixel
values or screenshot coordinates are used by the implementation.

Four new behavior tests cover stale queued OK, enabled Apply, Cancel, default
focus and the real shortcut system. The complete current check passes four
library, 327 application and 84 integration tests, with 15 explicit application
ignores. Scoped all-target/all-feature Clippy, WASM Clippy with warnings denied,
formatting and whitespace checks pass. Fresh release-browser interaction and
visual comparison remain a separate gate.

## Local client-image input and browser display rows

The existing archive picker accepts one optional client executable or DLL beside
the MPQs. The client image is bounded separately and commits only after the
archive dependency closure hydrates successfully. It is kept outside the asset
store and is neither uploaded nor persisted. Cancel and a failed commit preserve
the prior image, assets and revision. A successful replacement with no image
clears the old image. Installer imports can supply a unique named engine member;
its filename and size do not establish a release or grant a display profile.

The browser admits that image through the same post-read parser as native.
Admission is cached by asset revision and selected skin. A qualified frameless
profile supplies source rows against the browser screen observation. Missing or
unsupported inputs keep measured render sizes; an admitted empty list stays
empty. These remain canvas render sizes, with no physical monitor identity,
color depth, refresh or exclusive mode switching.

The actual generated-WASM browser transaction passes eight checks, including
real picker input, cancel, failed hydrate, invalid input, archive-only
replacement and clear. A separate focused browser run opens Video Options and
the dropdown at the retained original capture's screen bound. Its five visible
rows match that conditional original witness in order. Both images were
reviewed; the browser capture reports no state change during capture. The
transaction and popup runs each record seventeen local code GETs, no uploads,
no external requests and no page errors. Forty JavaScript tests, 353 application
tests, strict native/browser Clippy and the browser build pass.

Evidence: `.scratch/browser-display-rows-root-20261017/`. The reviewed private
package contains code only. No package was published. The Options left panel,
popup typography and artwork, unsupported settings and complete mode-list
parity remain open.
