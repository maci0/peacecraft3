# Bounded map-script import

`pcx_play::map_scripts_import::import_map_scripts` takes a borrowed `pcx_map::Map`,
explicit map label and `AssetIdentity`, caller-supplied `common.j`/`Blizzard.j`
`ScriptAsset` values, and `MapScriptBudget`. It extracts authored `war3map.j` and
optional `war3map.wts`, preserves source labels and reproducibility digests, and
returns compiled `MapScripts`. It does not lower WTG, substitute preludes or
implement a game Host.

Compilation diagnostics retain the resolver's actual prelude or map source label
and render positions against that source. Prelude labels cannot alias the VM's
reserved main-source identity `the map script`; this is a caller-identity check,
not native script rejection behavior.

Resolution walks every call argument recursively, including calls nested inside
global initializers, return expressions and other calls. Unknown nested names and
wrong nested arities refuse compilation, and nested native calls contribute to
the admission statistics. Previously only the outer expression call was checked;
an undeclared call inside its arguments could escape this gate. Synthetic
regressions cover these refusal paths and repeated nested-native accounting.
Array access also resolves its base: undeclared arrays and indexed native
functions refuse during compilation, for both reads and assignment targets.
The correction passes 118 script, 1,427 core and 607 app tests, strict native/WASM
Clippy and formatting. Three resolver process checks cover failure exit codes,
stderr diagnostics and nested-native statistics. The selected Continue gate
still publishes Human01's 152 units before Human02 refuses its unsupported
nearby-harvest order; these compiler checks do not establish campaign completion.

## Allocation order and limits

Prelude lengths and the embedded archive length are checked first, including
platform layouts and checked aggregate arithmetic. `Header::parse` reads through
a borrowed `Cursor`; no archive clone or table allocation is needed to inspect
counts. Serialized table byte sums, typed table vector layouts, declared extents,
version-zero framing and mathematically valid nonzero u32 sector sizes are checked
before table decoding. `pcx_mpq::checked::CheckedArchive` borrows the embedded
archive; it does not copy the whole archive or the selected packed member.

After table decoding, unique member metadata is checked for independent packed,
decoded, script/WTS limits and packed extents before extraction. The final
aggregate counts archive bytes plus decoded script/WTS and both prelude lengths;
its initial archive-plus-prelude check runs before table decoding. Exact limits
are accepted. Layout failures and impossible arithmetic remain typed refusals.

`DEFAULT` limits are harness policy, not values measured from the game. Table
buffers are bounded separately. Decoder temporaries, error/source strings, parsed
WTS structures, compiler/AST allocations and runtime state are not covered by a
total process-memory guarantee. These are input-byte and allocation-layout gates.

## Shared member reader

`Map::extract_checked(source_label, member, ReadBudget)` exposes the same input
policy to other map consumers. The explicit budget has separate archive, serialized
table, packed-member and decoded-member byte ceilings. The shared reader has no
default policy. It preserves the script importer's version-zero, self-consistent
end-table profile; this does not expand supported MPQ formats. Existing
`Map::extract` and archive APIs retain their behavior.

A `Map` keeps the decrypted tables of its last checked read (keyed by source label
and budget, so every error still names its own label and limits) and of its first
`Map::extract`; later reads skip the table decryption. `Map::checked_archive`
returns the same reusable `CheckedArchive`. A clone starts without them, equality
ignores them, and a failed parse is never kept. The unchecked path holds a copy of
the archive bytes while the `Map` lives.

Missing members return `None`; matching malformed references and conflicting
locale/platform/block identities return contextual errors. A decoded member's
bytes are still owned. Table buffers and decoder temporaries still allocate.
Callers must choose member/parser and aggregate input limits separately; the
shared ceilings do not bound total live memory.

Ten actual 32-bit WASM consumer groups check limits, framing, selected extents,
ambiguity, exact duplicate identities, absence, corrupt references and importer
failure precedence. On an 8,528-byte synthetic archive with an 8,192-byte stored
member, a native allocator probe records one request at least archive-sized through
`Map::extract`, and zero through the checked path or its decoded-budget refusal.
Three independent executions agree. This measures allocation requests for those
inputs, not RSS, timing or native game behavior.

