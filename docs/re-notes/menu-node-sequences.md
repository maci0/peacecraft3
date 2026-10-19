# Menu node animation sequence boundaries

Local node transforms now sample only keys inside the selected authored sequence.
A channel without keys in that interval keeps its identity transform. The menu
passes the source interval through background skinning and each panel's current
Birth, Stand or Death phase. Global channels retain their independent clocks.
The shared borrowed sampler finds key bounds without allocations and preserves
STEP, linear, Hermite and Bézier interpolation within those bounds. Raw sampling
without a selected sequence remains available to its existing callers.

## Cause and original comparison

Fresh original RoC menu captures from the supplied Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
show the sky meeting hills and ruined silhouettes. The recreation instead showed
a straight bright-sky cutoff. Source inspection joins that edge to a gradient
surface whose node has no transform key in the first Stand sequence. The old
menu caller sampled its first key from a different sequence, moving the surface
into the sky. These source relationships identify the cause; no geoset index,
measured coordinate, camera angle, color or screenshot fit is a production rule.

The corrected native render restores the skyline and dark upper sky at both
1024×768 and 3840×2160. Original PNG and Warcraft client-window extents match
those sizes. References use isolated Wine/software rendering, not native Windows.
Animation clocks, particle ages and RNG are uncontrolled, so this is a visible
horizon correction rather than exact pixel or simulation parity. Other texture,
lighting, animation and UI differences remain open.

## Verification

The frozen combined candidate passes 116 model, 171 graphics and 411 menu binary
tests, strict native/browser Clippy, both application builds and formatting.
Two borrowed-pose tests execute in actual WASI, including the new allocation-free
sequence-boundary regression; fifty production particle CPU/quad tests also
execute there. A menu regression checks an unkeyed local node beside a global
animation. The explicit original RoC two-loop particle gate passes. Fresh native
TFT/RoC/TFT journals at both extents retain source backgrounds, logos and particles
without renderer errors. All owned reference and recreation runtimes are stopped.

Local source hashes, commands, failed attempts, client-window receipts and raw
before/original/after captures are retained under
`.scratch/menu-horizon-root-20261019/`; `comparison.html` presents those images.
The first test attempt missed three internal sampler test call sites and failed
to compile; it is retained separately from the passing candidate. Fresh browser
visual verification and synchronized original/native/WASM equality remain open.
No original assets or generated content are distributed, and the public site
was not rebuilt or deployed.
