# Explicitly profiled map cameras

`pcx_map::cameras::CameraFile` validates borrowed `war3map.w3c` records without
allocating names or record storage. `Map::cameras` extracts the member through the
shared checked reader and retains its caller-supplied source label. The smaller
of `ReadBudget.max_decoded_bytes` and `CameraLimits.max_bytes` applies before
member decoding. Missing members, extraction failures and malformed camera
framing remain distinct contextual errors.

The format basis is the pinned [primary W3C reader](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/parsers/w3x/w3c/camera.ts)
and its file reader. This is source-convention evidence, not native camera code.
The signed header word is preserved; it does not choose the record profile.
The caller selects `Base` or `LocalAngles` from explicit input provenance. The
primary implementation's external build selector adds three words after the
NUL-terminated name in its extended branch. No relationship between that selector
and MapInfo's unnamed metadata words is asserted.

Each record exposes its member-relative span, ten unmodified float-word bytes,
raw name bytes and span, and the optional three-word suffix. Non-UTF-8 names and
unusual float bits remain raw. There are no angle conversions, assumed defaults,
projection rules, generated transforms or player-camera selection.

Caller limits separately bound member bytes, record count and name bytes. Header
spans and a physical minimum-record quotient are checked before traversal;
unterminated names, truncated suffixes and trailing bytes are refused. Immutable
validated bytes support subsequent borrowed iteration. These framing rules are
checked import policy, not native malformed-input parity. Nested extraction
errors are boxed to keep the result error compact while preserving source chains.

Seven public framing tests cover both profiles, raw data, every truncated prefix,
version/profile independence, exact limits, forged counts and empty records.
Ten extraction groups exercise the actual Map accessor with independent budgets,
ambiguity, corrupt references, absence and framing failures. All run through an
actual 32-bit WASM consumer with no host imports, in addition to native tests.

One bounded local community-map gate, `MY SHEEP TAG (with cameras).w3x`, supplies
an eight-record, 403-byte member that tiles exactly with the explicit Base profile.
The alternative profile fails physical count framing. This establishes which
framing fits those bytes, not the engine's runtime profile-selection contract.
Release identity remains unestablished. Original bytes and detailed comparison
logs stay local in scratch.

The container is available for explicit inspection. Game camera application,
coordinate and angle semantics, animation and rendered/native behavior remain
open.
