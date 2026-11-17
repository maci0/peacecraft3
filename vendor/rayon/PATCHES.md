# rayon 1.12.0 compatibility patch

Based on the published `rayon` 1.12.0 crate (MIT OR Apache-2.0), SHA256
`fb39b166781f92d482534ef4b4b1b2568f42613b53e5b6c160e24cfbfa30926d`.

Remove the 44 redundant plumbing glob imports from upstream commit
[436ff3c5742c31d01d349203e2c8315e909c8463](https://github.com/rayon-rs/rayon/commit/436ff3c5742c31d01d349203e2c8315e909c8463).
The published `repeat.rs` has different context; the same import-only removal is
applied there. The broader parent-module imports remain.

Four atomic `fetch_update` calls and the corresponding iterator doctest use
`try_update`. Ordering arguments, closures, returned results and retry behavior
are unchanged. The normalized and original manifests declare the local Rust
1.95 floor, replacing upstream 1.80: the standard-library
[`atomic_try_update` stabilization attribute](https://github.com/rust-lang/rust/blob/1.100.0/library/core/src/sync/atomic.rs)
records 1.95.0. The workspace requires 1.99 and validation uses 1.100.
No deprecation allowance is added.

Published license notices and other source files are retained. Package lockfiles
and Cargo cache/package stamps are omitted; the workspace lockfile owns product
resolution. The only manifest changes are the explicit local Rust floors noted
above, where applicable.

See [the workspace compatibility note](../../docs/re-notes/rust-dependency-warnings.md)
for provenance, validation scope and remaining upstream Clippy style debt.
