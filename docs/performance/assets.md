# Asset loading and decode performance

Gate metrics are user-mode instructions, allocation calls and requested bytes
over fixed workloads, plus an output digest that must not change. Elapsed time
is printed for orientation only. See
[performance testing](../performance-testing.md) for the shared rules.

## Running

```sh
python3 tools/perf/assets.py list
python3 tools/perf/assets.py run --label before --repeat 3
python3 tools/perf/assets.py run --binary <other build> --label before   # build against another revision
python3 tools/perf/assets.py compare .scratch/perf-assets/results/before.json .scratch/perf-assets/results/after.json
python3 tools/perf/assets.py check                                       # allocation and fixed-work budgets
python3 tools/perf/assets.py lower .scratch/perf-assets/results/after.json
```

`PEACECRAFT3_LEGACY_DIR` (or `--install`) names the Legacy install. The
program is `tools/perf/assets/` (own manifest and lockfile, path dependencies on
the game crates, `release` profile with O3, thin LTO and symbols). `perf stat`
needs only `kernel.perf_event_paranoid=2`. Per-function attribution used
`perf record -e instructions:u -c 20000` with the same control FIFOs, so
samples cover only the measured window.

Method details:

- Inputs are read from the archives before the window opens. The window is
  opened and closed by the program over `perf stat --control` FIFOs (std only).
- Decoded results are kept alive until the window closes and hashed afterwards,
  so digest work is not counted.
- Allocation counts come from a forwarding global allocator (`alloc` and
  `alloc_zeroed` count as allocations, `realloc` separately).
- `RAYON_NUM_THREADS=1` keeps `jpeg-decoder` single-threaded; `run --threads N`
  sets another count (instructions then include every thread). Three identical
  runs differ by 0 to 1.4 million instructions out of 10 million to 3.5
  billion (largest: `mdx-sample-v1`, libm and allocator variance).
- `compare` fails on a differing input digest, output digest or fixed-work
  counter, on more allocation calls, or on instruction growth above 2%.

Measured on AMD Ryzen 9 9950X, rustc 1.99.1, perf 7.2.11, Legacy 1.29 install.
Absolute counts depend on compiler and code layout; compare only like with like.

## Budgets

`tools/perf/budgets/assets.json` (version 1) holds, per workload, the input and
output digests, the fixed-work counters, `allocation_calls` (allocations plus
reallocations) and `allocated_bytes` measured at the last accepted change, and
the rustc they were taken with. `python3 tools/perf/assets.py check` runs the
workloads once (`--repeat` for more) and exits 1 when

- an allocation counter is above its budget,
- a fixed-work counter or a digest differs (the digests belong to the Legacy
  1.29 install the budget was taken from; another install is not a comparison),
- a workload has no budget, or a budget has no workload.

Instruction counts are printed and never decide the exit code. Budgets only
move down. After a change that lowers a counter, measure with `run`, then
`python3 tools/perf/assets.py lower RESULT.json`: it lowers each counter to the
measured value and refuses (exit 1, file untouched) when any allocation counter
would rise or any digest or work counter differs. A deliberate change of output
or work is a hand edit of the file with the reason in the commit and the
evidence in this document. Add a new workload by running `lower` on a result
that contains it.

## Workloads

