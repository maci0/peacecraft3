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
not count as mouse Apply acceptance.

## Caption review correction

The earlier report of captions disappearing in a larger centered viewport was
a visual-review error. A fresh run on the final release retains all three
captions. Comparing the saved image regions at their own runtime label bounds
also disproves the earlier report: in old browser run 6, Gameplay, Video and
Sound are byte-identical between Options entry and the later centered Sound
screen. The new run's Gameplay and Video regions match those same old regions;
the hovered Sound region changes without losing its text. Nonzero caption-color
pixels independently corroborate the exact comparisons. No caption repair is
attributed to the span-picking fix, and no rendering workaround was added.

The fresh large-centered entry/Sound/origin-zero resize run passes with three
captures, no saved preferences, unexpected requests, page errors or sockets.
The comparison records the screenshot raster/bitmap boundary explicitly and
does not assume they have identical dimensions. Its image regions come from
runtime diagnostics, not fitted screenshot coordinates. Evidence and exact
comparison hashes remain local in
`.scratch/options-large-caption-rootcause-20261016/`. This corrects the specific
caption claim; it does not establish full menu or native-rendering fidelity.

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

## Disabling a pressed button

The stock widget leaves its temporary `Pressed` component behind when a control
becomes disabled between pointer down and release. Its release/out/cancel
handlers skip disabled controls. The input prototype independently reproduces
this in both the stock and ordered dispatcher, so it is a separate lifecycle
bug, not evidence against the event-location fix.

The shared app button owner now removes that temporary component when either
`InteractionDisabled` or `Pressed` is added to an already-disabled button. Both
additions matter because a press insertion can be deferred until after disable.
The removal tolerates the old control being despawned before its command runs.
It emits no activation and leaves enabled held buttons alone. A regression fails
on the old owner, then passes immediate and deferred disable, release without
activation, and a fresh successful click after re-enable. This does not fix
same-frame pointer hit resolution or enable any unsupported setting.

The complete native suite, strict native/WASM Clippy, WASM application build and
format checks pass. Evidence, including the baseline failure and intermediate
compile/test corrections, remains local in
`.scratch/button-disable-root-20261016/`. This is ECS lifecycle evidence; no new
browser capture or native-client parity is claimed for this additional change.

## Event-local pointer hits

The application now opts into an event-local picking path in three locally
patched Bevy 0.20 crates. The normal UI, sprite and window resolvers use each
input's own location instead of sharing the frame's final hover. Input dispatch
keeps the existing pointer state machine and applies observer commands between
inputs. Final live hover also reconciles targets moving under a stationary
drag, with ordinary Enter/Leave events and no fabricated pointer movement.
Cached layout is not replayed. See [the patch scope](../../vendor/BEVY_PATCHES.md).

The new application tests use real window input, picking and widgets, including
the current shared button owner and slider adapter. Fresh clicks, an A click
ending over B, capture, cancellation, touch, multiple pointers, observer changes,
mixed backend priority and stationary drag targets pass. The unchanged opt-out
fresh-click case remains an explicit negative control, ignored in the ordinary
suite and separately demonstrated to fail without the opt-in. Native tests pass 301 app
cases with 14 explicit ignores, four library and 84 integration cases; the
patched picking crate also passes 22 unit tests. Strict native/WASM Clippy,
production-library checks, warning-denied documentation and the release browser
application build pass. Upstream sprite/UI packaged test fixtures are not a
complete local test suite: the sprite tests reference a sibling font and an
unavailable test macro. No suppression or test deletion substitutes for them.

The browser harness has a trusted native-input `--fast-click` route. It checks
fresh hover A, then A-click/final-pointer-B, using control rectangles from runtime
diagnostics. Each case must show press and release within one current update and
the requested final pointer location. Split-frame timing is unqualified. Exactly
one identical press/release/click sender and one matching logical action are
required, with no held state. A tab rebuild can despawn that sender before the
late diagnostic owner lookup; the captured unique action identifies the sender
without inventing a replacement mapping. Wrong known owners, different senders,
extra events and extra actions still fail. The first real run exposed that
late-owner lookup limitation; its failed assertion is retained separately.

The corrected combined `--fast-click --slider` run passes on the fresh final
release package. Both fast cases have trusted canvas input and qualified
current-update witnesses. The existing source sliders also pass midpoint,
keyboard, outside-drag/release, mouse Apply and Cancel checks at both viewport
layouts. Five captures were reviewed. All 24 package files match their receipt;
15 local code GETs, one preference save, and zero uploads, sockets, unexpected
requests or page errors are recorded. This remains a local repacked-original
fixture check. The PeaceCraft cog still overlaps a disabled field on Options;
that separate presentation defect is not closed by the input fix.

Local app/browser evidence is in `.scratch/options-ordered-input-root-20261016/`,
including the failed assertion and documentation checks. Earlier prototypes
remain in their separate frozen evidence directories. Source defaults,
physical display modes and complete original-menu fidelity remain separate.
