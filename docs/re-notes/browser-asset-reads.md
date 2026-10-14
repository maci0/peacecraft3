# Browser asset reads

The browser store now distinguishes an absent member from a present member that
cannot be selected or decoded. Font discovery uses this fallible path: decoder
errors retain their source archive instead of becoming a missing-font report.
The existing optional-byte API remains available for its older callers.

`peacecraft3::browser_read` exposes receipts containing the requested path,
normalized key, import revision and decoded length. Archive reads retain every
equal-content contributing archive's ordinal and name. Unequal content refuses
with `AMBIGUOUS_MEMBER`; no archive priority is inferred. Hydrated cache entries
report unavailable origins because the flattened store did not retain them.

`read_checked` checks the cached slice before copying it. It refuses uncached,
present archive members rather than using an extractor without a supplied
bounded policy. `read_bounded` accepts explicit copy, packed-member and decoded
limits plus an optional caller-supplied Huffman table. The copy limit intersects
the decoded limit before extraction. A malformed matching hash row, including
one unreachable through the archive's lookup chain, is an error; it does not
permit fallback to a later archive. Successful extraction does not implicitly
publish bytes into the cache.

The actual WASM exports include `stored_asset_checked` and
`stored_asset_bounded`. The bounded JavaScript route requires primitive finite,
nonnegative integer budgets representable by the target `usize`, and the exact
explicit codec policy `no-huffman`. Invalid arguments refuse before accessing
the store or archive locks. Table-backed JavaScript codec configuration is not
implemented; Rust callers can supply a table. Exported read handles retain the
receipt and expose the decoded bytes. Font/FDF/skin errors retain their existing
JavaScript string boundary; read failures become JavaScript errors.

These are per-read admission limits, not a total-memory ceiling. Archive mounting
and table allocations, decoder working memory, receipt metadata and JavaScript
input/output copies remain outside them. The revision denotes committed import
generations: legacy `add_asset` and `add_archive` do not increment it, so it is
not a complete store snapshot identity. Path normalization remains the existing
browser convention, not evidence for native namespace or locale policy.

## Verification

The application passed 124 unit tests and 57 public integration tests, with six
original-input tests explicitly ignored by default. Strict workspace Clippy
passed for all native and WASM targets/features with warnings denied.

The actual Cargo-built browser binary, with Bevy 0.20 and matching wasm-bindgen
0.2.129 glue, passed 14 Node test groups. They cover receipts, cache and archive
budgets, unequal archives, decoder failures, malformed matching hash rows and
the production font consumer. They also reject 36 invalid numeric arguments
and eight invalid codec arguments without coercion.

A separate local original-input gate extracted three explicitly selected
`War3x.mpq` members: `UI\FrameDef\Glue\StandardTemplates.fdf`,
`UI\FrameDef\Glue\MainMenu.fdf` and `UI\war3skins.txt`. Their decoded hashes and
lengths matched the earlier supplied fixture's provenance. Staging these bytes
through the actual WASM exports derived the same authored font file list. The
temporary extracted bodies were deleted after the check. This does not establish
archive priority, release attribution, browser drawing or native fidelity.

Captured commands, exit codes and hashes are local under
`.scratch/browser-read-root-review/`. No original bytes belong to the commit.
The public demo has not been rebuilt or deployed by this change.