| Name | Input | Work |
| --- | --- | --- |
| `mpq-open-v1` | four campaign archives | open, decrypt hash and block tables |
| `mpq-codec-table-v1` | `Warcraft III.exe` | Huffman and ADPCM table discovery |
| `mpq-extract-v1` | 24 members (models, textures, tables, scripts, wav, mp3, two maps) | unchecked `MpqSet::extract` |
| `mpq-extract-bounded-v1` | same list | `extract_bounded` with the codec table |
| `mpq-audio-v1` | 24 unit speech files | Huffman and ADPCM sectors |
| `mpq-listfile-v1` | four `(listfile)` members | `MpqSet::file_list` union |
| `mpq-checked-v1` | four maps | `CheckedArchive` parse and eight members each |
| `blp-jpeg-v1` | 13 textures, every mip | JPEG-content decode |
| `blp-palette-v1` | 10 textures, every mip | palette decode |
| `mdx-parse-v1` | footman, great hall, four heroes, main menu, a campaign background | every decoder the importers use |
| `mdx-sample-v1` | same models | pose and bone matrices every 33 ms of every sequence |
| `slk-parse-v1` | 11 tables | `Table::parse` |
| `slk-lookup-v1` | three tables | every data row by every column name, twice |
| `editor-open-v1` | startup table and profile members | `Editor::from_reader` as headless startup calls it |
| `editor-open-install-v1` | four archives | `Editor::open_in` (listfile union, extraction, decode) |
| `map-load-v1` | four maps | `extract_checked` plus parse of each gameplay member |
| `map-unchecked-v1` | same maps | `Map::terrain`, `shadows`, `doodads`, `info`, `units`, `regions`, `content_files` |
| `map-members-v1` | same maps | 15 members extracted three times each through `extract_checked` and `Map::extract` from one parsed `Map` (360 extractions) |
| `resolver-mount-v1` | unit, script and frame-definition members | `MpqSource` and `Resolver` mount, 128 resolves |
| `terrain-mesh-v1` | four maps | `TerrainMesh::build`, `manifold`, `digest` |
| `terrain-ground-v1` | four maps | tile catalog, slot selectors, census, 65,536 height samples |

The two BLP corpora are checked at setup (`content` 0 or 1). The map list,
member lists and model list are constants in `tools/perf/assets/src/`.

## Round 1 results

Before is the tree at `389d316`, after is `788feef`. All output digests are
identical and no fixed-work counter changed. Allocation calls include
reallocations.

| Workload | Instructions before | after | Alloc calls before | after | Bytes before | after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `mpq-open-v1` | 10,633,747 | 10,633,747 | 31 | 31 | 4,105,880 | 4,105,880 |
| `mpq-codec-table-v1` | 301,159,910 | 69,442,815 | 10 | 10 | 132,819 | 132,819 |
| `mpq-extract-v1` | 172,847,576 | 164,420,587 | 8,431 | 496 | 87,695,434 | 12,374,624 |
| `mpq-extract-bounded-v1` | 172,760,302 | 164,301,450 | 8,329 | 394 | 87,693,069 | 12,372,259 |
| `mpq-audio-v1` | 149,412,385 | 140,788,504 | 3,800 | 1,055 | 10,663,959 | 11,383,465 |
| `mpq-listfile-v1` | 78,356,700 | 72,754,336 | 87,511 | 34,239 | 21,853,412 | 6,364,391 |
| `mpq-checked-v1` | 103,118,854 | 97,236,020 | 6,814 | 286 | 69,096,050 | 6,704,864 |
| `blp-jpeg-v1` | 310,005,888 | 272,368,426 | 41,736 | 41,617 | 50,067,012 | 48,808,337 |
| `blp-palette-v1` | 12,622,269 | 12,624,419 | 64 | 64 | 2,381,376 | 2,381,376 |
| `mdx-parse-v1` | 2,386,264 | 2,404,777 | 5,001 | 5,001 | 4,802,492 | 4,802,492 |
| `mdx-sample-v1` | 3,510,022,068 | 2,608,202,968 | 101,596 | 101,596 | 238,070,855 | 247,525,172 |
| `slk-parse-v1` | 812,514,778 | 450,213,219 | 1,586,049 | 329,425 | 62,994,457 | 57,696,970 |
| `slk-lookup-v1` | 243,750,823 | 175,086,393 | 2 | 2 | 56 | 56 |
| `editor-open-v1` | 988,343,132 | 575,985,378 | 1,911,899 | 477,086 | 77,282,766 | 70,559,537 |
| `editor-open-install-v1` | 1,170,828,306 | 743,364,683 | 2,039,688 | 545,698 | 163,325,388 | 84,954,345 |
| `map-load-v1` | 93,183,424 | 87,979,600 | 6,962 | 1,359 | 64,238,633 | 9,907,012 |
| `map-unchecked-v1` | 146,105,388 | 139,262,090 | 8,762 | 1,500 | 96,959,877 | 26,472,732 |
| `resolver-mount-v1` | 1,297,935,813 | 438,980,696 | 104,490 | 90,371 | 53,536,612 | 39,371,339 |
| `terrain-mesh-v1` | 984,163,810 | 187,670,339 | 144,214 | 31 | 35,261,616 | 14,866,800 |
| `terrain-ground-v1` | 47,657,847 | 45,469,590 | 15,324 | 3,330 | 4,090,927 | 4,009,377 |

