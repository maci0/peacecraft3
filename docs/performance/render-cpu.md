# Render and HUD CPU work

Fixed-work counters for the per-frame render and HUD systems of the mounted Prologue02
campaign scene and the main menu. Counts and instruction totals gate regressions; elapsed
time, FPS and rasterizer work are report-only (see [performance testing](../performance-testing.md)).
Inputs are the local Legacy install (not committed); nothing from the assets is copied here.

## Workload `render-cpu-v1`

The ignored GPU fixture `campaign_entry_gpu_tests::mounted_campaign_render_cpu_workload`
(`crates/peacecraft3/src/campaign_entry_gpu_tests/bench.rs`) drives the production app
through launcher, main menu, campaign selection and Prologue02 (RoC, campaign seed 1,
manual 1/64 s step, offscreen software Vulkan, no window, no audio). Pipelined rendering is
disabled so extraction and render-graph preparation run inside `App::update`. Each phase
runs 256 `App::update` calls after 12 settling updates:

| Phase | Work |
| --- | --- |
| `menu` | main menu scene idle |
| `campaign_idle` | control returned, one hero selected, no input |
| `campaign_hud` | every 32 updates a timed text message or a selection change |
| `campaign_motion` | the selected unit receives one move order |

Per phase and per frame (`<phase>-frames.json`) the summary records allocation calls,
reallocation calls and requested bytes inside `App::update` (the test binary's counting
allocator, `counting_allocator.rs`, shared with the simulation bench), entities spawned and
despawned, systems that ran (main and render world, by change tick), asset added, modified
and removed events with mesh vertices and image bytes written for Mesh, Image, StandardMaterial,
MenuMaterial, ParticleMaterial and the UI/ground materials, and live entity, render-world
entity, visible mesh and UI node counts. A SHA-256 of the captured RGBA frame ends each phase.

Round 2 adds, per frame, the renderer submission counters from the vendored `bevy_render`
(`bevy::render::render_stats`, see `vendor/BEVY_PATCHES.md`): draw calls, `multi_draw_*` calls,
pipelines and bind groups actually set, and bind groups, buffers (with requested bytes) and
textures created. Mesh payloads are counted in bytes as well as vertices (vertex plus index
buffer bytes of each added or modified mesh, the bytes the mesh allocator copies). Modified
meshes are split into those whose entity was in view that frame and those that were not
(`modified_unseen`, with `_hidden` for `Visibility::Hidden` and `_culled` for inherited-visible
but rejected by the frustum). Visibility and entity counts add hidden mesh entities, distinct
visible meshes and materials, and the render world's mesh and material instance tables.
Two further intervals, `enter_menu` (launcher action to the settled main menu) and
`enter_campaign` (mission button to the started campaign), write `entry-<name>.json` with
allocation, draw, bind-group, buffer and texture counters. Their update counts depend on
asset-load timing, so they are reports and carry no budget.

With `profile-cpu` and `PCX_SPAN_PROFILE=1` every Bevy system span is charged the allocation
calls and bytes its own thread made inside it, excluding child spans (`bench/spans.rs`).

### Running it

```sh
# build once (profiling profile, thin-LTO off for compile time)
CARGO_PROFILE_PROFILING_LTO=off CARGO_PROFILE_PROFILING_CODEGEN_UNITS=16 \
  cargo test -p peacecraft3 --bin peacecraft3 --features test-fixtures \
  --profile profiling --no-run --locked --offline
# run: headless env of the other GPU fixtures (fresh XDG_DATA_HOME, null ALSA, lavapipe ICD),
# PCX_NATIVE_CHAPTER_MAP='Maps\Campaign\Prologue02.w3m', PCX_NATIVE_CHAPTER_ADDON=roc,
# a new PCX_CAMPAIGN_CAPTURE_DIR, then
taskset -c 0-3 <test-binary> campaign_entry_gpu_tests::mounted_campaign_render_cpu_workload \
  --ignored --exact --nocapture --test-threads=1
```

