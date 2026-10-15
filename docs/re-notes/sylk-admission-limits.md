# SYLK coordinate limits and repeated cell writes

`Table::parse_bounded` takes explicit `CoordinateLimits` from its caller. Both
sticky coordinates are checked for C and F records before cells are inserted
or grid-extent postprocessing runs. An oversized coordinate returns
`CoordinateLimit` with the source record index, byte offset, axis, actual value
and caller limit. Zero limits refuse positive coordinates; invalid zero indices
retain the existing format error.

These are checked import limits, not game rules or native rejection behavior.
The advisory B dimensions do not select grid extent and are not these limits.
The ordinary `Table::parse` entry keeps its previous uncapped policy, using the
same parser implementation. A consumer must bound encoded source bytes
separately. This API does not promise a total record, work, allocator-memory or
process-memory ceiling; String, BTreeMap and anomaly allocations remain as before.

Repeated K writes now produce `DuplicateCell` warning diagnostics with the
actual row, column and zero-based source record index. Equal repeated values
also diagnose ambiguity. Extra K parameters within one record count as repeated
writes. The parser retains the same first cell/header value and generated-cell
metadata; this preservation policy is not an inferred native precedence rule.
A missing-K record followed by its first valid K value is not a repeated write.

Strict consumers can reject diagnostics affecting the headers or row identities
they consume. They must not take a successful permissive parse as proof of raw
cell uniqueness. Existing diagnostic numbering outside these new cases is
unchanged. Consumers that reject all warnings may now refuse inputs with repeated
writes that previously went unreported.

Synthetic tests cover sparse oversized C/F coordinates, exact and zero limits,
implicit/sticky cursors, advisory dimensions, equal/different repeated values,
header and quoted-byte preservation, missing K and generated-cell metadata.
The checks also execute in actual 32-bit WebAssembly. No original asset values,
layout defaults, native player rules or runtime binary observations are encoded
in this change.

This is a prerequisite for explicitly sourced map-terrain input admission. It
does not yet bind those inputs to an active map, decode terrain textures or
establish terrain UVs, atlas selection, blending or rendered/native fidelity.