`mpq-audio-v1` bytes rise because each Huffman sector now starts with room for
the full adaptive tree (about 24 KB) instead of growing it; `mdx-sample-v1`
bytes rise by the per-call node-id table. Both trade a few allocations or
growth copies for fewer calls.

## Round 2 results

Before is the tree at HEAD `b4a28f5` and after is the working tree, both built
with the same harness (which gained `map-members-v1`), `--repeat 2`, one
thread. Every output digest and fixed-work counter is identical; no allocation
call count rose. The suite total falls from 7,051,886,819 to 6,548,838,574
instructions (-7.1%).

| Workload | Instructions before | after | Alloc calls before | after | Bytes before | after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `blp-jpeg-v1` | 272,363,391 | 250,999,565 | 41,617 | 41,617 | 48,808,337 | 48,808,337 |
| `blp-palette-v1` | 12,624,421 | 12,623,729 | 64 | 64 | 2,381,376 | 2,381,376 |
| `editor-open-install-v1` | 743,358,440 | 616,360,060 | 545,698 | 172,097 | 84,954,345 | 45,620,716 |
| `editor-open-v1` | 575,985,376 | 450,093,024 | 477,086 | 103,485 | 70,559,537 | 31,225,908 |
| `map-load-v1` | 87,979,615 | 87,582,430 | 1,359 | 1,155 | 9,907,012 | 9,771,373 |
| `map-members-v1` | 592,613,282 | 587,710,449 | 3,983 | 2,223 | 89,613,851 | 45,152,552 |
| `map-unchecked-v1` | 139,262,102 | 138,216,902 | 1,500 | 1,184 | 26,472,732 | 13,470,991 |
| `mdx-parse-v1` | 2,404,778 | 2,402,059 | 5,001 | 5,001 | 4,802,492 | 4,802,492 |
| `mdx-sample-v1` | 2,608,302,934 | 2,550,910,233 | 101,596 | 101,596 | 247,525,172 | 247,525,172 |
| `mpq-audio-v1` | 140,788,506 | 140,732,888 | 1,055 | 1,055 | 11,383,465 | 11,383,465 |
| `mpq-checked-v1` | 97,236,021 | 97,228,254 | 286 | 286 | 6,704,864 | 6,704,336 |
| `mpq-codec-table-v1` | 69,442,816 | 69,442,815 | 10 | 10 | 132,819 | 132,819 |
| `mpq-extract-bounded-v1` | 164,301,438 | 164,298,622 | 394 | 394 | 12,372,259 | 12,372,259 |
| `mpq-extract-v1` | 164,420,587 | 164,419,522 | 496 | 496 | 12,374,624 | 12,374,624 |
| `mpq-listfile-v1` | 72,748,640 | 72,764,180 | 34,239 | 34,239 | 6,364,391 | 6,364,391 |
| `mpq-open-v1` | 10,633,748 | 10,627,970 | 31 | 27 | 4,105,880 | 3,773,992 |
| `resolver-mount-v1` | 438,980,768 | 438,983,736 | 90,371 | 90,371 | 39,371,339 | 39,371,339 |
| `slk-lookup-v1` | 175,086,395 | 118,087,135 | 2 | 2 | 56 | 56 |
| `slk-parse-v1` | 450,214,133 | 343,695,400 | 329,425 | 1,566 | 57,696,970 | 23,051,349 |
| `terrain-ground-v1` | 45,469,595 | 43,989,922 | 3,330 | 616 | 4,009,377 | 3,737,696 |
| `terrain-mesh-v1` | 187,669,833 | 187,669,679 | 31 | 31 | 14,866,800 | 14,866,800 |

