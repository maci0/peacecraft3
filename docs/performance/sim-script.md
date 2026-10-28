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

## Remaining items

Ranked from the candidate profile (startup 31.3 G, ticks 35.5 G of the Human03 workload). None is
fixed here.

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
