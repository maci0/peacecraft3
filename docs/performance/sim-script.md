# Simulation, script VM and campaign runtime CPU

Gates here are fixed-work counters: user-mode instructions (`perf stat -e instructions:u`),
allocation calls and requested bytes from a counting allocator, and an output digest. Elapsed time
is report-only. Instruction counts are load-independent on this host; they are not portable
across compilers or CPUs. Baseline is the tree at commit 389d316; candidate is the working tree
with the changes below. Every candidate digest and route checkpoint equals its baseline.

## Workloads

| Id | What runs | Counters |
| --- | --- | --- |
| `sim-movement-v1` | `tools/perf/run.py`: 512 units, 4 players, 2,000 ticks through `pcx_sim::step` | allocations, digest `867f3f6327d60281` |
| `sim-script-vm-v1` | `sim_bench script-vm`: 40 rounds of a synthetic JASS program (calls, loops, arrays, string concatenation, real arithmetic, native dispatch through a host) | instructions, allocations, VM globals digest |
| `sim-script-front-end-v1` | `sim_bench script-front-end load\|parse\|resolve`: `common.j`, `Blizzard.j` and the Human03 `war3map.j` (972,601 source bytes) | instructions, allocations, AST/native-call digest |
| `sim-human03-v1` | `sim_bench human03 N`: production `headless_startup::start` on the selected Human03 map (RoC, seed 1), shared vision sampled once as the application runtime does, then N ticks, no orders | instructions, allocations, unit projection + source RNG state + clock digest |
| Human03 / Human04 route | `selected_human03_route_reaches_authored_victory` and `selected_human04_route_reaches_authored_victory` in the `peacecraft3` test binary; with `PCX_SIM_BENCH=1` the Human04 route stops after `PCX_HUMAN04_DECISIONS` decisions (64 ticks each) and prints counters and a state digest | instructions, allocations, route checkpoint lines |

Timer and event delivery are not separate workloads. They run inside the Human03 and Human04
ticks (`advance_timers`, queued events, region polling); the script VM cost they carry is the
`Vm::step` share in the profiles below.

Instruction counts include process setup. The setup-only siblings (`script-vm baseline`,
`script-front-end load`, `human03 0`, `PCX_HUMAN04_DECISIONS=0`) are subtracted to isolate the
measured work. Their allocator counters cover only the measured region.

```sh
# synthetic VM only, no assets needed
flock .scratch/cargo.lock python3 tools/perf/sim_run.py --no-assets

# real-asset modes: the local install and the extracted Human03 map
export PCX_SIM_BENCH_INSTALL='<Warcraft III (Legacy) directory>'
export PCX_SIM_BENCH_MAP='<extracted Human03.w3m>'
python3 tools/perf/sim_run.py

# the same runs, failing when a counter exceeds tools/perf/budgets/sim.json (see Budgets)
python3 tools/perf/sim_run.py --check

# routes (release test binary; scripts in .scratch/perf-sim/ set the isolated profile, muted audio, no display)
cargo test --release -p peacecraft3 --bin peacecraft3 --features test-fixtures --no-run --locked --offline
```

The Human03 map used here has SHA-256 `8345a25e39c4b50b0efd85ff968fc2a7c6c83306534126e19144150b8521d8b8`
(extracted from the selected install). Route runs use the earned-carryover profiles the existing
route harness pins (`PCX_NATIVE_CAMPAIGN_CACHE_SHA256`). Nothing from the install is copied into the
repository.

Profiling used `perf record -e cycles:u` flat profiles (call-graph recording is refused for
unprivileged users here) plus a gdb signal sampler for callers. Valgrind is unusable on this CPU.
Sampled percentages rank work; they are not gates.

## Results

Instructions are `instructions:u`. "Ticks" is total minus the setup-only run on the same binary.
Startup numbers also include changes made concurrently by the asset work (Huffman, SLK) and are
not attributed to the changes below, except where stated.

