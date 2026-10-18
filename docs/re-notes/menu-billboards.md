# Authored camera-facing menu joints

The background now consumes full-billboard flags from its checked BONE/HELP
hierarchy. The pose plan derives its camera basis from the selected source
CAMS record. It composes authored tracks and pivots in parent order, rotates
around the transformed pivot, and cancels the actual parent's rotation once.
Nested full billboards therefore face the camera once rather than inheriting
an extra camera turn. Local spin, translation and scale remain authored.
Mesh skinning and lighting consume the same resulting skeleton.

This reconstruction convention follows the pinned primary renderer
[war3-model updateNode](https://github.com/4eb0da/war3-model/blob/52d7f421a5505a0434d764294057f2009ce3987b/renderer/modelRenderer.ts#L3540).
It is not native matrix-identity proof. The selected TFT model's affected
family belongs to its lens flares; this does not establish a particle-ray fix.
No model-specific ID, camera angle, pivot or scale is copied into production.
The immutable plan caches hierarchy order; each pose visits each joint once.

A bad camera, wrong pose count, nonfinite transform or unsupported transform
flags in a full-billboard family refuse before mesh updates. Degenerate,
reflected or sheared parent bases refuse rather than being silently normalized
into a different pose. Axis locks and inheritance flags outside those families
retain the ordinary path's existing behavior; they are not implemented here.
Panels retain their separate ordinary posing path. Their caller now propagates
pose errors, preserving the shared function's checked result.

Six synthetic tests cover nested/late-parent/sparse IDs, pivot preservation,
unchanged plain hierarchy composition, local spin/scale and refusal cases.
The causal old-path test fails the camera-facing assertion; the corrected
renderer suite passes 53 tests, plus its plain-pose integration test. The model
decoder passes 57 tests with three explicit local-input ignores. Both explicit
source-model lighting gates (Legacy TFT and the separately identified compatibility RoC input) pass 101 poses using the same camera-aware plan.
Strict native/WASM checks and builds pass for the composed application.

Fresh native and local packaged WASM applications each complete Main → Options
landing → Video → Main at 1024×768, 1280×960, 1920×1080, 2560×1440 and
3840×2160. Forty new journal captures were visually reviewed against twenty
retained original WC3/Wine captures. All sixty unscaled pairwise RGB comparisons
differ; no pixels are excluded, including the unqualified version-text footprint.
Navigation acceptance passes; exact output acceptance fails. The browser records
85 admitted local code GETs and no uploads, sockets or page errors. All owned
capture runtimes stopped. The public site was not deployed. The artifacts use
one frozen current working tree, including preserved foreign gameplay edits;
these are not clean-checkout build receipts. The report is
`.scratch/menu-alpha-root-20261019/comparison-1/index.html`.


Source values are unchanged. [Source-alpha addition](menu-additive-blending.md)
is a separate correction included in that runtime candidate. Particle rays are
visible in fresh native preparation frames, but their phase, RNG and intensity
are not certified against the original. Native camera-facing equivalence,
axis locks, culling/cutout, Windows execution and complete rendered parity
remain open. Receipts are in `.scratch/menu-billboard-root-20261019/` and
`.scratch/menu-alpha-root-20261019/`; no original asset contents are committed.
