# Executed camera-setup state

`ObjectHost::with_camera_setups(max_fields)` admits `CreateCameraSetup`,
`CameraSetupSetField`, `CameraSetupSetDestPosition` and, when present,
`CameraSetupGetField` plus the X/Y destination queries. Getter declarations are
optional. A present getter must match the selected source and signature; an
absent getter stays unsupported and does not block setup storage. Setup objects use
the existing bounded handle arena. `camerasetup` can extend `handle` directly
or through `agent`; its identity cannot be retargeted to another handle family.
The selected `ConvertCameraField` contract must produce `camerafield`, and
setup operations must share its declaration source.

Fields use the admitted conversion domain rather than copied camera-field
numbers. Each executed assignment retains its finite value and duration;
destination assignments retain both coordinates and duration. A repeated
assignment replaces that field on the same setup. Unset fields or destinations,
unknown identities, other handle families and non-finite inputs are refused.
No authored value is clamped, replaced or converted into a guessed default.

`max_fields` bounds distinct records per setup and is checked before adding a
record. Updating an existing field still works at the limit. The shared arena
continues to bound setup identities alongside timers, groups, players, sounds
and rectangles. There is no second identity pool.

Setup storage is separate from application. Present `CameraSetupApplyForceDuration`
declarations must match the selected source and exact signature. Only zero forced
duration is currently admitted. An application snapshots the executed fields and
destination; subsequent edits to that setup do not move the view. Non-panning
application retains the prior applied target and refuses if none exists. Timed
applications refuse before changing that snapshot. This does not implement the
other camera-application or location-returning natives. Finite-input refusal and
resource budgets are recreation policies; native exceptional-input behavior is
not established.

## Verification

Four synthetic tests execute typed native calls and a fresh JASS VM. They cover
field replacement, exact retained float bits, unset queries, invalid mutations,
capacity, shared identity separation, source mismatches and consistently
retargeted declaration families. All four also execute under WASI. Full
`pcx-play` tests, strict native/WASM Clippy and downstream native/WASM application
checks pass. The initial all-target Clippy check found two unnecessary test-only
clones; those calls now borrow their single-element argument slices.

The retained Human01 gate uses the same admitted Program, localized source and
explicit compatibility object catalog as the preceding sound/rectangle gates.
Globals and `config` still complete. It creates 18 setups, assigns 126 distinct
fields and 18 destinations, and verifies every retained value/duration against
the actual executed call bits. The next refusal is `SetPlayerTechMaxAllowed`
at map span `376367..376419`, declared in the selected `common.j` at
`112972..113076`. Input identities and their limitations remain as recorded in
`script-stacked-sounds.md`; no archive priority or release is inferred.

This is script-state evidence, not an applied camera, native pan comparison or
playable campaign acceptance.

## Application camera consumer

The launcher installs `campaign_camera`. It reads an instant application from the
published VM and resolves the seven required field identities through that VM's
initialized `CAMERA_FIELD_*` globals. No conversion numbers, camera coordinates,
angles or authored clipping values are copied into the renderer. A setup with
missing required fields cannot produce a guessed camera.

The selected source resolver supplies `UI\MiscData.txt`; the shared object-profile
lexer reads `Camera/NearZ` at the application's selected zoom-list index. The
launcher selects index zero and a 64-KiB read cap as explicit application policy,
not as inferred original zoom state. The source receipt is retained. Missing,
ambiguous, over-budget or invalid configuration refuses camera creation.

The camera consumes the ground renderer's terrain surface and `(x,z,-y)` mapping.
Vertical perspective FOV, target-height sampling, angle/roll composition and full
window aspect handling are recreation conventions pending original gameplay
projection comparison. There is no native smoothing or timed pan claim.

Camera and terrain use a separate application render layer, excluding menu
geometry. The view activates only over the same admitted terrain as its published
VM. A refused session replacement keeps the old view; a successful replacement
removes it and reads the new source clip. An invalid replacement camera does not
leave the old session's camera mounted. Escape suspends/resumes this view and
excludes suspended elapsed time through a separate runtime pause input; it does
not clear an explicit simulation pause. The retained launcher/menu is a temporary
return surface, not a recreation of the original in-game pause menu.

The synthetic integration gate exercises the production factory, owning VM,
source resolver and Bevy camera components, including deliberately different
field enumerands and changed source clipping. It does not render a playable
chapter. Automatic Host-input preparation, local-player camera selection, mission
initialization, actors/controls and original/native/WASM gameplay comparison are
still open. Local command receipts are under
`.scratch/campaign-camera-root-20261019/`.

The 2026-10-19 follow-up passes eight camera-host cases under native/WASI,
the selected local Legacy config gate, 55 object tests, 579 playback tests and
543 application cases (library, binary and map-loading integration). Strict
native/WASM checks and both application builds pass; the actual Cargo WASM module
executes 17 integration cases, including two camera cases. A targeted WASI config
case also passes. The parser uses an explicit config mode for inline comments
outside quotes; existing unit-profile parsing remains unchanged. Degenerate
precision and overflowing projection refuse, while pole directions use the
requested rotation to define camera up.

A bounded native startup smoke creates the Vulkan-backed application window with
no scheduling panic; its timeout is an intentional stop, not a zero-exit test.
It reports the existing unsupported menu-click sound compression. No rendered
campaign, fresh browser visual, original runtime or cross-resolution gameplay
comparison was performed.
