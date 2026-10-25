# Source-bound menu lighting

The background uses numeric Gouraud vertex lighting before texture modulation.
It replaces the old source-intensity-to-lumens multipliers, white ambient floor
and use of MDX attenuation end as a point-light range. Authored opacity, texture
and sampler selection, blend mode, fog and independent menu panels keep their
existing paths.

## Source and reconstruction

The parameter reader admits only the reviewed Legacy executable, VERSIONINFO
1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Identity and instruction addresses qualify the profile; parameter values are
read from the supplied executable on both native and WASM. Missing, altered or
unsupported images refuse background lighting instead of substituting values.
No executable code is run by this reader. Imported originals stay local.

Startup at 0x42e9ac calls 0x7d0cd0 with the linear/quadratic operands. That wrapper
dispatches renderer slot 0x8c, whose reviewed setter stores them at object+0x378
and +0x37c. Both D3D9 update paths copy those values into the light state.
The independently stored constant attenuation, range and submitted light-slot
limit are read from their instruction operands; the reciprocal-byte scale comes
from the image data section. The OpenGL update passes the same stored
coefficients to its scalar light parameters. This is static native-code evidence,
not a claim that the interrupted live OpenGL probe ran successfully.

Colour channels are packed to bytes before intensity scaling. A bounded join
of the retained original TFT trace to the supplied model matches all twenty
submitted diffuse f32 channels across its five lights, including alpha. Using
raw model colour floats fails that comparison. The CPU recreation follows this
packing and multiplication order. The original renderer's light summation order
and exact GPU arithmetic remain fidelity questions.

The chosen profile is the uncapped D3D9 path, consistently applied across native
and WASM regardless of the recreation host GPU. The original AMD-specific
quadratic cap is separately decoded but is not selected by this profile.
This preserves cross-target policy; native hardware equivalence remains open.

D3D9 defines distance attenuation separately from MDX editor attenuation fields.
Vertex diffuse and ambient contributions multiply the animated material/geoset
colour before saturation, then the existing unlit menu material modulates the
texture. Alpha is preserved. No white ambient floor is added.
See [diffuse lighting](https://learn.microsoft.com/en-us/windows/win32/direct3d9/diffuse-lighting),
[ambient lighting](https://learn.microsoft.com/en-us/windows/win32/direct3d9/ambient-lighting)
and [attenuation](https://learn.microsoft.com/en-us/windows/win32/direct3d9/attenuation-and-spotlight-factor).

## Admission and limits

Light records, pivots, colours, intensities, track layouts, keys, global clocks
and budgets are validated before scene construction. Supported lights are
unparented point lights, within the source slot budget. Authored node transforms
use the checked joint plan and the same local/global clip sampler as geometry;
their matrices transform the authored light pivot before vertex illumination.
Scalar intensity and three-channel light colours reuse the shared animation
samplers. Visibility currently admits discrete binary step tracks; unsupported
parent hierarchies, fractional visibility and excess-light ranking refuse explicitly.

The selected RoC `TutorialCampaign3D.mdx` contains two unparented light nodes
with translation tracks. They previously refused the whole background. A
nonstock fixture now tests sampled positions at three clip times and refuses
a missing global clock. This is source sampling support, not a new native
lighting or rendered-pixel comparison.

Normals follow the existing position skinner's first-matrix group convention,
using the inverse transpose and excluding translation. Multi-matrix skinning and
exact original normal transformation remain open together. The supplied TFT and
compatibility RoC models pass source-plan and 101-frame CPU consumer gates.
These are input-specific checks, not general model support or rendered parity.

An unavailable lighting source reports a menu notice while independent authored
panels can still load. Edition/import revision replacement continues to own
scene invalidation; there is no separate lighting cache.

Primary ISO filenames retain their version suffix in provenance. Client-image
selection strips that suffix only for the ISO basename comparison; multiple
versions still refuse as ambiguous. Installer/archive basename selection is
unchanged.

## Verification scope

Private command receipts, source manifests and diagnostic PNGs are under
`.scratch/menu-lighting-root-20261018/`. Ordinary parser/consumer tests, strict
native/WASM checks, actual Wasmtime execution and software Vulkan rendering are
recorded separately. The source-background diagnostic includes original model,
textures and source-derived lighting at a fixed pose. It excludes UI and does
not advance animation, so it cannot establish a full original/native/WASM pixel
comparison. Earlier full-menu captures belong to the preceding fog revision.

The frozen implementation passes 653 ordinary application tests (29 explicit
fixture/GPU ignores), eleven ISO engine-selection tests, the explicit missing
client-image panel recovery gate, strict native/WASM Clippy and formatting.
Actual Wasmtime runs pass 57 MDL tests, three parameter tests and the explicit
original-image parameter reader. The unchanged lighting/fragment path also
passes seven synthetic software Vulkan cases. Five fresh software Vulkan source
backgrounds at 1024×768, 1280×960, 1920×1080, 2560×1440 and 3840×2160 have the
requested dimensions and nonuniform pixels. Both original model gates admit
five lights and complete 101 CPU frames on the final candidate.

Earlier restricted-session GPU and display-socket failures are retained. After
access was restored, all seven hardware Vulkan cases passed. The final combined
implementation also passes 654 application tests with 29 explicit ignores,
strict native/WASM checks, formatting, both application builds and local browser
packaging. Fresh native and browser runs complete Main, Options landing, Video
and return at five resolutions through 3840×2160. The private comparison contains
forty fresh captures and twenty retained WC3/Wine references. All sixty full-size
RGB pairwise comparisons still differ; no pixels are excluded or rescaled and no
comparison tolerance is applied. The version-text footprint remains unqualified.
See `.scratch/menu-notice-scope-root-20261019/comparison-1/index.html`.

The interrupted original runtime probe is now confirmed stopped, with exit 137
after the owner's bounded stop request. Its files are retained, but it has no
accepted trace. No replacement original-game probe was launched.

[Contextual settings guidance](original-options-defaults.md#contextual-menu-guidance-2026-10-19)
removes passive status text from idle views while keeping operational failures
visible. The live browser hover/unhover check separately confirms that resolution
guidance is still available. Native preparation uses an isolated fixture working
directory to avoid an unrelated workspace intro fallback. Earlier invalid and
partial preparation observations are retained separately from accepted journal
captures. Normal source movie behavior is unchanged.

Light-ray/material appearance, particle and animation phase, settings consumers
and initialization, cursor/font/render details, native Windows behavior and
original/native/WASM deterministic lockstep remain open. The public site has not
been updated by these local captures or packages. No full-menu fidelity or
original-baseline gate closure is claimed.

The same qualified client reader now also supplies the PRE2 appearance operands
to live scene preparation. These are read from image data and the reviewed
packing instruction, then validated before use. They cover biased phase and
packed color/opacity interpolation, not lighting attenuation or a replacement
blend mode. See [particle appearance evidence](menu-particles.md#source-bound-color-opacity-size-and-atlas-phase-2026-10-19).
