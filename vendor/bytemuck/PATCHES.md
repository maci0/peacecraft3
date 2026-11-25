# bytemuck 1.25.2 compatibility patch

Based on the published `bytemuck` 1.25.2 crate (Zlib OR Apache-2.0 OR MIT), SHA256
`95832e849adfb21180ccb6826a99da14e5d266ae5c2e668e1602cf234f153797`.

Replace 24 deprecated primitive `min_value`/`max_value` calls in
`src/contiguous.rs` with `MIN`/`MAX`. The unsafe `Contiguous` implementations,
representation types, valid ranges, bool bounds and nonzero lower bounds are
unchanged.

Both manifests declare the local Rust 1.43 floor required by the integer
associated constants. The preserved upstream README and crate documentation
advertise Rust 1.34; that promise does not apply to this local patch.
The workspace floor is 1.99 and the November 18, 2026 validation used Rust 1.100.

Published license notices and other source files are retained. Package lockfiles
and Cargo cache/package stamps are omitted; the workspace lockfile owns product
resolution. The Rust-version declaration is the only manifest change.

See [the workspace compatibility note](../../docs/re-notes/rust-dependency-warnings.md)
for provenance, validation scope and remaining upstream Clippy style debt.
