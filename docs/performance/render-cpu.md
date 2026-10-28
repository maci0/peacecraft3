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

## Results

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

### Fidelity

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

## Hotspots and fixes

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

### Tests

New: `extents_outside_the_inflated_view_are_culled_and_degenerate_ones_never`,
`update_key_tracks_only_the_inputs_the_outputs_depend_on` (`campaign_actors.rs`),
`view_pass_matches_ordering_every_owner_including_absent_ones` (`menu_particle_order.rs`, compares
`order_view` against the former all-owners loop, including a refused budget),
`a_failed_projection_keeps_the_last_alpha_and_buffers_are_reused` (`campaign_visibility.rs`).
Changed: the minimap fog test reads the composed buffer; the unit-fade tests call
`with_unit_opacity`. `cargo test -p peacecraft3 --bin peacecraft3 --features test-fixtures`
772 passed, 92 ignored; `cargo test -p pcx-gfx` passes; native and WASM clippy with
`-D warnings` pass.

## Remaining items

Candidate span table for `campaign_idle` (same workload):

- `prepare_clusters_for_gpu_clustering` 583MB (2.2MB per frame) and `CommandEncoder::finish`
  633MB: Bevy internals; not touched.
- `campaign_actors::project` 172MB and 483k allocs (1.9k per frame): per-unit work that is
  not yet cached: `team_color` and `transform_for` read `teamColor` and `moveHeight` through
  `campaign_actors::scalar`, which scans every field of the definition; the live-handle set;
  water and look-at lookups.
- `advance_doodad_particles` 76MB and 67k allocs; `unit_panel::project` 1.5k allocs per frame;
  menu `position_screen_nodes` 946 allocs per frame; menu scene `advance` 700 allocs per
  frame (CPU skinning of the animated menu model).
- `campaign_minimap::project` still samples about 65k vision cells per frame when fog is on.
  It is the largest report-only elapsed time of our systems (about 2.2 ms per frame on the
  software-Vulkan host). Skipping it needs a vision-state revision from `ObjectHost`.
- Actor culling uses a model-extent sphere with a fixed factor. Sequence bounds from the
  model would be tighter; a geoset animated past twice its rest extents while its model
  stays off screen would appear one update late.

### Hand-offs outside this area

- `pcx-play` (`ObjectHost`): no vision or unit-position revision counter, so the minimap
  re-samples the full vision field every frame.
- `pcx-sim`/`pcx-objects` `Definition`: no column index; `campaign_actors::scalar` is a linear
  scan with string compares, called per unit per frame.
- Bevy cluster buffers are reallocated every frame (`prepare_clusters_for_gpu_clustering`).

### GPU-side suspects (report-only, not judged here)

The scene draws about 1,930 mesh entities (one per geoset) of which about 100 are visible,
with 1,631 images and 22k mesh uploads per 256 updates after the fixes (before: 285k). CPU
skinning with a vertex-buffer re-upload for every visible animated geoset, one material bind
group per geoset, and per-actor transparent sorting are the likely GPU-side costs. The
lavapipe rasterizer (about 100G instructions per 256 updates per thread) says nothing about
a real GPU; measure these on the RX 7900 XTX with the [GPU capture route](../profiling.md).