A bounded local community-map comparison (`MY SHEEP TAG (with cameras).w3x`,
release identity unestablished) returns identical bytes through old and checked
readers for all seven inspected members: camera, JASS, WTS, info, terrain, pathing
and placements. Each checked read uses exact declared table/packed and observed
decoded limits. Original bytes and detailed source metadata stay in local scratch;
this is extraction correspondence, not map execution or native camera proof.

## Member and WTS selection

The lookup uses the existing MPQ reader's bounded probing policy. Two differing
locale/platform/block identities are refused; no locale or archive priority is
chosen. Exact duplicate identities are allowed. The ambiguity count is a witness
of the first two distinct candidates, not a full-table census.

A matching corrupt block reference, deleted/nonexisting state or out-of-bounds
payload is an error, not absence. Optional WTS absence remains `None`. Present
WTS extraction, framing or duplicate-ID failure yields `MalformedWts`. Bodies stay
raw; unused non-UTF-8 text is not replaced or rejected wholesale. The existing
`MapScripts` localization contract applies when a string is actually requested.

Budget, archive, missing-member, ambiguity, layout/reservation, extraction/WTS and
compilation errors retain map/member or allocation-region context. Unsupported
compression stays a refusal; memory-archive imports do not infer a neighboring
Storm.dll or embed codec tables.

## Verification

Synthetic integration tests compile actual supplied preludes and map scripts,
check source digests/localization, and distinguish required-script absence from
malformed input. Additional tests deliberately combine oversized declarations
with malformed tables/payloads to check failure precedence. They cover independent
limits, exact aggregate boundaries, ambiguity variants, corrupt WTS references,
malformed/duplicate WTS, invalid sector shifts and payload bounds.

Four actual 32-bit WASM source-leaf cases exercise table sums that exceed usize,
table layouts that exceed isize, and invalid sector shifts. All four refuse before
archive/decoder work; the isolated integer-ABI harness is local scratch evidence,
not production unsafe code or a native game probe.

The explicit original gate uses local inputs, never directory-based release
labels or silent skip-on-missing. Its required environment variables are:

- `PEACECRAFT3_MAP_IMPORT_COMMON`, `PEACECRAFT3_MAP_IMPORT_BLIZZARD`,
  `PEACECRAFT3_MAP_IMPORT_MAP`: exact local decoded source paths.
- `PEACECRAFT3_MAP_IMPORT_EDITION`, `PEACECRAFT3_MAP_IMPORT_VERSION`,
  `PEACECRAFT3_MAP_IMPORT_SOURCE`: caller-verified identity, with unknown archive
  release identity stated explicitly rather than inferred from adjacent binaries.

Run with `cargo test -p pcx-play --locked --test map_scripts_import
-- --ignored --nocapture`. The ordinary suite has 18 synthetic importer checks;
the original gate is opt-in.

## Measured local original result

On the inspected Legacy-directory archives, common/BJ members from `War3.mpq` and
`War3x.mpq` were decoded independently and compared byte-identical. Human01 was
found in `War3Local.mpq`. Explicit source selection used the base prelude copies
and that unique map; this establishes no engine archive-priority rule.

The archive SHA-256 identities used for this local gate are:

- `War3.mpq`: `60a4e380b3ae753b304be837edac53bebed0ae69b611e7f2ebfc2b036bf00d47`
- `War3Local.mpq`: `f986d2767f341e9a665ece3807e9242b254ba1efad6d2858304eb11166aef8fd`
- `War3x.mpq`: `69fb8fd8aefe1657d75d53f970e20dbd9d9c23d42a3c2582cca4a97a16becfc7`

Compilation passed and retained all four supplied/extracted source digests.
The actual first NullHost execution refusal was `GetPlayerNeutralPassive` at the
supplied common.j line 176, column 67. The previous compatibility-source
`ConvertPlayerColor` observation is not substituted for this measured outcome.
Archive release identity was explicitly recorded as unverified.

Decoded originals, per-member SHA-256 values, extraction provenance and the actual
gate log remain only in `.scratch/map-import-root-original/`. They are not bundled
or uploaded. Native neutral-player semantics and active context remain unresolved
in `native-neutral-players.md`. This result proves original script ingestion and
compilation plus an honest execution boundary, not campaign success, cinematic
playback, editor-trigger execution or community-map gameplay parity.