`PCX_PERF_PHASE=<phase> PCX_PERF_CTL=<fifo> PCX_PERF_ACK=<fifo> PCX_PERF_OUT=<csv>` makes the
test attach `perf stat --per-thread -e instructions:u` to itself for that phase only
(`--control` enable and disable around the 256 updates). Sum the rows by thread name: the
`llvmpipe-N` rows are the software rasterizer, everything else is CPU-side work. Add
`PCX_SPAN_PROFILE=1` to a `--features test-fixtures,profile-cpu` build for the span table.
`PCX_PERF_RECORD=<N>` records an instruction sample every N user instructions instead (`perf
record -e instructions:u`, flat profile) into
the file named by `PCX_PERF_OUT`, for the same phase names plus `enter_menu` and `enter_campaign`;
read it with `perf report --sort comm,symbol`. Every run also writes `render-budget-observed.json`
(see [Budgets](#budgets)). The perf window covers the whole phase, including the harness's own
per-frame bookkeeping (entity-set diff, system ticks), which is the same in both columns of every
comparison here; in the `campaign_idle` sample profile it is roughly 5% of the CPU-side total.

## Round 1 results

Same workload, seed, step, CPU set (4 CPUs), compiler and profile for both columns. Baseline is
the tree at the start of this work plus the harness; candidate is the tree after the fixes
below. Other agents changed `pcx-*` crates between the two builds (not isolated). Counts are
over 256 updates. "CPU-side instr" is `instructions:u` summed over all threads except
`llvmpipe-N`; the rasterizer total (about 400G per phase, 800G for the menu) is unchanged by
these fixes and excluded.

| Phase | CPU-side instr | Allocs + reallocs | Requested bytes | Mesh modified | MenuMaterial modified | Image modified |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| menu | 63.6G to 22.3G | 7.22M to 3.36M | 7,051MB to 1,625MB | 13,447 to 13,311 | 87,292 to 8,683 | 0 to 0 |
| campaign_idle | 158.9G to 31.2G | 38.67M to 3.36M | 14,476MB to 2,081MB | 285,151 to 22,528 | 1,338,624 to 12,032 | 579 to 67 |
| campaign_hud | 161.9G to 31.7G | 38.68M to 3.36M | 14,477MB to 2,081MB | 285,151 to 22,528 | 1,338,624 to 12,032 | 522 to 10 |
| campaign_motion | 163.6G to 31.8G | 38.69M to 3.37M | 14,477MB to 2,104MB | 285,256 to 22,631 | 1,338,624 to 12,032 | 512 to 101 |

Entities spawned and despawned are identical per phase (32/0, 62/58, 135/139 in the campaign
phases). Repeating the baseline idle phase gives 158.65G and 158.9G (0.2% apart); counts
repeat exactly.

Per frame, campaign idle: about 620M CPU-side instructions and 151k allocations (56MB)
before, 122M and 13k (8MB) after. Mesh modifications fall from 1,114 to 88 per frame (the
visible animated actors), material modifications from 5,229 to 47.

### Round 1 fidelity

- `campaign_idle` and `campaign_hud` final frames have identical SHA-256 before and after
  (first 12 hex digits `b7cd768f6798` and `7ac88e296fd3`) in every run, and a pixel diff of 0.
- `campaign_motion`: the candidate frame equals an earlier baseline run exactly (0 pixels).
  A second baseline run ended one simulation tick later (game time 15.03125 s against
  15.015625 s) and differs from both by 32 pixels, so this phase's hash is not reproducible
  on the baseline either.
- `menu`: the baseline differs from itself between runs by 0.9% to 1.3% of pixels (the menu
  animation phase at capture depends on asset-load timing). Candidate against baseline is
  0.7% to 0.8%, inside that spread. Menu fidelity therefore rests on the unit tests and
  the unchanged logic below, not on a hash.
- The existing fixtures `mounted_campaign_rendering_state` and
  `mounted_campaign_hud_and_research_states` pass on the candidate.
  `mounted_menu_enters_the_rendered_campaign` fails at "diagnostic chapter needs a visible
  ground item" on the baseline and the candidate alike (Prologue02 has no visible item).

## Round 1 hotspots and fixes

Evidence is the baseline span table for `campaign_idle` (bytes and allocation calls over 256
updates, from `PCX_SPAN_PROFILE`; elapsed times are report-only and not listed).

| Span | Alloc calls | Bytes | Cause | Fix |
| --- | ---: | ---: | --- | --- |
| `prepare_erased_assets<MenuMaterial>`, `Device::create_bind_group` | 7.5M | 1.7GB | every actor rewrote every material each frame (alpha, colour, tint, sun), raising a modified event and a new bind group per geoset | one composed value per geoset, written only when it differs |
| `extract_render_asset<RenderMesh>`, `allocate_and_free_meshes` | 1.6M + 3.7M | 1.4GB | sun lighting re-inserted vertex colours into every lit mesh each frame | colours compared with the mesh's before `get_mut` |
| `campaign_actors::project` | 5.5M | 1.4GB | every doodad, destructable, item and unit re-posed, re-skinned and re-lit each frame, on or off screen; palette and parameters cloned; order tables and camera component rebuilt each frame | see below |
| `menu_settings::environment`, `launch_gui::refresh`, `native_campaign::refresh`, `render_campaign_panel` | 98k each | 1.07GB each | `selected_install` classified the install (opens its archives) on every call, every frame | `Launcher` caches the found install per source and mode |
| `menu_particle_order::order_backgrounds` | 624k | 884MB | one full phase pass (two vectors and a hash map) per actor owner, about 1,900 owners | only owners with an item in the phase are ordered |
| `campaign_minimap::project` | 2.3k | 101MB | fog and markers rewrote the image every frame (two modified events, 400kB of clones, a 65k-pixel alpha vector twice) | composed in a reused buffer, uploaded only on difference; `Raster::project` reuses its buffer |

`Actor::update` (`campaign_actors.rs`) now:

- returns early when the actor's bounding sphere (model extents transformed by its world
  transform, radius inflated by `CULL_RADIUS_FACTOR` = 2) is outside the camera frustum, once
  the actor has been posed. The frustum is built from the scene camera's transform of the
  current update; the frustum component still holds the previous one. The sky is never
  culled. The pose, materials and meshes are pure functions of the update key, so a returning
  actor is brought up to date by one full update. No scene light casts shadows, so stale
  off-screen meshes cannot reach the frame.
- skips every mesh and material write when its `UpdateKey` equals the last full update's:
  sequence interval, frame, each global sequence's phase (`elapsed_ms % duration`, which is
  how tracks sample), tint, look-at target and transform, and the sun with its model-space
  direction for models with lit geosets. Blended poses are never cached.
