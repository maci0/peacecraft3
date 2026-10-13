# Authored menu opacity

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

The material's alpha is the product of its sampled KMTA/static alpha and the
geoset's sampled KGAO/static alpha. Missing GEOA contributes multiplicative
identity. Zero opacity hides that scene entity. The material handle and entity
are retained alongside the authored geoset ID, leaving unrelated materials intact.
RGB geoset animation, additional material layers and exact blend behavior remain
unimplemented.

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

Native interpolation/clipping behavior, global clock phase and complete material
composition remain fidelity gates. Neither these tests nor a screenshot certify
native renderer parity. Browser runtime evidence is recorded separately.
