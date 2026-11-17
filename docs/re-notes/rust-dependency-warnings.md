# Rust dependency compiler warnings

The Rust 1.100 fresh core dependency closure reported 446 compiler warnings:
365 num-traits, 48 Rayon, 24 bytemuck, seven ttf-parser, one simd-adler32 and
one Symphonia MP3. These are dependency compiler diagnostics, not original-game
behavior evidence. No original game assets or runtime imports are involved.

## Published sources

The six versions were the latest published stable, non-yanked releases on
2026-11-18, checked through each crate's
[crates.io API](https://crates.io/api/v1/crates/num-traits) endpoint
(`/api/v1/crates/{name}`). Cached published archives were SHA256-checked against
both the API release checksums and the exact workspace lock records before
extraction. Sources and license notices are retained, with only the documented
compatibility edits. Package lockfiles and Cargo cache/package stamps are
omitted, following the existing Parley patch pattern.

| Crate | Version | Published archive SHA256 |
|---|---|---|
| `num-traits` | 0.2.19 | `071dfc062690e90b734c0b2273ce72ad0ffa95f0c74596bc250dcfd960262841` |
| `rayon` | 1.12.0 | `fb39b166781f92d482534ef4b4b1b2568f42613b53e5b6c160e24cfbfa30926d` |
| `bytemuck` | 1.25.2 | `95832e849adfb21180ccb6826a99da14e5d266ae5c2e668e1602cf234f153797` |
| `ttf-parser` | 0.25.1 | `d2df906b07856748fa3f6e0ad0cbaa047052d4a7dd609e231c4f72cee8c36f31` |
| `simd-adler32` | 0.3.10 | `3a219298ac11a56ea9a6d2120044824d6f01aeb034955e7af7bc16858527deea` |
| `symphonia-bundle-mp3` | 0.6.1 | `98ea5ffc8716bff677dfb3b01b420c7b758de901a72b8c330bf2040ab74b4add` |

The root manifest patches these exact versions to their vendor directories.
Only their six lock records lose registry source and checksum fields; package
versions, dependencies, older ttf-parser and Symphonia 0.5.5 records are unchanged.

## Change boundaries

- [num-traits](../../vendor/num-traits/PATCHES.md): primitive numeric constants;
  public traits and unsafe-cast checks retained, with equivalent test updates.
- [Rayon](../../vendor/rayon/PATCHES.md): 44 upstream redundant import removals
  and four atomic API renames with unchanged orderings and closures.
- [bytemuck](../../vendor/bytemuck/PATCHES.md): 24 primitive limit constants,
  preserving all unsafe representation contracts and ranges.
- [ttf-parser](../../vendor/ttf-parser/PATCHES.md): seven explicit lifetimes and
  consumer-based private helper/storage gating. PostScript array validation,
  name access and COLR format-byte reads remain unconditional.
- [simd-adler32](../../vendor/simd-adler32/PATCHES.md): remove an empty reexport,
  retaining the module and its trait implementations.
- [Symphonia MP3](../../vendor/symphonia-bundle-mp3/PATCHES.md): store bitrate for
  Layer II and sample-rate index for Layer III, while retaining every parsed
  input, validation and frame-size calculation for all layers.

The normalized and original Rayon manifests now declare Rust 1.95 because
[`try_update` stabilized in 1.95.0](https://github.com/rust-lang/rust/blob/1.100.0/library/core/src/sync/atomic.rs).
Both bytemuck manifests declare 1.43 for
[integer associated constants](https://github.com/rust-lang/rust/blob/1.100.0/library/core/src/num/int_macros.rs); its
preserved upstream Rust 1.34 documentation is not a promise of this local patch.
The workspace Rust 1.99 floor already covers these changes. Crate versions and
editions are unchanged; compiler validation uses installed stable Rust 1.100.

## Validation and limits

Accepted machine receipts are `accepted-results.json`,
`accepted-core-gates-results.json`, `accepted-core-ordinary-result.json` and the
`accepted-core-build-*` entries in `extra-results.json`. Published byte and
license inventories are in `published-file-inventory.json`.

Receipts are local at `/tmp/pc3-dependency-warnings-20261118/`; the exclusive
checkout target is `/tmp/pc3-dependency-warnings-20261118-target`, with two build
jobs and no artifact copying. Cargo JSON uses normal `-v`; non-JSON stdout is
recorded as a gate error. Receipts identify source hashes, local package paths,
features, target, compiler/driver hashes, flags and emitted artifact hashes.

Cargo refuses feature selection and dependency tests for packages outside its
workspace. Private manifests therefore point directly to the vendored library
and test sources, with private dependency lockfiles. Published dev/optional
packages needed by these tests were fetched without editing existing registry
sources or changing the product lockfile. These manifests do not ship. Their Symphonia package metadata omits a
redundant homepage identical to the repository URL to avoid Cargo 1.100 manifest
noise; both vendored published manifests retain it.

The accepted frozen-source checks pass:

- 34 strict compiler invocations, 17 native and 17 `wasm32-unknown-unknown`,
  covering the workspace-selected dependency closure and explicit minimal,
  default and affected feature combinations. No compiler or Cargo warnings.
- Published native library/integration suites: 659 passed, one explicit upstream
  Rayon ignore. PostScript/header feature reruns add eight passing executions;
  these overlap the broader suites.
- Fresh pure `pcx-play` library suite: 3,311 passed, 156 explicit ignores.
- Pinned system Clippy 1.100 on the project `pcx-play` library: native and WASM
  pass with `-Dwarnings`. Both actual library builds also pass.
- Workspace formatting and the six-record lock audit pass. Every published
  license file is unchanged; linked library and test executable hashes still
  match their build receipts after testing.

The direct upstream style audit is separate: bytemuck passes; the other five
crates report 37 preexisting Clippy diagnostics in total. These include precision
and approximate constants, iterator/array style and a derivable implementation.

Strict compiler checks and upstream tests are recorded separately from direct
upstream Clippy style checks. The latter expose preexisting style diagnostics
(such as excessive floating-point precision and array-chunk suggestions) and do
not pass a blanket `-Dwarnings` audit. No suppression, algorithm rewrite or style
sweep is included. Earlier attempts that discovered an older Clippy plugin,
Cargo package-selection restrictions or additional feature warnings are not
current-toolchain acceptance. Their raw outputs remain separate.

No full application cold build, original-client run, browser playback, audible
or rendered parity, Windows execution or WebAssembly test execution is claimed.
Product integration remains subject to a fresh main dependency-closure rebuild,
as required by [build provenance](build-provenance.md).
