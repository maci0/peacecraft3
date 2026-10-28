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
- `RAYON_NUM_THREADS=1` keeps `jpeg-decoder` single-threaded. Three identical
  runs differ by 0 to 1.4 million instructions out of 10 million to 3.5
  billion (largest: `mdx-sample-v1`, libm and allocator variance).
- `compare` fails on a differing input digest, output digest or fixed-work
  counter, on more allocation calls, or on instruction growth above 2%.

Measured on AMD Ryzen 9 9950X, rustc 1.99.1, perf 7.2.11, Legacy 1.29 install.
Absolute counts depend on compiler and code layout; compare only like with like.

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
| `resolver-mount-v1` | unit, script and frame-definition members | `MpqSource` and `Resolver` mount, 128 resolves |
| `terrain-mesh-v1` | four maps | `TerrainMesh::build`, `manifold`, `digest` |
| `terrain-ground-v1` | four maps | tile catalog, slot selectors, census, 65,536 height samples |

The two BLP corpora are checked at setup (`content` 0 or 1). The map list,
member lists and model list are constants in `tools/perf/assets/src/`.

## Results

Before is the committed tree at HEAD `389d316`, after is the working tree. All
output digests are identical (the digest of an unchanged result is the
acceptance check) and no fixed-work counter changed. Allocation calls include
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

## Hotspots and fixes

Evidence is user-instruction sampling inside each window.

| Where | Finding | Fix |
| --- | --- | --- |
| `pcx-slq` `Table::parse` | 2,700 instructions and 5 allocations per cell: three `String`s per field, a `Vec` per record, a B-tree descent for `contains_key`, another for `entry`; `Table::insert` 52%, `finish_grow` 23%, `push_field` 17% | fields share one buffer, cells arriving in ascending order are staged and bulk-built into the map (first out-of-order or repeated coordinate falls back to per-cell insert, anomaly order unchanged), CRLF search without `windows(2)` |
| `pcx-slq` `Table::column_of` | linear scan of the header map on every `by_name` (28% of lookups) | name index built once in `finish` (lowest column wins, as before) |
| `pcx-mdl` `bone_matrices_from_poses` | a `HashMap` of node ids, three more vectors and two `parent` closure calls per bone, per call: 29% self plus 7% hashing | flat id table (map only for ids far past the bone count), one `parent` call per bone, one links vector |
| `pcx-mdl` track sampling | translation and scale tracks searched their keys once per component (`span` 6%) | one key search per track |
| `pcx-mpq` `HuffmanTable::parse` | byte-by-byte scans of 15 MB: `find_adpcm_tables` 64%, instruction-window scan 36% | eight-byte-at-a-time search for the opcode byte and the trailing step word, same candidates in the same order |
| `pcx-mpq` `payload` | per sector: a copy of the sector, a zeroed output buffer then a second copy, a stage `Vec`, an input copy, a fresh inflate state | decode in place, borrow unencrypted sectors, no stage allocation for single-stage masks, one inflate state per block |
| `pcx-mpq` `HuffmanTable` | initial adaptive tree rebuilt from the 256 weights for every sector | tree per row built once, cloned with room for growth |
| `pcx-mpq` `file_list` | `normalize` made four strings per line, plus a key for every duplicate line | one-pass `normalize_into` and a reused key |
| `pcx-assets` `MpqSource::new` | full hash-table scan per declared member (56% of the mount) | one sorted pass over the table, same ambiguity rule and message |
| `pcx-tera` `manifold` | two `BTreeMap`s, 590,000 inserts (76%) | sort of packed edge words and run counting, same report |
| `pcx-blp` JPEG | format matched per pixel, stream copied, output indexed per pixel (34% of decode outside the JPEG library) | per-format pass over `as_chunks`, header and scan data read through `Read::chain` |
| `pcx-tera` selectors | generic sort of four bytes per tile | five compare-exchanges |
| `pcx-map` doodads | `Vec` growth for 15,000 instances | reserve bounded by the file length |

Existing tests for every touched crate pass, with new tests for out-of-order and
repeated cells, lone line terminators, non-UTF-8 fields, repeated header names,
huge and repeated bone ids, parent cycles, flipped, tripled and degenerate
mesh edges, `find_byte`, per-row tree reuse and mixed stored and zlib sectors.
With the install present, all 16,839 payloads of `War3.mpq` and `War3x.mpq`
decode (`local_payload`).

## What remains

- `mdx-sample-v1` (2.6 billion): about 36% is quaternion slerp with `f64` `sin` and
  `acos`; changing that changes results, so it needs a fidelity decision.
- `editor-open-v1` (576 million): most of the rest is `Table::insert` and its per-cell `String` allocation for about 300,000 cells. Storing cells in one arena would
  change `Table::row_cells` (returns `&String`) and every caller.
- `slk-lookup-v1`: each `get` is a B-tree descent over 100,000 cells.
- `blp-jpeg-v1`: about 190 instructions per pixel inside `jpeg-decoder`; the
  mips of one texture each reparse the shared JPEG header.
- `mpq-audio-v1`, `mpq-extract-v1`: inflate and the adaptive Huffman symbol
  walk dominate. Both are format work; the walk touches the tree per bit.
- `mpq-checked-v1`, `map-load-v1`: `extract_checked` decrypts the hash and block
  tables per member; this was not separated from the inflate work. Caching
  them needs state inside `Map`, which is `Clone + PartialEq`.
- `pcx-casc` has no workload: no CASC install is present on this host.
- Browser ISO import and WASM paths are not measured; only native instruction
  counts exist here.

## Hand-off (outside this area)

- `campaign_data_set.rs` copies each archive to a private snapshot in chunks
  and hashes it with SHA-256 (about 5% of the Human03 startup capture); it
  uses `MpqArchive::read_at`, which zero-fills each chunk before reading.
- `pcx_blp::decode_jpeg` decodes one mip per call; whether `pcx-gfx` decodes or
  regenerates mips the file already stores was not examined.
- `sample_decoder_frame` and similar callers in `pcx-gfx` pass a `parent`
  closure backed by a `BTreeMap`; with `bone_matrices_from_poses` now calling it
  once per bone this matters less, but a flat index would remove it.
