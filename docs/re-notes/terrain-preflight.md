# Terrain framing before allocation

`Terrain::from_bytes` and `parse_terrain` validate the complete W3E frame before
reserving decoded arrays. Primary and secondary fourcc lists, origin floats and
corner data use checked spans; corner counts and allocation layouts use checked
arithmetic. A truncated origin returns `TerrainTruncated` instead of panicking.
Zero corner dimensions cannot represent the stored tile-dimension-plus-one
relationship and return a contextual error. One-corner axes retain their existing
zero-tile behavior.

Fallible reservations preserve the section and source byte offset. Borrowed
`parse_terrain` validates before copying raw bytes. Successful decoding retains
authored lists, ordering, origin bits, corner words and raw bytes. `Map::terrain`
preserves MPQ extraction errors, including the missing member name, rather than
turning them into a synthetic zero-length terrain error.

The public synthetic tests cover proper prefixes, forged list counts, truncated
origins, dimension/product overflow, raw-field preservation, extra data and the
embedded map/archive consumer. A separate executed 32-bit WASM gate covers count,
span and decoded-layout overflow. These establish parser behavior, not native map
admission or rendered terrain behavior.

This is preallocation validation, not a caller memory budget. Large valid inputs
can still require large arrays; archive extraction and the caller's owned input
already exist before this parser runs. World scale, passability, height conversion
and native terrain semantics remain separate requirements.
