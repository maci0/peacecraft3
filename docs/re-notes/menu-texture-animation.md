# Authored menu texture-animation bindings

This is local-asset and reconstruction-source evidence. First-layer translation
is now implemented as described below. It does not establish native UV matrix
conventions or explain the screenshot's streaks or lower black region.

## Inputs and reproduction

The local version-800 model files have these identities:

| Model | Bytes | SHA-256 |
| --- | ---: | --- |
| RoC served `MainMenu3d.mdx`, compat archive member | 2,142,419 | `cc2f5bc76ce3d0d456bf193338ed02ffdbebbf6271fbbefe630d7f954e89bf6a` |
| TFT `MainMenu3D_Exp.mdx` | 798,240 | `15586ffd9dc25b82a4f8ce2e679581f6e14cbcdc0916c7422d89308f010f6293` |

The TFT digest matches the explicitly selected expansion member in the local
115-member browser fixture. Archive adjacency and filenames do not identify
the archive's game version or native lookup priority.

The RoC digest above identifies `web/assets/menu3d/mainmenu3d.mdx`, not the
measured Legacy `War3.mpq` member. That member has the same byte length but
SHA-256 `98351d0e6f79023832d94959c91fef540f6c59692118544770fbcd4422669e64`;
133,847 bytes differ. The entire served file matches the named member of
`wc3/game/app/war3.mpq` (container SHA-256
`95f1c599e083dd1970b2d972fb2e8b9c5a050b3318f0c024c389207d660e63fe`).
The two archive members have identical chunk framing; differing bytes are
confined to BONE, HELP and PRE2. This proves byte identity, not release attribution,
native source priority or the staging command. RoC observations below apply to
the compat member, independently of the distinct Legacy sampling fixture.
The local comparison is recorded in `.scratch/roc-model-provenance/report.md`.

The local probe uses the existing checked `pcx-mdl` readers for materials,
textures, geosets, TXAN tracks and raw GLBS durations. Its source and output
are `.scratch/menu-uv-contract/src/main.rs` and
`.scratch/menu-uv-contract/observations.log`. It was linked against an existing
private-target library and run with warnings denied; no renderer source or
shared build artifacts changed. Original bytes stay local.

## Stored relationships

All indices below are zero-based and describe these inputs, not runtime constants.

| Input | Geoset/material/texture relationship | Texture animation |
| --- | --- | --- |
| TFT | geoset 14, 25 vertices → material 12, layer 0 → TEXS 16 `SKY2.blp` | TXAN 0, `KTAT`, linear, global sequence 20 |
| RoC | material 3, layer 0 → TEXS 3 `cloudstile2.blp` | TXAN 0, `KTAT`, Bézier, global sequence 0 |
| RoC | material 27, layer 0 → TEXS 3 `cloudstile2.blp` | TXAN 1, `KTAT`, linear, global sequence 20 |
| RoC | geoset 29 → material 29 → TEXS 30 `white32.blp` | `NO_ID`; this layer has no TXAN |

The TFT sky uses coordinate set 0, TEXS wrap flags 3, filter mode 0 and
shading flags 1. Global sequence 20 has duration **999,033 ms**, independent
of the menu's shorter Stand loop. Its translation track has two authored keys:
frame 0 at `(0,0,0)`, and frame 999,033 at approximately
`(-16,0.00700456,-5.00007)`. These are three stored components, not a chosen
two-dimensional UV offset.

RoC's global sequence 0 is **144,967 ms** and has two Bézier translation keys;
global sequence 20 is **999,033 ms** and has two linear translation keys.
The probe preserves values and tangents through the decoder rather than
replacing either clock with the scene-loop duration.

## First-layer translation implementation

`pcx_gfx::mdx_uv_translation::UvTranslationPlan` consumes the checked runtime
plan, selected source interval and actual UV-set metadata. The existing MDX
mesh walk now retains each UVAS count and validates its UVBS records. No UV
declaration stays unknown; an authored zero stays zero. General runtime/rig
parsing still accepts an otherwise valid mesh without UV metadata. Only the UV
binding consumer refuses unknown metadata, with source and geoset context.
Typed-only runtime inputs likewise retain unknown counts rather than assuming
one set. The existing last-UVBS projection is unchanged, so the translation
consumer explicitly refuses multiple sets and invalid coordinate references.

The bounded transform is `uv + translation.xy`. This is a reconstruction
convention corroborated by the pinned
[war3-model renderer](https://github.com/4eb0da/war3-model/blob/52d7f421a5505a0434d764294057f2009ce3987b/renderer/modelRenderer.ts)
and [mdx-m3-viewer layer sampling](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/viewer/handlers/mdx/modelinstance.ts).
The stored third component remains preserved; its value is not reassigned to
another UV axis. Rotation, scale, unknown channels and unsupported signed time
domains report errors rather than receiving an invented TRS/pivot convention.
STEP, Linear, Hermite and Bezier translation reuse the existing vector sampler.
Local keys are projected to the selected interval; global keys use independent
u64 elapsed time modulo the authored GLBS duration. The existing endpoint clamp
is an explicit local reconstruction convention. The retained required menu UV
tracks span their global endpoints; generic cyclic-endpoint differences remain
outside this bounded implementation.

Background and panel materials receive their sampled transform at spawn.
Panel Birth/Stand plans retain the continuous global clock. Advance validates
all UV samples and material targets before writing, assigns absolute Bevy
`Affine2` translations, and leaves unchanged transforms untouched. It neither
accumulates offsets nor rewrites mesh UVs. TEXS identities, samplers, KMTF
selection, alpha, shading and culling retain their existing behavior. This is
a model-local UV transaction, not a new all-menu/all-property transaction.

## Verification and remaining fidelity

The complete affected native run passes 460 tests with 24 explicitly ignored
input-dependent cases, including 184 application tests. New actual Bevy
AssetEvents/background-system tests cover initial transforms, independent global
time across local wrap, panel phase selection, unchanged-frame behavior,
preservation of unrelated material state and refusal before partial writes.
Strict native and wasm32 Clippy pass across the application, graphics and model
crates, all targets/features, with dependency diagnostics excluded.

The explicit retained-model gate admits seven source-qualified models covering
310 geosets, using parser-derived metadata and checking exact key/wrap values.
Both original menu backgrounds also pass the helper-parent import gate with
real decoded textures; six retained models pass the actual image/material binding
gate. The target Legacy parser corpus remains 3,249 parsed with zero failures.
Sources, exact commands and logs are in `.scratch/menu-uv-root-review/` and
`.scratch/menu-uv-app-root-review/`. These are data and CPU material checks;
release WASM and new browser/rendered acceptance remain pending.

Do not infer native clock origin, live UV orientation, full TRS behavior or sky
projection from these checks. No camera angle, sky vertex, clear color or UV
offset was fitted to a screenshot. Texture motion alone does not repair the
uncovered region, geometry, lighting, particles or panel transition timelines.
