# Checked MDX runtime plan

`pcx-gfx/src/mdx_runtime.rs` is a data/runtime primitive independent of Bevy
components. `RuntimePlan::parse` uses the existing MDX decoders, then checks the
joint hierarchy and binding references. `RuntimePlan::checked` accepts typed
inputs for synthetic fixtures. Nothing here spawns a scene or draws extra layers.

`JointPlan::checked` includes every BONE and HELP record, preserving names,
object IDs, parents, flags, pivots and keyed transforms. Helpers are adapted to
the decoder's Bone sampling representation without replacing their authored
tracks. Output order is all BONE records followed by HELP records; object IDs
remain explicit and may be sparse. The plan rejects duplicate IDs, missing
BONE/HELP parents, invalid parent sentinels, cycles, missing/nonfinite pivots,
malformed transform tracks and nonfinite key components. A parent in an
unsupported node family is a missing-parent error, not an identity transform.

`sample_decoder_frame` reuses the existing decoder sampler and matrix composer
with the complete checked joint list. The graph cannot silently truncate through
a helper or fall back to rest on a missing parent/cycle. This operation follows
the decoder convention; it does not establish native quaternion interpolation,
inheritance/billboard flags, global-sequence clocks or sequence clipping. Authored
flags and global references remain available. Structural root transforms and
sentinels are format/transform identities, not measured asset defaults.

Each `GeosetBinding` carries its GEOS ordinal, material reference and matrix
joint IDs. GEOA records bind through their actual `geoset_id`; NO_ID records are
retained separately as unbound. Sparse subsets and multiple records targeting a
geoset remain intact rather than being paired by record order. Materials retain
priority, flags and every `checked_layers()` result, including authored alpha,
KMTA/KMTF tracks and texture-animation references. TEXS, TXAN and GLBS records
remain data. Static/keyed texture, material, geoset, matrix-joint and global
sequence references are checked against their actual source tables. No shader
threshold, blend implementation or native rendering parity is inferred.

Parsing provenance and native field evidence are recorded in
[the binding note](format-mdx-bindings.md) and
[the material note](format-mdx-materials.md). These establish the source fields;
the runtime tests below establish the new data wiring, not rendered behavior.

## Discriminating verification

Four synthetic leaf tests passed under `rustc --edition 2024 --test -D warnings`,
linked only to already-built isolated pcx-mdl, pcx-mpq and thiserror artifacts.
No Bevy/shared build ran. The animated-helper test puts a sparse-ID bone under
an animated HELP parent: its midpoint translation follows the helper, whereas
dropping that parent leaves the bone at zero. Other cases exercise graph errors,
track truncation, sparse GEOA binding, NO_ID, all material layers, authored alpha
and texture tracks, and invalid matrix/material/texture/geoset references.

The original model test is explicitly ignored unless
`PEACECRAFT3_MDX_INSTALL`, `PEACECRAFT3_MDX_MODEL` and
`PEACECRAFT3_MDX_SOURCE` are supplied. It fails on extraction/plan errors and
requires actual mesh bindings. It reads originals in memory without copying
model contents into the repository. Local source identity was supplied as
`Legacy target executable VERSIONINFO 1.29.2.9231; archive adjacency not version
proof`.

Both original menu models passed:

| Archive member | BONE | HELP | Bones directly under HELP | Retained HELP tracks | GEOS | GEOA | Layers | TXAN |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| UI/Glues/MainMenu/MainMenu3d/MainMenu3d.mdx | 374 | 11 | 14 | 22 | 33 | 12 | 31 | 2 |
| UI/Glues/MainMenu/MainMenu3d_exp/MainMenu3d_exp.mdx | 163 | 10 | 13 | 11 | 18 | 8 | 14 | 1 |

These are observations, not parser constants or corpus-wide acceptance claims.
The existing pcx-gfx library/viewer and full native/WASM integration were not
built or changed by this slice. Root owns module declaration and integration.
After that wiring, the targeted Cargo test is:

```sh
cargo test --locked -p pcx-gfx --test mdx_runtime

PEACECRAFT3_MDX_INSTALL="$install" \
PEACECRAFT3_MDX_MODEL="$archive_member" \
PEACECRAFT3_MDX_SOURCE="$explicit_source_identity" \
cargo test --locked -p pcx-gfx --test mdx_runtime -- --ignored --nocapture
```

## Viewer vertex-count binding defect

The current viewer constructs `rest_by_vertex_count` by collecting all loaded
meshes into a HashMap keyed by position count. Later skinning chooses rest data
by the spawned mesh's vertex count. Distinct geosets with the same count overwrite
one another. The original models demonstrate actual data differences:

| Model | Vertex count | Distinct GEOS ordinals | Rest positions differ | Skin bindings differ |
|---|---:|---|---|---|
| MainMenu3d | 25 | 13, 27, 29 | yes | no |
| MainMenu3d | 258 | 5, 6 | yes | yes |
| MainMenu3d_exp | 4 | 12, 13 | yes | no |

`RuntimePlan::duplicate_vertex_counts()` reports those collisions from the
current asset, with no baked-in measured counts. Existing `LoadedMesh.geoset`
already retains the source ordinal. The next caller fix must carry that geoset
identity on each spawned mesh/entity together with its model instance, retain
rest meshes by geoset ordinal, and select rest data through that marker during
skinning. Root owns that viewer/scene change; this primitive does not claim it
has already repaired the current rendering path.