- composes opacity, colour, tint and unit-fade alpha mode into one material value per geoset
  and writes it only when it changed (`MenuBlend::with_unit_opacity` is the pure form of the
  former in-place call). Visibility is written only on change.
- `apply_sun` compares the new vertex colours with the mesh's and the material with its
  white-lit form before taking `get_mut`.

Menu scene: `apply_opacity` and `apply_geoset_colour` compare before writing (menu material
modifications 87k to 8.7k).

The `UnitActors` state holds `Arc` parameters and palette, and the source-order tables with a
fingerprint of actor roots, particle order tables and geoset counts. Minimap markers cache
the per-unit definition columns they read (`hideOnMinimap`, `isbldg`) by unit and definition
revision.

Behavior that changed on purpose: the launcher's "detected as" label and the settings
display policy no longer re-classify the install while the launcher is hidden or unchanged,
and a found install is cached until the source or mode changes (a missing install is
re-checked every call).

### Round 1 tests

New: `extents_outside_the_inflated_view_are_culled_and_degenerate_ones_never`,
`update_key_tracks_only_the_inputs_the_outputs_depend_on` (`campaign_actors.rs`),
`view_pass_matches_ordering_every_owner_including_absent_ones` (`menu_particle_order.rs`, compares
`order_view` against the former all-owners loop, including a refused budget),
`a_failed_projection_keeps_the_last_alpha_and_buffers_are_reused` (`campaign_visibility.rs`).
Changed: the minimap fog test reads the composed buffer; the unit-fade tests call
`with_unit_opacity`. `cargo test -p peacecraft3 --bin peacecraft3 --features test-fixtures`
772 passed, 92 ignored; `cargo test -p pcx-gfx` passes; native and WASM clippy with
`-D warnings` pass.