| Workload | Baseline | Candidate | Digest / checkpoint |
| --- | ---: | ---: | --- |
| `sim-movement-v1` | 46,534,675 (0 allocations) | not touched | `867f3f6327d60281` |
| `sim-script-vm-v1` (40 rounds) | 334,720,305; 793,575 allocations; 58.8 MB | 229,229,329; 398,615 allocations; 37.4 MB | `46ab4f7898e3b995` equal |
| script front end, parse of 3 files | +133.3 M over load; 148,816 allocations | +133.3 M; 148,816 | equal |
| script front end, resolve | +42.9 M over parse | +43.0 M | equal |
| `sim-human03-v1`, startup (0 ticks) | 37,586,811,980; 9,561,153 allocations | 31,303,643,138; 3,233,203 | `a2517f75b75e84cf` equal |
| `sim-human03-v1`, 4,096 ticks | 183,537,268,315 total; ticks 145,950,456,335 | 66,806,982,626 total; ticks 35,503,335,113 (4.1x fewer) | `97c1d17bf808534c` equal |
| Human03 route to victory (17,857 ticks) | 924,108,877,872 | 247,421,767,436 | 36 checkpoints and victory line byte-equal |
| Human04 route, 120 decisions (7,680 ticks) | 248,659,790,229; ticks 212.5 G | 64,848,147,030; ticks 34.0 G | `6bc844b0b15e6f06` equal, RNG state included |
| Human04 route to victory (28,737 ticks) | 821,884,338,240 | 200,201,145,241 | 57 checkpoints and victory line byte-equal |

Tick-phase allocations are unchanged within 2% (Human03: 1,588,521 baseline, 1,613,093 candidate
calls; Human04 route: 3.13 M, 3.19 M). The tick-phase win is instruction count, not allocation
count. The extra calls are the per-sample region lists and the retained sight-radius map.

## Hotspots found and fixes

Ranked by instruction share of the baseline Human03 4,096-tick workload (flat `cycles:u`, callers
from stack samples). Percentages are of the tick phase unless noted.

