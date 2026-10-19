# Menu texture wrap and filtering

The shared TEXS sampler helper changed both addressing and filtering when either
authored wrap flag was enabled. Non-wrapping images inherited `ImagePlugin`'s
linear filtering, while repeating images used `ImageSamplerDescriptor`'s nearest
defaults. The previous test compared against the descriptor and preserved this
unintended difference.

Repeating images now retain the same default image filtering as non-repeating
images. TEXS still selects each repeat axis independently. No texture pixels,
alpha, animation, coordinates or asset values change. This restores the existing
renderer policy; it does not establish the original client's complete per-draw
sampler policy, quality settings or mipmap selection.

The selected Legacy RoC member uses repeat flags on clouds, ground, grass and
several prop textures. Native before/after captures at 1024×768 and 3840×2160
show smoother texture sampling. All 33 recorded RoC geoset vertex and projected
position arrays match the prior fixed-step journal exactly. The original WC3/Wine
references are retained from the horizon check, with uncontrolled original
animation phase and particle RNG. This is not a pixel-equality pass or a new
original-game run. Rain textures follow the unchanged non-wrapping path.

The frozen correction passes 53 graphics-library and 411 menu tests, strict
native/all-target and browser-WASM checks, both full application builds and
formatting. The regression checks both individual wrap axes and their combination
against the default image filter, LOD and anisotropy settings. Fresh native
TFT/RoC/TFT journals retain the expected backgrounds, particles and logos at both
extents. An unavailable gamepad device is recorded separately from renderer errors;
the first classifier rejected that environment message as a rendering failure.

A fresh browser package builds, but its first 1024×768 adapter run times out before
menu readiness. That run is rejected, stopped and not retried; 4K browser capture
was not attempted after it. Rendered WASM acceptance, rain appearance, exact
transparency and original/native/WASM output equality remain open. No public site
deployment occurred. Commands, source hashes, rejected attempts and the local
`comparison.html` remain under `.scratch/menu-wrap-filter-root-20261019/`.
