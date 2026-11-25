# num-traits 0.2.19 compatibility patch

Based on the published `num-traits` 0.2.19 crate (MIT OR Apache-2.0), SHA256
`071dfc062690e90b734c0b2273ce72ad0ffa95f0c74596bc250dcfd960262841`.

Numeric limits now resolve to primitive associated constants instead of imported
legacy `core` numeric modules. Float radix parsing uses the same primitive
infinities and NaN, including the original sign handling. Mathematical `consts`
remain explicit module paths. Public numeric traits, checks before unsafe casts,
width comparisons, arithmetic and decoding are unchanged.

Existing test-only numeric module imports and deprecated primitive limit methods
receive the same mechanical substitution. Trait methods such as
`Bounded::min_value` are retained, including their assertions. This remains
within the published Rust 1.60 floor.

Published license notices and other source files are retained. Package lockfiles
and Cargo cache/package stamps are omitted; the workspace lockfile owns product
resolution. The published manifests are unchanged.

See [the workspace compatibility note](../../docs/re-notes/rust-dependency-warnings.md)
for provenance, validation scope and remaining upstream Clippy style debt.
