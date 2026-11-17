# ttf-parser 0.25.1 compatibility patch

Based on the published `ttf-parser` 0.25.1 crate (MIT OR Apache-2.0), SHA256
`d2df906b07856748fa3f6e0ad0cbaa047052d4a7dd609e231c4f72cee8c36f31`.

Make seven returned borrowed types' elided lifetimes explicit, including three
additional layout/variation paths exposed by the default feature set. Gate the
private glyf outline-point helpers on their sole `variable-fonts` consumer.
Gate stored PostScript glyph indexes on `glyph-names`, retaining the unconditional
array read, bounds check and `names_data` position. The public `names()` API is
available without `glyph-names`; a synthetic truncated-array/name test covers
both feature configurations.

The COLR clip-box format byte is still read with `?`; its local binding is
underscore-prefixed because only `variable-fonts` consumes the value. Parsing,
advancement and rejection behavior are unchanged. No public format API or parser
validation is removed. The published Rust 1.63 floor is unchanged.

Published license notices and other source files are retained. Package lockfiles
and Cargo cache/package stamps are omitted; the workspace lockfile owns product
resolution. The only manifest changes are the explicit local Rust floors noted
above, where applicable.

See [the workspace compatibility note](../../docs/re-notes/rust-dependency-warnings.md)
for provenance, validation scope and remaining upstream Clippy style debt.
