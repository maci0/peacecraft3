# Authored menu texture mipmaps

The menu previously decoded and uploaded only BLP mip zero. The selected local
Legacy RoC background names 33 textures containing 257 authored levels, including
lower-resolution grass, clouds and ground. Dropping those levels left distant
surfaces sampling the full-resolution image. This identifies missing source data;
it does not establish the cause of every reported horizon or particle artifact.

The bounded model-image loader now retains the supplied lower levels unchanged.
It checks their dimensions, contiguous header entries, payload ranges and total
decoded byte budget before decoding. Geometry and PRE2 admission include every
level in image identity and backing-memory accounting. Both upload paths preserve
the authored mip order, RGB and alpha. Browser import uses the same admission.
Base-only consumers, including terrain-atlas slicing, keep their existing contract.
No mip pixels are regenerated and no opacity, geometry or camera values are fitted.
The existing application sampler policy remains a recreation choice; the original
client's per-draw mip selection, quality settings and LOD bias remain unqualified.

## Verification

The frozen candidate passes 55 graphics-library, 17 particle-render integration
and 412 menu tests, strict native/all-target and browser-WASM Clippy, both full
application builds and formatting. Synthetic fixtures distinguish authored lower
pixels from a downsampled base, retain alpha and reject incomplete budgets,
malformed levels and unequal duplicate chains. A software-Vulkan readback verifies
three independently colored levels through the menu material. The selected local
RoC textures also match the direct decoder and upload-buffer bytes at all 257
levels. That byte check is separate from GPU readback.

Fresh native TFT/RoC/TFT journals pass at 1024×768, 1280×960, 1920×1080,
2560×1440 and 3840×2160. All fifteen captures were reviewed. Distant texture edges
are softer, while all 33 RoC vertex/projected-position arrays exactly match the
previous fixed-step journal at 1024×768 and 4K. The captures use isolated software
Vulkan and null audio; they do not verify audibility or native Windows output.

Fresh trusted-entry browser RoC captures pass at 1024×768 and 4K using an explicit
isolated SwiftShader WebGL2 context. Each run has eighteen manifest-qualified code
requests, no page/renderer errors and no sockets. Original content stays in a
local imported fixture. Its RoC model/texture closure comes from the selected
Legacy installation; eleven overlaps with the retained fixture match exactly.
Other retained fixture content keeps its earlier mixed provenance.

The first application attempt exposed an iterator-reference mismatch in unfinished
hero-skill work; only that compilation issue was corrected. Validation includes
that unfinished worktree, not a separate clean-checkout build. A default-host GPU
attempt blocked on a Unix peer before adapter identification and was terminated;
hardware acceptance is not claimed. The first private visual build omitted Cargo
macro context, and the first fresh browser input lacked RoC assets. Those failed
attempts remain beside the corrected runs; admission rules were not weakened.

All owned processes are stopped. Raw captures, input identities, checks and the
local three-target `comparison.html` are under
`.scratch/menu-mipmaps-root-20261020/`. Original references are retained WC3/Wine
captures from the horizon check, not a new original runtime. All six full-frame
RGB comparisons differ, without tolerance or a version mask. Original particle
RNG, animation phase, full transparency/sampler parity, ordinary hardware-browser
startup and exact original/native/WASM output remain open. No public site was
deployed and no original asset content was committed or uploaded.