## Round 2

Baseline is HEAD `b4a28f5` plus the round 2 harness and counters (binary `r2base`); candidate
is the tree after the changes below (`r2c6`). Same workload, seed, step, CPU set and profile.
Other agents changed `pcx-*` crates while this ran (not isolated). Counts are over 256
updates; "CPU-side instr" is `instructions:u` of all threads except `llvmpipe-N`, including the
harness bookkeeping.

| Phase | CPU-side instr | Allocs + reallocs | Requested bytes | Mesh modified | Mesh upload | Buffers created | Buffer bytes created | Bind groups created |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| menu | 22.00G to 20.50G | 3.03M to 2.88M | 1,619MB to 1,201MB | 13,510 to 13,526 | 301MB to 305MB | 33,889 to 32,406 | 272MB to 1.9MB | 34,188 to 32,172 |
| campaign_idle | 31.22G to 23.11G | 3.25M to 2.87M | 2,057MB to 1,187MB | 22,528 to 18,176 | 122MB to 74MB | 7,238 to 4,934 | 391MB to 0.68MB | 22,342 to 19,270 |
| campaign_hud | 31.69G to 23.58G | 3.24M to 2.86M | 2,055MB to 1,186MB | 22,528 to 18,176 | 123MB to 75MB | 7,144 to 4,840 | 391MB to 0.66MB | 22,252 to 19,180 |
| campaign_motion | 31.76G to 25.66G | 3.24M to 2.87M | 2,078MB to 1,212MB | 22,631 to 18,279 | 123MB to 75MB | 7,153 to 4,849 | 391MB to 0.66MB | 22,358 to 19,286 |

Unchanged by round 2 (submission side): draw calls 15,426 / 18,432 / 18,432 and multi-draw
calls 25,344 / 25,344 / 25,600 (idle / hud / motion), pipelines set 9,060 / 11,520 / 11,520,
bind groups set 46,196 / 51,687 / 51,863, render-world mesh instances 102 to 103. Per frame in
`campaign_idle` that is 60 draws, 99 multi-draws, 35 pipeline and 180 bind group switches for
102 visible mesh entities, with 75 bind groups and 19 buffers created. The scene is not
draw-call bound, and no material or mesh batching change was needed to keep these low (see
below). Allocator totals vary between runs of one binary by under 0.01% (calls) and under 0.1%
(bytes). Every other campaign counter repeated exactly across five runs; the menu phase counters
vary slightly because its animation phase at capture depends on asset-load timing.

Instruction steps in `campaign_idle` (same workload, builds cumulative): 31.22G baseline;
28.26G with hidden-geoset skipping and CPU light clustering (`r2c4`); 23.09G with the minimap
vision revision (`r2c5`); 23.11G with the texture cache (`r2c6`, which only changes scene
entry). The individual shares inside the first two steps were not measured separately.

### Fidelity

- `campaign_idle` and `campaign_hud` final-frame SHA-256 (first 12 hex digits `b7cd768f6798` and
  `7ac88e296fd3`) equal the baseline in all 27 plain-build runs of the candidate builds `r2c1`
  to `r2c6`, and `campaign_motion` (`fb5e432b21e0`) in 26 of 27; the other ended one simulation
  tick later (game time 15.03125 s against 15.015625 s), the known round 1 irreproducibility.
  Each intermediate build (cluster change, hidden skip, minimap skip, texture cache) was hashed
  on its own. Builds with `profile-cpu` render about 5 pixels differently from plain builds on
  the round 1 tree as well, so span runs are not hash-compared.
- `menu` differs between runs of one binary (see round 1), so it has no hash. Its meshes and
  materials go through the same code except the hidden-geoset mask, which the menu scene does not
  pass, and the cluster configuration, which the unchanged campaign hashes cover.
