# Options slider input in centered viewports

The admitted Music and Sound volume sliders keep their FDF ranges, steps,
rectangles and thumb art. The installed Bevy 0.20 slider press handler normalizes
full-target pointer coordinates against a viewport-local UI transform. With the
menu's centered UI camera, a synthetic midpoint therefore selects 83.63637
instead of 50. An origin-zero control passes. This is an executable widget
regression, not a measured original-game slider value.

The existing admitted slider owner now adds an invisible child covering the
parent's bounds. Its press observer subtracts the matching camera's physical
viewport origin in logical target units before the event bubbles to the stock
Slider. This avoids ordering assumptions between observers on the same entity.
Missing or mismatched camera context refuses the press. The stored pointer and
global picking coordinates remain full-target; drag distances, capture, focus,
keyboard input, source units and Apply/Cancel behavior keep their existing owners.
No authored geometry or preference value is replaced.

Seven synthetic app regressions execute the real Slider, UI picking, pointer
focus and existing button-action systems. They cover both viewport origins,
target scales and UI scales, endpoints, keyboard stepping, drag/release,
parent hover/focus, one parent activation and disabled/missing-camera refusal.
Removing only the coordinate correction causes four failures. The native app
suite passes 281 tests with 13 explicit local-input ignores, plus four library
and 83 integration tests. Strict native/WASM all-target/all-feature Clippy and
the full WASM build pass.

## Options entry and explicit tabs

The browser baseline also exposes a separate application defect. Animated entry
admitted a pending FDF without remembering its entry page. The first later tab
parse then discovered that entry and replaced the explicitly selected Sound tab
with Gameplay. The correction qualifies the entry candidate before panel
handoff and publishes it only after target admission succeeds. Later parsing
may remember the entry, but cannot change the displayed tab. Changing the asset
selection clears the prior entry. Three synthetic cases cover early admission,
late explicit selection, draft preservation, ambiguity and changed source flags.

This retains the existing focus-default-based entry policy; it does not prove
that a focus flag is the native initial-page controller or recover first-run
preference values. Original first-run/default resolution selection remains open.

The existing explicit local browser gate has a `--slider` route. Its expected
values come from runtime FDF range/step diagnostics, rather than a copied original
range. It checks midpoint and keyboard input, outside-track drag/release,
unapplied audio/storage preservation and Apply/Cancel at centered and origin-zero
UI viewports. Browser execution and rendered review are recorded separately from
the synthetic widget checks. The first browser attempt fails before Options:
CDP-only pixel-ratio emulation reports ratio two while the device-pixel
ResizeObserver box remains at one. A separate synthetic canvas reproduces that
inconsistency. The calibrated gate sets Chromium's native scale as well and
requires observer/display measurements to agree; no production coordinate or
resolution workaround is introduced for the emulation mismatch. These checks do
not establish original gain response, source-qualified first-run resolution
choices or full menu fidelity.

Separate actual input-pipeline tests reproduce lost clicks when movement onto a
new target and press/release share one update. Existing widget collection tests
do not cover that upstream loss. The browser gate now acknowledges movement,
press and release before continuing. This validates a normal multi-frame
interaction, not a repair of the separately tracked fast-click defect. Retained
evidence: `.scratch/options-fast-click-pipeline-20261016/`.

## Pressed labels and button activation

The complete browser runs reached all four slider cases but exposed a second
input defect on OK. The pointer remained over the same logical button, while
its raw press and release targets were different generated text spans. Bevy
picks text sections independently of their root. The authored pressed-label
offset moved a different section under the stationary pointer, so the standard
click matcher correctly saw different targets and emitted no activation.
The label root already ignored picking; every decorative generated span now
does so too, leaving the existing button as the stable hit surface. The source
offset, hotkey styling, normal button actions and Apply transaction are retained.

A real text-pipeline regression shapes a synthetic two-section label with the
installed dependency font and executes the window-input/UI-picking lifecycle.
Unmarked moving spans reproduce the lost click; unchanged unmarked text is a
successful control. Ignoring the spans restores exactly one button action at
both target scales. This does not replace the shared picking dispatcher or
claim that the separate same-frame pointer batching defect is fixed.
Evidence: `.scratch/options-text-span-picking-20261016/`.
The persisted app regression uses the production `apply_menu_text` span creator,
then shapes those generated runs and executes actual window/UI input at both
scales. It passes with the fix and fails with zero clicks/actions when only the
production span marker is removed. The source is restored before subsequent
checks; private fixture insertion of the marker cannot make this gate pass.

The failed full browser runs and the focused diagnostic runs remain separate.
Keyboard activation in a diagnostic successfully saved the candidate, but did
not count as mouse Apply acceptance. An older larger centered viewport also
showed missing tab captions after a rebuild; their recovery on resize and their
visibility at a smaller centered viewport do not establish a repair. That
rendering issue remains open independently of the input corrections.

## Actual browser acceptance

Fresh local packaged release run 10 passes the complete `--slider` route without
keyboard fallback for Apply. Both Music and Sound sliders select the source-range
midpoint, step through the keyboard, clamp outside-track drags and stop capture
on release at centered and origin-zero UI viewports with native device scale two.
Unapplied edits leave active audio and preferences unchanged. Mouse OK saves the
exact candidate and applies its normalized gains; reopening, editing and pressing
Cancel preserves the committed settings and audio. All four captures were viewed;
the labels, slider thumbs and panel controls are readable in these host shapes.

The run records 15 allowlisted local code GETs, no request bodies, unexpected
requests, page errors or sockets, and one successful preference save. Its original
fixture is a local repacked import, not proof of a retail patched ISO. No original
assets, captures or fixture metadata are published. Native/WASM strict checks,
the complete app suite and the browser harness self-check pass; the Node suite
passes 58 tests with two explicit local-input skips. Parser, synthetic input and
browser evidence do not establish original first-run choices, gain response,
physical mode switching or complete menu appearance.

Local evidence: `.scratch/options-slider-root-20261016/`. Original assets,
fixture metadata and screenshots remain local. The full original Options scope
continues in [the defaults note](original-options-defaults.md) and
[the display-mode contract](original-display-modes.md).