Rows within 0.1% are run-to-run variance, not changes.

### Fixes

| Where | Finding | Fix |
| --- | --- | --- |
| `pcx-slq` cell storage | `Table::insert` plus one `String` per cell: 40% of `slk-parse-v1`, 330,000 allocations | cells are `(row, column)` entries over one text buffer (`cells.rs`); ascending cells append, the first out-of-order or repeated coordinate adds a coordinate index and `finish` sorts once (anomaly order unchanged); a row offset table (kept while the highest row is at most 4 rows per cell plus 64) makes `get` a search inside one row. `Table::row_cells` yields `&str` instead of `&String` (only `pcx-tera` `water.rs` called it) |
| `pcx-slq` field scanning | three `String` copies and three UTF-8 checks per record | field values go to one buffer per record and are checked once; a value must still be valid on its own (a character split across two fields, a bad field before an unterminated quote, an untagged bad value are covered by `tests/field_text.rs`) |
| `pcx-blp` JPEG | the CMYK arm built each pixel byte by byte (22% of `blp-jpeg-v1`, SSE2 element gathers) | one 32-bit word swap and invert per pixel; `rgba_from_jpeg` calls the same function, exhaustive test over every pair of leading bytes |
| `pcx-mpq` + `pcx-map` member reads | `Map::extract` copied the whole archive and decrypted both tables per member; `extract_checked` parsed per member | `CheckedArchive` shares its decrypted tables as `Arc<CheckedTables>` (`from_tables` refuses bytes of another length); `Map` keeps the tables of its last checked read (keyed by source label and budget, so every error still names its label and limits) and the `MpqArchive` of its first `Map::extract`. `Map::checked_archive` exposes the reusable handle. A clone starts empty, equality ignores the cache, a failed parse is not kept. `read_tables` reuses one buffer for both tables (one allocation fewer per archive open) |
| `pcx-mdl` `bone_matrices_from_poses` | the parent matrix was copied twice per bone (14% of the function) | `compose` delegates to an internal version that reads the parent in place |

Tests added: `crates/pcx-slq/src/cells.rs` (ordering, repeats, row bounds, dense
and sparse row lookup), `crates/pcx-slq/tests/field_text.rs`,
`crates/pcx-map/tests/member_reuse.rs` (shared tables per label and budget,
error labels, no cached failure, repeated unchecked reads, clone), the word
inversion test in `pcx-blp`.

`map-members-v1` isolates the repeated read. The decrypt itself was small
(587.7 against 592.6 million instructions); what the cache removed is the
per-member archive copy and table buffers (89.6 to 45.2 MB, 3,983 to 2,223
allocation calls). The rest of that workload is inflate.

### Looked at and not changed

- `mdx-sample-v1` quaternion slerp (about 33% with `sin` and `acos`): a call's
  operands are all new, so normalization, `acos` and the sines cannot be hoisted
  inside it, and the zero-weight and equal-key exits already exist. A 1,024-slot
  memo of the normalized keys, angle and sine, keyed by the key pair bits and
  tried in an isolated copy, left the digest unchanged and saved 1.9%
  (2,608 to 2,559 million): consecutive frames rarely reuse a key pair. Not
  landed. A per-track cache would need a new field on `BoneTrack`, which other
  crates construct with struct literals.
- JPEG threads: `RAYON_NUM_THREADS=4` gives 256.4 million instructions against
  250.9 million at one thread (+2.2%, all threads counted) with the same elapsed
  time within noise at these texture sizes, so the rayon feature moves work to
  other threads and adds some. `jpeg-decoder` built without default features
  (isolated copy, not landed) gives 205.2 million (-18%), identical digest,
  1,097 more allocation calls and 0.4 MB more. Landing it changes `pcx-blp`'s
  `Cargo.toml` and three lockfiles and drops multi-core decode of the large
  loading-screen textures, so it is a decision for the owner of those files.
