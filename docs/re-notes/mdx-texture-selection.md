# First-layer MDX texture selection

`pcx_gfx::mdx_texture_selection::TextureSelectionPlan` consumes the existing
checked `RuntimePlan` and a selected sequence interval. It returns an authored
TEXS index for each geoset's first material layer. It does not bind images,
interpret TXAN, add material layers, or change opacity, UVs or samplers.

KMTF keys are signed times and integer texture IDs. The plan supports STEP
(`Interpolation::None`) only and holds each key until the next. Local channels
keep keys inside the selected clip, inclusive; global channels keep all keys and
use independent `u64` elapsed milliseconds modulo their authored GLBS duration.
The clip clock must not replace that elapsed clock. Sample at initial spawn as
well as on advance: an active frame-zero key overrides the static texture.

No channel, an empty channel, or a local channel with no keys in the selected
interval uses the static texture. Before the first applicable key, sampling
clamps to that key, matching the existing scalar sampler's endpoint convention.
This is an explicit reconstruction convention, not native runtime proof.

Admission rejects reversed intervals, inconsistent geoset ordinals, absent
materials/first layers/TEXS records, multiple channels, duplicate or unordered
key times, unsupported interpolation and absent or zero-duration global clocks.
Validation covers original keys before local projection. Errors retain geoset,
material and first-layer context; callers add their model/source identity.
Source records remain unchanged. Missing sampled geosets return `None`.

The caller must resolve the returned TEXS **index**, preserving that record's
path and sampler flags. Equal paths do not imply identical sampler policy.
The application integration below resolves these IDs into image handles and ECS
updates without collapsing their authored identity.

The retained local TFT model SHA-256
`15586ffd9dc25b82a4f8ce2e679581f6e14cbcdc0916c7422d89308f010f6293`
exposes the missing caller behavior: its water material's static record names
iceberg artwork, but its KMTF channel selects ocean textures from frame zero.
The model supplies every key, texture reference and global duration; no measured
value belongs in the application. This fixture's release identity is unresolved.

Synthetic source-leaf tests vary keys, texture identities, clip windows
and clocks, including `u64::MAX`, duplicate/malformed refusals and source
preservation. The ignored original boundary test requires an explicit local
`PEACECRAFT3_TEXTURE_SELECTION_MODEL` path and derives all expected key values
and global wrap boundaries from it. No original payload is included. Parser and
selection tests do not establish rendered water, sky projection, uncovered
clear color or native animation timing.

## Application binding

The application exports this module and supplies the already
checked runtime/selection plans to the shared menu geoset spawner. It admits all
rendered first-layer static and KMTF image dependencies before spawning any
geoset for that model, using canonical source paths and checked RGBA shapes.
Unequal canonical decoded candidates, missing paths and unknown TEXS sampler
bits refuse with source/material/geoset context. Material shading and blend flags
retain their existing behavior; TEXS sampler admission is a separate check.

Each model retains its image lookup and selection identity by TEXS ID. After
all requested canonical decoded images are admitted as equal, records with the
same canonical path and sampler flags share a backing Image; different sampler
flags remain distinct. Initial materials bind `sample(frame, 0)` rather than
the static texture word. Texture updates validate all bound targets before any
write and assign only changed selections, preserving material color, alpha,
blend, culling and unlit state. Distinct source IDs that share an image still
advance the selected-ID state without dirtying an unchanged material handle.
The independent elapsed clock is checked as u64 milliseconds and advances even
when the local clip is absent or has zero duration. Such a clip holds its local
frame; it does not freeze global tracks. Panels select the matching Birth/Stand
plan without resetting that clock. Existing skeleton-clock behavior, including
its separate u32 elapsed conversion, is outside this change.

The strict native source-leaf gate exercises the actual
spawner/cache/update blocks with CPU Bevy Assets and AssetEvents, plus synthetic
panel phase clocks and an explicit retained local fixture. It confirms sampled
initial handles, per-TEXS samplers, global wrap, no texture-induced modification
events for unchanged selections, and refusal before per-model partial spawning.
These gates do not establish GPU output or native clock/projection semantics.
Separate menu model calls retain the caller's existing transaction boundary.

The final native application suite passes 178 tests with eight explicitly
ignored input-dependent cases. Two of those gates were then run with their
required inputs: both original RoC/TFT backgrounds import with actual decoded
textures while preserving helper parents, and six retained source-qualified
background/panel/logo models pass the real image-cache and material-handle
boundary checks. The public selector's separate original TFT key/wrap test also
passes. Strict native and wasm32 Clippy cover both application and graphics
crates across all targets/features with dependency diagnostics excluded. These
gates are captured in `.scratch/menu-texture-selection-root-review/`.

The checked source at `15da7c4` also passes the release WASM build, bindgen
0.2.129 and the allowlisted asset-free package. A fresh browser run imports the
explicit local original-asset repack through the real file input and captures
Main, Video Options and the return to Main. All three captures render; manual
review confirms visible water and the corrected gold/blue logo. The run records
15 local code GETs, no uploads, sockets or page errors, and preserves the exact
build/source/package hashes in `.scratch/original-options-browser/texture-1/`
and its build receipt. Key selection and clock boundaries are established by
the separate public-API/ECS gates, not inferred from screenshot timing.

The same 23 code/WASM/GPL corresponding-source files are published at demo
commit `6e2c4de`; every live file matches the checked package and asset-free
startup passes. No original textures, models, fonts, archives or screenshots
are hosted. The browser evidence is for this local repack, not retail installer
or native rendering parity. Sky projection, the uncovered black region,
TXAN translation, particles, broader material fidelity and panel screen
transition timelines remain open.
