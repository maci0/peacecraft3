# Checked shadow-grid dimensions

`Shadows::from_bytes` checks both cell axes and their byte-count product before
comparing the supplied buffer length. A grid that cannot fit the target's
`usize` returns `Error::ShadowSizeOverflow`; a representable size with different
input length remains `BadShadowSize`. Zero axes retain empty-grid behavior when
the other cell axis is representable.

The checked layout indexes the original file buffer directly. Raw bytes,
canonical bytes, cell values and public accessors retain their existing format;
the duplicate cell-buffer allocation is removed. `Map::shadows` already uses
this parser, so no alternate loader or renderer path was introduced.

Synthetic overflow, empty-grid and cell-order cases pass on native and actual
32-bit WASI execution. Native/WASM Clippy, downstream compilation and strict
rustdoc pass. Original shadow sweeps were not rerun; these checks establish
arithmetic and parser behavior, not rendered shadow fidelity.