- Mips: `pcx-blp` decodes one mip per call and no mip is decoded twice inside
  the crate; each call re-reads the shared header, which is small. Whether
  `pcx-gfx` decodes a mip the file already stores was not examined.
- `pcx-assets` `MpqSource` holds one parsed `MpqArchive` behind a mutex, so it
  never decrypts tables per member. `resolver-mount-v1` is 67% Huffman and
  ADPCM sectors.
- `pcx-casc`: no workload. The fixture builders (`index_bytes`, `multi_blte`,
  `encoding_bytes`, the TVFS fixture) are private to the crate's unit tests and
  build a three-entry install; timing that measures setup cost and nothing that
  scales with a real install's index and block counts, and a scalable generator
  would be new code with its own proof burden. The synthetic round trip stays a
  functional test.
- `pcx-objects` `extract_object_file` and `map_object_files` extract the whole
  map from the archive set on every call. They serve the `objx` tool and tests
  and have no workload, so they are unmeasured and unchanged.

## What remains

- `mdx-sample-v1` (2.55 billion): slerp with `f64` `sin` and `acos`, then
  `bone_matrices_from_poses` and `sample_joint_pose`. The workload's own
  `parent` closure uses a `HashMap` (about 15% of the workload, harness code).
- `editor-open-v1` (450 million), `slk-parse-v1` (344 million): record scanning
  in `Table::parse_with_limits` (one pass per field with scalar searches) and
  UTF-8 checking; `Editor::from_reader` still makes about 100,000 small
  allocations.
- Inflate: `miniz_oxide` `init_tree` is 22 to 35% of the extraction workloads
  because every 4 KiB MPQ sector is its own zlib stream with its own table
  build. Replacing the inflate backend is a dependency decision.
- `resolver-mount-v1`, `mpq-audio-v1`, `mpq-extract-v1`: the adaptive Huffman
  symbol walk and ADPCM decode touch the tree per bit.
- `blp-jpeg-v1`: `jpeg-decoder` itself (about 150 instructions per pixel in total,
  including its CMYK line conversion).
- `terrain-mesh-v1`: `manifold` sorts 590,000 `u128` edge words (about 60%).
  Edges need 65 bits, so a `u64` sort needs a fallback for indices of 2^31 or
  more.
- `mpq-listfile-v1`: `normalize_into` (`split` on two separators) and SipHash.
- Browser ISO import and WASM paths are not measured; only native instruction
  counts exist here.

## Hand-off (outside this area)

- `pcx-play` reads several members per map through `Map::extract_checked`
  (`editor_triggers.rs`, `campaign_inputs.rs`, `campaign_placement.rs`). They
  now share one table decryption per `Map` as long as the source label and
  `ReadBudget` values match between calls; building a different budget per call
  defeats it.
- `campaign_data_set.rs` copies each archive to a private snapshot in chunks
  and hashes it with SHA-256 (about 5% of the Human03 startup capture); it
  uses `MpqArchive::read_at`, which zero-fills each chunk before reading.
- `pcx_blp::decode_jpeg` decodes one mip per call; whether `pcx-gfx` decodes or
  regenerates mips the file already stores was not examined.
- `sample_decoder_frame` and similar callers in `pcx-gfx` pass a `parent`
  closure backed by a `BTreeMap`; a flat index would remove it, as it would in
  this harness.
- Decision for the owner of the workspace lockfiles: build `jpeg-decoder`
  without its `rayon` feature (measured above, -18% instructions, same pixels).
- `cargo clippy -p peacecraft3 --lib --bins --features test-fixtures --target
  wasm32-unknown-unknown -- -D warnings` stops at `crates/pcx-play/src/pe_image.rs:35`
  (`chunks_exact_to_as_chunks`), outside the asset crates; `cargo check --all-targets`
  for `peacecraft3` stops at unused `totals` and `thread_totals` in
  `crates/peacecraft3/src/counting_allocator.rs`.
