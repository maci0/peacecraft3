# simd-adler32 0.3.10 compatibility patch

Based on the published `simd-adler32` 0.3.10 crate (MIT), SHA256
`3a219298ac11a56ea9a6d2120044824d6f01aeb034955e7af7bc16858527deea`.

Remove only `pub use hash::*`. The private `hash` module declares trait
implementations and no reexportable items. The module, its implementations and
all public hashing APIs remain compiled. No SIMD or scalar algorithm changes.

Published license notices and other source files are retained. Package lockfiles
and Cargo cache/package stamps are omitted; the workspace lockfile owns product
resolution. The only manifest changes are the explicit local Rust floors noted
above, where applicable.

See [the workspace compatibility note](../../docs/re-notes/rust-dependency-warnings.md)
for provenance, validation scope and remaining upstream Clippy style debt.
