# Browser asset reads

The browser store now distinguishes an absent member from a present member that
cannot be selected or decoded. Font discovery uses this fallible path: decoder
errors retain their source archive instead of becoming a missing-font report.
The existing optional-byte API remains available for its older callers.

`peacecraft3::browser_read` exposes receipts containing the requested path,
normalized key, import revision and decoded length. Archive reads retain every
equal-content contributing archive's ordinal and name. When every matching
archive has a distinct name in the selected 1.29b client family, its native
priority selects one source, independent of import order: `Deprecated` (9),
`War3xLocal` (4), `War3x` (3), `War3Local` (2), then `War3` (1). The selected
`Warcraft III.exe` SHA-256 is
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
At PE addresses `0x428dd8–0x428e56` it passes these archive priorities to
`0x439740`; `0x4398ba` stores the priority in each archive node,
`0x437637–0x4376a2` inserts nodes in priority order, and
`0x435cfe–0x435e06` searches from the high-priority end. This is
version-specific native-code evidence, not a claim about other builds or
archive authenticity. ISO member labels retain path/version/offset provenance.
Unknown names
or duplicate recognized names with unequal content still refuse with
`AMBIGUOUS_MEMBER`. Hydrated cache entries
report unavailable origins because the flattened store did not retain them.

The ISO picker preselects one distinct, recognized 1.29b archive family,
including its local and expansion members, when each source fits the 512 MiB
per-archive budget and the set fits the 1.25 GiB aggregate budget. A lone
archive remains selectable by default. Duplicate names, unknown companion
archives and over-budget sets require an explicit choice; the picker does not
silently discard a source to make an incomplete set fit.

A local ISO made from the five supplied 1.29b archives selected all five by
default in Chromium and committed 279 runtime paths. The browser returned
`GlobalStrings.fdf` with SHA-256
`641f5df8d6bdd7f73074d1afbaff49924398766867333b5f8fec5fc2d7917788`,
equal to a separate extraction from the supplied `War3xLocal.mpq`. The local
receipt is `.scratch/integration-preservation-20261021/original-browser-gate.json`;
the temporary ISO and extracted FDF were removed. This checks archive choice and
import, not rendered menu or full gameplay fidelity.

`read_checked` checks the cached slice before copying it. It refuses uncached,
present archive members rather than using an extractor without a supplied
bounded policy. `read_bounded` accepts explicit copy, packed-member and decoded
limits plus an optional caller-supplied Huffman table. The copy limit intersects
the decoded limit before extraction. A malformed matching hash row in the
selected source, including one unreachable through its lookup chain, is an
error; it does not permit fallback to a later archive. A selected higher-priority source likewise
cannot fall back to a lower one. Successful extraction does not implicitly
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
temporary extracted bodies were deleted after the check. That extraction gate
does not establish release attribution, browser drawing or native fidelity;
the selected 1.29b archive order above comes from the separate client-code trace.

Captured commands, exit codes and hashes are local under
`.scratch/browser-read-root-review/`. No original bytes belong to the commit.
The public demo has not been rebuilt or deployed by this change.

## Selected archive sources for map admission

`browser_archive_source::CommittedArchiveSource` implements the existing
`ContentSource` boundary over one explicitly selected committed archive. The
browser module's ordinary Rust factories inspect the real archive index/revision
and mount that selection. Callers supply exact logical/member mappings,
`SourceInfo`, packed/decoded limits, resolver priorities and map-binding inputs.
The explicit [map configuration API](browser-map-configuration.md) builds on this
selection boundary; neither layer supplies automatic source selection or UI policy.

Each inventory query and read checks the selected revision, index, UTF-8 name,
byte length and header while holding revision then archive guards. Reads borrow
the existing archive and use bounded extraction with an explicit absent Huffman
table. They neither clone the archive nor consult flattened renderer aliases.
Unadvertised or physically absent members remain absent; malformed matching
entries and decoder failures retain typed extraction causes. A failing selected
source cannot fall through to another archive.

Selection is metadata plus the owner's generation contract, not a content hash
or unique instance witness. Legacy appends do not advance revision; they preserve
existing indices. Arbitrary replacement with identical metadata and no revision
bump would be undetectable. Existing drop clears and increments in separate
steps. These APIs do not promise threaded, whole-store or cross-source snapshots.
Ingress, decrypted tables, caller inventory and decoder working memory remain
outside the per-read payload policy.

Five public native cases cover exact selection, identity/inventory errors,
limits, unreachable matching entries and resolver-to-application publication
with stale-source rollback. A separate test-only export in the actual Cargo-built
application WASM exercises the real browser globals: four mounted synthetic
archives feed bounded map admission, a real drop invalidates their sources and
preserves the old context/receipts, and explicit selection refuses lower-source
fallback. All three application gate sections pass under Node with matching
wasm-bindgen glue. The existing 14 browser-export groups also pass on that test
artifact. This is actual application code with test-only invocation, not a new
production FFI route, live-browser acceptance or gameplay execution.

Root acceptance records are local under `.scratch/browser-source-root-review/`.
Source mounting UI, script Host effects, game rendering and the public-site
refresh remain separate work.
