# Campaign unit material culling and lighting

Status: culling is implemented in source and not yet compiled against Bevy. Rendered
acceptance is open. Missing sun inputs are reported as a gap, not replaced.

## Culling

- Layer `shadingFlags` bit `0x10` is `TwoSided`, which is authoritative for each
  layer. The name comes from published format docs (`main-menu-rendering.md`). Its
  use by the original renderer is not observed here.
- `GeosetSource.culls_back_faces: bool` gates back-face culling of layers without
  `TwoSided`. Authored index order is kept.
- The flag is `true` only for a root with positive determinant (see "Positive root
  and mirrors"). `Actor::spawn` checks its `Transform` (`to_matrix().determinant()`)
  and refuses mirrors, so no caller can set `true` for one.

## Caller audit

| Caller | Visual kind | `culls_back_faces` | Source |
|---|---|---|---|
| `campaign_actors` `Actor::spawn` | `Visual::Unit` | `true`, after a determinant check | `transform_for`: `rotation_y(face) · rotation_x(-90°)`, positive scale |
| `campaign_actors` `Actor::spawn` | `Visual::Item` | `true`, after a determinant check | item transform: `rotation_x(-90°)`, positive scale |
| `mainmenu_scene::spawn` | scenery (menu background) | `false` | unchanged `None` cull |
| `menu_sprites` (2 sites) | sprites and panels | `false` | unchanged `None` cull |
| destructable | none | no path in `campaign_actors` | n/a |
| `campaign_sky` through `Actor::spawn` | sky | `true`, after the same determinant check | `Visual::Sky` transform |
| `campaign_score_model` | score background | `false` | shared scenery path |

Units and items both reach `Actor::spawn`. The determinant check is per root
transform, not per `Actor`. The sky shares that check; score art keeps the scenery path without back-face culling.

## Positive root and mirrors

- A positive-determinant root preserves handedness. It does not prove that every
  authored normal agrees with its winding, and it does not prove the renderer
  compensates mirrors.
- Bevy 0.20 (`bevy_pbr-0.20.0`, `src/render/mesh.rs` near line 788) sets
  `SIGN_DETERMINANT_MODEL_3X3` for positive determinants. The shader
  (`pbr_functions.wesl`, `winding_corrected_front_facing`) uses that bit to correct
  only logical front facing, for double-sided normals and normal maps. The
  pipeline does not flip `cull_mode` or `front_face` for negative determinants.
  The rasterizer still culls by screen winding, which a mirror reverses.
- A negative root would therefore cull its visible side. Disabling culling for it
  would turn one-sided layers into two-sided ones, and flipping the cull would be a
  new policy with no source evidence. `Actor::spawn` refuses a nonpositive
  determinant with `unsupported mirrored actor transform`. Unit and item roots
  never reach that error (rotations are proper, scale is positive), so the check
  documents the boundary rather than handling a live case.
- Front face: Bevy's mesh pipeline sets `cull_mode: Some(Face::Back)` (line near
  3774). It leaves `front_face` at the wgpu default, counter-clockwise (`wgpu-types`
  `FrontFace`, `#[default]` on `Ccw`). The menu material overrides neither.

## Skinning and bone scale

- Skinned positions use `pcx_mdl::compose`, which computes `parent · T(pivot)·T(t)·R·S·T(-pivot)`.
  `S` comes from the sampled bone scaling track without a sign check. A negative
  determinant would reverse a bone's winding. Nothing in this path refuses or
  corrects it. Only the root transform is checked.
- Legacy corpus, probe `.scratch/original_geoset_sign_probe.rs.txt`: 66,809 bone
  scaling keys, none with a negative component. This is a measurement of this
  corpus. It does not prove the native engine has no negative bone scale.

## Gates

Committed, ignored, explicit install (`PEACECRAFT3_FORMAT_INSTALL`):

- `crates/pcx-mdl/tests/original_geoset_index_bounds.rs`
  (`original_geoset_index_bounds_and_footman_presence`). It asserts Footman is present
  and that every triangle index is inside its geoset's vertex array. It does not
  assert winding, sign counts, or any percentage.

Local probes (print only, not gates, outputs under `.scratch/`):

