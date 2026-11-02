# Tier C fuzz corpus + runner ownership (fuzz-tierc lane)

Process note, not game behavior. No shipped-byte, native-code or runtime
evidence applies. Nothing here changes parsers, the sim, rendering or any
admitted rule. It records what the `fuzz-tierc` lane owns, where the
`mechanics` lane boundary is, and what remains open.

## Ownership

Lane `fuzz-tierc`, worktree `../peacecraft3-fuzz-tierc`, branch
`work/fuzz-tierc/corpus`. Claimed paths (see `tools/parallel-claim.sh`):

- `crates/pcx-play/src/fuzz_corpus.rs` — fixed 32-seed corpus, recipe v1.
- `crates/pcx-play/src/fuzz_scenario.rs` — seeded generation over the
  existing movement generator (random unit comps, synthetic map tags).
- `crates/pcx-play/src/fuzz_resume.rs` — mid-run checkpoint/resume with
  RNG continuity (`run_head` / `resume_to_end` / `run_via_resume`).
- `crates/pcx-play/src/fuzz_report.rs` — bounded TRACKER-ready divergence
  filings + artifact names.
- `crates/pcx-play/src/lib.rs` — four `pub mod fuzz_*;` lines only.
- `docs/re-notes/fuzz-tierc.md` — this note.

## Mechanics-lane boundary (hands off)

Claimed by `mechanics` and never edited here: `rpl`, `runner`,
`scenario`, `trace`, `diff` (`crates/pcx-play/src/`), plus
`camera_setup_host`, `native_coverage_host`, `widget_enumeration_host`,
`world_snapshot_host`, `tests/minimize.rs`, `docs/re-notes/script-items.md`.
This lane reads their public types (`Scenario`, `RunConfig`, `Trace`,
`Divergence`, `run`, `generate`) and mirrors the runner's schedule/RNG
order in `fuzz_resume.rs`. Grammar, ordering or codec changes there must
be mirrored here; the mirror is documented at the top of each module.

Seed minimization (`tests/minimize.rs`) stays with `mechanics`: the
"minimal reproducing seed" filed by `fuzz_report.rs` is the corpus seed +
recipe version. Shorter-horizon / fewer-unit reduction is their lane.

## Design

- Corpus: 32 fixed seeds (small, large, bit-pattern edges) × 3 cycled
  shapes (100t/4u/8a, 300t/8u/16a, 600t/12u/24a). Work per entry is at
  most ~7,824 (`ticks * (units + 1) + actions`).
- Generation: per-seed `Rng` picks the synthetic map tag; a derived
  `Rng(seed, index, recipe)` feeds `generator::generate` modulo selection.
  Names are `fuzz.s<seed>.<index>`; assets are `vanilla`, no fix rows.
- Resume: `run_head` executes `0..upto` with the same grouping as
  `runner::run` (BTreeMap schedule, BTreeMap player commands, single `Rng`,
  sorted unit snapshots, sorted fix rows) and keeps the live `Rng`,
  `draw_index`, `timer_seq` and prefix records. `resume_to_end` continues
  without re-seeding, so `run_via_resume == run` for every split point.
- Report: `format_tracker_entry` emits one append-only TRACKER line with
  seed, recipe, both trace hashes, the one-line `pcx-diff` report and a
  bounded RPL prefix (2,048 bytes; filing capped at 4,096). Full RPL is
  reproducible from the seed + recipe; the artifact name is
  `diverge-fuzz-s<seed>-<index>-<tick|header>.rpl`.

## Open items

- `run_via_resume == run` passes at four split points in the smoke scenario,
  at the midpoint split for every one of the 32 corpus entries end to end,
  and a zero-length checkpoint of a `u64::MAX`-tick synthetic scenario succeeds
  without reserving the full trace (`fuzz_resume::tests`). The fixed 32-entry
  corpus generates within `default_limits` and every entry stays inside the
  tick/unit/action/work/output ceilings (`fuzz_corpus::tests`). The
  4,096-byte filing bound is enforced with a UTF-8 char-boundary backoff and
  covered by `fuzz_report::tests`.
- Every corpus entry regenerates byte for byte, generates a distinct scenario
  body — compared without the scenario and map identity lines, so a generator
  that ignored the seed would be caught — and traces byte-identically across
  repeated runs (`fuzz_corpus::tests`).
- The candidate domains are refused rather than selected from: an empty map
  list, an empty owner list and an empty action-kind list each come back as a
  typed error instead of a division by an empty list (`fuzz_scenario::tests`).
- Real terrain/tileset fuzzing waits on map-container wiring owned
  elsewhere; the map tag here is a synthetic identity only.
- Whether the corpus should also cover `hd`/`reforged` asset modes or
  non-empty fix rows is undecided; currently `vanilla` with no fix rows
  so mode confusion is impossible.
- Stale-claim expiry follows the parallel-ownership note (claims persist
  until released).
