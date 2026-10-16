# Authored menu opacity and geoset colour

The menu renderer now binds GEOA through its explicit `geoset_id`, rather than
assuming that GEOA record order matches GEOS order. The native binary/keyword
reader evidence establishing this field is in `format-mdx-bindings.md`; the
checked layer alpha offset and KMTA framing are in `format-mdx-materials.md`.

`pcx_gfx::mdx_opacity::OpacityPlan` retains a first-layer and geoset channel for
each GEOS ordinal. Local scalar keys are restricted to the selected authored
sequence interval; an absent local curve holds the authored static alpha.
Global curves use their referenced GLBS duration and the existing elapsed clock.
Track size, order, finite values/tangents and global references are checked.
Duplicate channels/records and unestablished KGAA semantics are refused.

The checked import also refuses duplicate alpha-key timestamps before restricting
local keys to the selected sequence. This includes equal values, all four stored
interpolation modes, and duplicates outside that sequence. The diagnostic retains
the geoset ordinal, track tag and frame; the menu scene adds its input source path.
This is a checked-import policy because precedence is unverified, not a claim that
the native engine rejects these tracks. Raw runtime tracks remain available and
unchanged. No keys are sorted, merged or assigned a precedence.

The material's alpha is the product of its sampled KMTA/static alpha and the
geoset's sampled KGAO/static alpha. Missing GEOA contributes multiplicative
identity. Zero opacity hides that scene entity. The material handle and entity
are retained alongside the authored geoset ID, leaving unrelated materials intact.
The same plan now retains authored GEOA static colour and KGAC keys by geoset ID.
Scalar and colour curves share validation, sequence clipping and global clocks.
Colour sampling supports the stored interpolation modes with width-aware tangent
offsets. Missing GEOA contributes white RGB. Malformed or ambiguous colour keys
refuse before clipping, and a nonfinite interpolation result is a contextual
error. Finite source values are preserved without clamping.

The material mapping follows a pinned reconstruction-source profile, Warsmash
`f9e0aeed4be372d6016519d0e97b384aa873f374`:
[GeosetAnimation](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/mdx/GeosetAnimation.java)
reverses the static components, while
[MdxComplexInstance](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/mdx/MdxComplexInstance.java)
samples active KGAC without that reversal. The
[vertex shader](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/mdx/MdxShaders.java)
applies BGRA. Together, static file components pass through to material RGB;
active KGAC swaps the first and third components. The tint path in this reference
does not gate on GEOA flags. This establishes the selected source profile, not
native Warcraft III flag or colour-space semantics.

Background and panel spawn/advance apply that RGB beside opacity using the same
selected frame and elapsed clock. Updates validate every target and sample before
writing, preserve alpha, and leave unrelated materials alone. Additional material
layers and complete native blend behavior remain open.

The independent [MDX viewer geoset-animation implementation](https://github.com/flowtsohg/mdx-m3-viewer/blob/master/src/viewer/handlers/mdx/geosetanimation.ts)
also identifies KGAO as alpha and keeps its static value when no curve applies.
Its [batch update](https://github.com/flowtsohg/mdx-m3-viewer/blob/master/src/viewer/handlers/mdx/modelinstance.ts)
separates geoset and layer channels. This is interoperability corroboration,
not native game runtime evidence.

Synthetic typed-plan regressions distinguish sparse GEOA IDs, unbound records,
local keys from another sequence, independent global clocks and malformed or
ambiguous tracks. A real ECS regression exercises the scene material/visibility
update, including zero opacity and an unrelated material. Both supplied original
menu models pass the explicit local import fixture.

Five public imported-MDX test groups exercise the duplicate-time policy and valid
sampler preservation on native and actual 32-bit WASM execution. An explicit local
TFT panel gate runs the production spawn path, constructing both Birth and Stand
opacity plans and publishing nonempty geosets. This does not validate panel sound
timing; the separate first-frame sound assertion remains unresolved.

The colour integration passed six typed-plan regressions, the five existing
duplicate-time groups, seven runtime-plan regressions and a real ECS material
update regression. The six colour, five duplicate-time and seven runtime cases
also executed under Wasmtime using the exact production math/validation modules;
that leaf excludes Bevy rendering. The four-mode colour sampler executed separately
under WASI. The affected application suite passed 419 tests with 15 explicit
original-input gates ignored. Native and browser-WASM application builds, strict
native/WASM Clippy and rustdoc passed. Both supplied compatibility-install menu
models passed the explicit local import gate. These are import and execution
checks; no new colour render readback or paired original-game capture was run.

Native interpolation/clipping behavior, global clock phase and complete material
composition remain fidelity gates. Neither these tests nor a screenshot certify
native renderer parity. Browser runtime evidence is recorded separately.