- The fixtures `mounted_campaign_rendering_state` and `mounted_campaign_hud_and_research_states`
  and the unit tests are listed under [Gates](#gates).

### Hotspots and fixes

Evidence: round 1 counters, baseline and candidate span tables for `campaign_idle`
(`PCX_SPAN_PROFILE=1`) and an instruction-sample profile (`PCX_PERF_RECORD`) of the candidate.

| Where | Evidence | Fix |
| --- | --- | --- |
| Bevy GPU clustering | `prepare_clusters_for_gpu_clustering` 583MB and 391MB of buffers created per phase (1.5MB per frame) with no light in any scene | `skip_light_clustering` (`launch_gui.rs`) gives every `Camera3d` `ClusterConfig::None`, and `use_cpu_light_clustering` sets `GlobalClusterSettings::gpu_clustering = None` at startup. `ClusterConfig::None` alone hangs the first capture on the GPU path; `Single` alone works and halves the bytes (cause of the hang not investigated). Buffer bytes created fall to 0.7MB per phase; requested bytes fall 870MB together with the hidden-geoset change |
| `campaign_minimap::update_fog` | sampling 65k vision cells per frame: `GridShape::index_x`, the raster closure, `floor`, `vision_cell`, `vision_presented` were 1.14% of all samples, about 16% of the CPU-side instructions | `FogImage::projected` keeps the inputs of the last accepted projection (`ProjectionInputs`: the host's `vision_revision`, viewer, fog and mask switches, mapped bounds); equal inputs reuse the raster's alpha. A refused projection clears it. Idle drops 5.2G together with the `scalar` change; a moving unit changes the revision every tick, so motion still samples (28.74G to 25.63G in that step) |
| CPU skinning of hidden geosets | 6,656 of 22,528 mesh modifications per phase were geosets with alpha 0 that no frame shows | `Rig::hidden` and `SunGeosets::hidden` leave their vertices and vertex colours alone. `Actor::update` composes opacity, colour and tint before skinning and passes the mask from the second pose on. The first pose skins everything because Bevy takes each mesh's culling bounds from the vertices it holds when the entity first appears. Hidden depends only on the update key, so a geoset that becomes visible forces a full update. Mesh modifications 22,528 to 18,176, mesh upload 122MB to 74MB |
| `Actor::spawn` texture decode | scene entry: 1,519 texture decodes for 79 distinct paths | see [Scene entry](#scene-entry) |

Examined and not changed:

- Material bind groups. A material's bind group is rebuilt only when its asset is modified
  (17.7 per frame in idle, the animated geosets), so sharing identical materials would not
  lower the per-frame count, and animated materials cannot share a handle. The remaining
  bind-group creations are Bevy's per-view groups. One group per geoset costs at spawn only
  (1,883 groups at campaign entry).
- Render entities for hidden geosets. Only visible mesh entities reach the render world
  (`render_mesh_instances` is 102 for 1,928 mesh entities, 423 of them `Visibility::Hidden`),
  so hidden geosets cost main-world visibility work only. Dropping geosets that are never
  visible needs the opacity plan of every sequence, not the current interval's.
- Per-geoset frustum skipping of the 3,584 modified-but-culled meshes per phase. Bevy culls a
  geoset by the bounds of its first pose, which an update could test, but a geoset skipped
  while culled would keep a stale pose when the update key does not change on its return.
  It needs a per-geoset "skinned at key" marker; not built.
- Batching. Draws are already 60 per frame for 102 visible entities with indirect multi-draw.
- GPU skinning would remove the remaining 290KB per frame of mesh uploads but changes the
  float rounding of vertex positions, so the frame hash could not prove equality.
- `campaign_actors::scalar` now looks the column up with `Definition::column_field`. A cache of
  the `teamColor` column per unit and definition revision moved the `ObjectField` scan symbol
  from 0.08% to 0.07% of samples, so it was dropped. The scan that remains in the profile is
  `campaign_unit_panel::profile_name` (see remaining items).

### Scene entry

`enter_campaign` instructions (`instructions:u`, all threads except `llvmpipe-N`; the compute
task pool threads run Bevy systems and their async tasks, including the actor spawns):

| Interval | Compute pool | Main thread | Allocs + reallocs | Requested bytes | Textures created |
| --- | ---: | ---: | ---: | ---: | ---: |
| `enter_campaign` baseline | 82.73G (82.74G) | 0.30G | 9.75M | 7,150MB | 1,487 |
| `enter_campaign` candidate | 26.62G (26.61G) | 0.30G | 3.71M | 3,703MB | 1,486 |
| `enter_menu` baseline | 10.51G | 2.13G | 2.75M | 1,385MB | 74 |
| `enter_menu` candidate | 10.46G | 1.84G | 2.69M | 1,248MB | 72 |

Values in parentheses are a second run. Profile of `enter_campaign` before the change: BLP
JPEG decoding (`jpeg_decoder`, `color_convert_line_cmyk`, `pcx_blp::fill_rgba`) was about half of
the samples. `Actor::spawn` decoded every texture of every actor (a debug print counted 1,519
loads of 79 distinct paths for 127 models), and each load also extracted the file from the
archive. `DecodedTextures` (a resource reached through `DrawAssets`, cleared with the actor state
on a session change, capped at `DECODED_TEXTURE_CACHE_BYTES`) now decodes a path once; each actor
still gets its own copy of the pixels, so budgets and materials are unchanged. The candidate
profile is led by archive decompression (`pcx_mpq::huffman::decompress`, ADPCM), the sound
tables, a `BTreeSet<AssetPath>` walk in `pcx_play::campaign_placement` and SLK parsing, all
outside this area. The textures are still uploaded once per actor (1,486 GPU textures for 79
distinct images); sharing the image handle needs a cache in `GeosetAssets`, which 25 call sites
construct. Model parsing per actor (127 models) does not appear among the top symbols. Textures already carry their authored mips; nothing in
`pcx-gfx` generates them.

`enter_menu` is dominated by archive opening (`pcx_mpq::crypt::decrypt`, `find_adpcm_tables`),
shader preprocessing and driver allocation; nothing there is in this area.

### Budgets

`tools/perf/budgets/render.json` (version 1, workload `render-cpu-v1`) holds one upper bound for
20 counters in each of the four phases: allocator calls and bytes, entities spawned and
despawned, mesh entities, render-world entities and mesh instances, draw, multi-draw, pipeline
and bind-group-set counts, bind groups, buffers (with bytes) and textures created, and mesh,
image and menu-material asset writes (with upload bytes). Instruction counts stay report-only.

Check: run the fixture with `PCX_RENDER_BUDGET=<abs path>/tools/perf/budgets/render.json`. After
the summary is written the test fails and lists every counter above its bound, budgeted but not
measured, or measured without a budget. `render-budget-observed.json` next to each summary holds
the run's counters in the same format. The comparison has unit tests
(`campaign_entry_gpu_tests/bench/budget.rs`).

The bounds are the largest value of six runs. Allocator calls and bytes carry 1% headroom
over that maximum, and every counter of the `menu` phase 5%, because those vary between
identical runs (the menu phase by up to about 2% in the runs made here); every other bound is
exact.

Update (lower only): rerun the fixture at least three times on one binary, then

```sh
python3 tools/perf/render_budget.py run1/render-budget-observed.json run2/... --write
```

The tool takes the maximum per counter, adds the headroom above, refuses to write when any
bound would rise (a rise is a regression to find) and keeps the counter set equal across runs.
Put the before and after counters and the change that lowered them in the commit message.
Raising a bound or adding a counter needs a new workload version and the reason in this page.

### Gates

`cargo test -p peacecraft3 --bin peacecraft3 --features test-fixtures` 780 passed, 92 ignored
(new: `hidden_geosets_keep_their_last_pose_while_drawn_ones_move`,
`hidden_geosets_get_their_material_state_but_no_vertex_colours`,
`a_texture_shared_by_actors_is_decoded_once_and_failures_are_not_kept`,
`a_projection_is_reused_only_when_every_input_is_equal`, and the budget comparison tests).
`cargo test -p pcx-gfx`, clippy with `-D warnings --all-targets` for `pcx-gfx` and for
`peacecraft3 --features test-fixtures`, and WASM clippy
`--lib --bins --features test-fixtures --target wasm32-unknown-unknown` pass. The ignored
fixtures `mounted_campaign_rendering_state` and `mounted_campaign_hud_and_research_states` pass
on the final tree, and the render workload passes the budget file on a fresh run of it (a run
with a bound 1% too low for a menu counter failed with the counter named, which is how the
menu headroom was found).

### GPU observations on the RX 7900 XTX (report-only)

Nothing below was run on a real GPU; the lavapipe numbers here say nothing about it. Steps for
the maintainer, using the existing overlay and [profiling](../profiling.md):

1. Build once: `cargo build --locked --profile profiling -p peacecraft3 --features profile-gpu`.
   `profile-gpu` adds Bevy's render diagnostics and a diagnostics log line per interval; the
   binary is `target/profiling/peacecraft3`.
2. Start it on the real display (`WGPU_BACKEND=vulkan`), enter Prologue02 and wait until
   control returns, so the scene matches the `campaign_idle` phase. Do not move the camera.
3. Press F8 for the frame-time graph and F7 for renderer, scene and device-memory data; both
   report Bevy frame cadence and allocator usage, not draw counts. Let the scene run 60 s with
   F8 open and note the smoothed frame time and the peak buckets.
4. With `profile-gpu` the log carries per-pass GPU elapsed times (`render/<pass>/elapsed_gpu`
   for the opaque, transparent and UI passes). Take the median over at least 100 reports.
5. For per-draw timing, run `python3 tools/profile.py gpu -- target/profiling/peacecraft3`, press
   the same scene, then `python3 tools/profile.py trigger` and open the `.rgp` capture in RGP.
   Do not sum overlapping draw durations into a pass time.
6. Repeat steps 2 to 4 on the commit before and after a change, same session, same camera. Record
   the host clocks and load beside each result; none of these numbers is a pass or fail
   threshold.

What to look at, from the counters here: 290KB of mesh data re-uploaded per frame from CPU
skinning (88 to 71 meshes), 19 buffers and 75 bind groups created per frame, 38 command
encoders per frame (`CommandEncoder::finish` is the largest remaining allocator span), 35
pipeline and 180 bind-group switches, and the transparent pass (`order_backgrounds` reorders
its sorted items every frame).

## Remaining items

Candidate span table for `campaign_idle` (allocation calls and bytes over 256 updates):

- `CommandEncoder::finish` 504k calls and 634MB (38 encoders per frame) and `Device::create_bind_group`
  315k calls: Bevy and wgpu internals.
- `campaign_actors::project` 375k calls and 106MB (1.5k calls per frame); `campaign_unit_panel::project`
  375k calls (`wanted` rebuilds the view strings and scans the definition with `profile_name`
  every frame; a cache needs an audit of every input of `unit_stat_snapshot`);
  `advance_doodad_particles` 67k calls and 76MB (particle state advances with time, so
  off-screen emitters cannot be skipped); menu `mainmenu_scene::advance` 185k calls and 443MB
  (skinning plus `lighting.apply`; the hidden mask is not passed there).
- Bevy visibility (`check_visibility`, about 6% of CPU-side instructions): 1,928 mesh entities
  of which about 1,400 are inherited-visible but off screen. Hiding the geoset entities of a
  culled actor would skip them, and the particle children must stay untouched.
- The unit portrait scene skins every geoset each frame (about 9 hidden-geoset modifications
  per frame remain); it would need the same first-pose rule.
- Actor culling still uses a model-extent sphere with a fixed factor (round 1).
- Lavapipe's driver work on the main thread (`libvulkan_lvp.so`, about 4G per phase) and the
  rasterizer are software-Vulkan costs, not game costs.

### Hand-offs outside this area

- `pcx-play`: `vision_presented` looks the viewer up in a `BTreeMap` for each of the 65k minimap
  pixels when the revision changes (every tick while a unit moves). A per-viewer sampler
  obtained once per projection would remove that lookup, and a changed-cells list would allow a
  partial projection.
- `pcx-play`/`pcx-slq`: a `BTreeSet<AssetPath>` iteration in the campaign placement code is 7% of
  the candidate `enter_campaign` samples; MPQ Huffman and ADPCM decompression is 21%.
- `pcx-blp`: JPEG decode (`color_convert_line_cmyk`, `fill_rgba`) is the next cost whenever a
  texture is first decoded.