| Rank | Hotspot | Evidence | Fix |
| --- | --- | --- | --- |
| 1 | Shared vision restamp (`SharedVision::apply`, `ViewerGrid::stamp`): every tick, 4.40 G cell visits in 4,096 ticks (about 1.07 M per tick) because one moving unit reset and restamped every source of its viewer; each cell recomputed its sample coordinates | 38% self in `apply` (stamp inlined); `PCX_SIM_BENCH_DEBUG=1 human03 4096` prints the visit totals | Regional update: when the source list keeps its length, only the cells under changed sources (old and new rectangle) are reset and restamped, with every source clipped to those regions in the original order. Per-column and per-row sample coordinates are tables built once (same `f64` values). Circle test hoists the row term with the same expression order. A full update remains for length changes and for more than 32 changed sources. `regional_updates_match_a_full_recompute_cell_for_cell` compares `current` and `explored` with the former algorithm over 400 random mutations |
| 2 | `list_vision_sources`: per viewer, per unit, `balance_real` scans the whole field map for the sight column, and `shares_vision` repeats string-keyed constant lookups per unit | 16.6% inclusive in `balance_real`, 50 of 51 `balance_real` stack samples were under this function | Sight radius cached per (unit, `definition_revision`, day/night) across viewers and ticks (a unit's definition changes only through a form transition, which bumps the revision); alliance answer cached per owner within a viewer scan, evaluated only where the old `||` chain evaluated it |
| 3 | `unit_hero_bonus` scanned the whole arena for held items, once per unit per tick (regeneration) | 5.5% to 30% inclusive after rank 1 and 2 were fixed | `executed_units()` and `executed_items()` iterate recorded arena positions (set at allocation; an entry never becomes a unit or an item later) instead of every entry |
| 4 | `unit_construction_order` scanned all units for each unit inside `combat_step_eligible` (quadratic per combat step) | 6.8% inclusive, 8.5% self in Human04 | One pass builds the sorted list of workers with a positioned construction site; the per-unit check is a binary search with the same truth value |
| 5 | The executable image (14.9 MB) re-hashed by about 91 rule readers during startup | `gdb` breakpoint on `sha2` compress: 91 calls of 232,171 blocks; sha2 35% of startup samples | `neutral_player::sha256` remembers the latest input of 8 MiB or more by content (an equal-bytes compare against the retained copy, then the stored digest). Startup sha2 self share 35% to 22% (about 6.3 G fewer instructions) |
| 6 | Script VM copied the function body AST on every user function call | `Expr::clone` 36% self in the VM profile | `Function::body` is `Option<Arc<[Stmt]>>`; entering a function clones the `Arc`. Instructions -31%, allocations -50% on `sim-script-vm-v1` |

## Remaining items after round 1

Ranked from the round-1 candidate profile (startup 31.3 G, ticks 35.5 G of the Human03
workload). Round 2 below fixes the first, second and fourth rows; its own list supersedes this one.

| Where | Share | Why it remains |
| --- | ---: | --- |
| `ViewerGrid::stamp` | 17% of the 4,096-tick run | Inherent to the grid sampler after the regional update; a per-row circle span would need proof that it matches the `<=` test bit for bit |
| `sha2` compress in startup | 22% of startup | `campaign_data_set` streams the install archives in 1 MiB reads (about 1.1 GB per start) to pin the data set. Skipping it needs a cache keyed on file identity, which changes what the pin proves |
| `pcx_mpq::huffman::decompress`, ADPCM, MP3 | 17% + 4% + 2% of startup | Asset decoding at startup; owned by the asset work |
| `acquisition_candidate`, `list_vision_sources`, `poll_region_events` | 4.7%, 3.6%, 1.4% | Linear scans over units per unit; a spatial index would change tie order unless it reproduces arena order |
| VM frames and activations | not visible in the Human03 profile (under 7% inclusive) | `Frame` is a `BTreeMap<String, Slot>` and every activation owns `String` names; worth a slot-indexed frame if script-heavy maps dominate |
| `libc` memcpy/realloc | 9% of startup | Whole-buffer copies of archive and table data; needs call-graph attribution that this host does not allow |

Not built: a per-source incremental vision update (it needs the former merge order for
overlapping sources, which the regional form keeps by restamping), and an in-process instruction
counter (`perf_event_open` needs `unsafe`; the setup-only subtraction replaces it).

## Round 2

Baseline is HEAD b4a28f5 (the round-1 result). Candidate is the working tree with the changes
below. Other agents changed `pcx-mpq`, `pcx-map`, `pcx-blp`, `pcx-mdl` and `pcx-slq` in the same
tree, so startup is also reported for a control build: the same tree with only this round's files
(`pcx-play`, `pcx-sim`, the bench) reset to HEAD. The control isolates this round's startup effect
from the asset work. All instruction counts are `instructions:u` from `perf stat`, release build,
the same install, map (Human03, seed 1) and profile for every column.

`sim_bench human03` now steps one tick per call and reports `vision_cell_visits`,
`ticks_with_unit_change` and `ticks_with_vision_change` (the former `PCX_SIM_BENCH_DEBUG` mode
is gone). The step size does not change the digest or the allocation counts by more than one call.

| Workload | Baseline HEAD | Control (HEAD plus asset work) | Candidate | Digest / checkpoint |
| --- | ---: | ---: | ---: | --- |
| `sim-human03-v1` startup (0 ticks) | 31,303,659,833; 3,233,204 allocations; 2.31 GB | 30,278,868,257; 1,598,830; 2.15 GB | 17,564,158,608; 1,598,052; 1.04 GB | `a2517f75b75e84cf` equal |
| `sim-human03-v1`, 4,096 ticks (total) | 66,808,367,059 | 65,784,235,470 | 29,938,535,963 | `97c1d17bf808534c` equal |
| same, ticks only (total minus startup) | 35,504,707,226; 1,613,093 allocations | 35,505,367,213; 1,613,093 | 12,374,377,355; 1,777,547 | equal |
| Human04 route, 120 decisions, ticks only | 34,018,706,771; 3,186,264 allocations | not measured | 13,631,670,780; 3,379,232 | `6bc844b0b15e6f06`, RNG state included, equal |
| Human04 route to victory (28,737 ticks) | 200,208,185,346 | not measured | 122,094,911,890 | all 614 output lines other than the allocation lines equal, 57 checkpoints and the victory line included |
| Human03 route to victory (17,857 ticks) | 247,405,626,450 | not measured | 128,498,725,580 | all 214 output lines equal, 36 checkpoints and the victory line included |
| `sim-script-vm-v1` (40 rounds) | 229,419,270 | not measured | 229,424,015 | `46ab4f7898e3b995` equal |

The Human04 and Human03 baselines are the round-1 candidate test binary rerun on this host
(`h04-cand-bin`), the candidates are a fresh release `peacecraft3` test binary. Route startup
instructions include the asset work (Human04 startup alone: 30.83 G baseline, 16.43 G candidate).
Wall time of `human03 0` (report only): 3.4 s baseline, 2.0 s control, 1.2 s candidate.

The tick-phase allocation count rose 10% (1.61 M to 1.78 M calls): one `Arc` and one run vector
per distinct moved vision shape per tick. The win is instruction count.

### Hotspots found and fixes

Percentages are `perf record -e instructions:u` sample shares of the whole workload they name.

| Hotspot | Evidence | Fix |
| --- | --- | --- |
| `ViewerGrid::stamp`, 28% of the 4,096-tick run (18.7 G), 1.39 G cell visits | profile; `vision_cell_visits` | A same-length source list now recomputes only the cells where a changed source's old and new coverage differ (`vision_field.rs`: `changed_runs`, `recompute`). Each row of a circle or rectangle is one run, found by binary search over the column table with the exact `VisionShape::contains` test (monotone on each side of the centre column), so the cell set is bit-identical. A footprint (bounding rectangle plus one run per row) is computed once per distinct shape per sample and shared by every viewer that lists the unit, and kept for the next sample. Full restamp remains for new viewers, length changes and when it is cheaper. Cell visits 1,392,416,124 to 2,010,077 (700x fewer); the stamp and region-merge code is replaced. Differential tests below |
| `acquisition_candidate`, 6% (4.0 G) | profile | The range test runs first and reads the unit record without building a lookup error; protection and alliance checks run only for units in range. Every check only refuses, so results and order are unchanged. 2.1 G |
| `list_vision_sources`, 3.5% (2.3 G), 10% of tick wall time in `shares_vision` | profile and stack samples | `shares_vision` looked up two constants by scanning every script global for each owner and viewer pair; a per-sample `SourceTable` resolves the alliance kinds and fog states once, holds the started fog modifiers (a new `fog_slots` index replaces three arena walks per sample) and the executed units, and caches each unit's sight radius. 0.6 G |
| `next_timer_expiry` and `poll_region_events` walked the whole arena (2.3% and 1.4%) | profile | `timer_slots` and `region_event_slots` index the arena like `unit_slots`, in allocation order. `poll_region_events` 0.96 G to 0.77 G (its unit-by-registration loop remains) |
| Startup byte scans over the 14.9 MB executable: `windows(n)` plus a comparison call per offset, 27% of startup instructions in libc `memcmp` | profile of `libc` samples and stack samples (autocast, morph, construction, player resources, EAX tables, neutral rule discovery, options defaults) | `pe_image::windows_with_byte` and `occurrences` find candidate offsets with a block scan for the first byte and compare only there; equal to the `windows` plus filter they replace (`pe_image_tests`). `player_resources::shape` rejects on the first byte before comparing. Startup 30.28 G to 17.56 G with the snapshot change below |
| `campaign_data_set` snapshot: 1.1 GB read through 1 MiB `Vec`s, hashed, written; 22% of startup cycles | sample shares; `sha2::sha256::x86_sha::compress` is the SHA-NI path, so the crate already uses hardware SHA; CPU has `sha_ni` | The archive is copied inside the kernel (`io::copy` between files; a reflink on btrfs) and the copy is hashed in one pass with a reused buffer, so the pin digest covers the bytes the snapshot archive is parsed from. The source stamp is checked before the copy (path and handle), and after it, as before. One thread per archive copies and hashes in parallel; results are consumed in archive order so the first error is the one the sequential code reported. The hash itself stays: it is the pin, and a size or mtime check would not be. Allocated bytes 2.15 GB to 1.04 GB; the hash is 3.0 G instructions of the remaining 17.6 G |

Differential and unit tests added: `incremental_updates_match_a_full_recompute_cell_for_cell`
(5,200 random mutations of circle and rectangle lists against the former full recompute, partial
final cells and crowded lists included), `row_runs_equal_the_cell_test_for_every_row`,
`a_moving_circle_among_many_recomputes_only_its_crescent`, `occurrences_equal_the_window_scan_at_every_boundary`,
`windows_with_byte_equal_the_filtered_window_scan`, `snapshot_copy_hashes_the_private_copy_byte_for_byte`
(chunk multiples, partial chunks, one byte, empty), `snapshot_copy_refuses_a_source_whose_length_differs_from_the_opened_one`,
`concurrent_snapshot_copies_return_results_in_input_order`, `unit_revision_counts_unit_changes_and_not_reads`,
the revision assertions in the shared-vision host test, and `crates/pcx-sim/tests/column_lookup.rs`.

### Remaining items after round 2

Ranked from the candidate profile (startup 17.6 G, ticks 12.4 G of the Human03 workload).

| Where | Share | Why it remains |
| --- | ---: | --- |
| `pcx_mpq::huffman::decompress`, `decompress_adpcm`, MP3 decode | 23% + 10% + 4% of startup | `prepare_label_sound` extracts and decodes whole sound files during script start-up (`campaign_assets::SoundSources::label_sound`). The decoders belong to the asset crates; skipping the decode needs a header-only read in `label_sounds` |
| `sha2` compress | 17% of startup (3.0 G) | The pin hash of 1.1 GB. Parallel across archives, not skippable |
| `pe_image::find_byte` and the other executable scans | 7% of startup (1.2 G) | One block scan per rule lookup; about one instruction per byte. A vector compare would halve it |
| `SharedVision::apply` footprints | 8.5% of the 4,096-tick run (2.5 G) | About 100 instructions per row of each newly moved shape. An arithmetic estimate with exact fix-up would cut it; not built |
| `acquisition_candidate` | 6.9% (2.1 G) | Every idle unit tests every unit. A position snapshot valid while `unit_revision` is unchanged would skip most pairs, but the revision advances every tick in this workload, so it was not built |
| `poll_region_events`, `list_vision_sources` | 2.6%, 2.1% | Unit-by-registration loop; one pass per viewer over every unit |
| VM frames and activations | under 7% | Unchanged from round 1 |

Not built: the revision counters do not shorten this workload. In the first 4,096 ticks of
Human03, `unit_revision` changes in 4,096 of 4,096 ticks and `vision_revision` in 4,096 of 4,096,
because units are always moving. They save work only in idle stretches (menus, cinematics,
paused or stopped armies).

### Budgets

`tools/perf/budgets/sim.json` holds, for each workload of `tools/perf/sim_run.py`, the exact
allocation calls, reallocations and requested bytes of the measured region (and
`vision_cell_visits` for the Human03 modes), the output digest, and the SHA-256 of the local
`Warcraft III.exe` and Human03 map the numbers were measured on. Instruction counts are not in
the file: they vary with compiler and CPU.

```sh
flock .scratch/cargo.lock python3 tools/perf/sim_run.py --check            # all workloads
flock .scratch/cargo.lock python3 tools/perf/sim_run.py --no-assets --check # synthetic VM only
```

`--check` exits 1 when a counter is above its budget, a digest differs (behaviour changed), the
install or map hash differs from the one in the file (the numbers do not apply), or any budget
is above the one committed at HEAD. Counters are reproducible run to run; a second run matched
every counter exactly.

To change a budget: lower it only, in the same change as the code that lowered the counter, and
record the before and after counts and the command in this page. A budget goes up only by a
reviewed behaviour change that also changes a digest, with the reason written here; the check
refuses a raise relative to HEAD, so a deliberate raise needs the reviewer to commit the new
file first. A new workload gets its budget from a measured run of the commit that adds it.

The construction texture-channel change shares the immutable placement-rule binding across rule
clones, retaining its selected source windows while avoiding repeated metadata copies. With the
same Human03 inputs, `python3 tools/perf/sim_run.py --check` reports 5 fewer allocations and
1,392 fewer requested bytes in both the startup and 4,096-tick workloads. Their digests and vision
work are unchanged; the recorded budgets were not raised.

The startup-alliance reader now validates its instruction windows in fixed-size stack storage
and borrows the additional shared-control window from the selected image. The same locked,
offline `python3 tools/perf/sim_run.py --check` measured 8 fewer allocations and 50 fewer
requested bytes in both Human03 workloads than the preceding local run. Digests, reallocations
and vision work are unchanged; no budget was raised. The shared-control default adds the
selected client's self relationship to startup state. The placement search remains diagnostic;
the Human03 and Human04 route harnesses were not rerun.

The shared PE reader now holds up to 16 section descriptors inline and retains its bounded heap
path for larger files. This reduces startup allocations while preserving section order and input
validation. The placement event history stores its first eight events in the existing host and
allocates a bounded overflow only when needed. With both changes, the locked offline
`sim_run.py --check` measured 56 fewer allocations and at least 10,536 fewer requested bytes than the
pre-history Human03 measurements at both checkpoints; reallocations and digests were unchanged.
No budget was raised.

Queued-order admission now keeps the two bounds inline and allocates queue data only when an
order waits. After the input changes, the locked offline check measured 48 fewer requested
bytes in each Human03 workload, with allocation counts, reallocations, digests and vision work
unchanged. That leaves 24 bytes above the earlier pre-input placement measurement; the budget
was not raised.

The eight inline placement events now encode their handle and event kind compactly, retaining
the full-width overflow path. The locked offline check reports 64 fewer requested bytes in each
Human03 workload than the queued-order run, placing both 40 bytes below the earlier pre-input
measurement. Allocation and reallocation counts are unchanged; no budget was raised.

The placement-input retention adds two optional tracking pointers. When tracking is inactive,
they allocate no separate storage. Against committed `9b93f69`, the locked offline Human03
check has identical allocation and reallocation counts, digests and vision work, with 16 more
requested bytes at each checkpoint. The extra bytes hold those inactive pointers; the budget
was not raised. At Human04's first control checkpoint, a separate same-commit test with
equal-length isolated profile paths measured 146 more allocations and 558,808 more requested
bytes, with unchanged reallocations and state digest. The two runs used different checkout paths.
The pre-sized region history accounts for 524,288 bytes; retained texture flags and tracking
storage contribute to the remaining difference.

The spatial chain owner hooks (collision commit and hide, stationary toggles, region
additions, the activity tick) each start with an `Option` test on a field that is `None` in an
untracked session. Baseline and candidate were measured in the same checkout path (HEAD
`2b19328` against the working tree), locked, offline, same Human03 inputs:

| Workload | Allocations | Reallocations | Requested bytes | Digest / visits |
| --- | ---: | ---: | ---: | --- |
| `human03 0` before | 1,597,968 | 43,177 | 1,039,365,973 | `a2517f75b75e84cf` |
| `human03 0` after | 1,597,968 | 43,177 | 1,039,365,997 | equal |
| `human03 4096` before | 3,375,515 | 406,994 | 1,756,596,745 | `97c1d17bf808534c`, 2,010,077 |
| `human03 4096` after | 3,375,515 | 406,994 | 1,756,596,769 | equal |

Allocation and reallocation counts are equal. The 24 extra requested bytes are the new
`Placement` pointer and the captured spatial operands in the placement-loader closure. User
instructions rose 0.003% (17,638,122,871 to 17,638,601,658 at 0 ticks). `--check` passes; no
budget was raised. Tracked sessions (a map script that can start an AI) allocate the chain
pool, entry table and operands, and are not part of this workload.

The later live-pool reach diagnostic and movement-scale change leave the Human03 digests and
allocation counts unchanged. The same check reports 1,039,365,977 and 1,756,596,749 requested
bytes, respectively, 20 fewer than the plumbing measurements above and four more than their
earlier control. This is not a new paired control measurement; the remaining startup allocation
delta has not been isolated. Human03 and Human04 route checkpoints and RNG lines match `g1`.
The budget check passes without raising limits.

### Hand-off to render

All of these are read-only and cheap. They exist in `pcx-play` and `pcx-sim`; nothing in
`crates/peacecraft3` uses them yet.

- `ObjectHost::vision_revision() -> Option<u64>` (`pcx_play::playercolor_host`): `None` until the
  first sample, then a count that increases whenever a sample changes any viewer's raw cells or the
  set of viewers. While it is equal, every `vision_cell` and `vision_presented` answer for a given
  fog and mask setting is equal. The fog and mask switches are not part of it; read them from
  `vision_capability()` as before. The minimap can keep its composed fog image and skip sampling
  its roughly 65k cells when the value and the switches are unchanged.
- `ObjectHost::unit_revision() -> u64`: counts unit allocations and every mutable access to a
  unit record. Equal values mean no unit's position, owner, visibility, vitals, definition or
  other field changed. It is a superset (an access that writes nothing counts), so a changed value
  means "re-read", not "something moved". For a single unit's definition the existing
  `definition_revision` field of the executed unit is the finer signal.
- `pcx_sim::world::Definition::column_field(&self, column) -> Result<(&Fourcc, &ObjectField), ColumnError>`
  replaces the linear scan in `campaign_actors::scalar`; `ColumnError::{Missing, Ambiguous}` map to
  the existing `missing unit field {column}` and `ambiguous unit field {column}` messages, and the
  rest of `scalar` (metadata checks, value parsing) is unchanged.
- `Definition::column_index() -> ColumnIndex` and `ColumnIndex::field(&definition, column)` give
  the same answers in one hash lookup. Build the index once per `(unit, definition_revision)` and
  reuse it for `modelScale`, `moveHeight`, `red`, `green`, `blue`, `teamColor`, `scale` and
  `hideOnMinimap`; rebuild it when `definition_revision` changes. The index describes the
  definition it was built from, and returns `Missing` for a field that is gone.