- `original_geoset_sign_probe.rs.txt`, log `w07-sign-probe.log`: 6,563 one-sided
  geosets; 678 with both signs; 1 area-clockwise (`UI\Feedback\Confirmation\Confirmation.mdx`
  geoset 0, 24 and 24 triangles); 4,752 minority triangles of 554,734; 455 unit
  one-sided geosets with both signs; no negative bone scale key.
- `mixed_sample_probe.rs.txt`, log `w07-sample.log`: sample identities. `DruidoftheTalon.mdx`
  geoset 7 (Transparent, `filter 1`, `shading 0`): 151 triangles, 95 counter-clockwise,
  56 clockwise, e.g. triangle 8 `[11, 12, 10]` clockwise. Confirmation geoset 0: 48
  triangles, 24 and 24.

The earlier "single clockwise Druid geoset" came from an area-weighted sum over all
Druid geosets, including two-sided ones. It is not one-sided, so culling does not apply.

## Mixed triangles

Minority-sign triangles are kept as authored. Under this rule they are culled on
their normal side and visible from the back. The effect in the original is not
measured.

## Sun and day inputs

`project` sets `last_error` when the selected unit day/night state or day fraction
is missing. Lit geosets then keep Bevy PBR shading with no source sun. This is a
gap. It is not original fidelity, and the original's behavior for those inputs is
unknown here.

## Render gates, prepared not run

These need the root Bevy slot. Both use the explicit Legacy install.

1. Native compile and tests:
   `CARGO_BUILD_JOBS=1 cargo test -p peacecraft3 --locked --offline -- campaign_culling_keeps_two_sided_layers_and_scenery_unculled`
   and `cargo clippy -p pcx-gfx -p peacecraft3 --locked --offline -- -D warnings`.
2. WASM compile: `cargo build --locked --release -p peacecraft3 --target wasm32-unknown-unknown`,
   then the `wasm-bindgen` step from `docs/pages-demo.md`.
3. Rule-consistency render check (native): for each sample in `w07-sample.log`, the
   draw should keep a triangle whose sign is counter-clockwise, and cull one that is
   clockwise, when viewed from the normal side. This checks the implementation of the
   rule, not original fidelity.
4. Original fidelity render check: capture the same unit from the original and PC3
   at a fixed camera. Compare the coverage at the sample triangles only. Do not fit
   the expected output to a screenshot.

## Pending

- Items 1 and 2 above, not yet built.
- The rendered fidelity of minority triangles, `Unfogged` (`0x20`) and `TwoSided` (`0x10`)
  on units, and the opaque, cutout and blend pass order.
- The sun-less fallback (gap, above).

## Sample plan: source and menu comparison

Identities come from `.scratch/w07-sample.log` (local). Each line is one triangle:
model, geoset, triangle index, its three indices, and the sign of
`cross(b - a, c - a) . (na + nb + nc)`. The sample covers 199 triangles: 151
from `Units\NightElf\DruidOfTheTalon\DruidoftheTalon.mdx` geoset 7 (one Transparent
layer, `filter 1`, `shading 0x00`; 95 counter-clockwise, 56 clockwise) and 48 from `UI\Feedback\Confirmation\Confirmation.mdx` geoset 0
(one layer, `filter 1`, `shading 0x21`; 24 and 24). Overall 119 counter-clockwise and
80 clockwise.

Checks, each separate from the others:

1. Source reproducibility (no render): recompute every sample sign from the install
   and require the table. This is an identity check on the source, not a render.
2. Rule consistency (native render, later): a one-sided layer draws a sample triangle
   from its normal side only when the sign is counter-clockwise. Expected coverage is
   derived from the sign, not fitted to a capture.
3. Menu regression (native render, later): menu scenery and sprites keep `cull_mode:
   None`. Their captures before and after this change must match, because the flag is
   `false` there. This is a no-change check, not a fidelity check.
4. Original fidelity (later): compare PC3 and the original capture at the sample
   triangles' projected coverage. Report the result. Do not tune the expected output.

Check 1 and the index gate are source-only and need no Bevy build. Checks 2 to 4 need
the native and WASM render gates above.
