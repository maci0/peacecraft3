# Launcher keyboard focus

Returning to the project's Main menu control now reveals its complete heading.
The prior nearest-edge rule could leave the button visible while clipping the
title above it. Keyboard focus on that header control now returns the launcher
scroll origin to zero. Other controls keep their existing nearest-edge rule,
including controls without a Pick action. Pointer focus and manual scrolling
with unchanged keyboard focus retain their existing behavior.

The change is confined to the project launcher reveal system. Artwork, fonts,
layout, asset-derived game frames and settings values are unchanged. No pixel
offset was fitted from a screenshot. Three synthetic regressions cover header
return, ordinary controls at two scale factors, and the focus visibility/change
guards. The causal old-path run fails with scroll position 50 instead of zero.
Retained actual native and WASM before captures show the same clipped title.

All 654 application tests pass with 30 explicit ignores. Strict native/WASM
Clippy, formatting and both application builds pass with frozen source hashes.
Six natural-size native runs and six compiled-WASM component runs cover
640×800, 1024×768, 1280×960, 1920×1080, 2560×1440 and 3840×2160. Each follows
initial configuration, forward Tab to lower controls, then reverse Tab back to
the first Main menu control. All 36 new captures were visually reviewed; the
title and subtitle remain fully visible on return, and lower focus still
scrolls into view where necessary.

The browser fixture mounts the compiled application in a code-only canvas host
and calls the existing parent-sizing API after startup. Its initial geometry
refusal remains a failed attempt. No original assets are served; the accepted
runs record 24 hash-checked local code GETs, with no uploads, sockets or page
errors. Public import/fullscreen flows are outside this component check.
Native clients use natural monitor-fitted sizes and platform DPI; browser
clients use the declared full viewport at DPR1. These are UI regression checks,
not equal-size pixel comparisons. The native audio output is an isolated null
device; audibility is unverified.

The artifacts use the frozen current tree, including the preserved foreign
gameplay edits listed in the source manifest. They do not claim a clean-checkout
build. All owned capture processes stopped, and the public site was not
deployed. Receipts, including failures, source/build identities, before/after
images and the review report, remain local in
`.scratch/launcher-heading-root-20261019/`.

This closes the launcher heading defect. Original-game rendered appearance,
same-RNG deterministic simulation, decorated desktop work areas and native
Windows remain separate acceptance gates.
