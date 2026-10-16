# Executed camera-setup state

`ObjectHost::with_camera_setups(max_fields)` admits `CreateCameraSetup`,
`CameraSetupSetField`, `CameraSetupGetField`, `CameraSetupSetDestPosition` and
the X/Y destination queries from the loaded declarations. Setup objects use
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

This stores script requests. It does not install a renderer camera, interpolate
over the retained duration, schedule a transition or implement location-returning
and camera-application natives. Finite-input refusal and the resource budgets
are recreation policies; native exceptional-input behavior is not established.

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
