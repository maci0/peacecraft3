# Selected map data admission

The application library exposes `map_binding::bind_map` and
`map_requests::install`. A caller supplies a configured Resolver, independent
map/common/Blizzard/catalog identities, expected source identities, required
tables and fields, and input limits. Source mounts, archive ordering, release
identity and field semantics are not inferred from filenames or menu selections.

The binder retains exact resolver receipts and shares the selected map bytes.
Unit and item overlays come from that same map. Missing optional overlays remain
absent; present read or parse failures stop binding. Repeated custom target IDs
refuse before the existing overlay representation could overwrite them. This
is local import validation, not a native replacement rule.

`prepare_map_context` preflights the required terrain, pathing, map-info, unit,
script and optional WTS member metadata and aggregate input lengths before
extraction. The result owns terrain, measured grid alignment, imported raw world
records, independent source identities, member checksums and shared compiled
scripts. Unresolved object records retain their problems. Admission does not
initialize a VM, instantiate game entities or replace missing game rules.

`MapRequests::replace_selected` prepares a candidate binding and context before
publishing them with their receipts. Failed candidates preserve the old
publication. Requests validate binding revision, declared edition/version and
exact equality with the map used for object binding. Shared-byte requests avoid
another full map copy. Native file drops check metadata and reserve fallibly
before reading; missing configuration refuses before opening the dropped path.

The project launcher includes a load-selected-map-data action. It writes the
existing request with the retained identity, label and shared bytes, and reports
read/parse failures using their Display diagnostics. Missing configuration is
reported to the user. The detailed typed requirements remain available to
configuration callers. This action does not change the game edition or enable
extensions. Default native/browser source mounting remains unimplemented; a
caller must configure bindings explicitly.

`MapRequests::replace_selected` uses `bind_map_bounded`. Its selected Resolver
sources must enforce caller limits before payload reads and decoding. Unsupported
selected sources refuse without trying another layer or legacy reads. The legacy
`bind_map` API remains available with its post-read checks. Explicitly configured
MPQ sources retain independent packed/decoded admission limits and optional
caller-provided Huffman tables; archive mounting and table ownership precede
these per-member checks. Override files check length before reserve/read.
Configured CASC adapters now propagate the caller ceiling through verified
TVFS/ENCODING resolution and BLTE decoding. Declared oversize refuses before
encoded payload I/O; the decoder ceiling is also no larger than the resolved
declared size. Mounted encoded limits and existing integrity checks remain.
This adds no default namespace, member inventory or source priority.

Limits cover specified input byte/count admission, not combined memory use.
Table/profile/AST storage, receipts, queued messages, decoder working memory and
overlapping old/candidate contexts have no combined memory bound here. Source
revision/snapshot consistency is caller policy. Checksums describe reproducibility,
not authentication. Existing stock schema and text-encoding permissiveness remain
separate limitations.

Public tests use the actual application library and installed Bevy message
consumer, stock Editor/MetadataCatalog, same-map overlays and launcher activation
handlers. They check provenance, exact caps, ambiguity, malformed inputs, FIFO
delivery, file drops, stale bindings and rollback. Current Cargo-resolved Bevy
0.20 native and actual 32-bit WASM consumers pass. Earlier private WASM evidence
using Bevy 0.17.3 is not current application integration evidence.

A local `(4)LostTemple.w3m` data probe selected common.j directly from
`War3Patch.mpq`, Blizzard.j directly from `War3x.mpq`, and the existing stock
reader against `wc3/game/app`. These are explicit probe choices, not native
archive priority or archive-release attribution. The map checksum was
`4ef64ea58ed40134`. Admission retained 121 placement records, 21 definitions and
35 unresolved records. An earlier explicitly selected `war3.mpq` common.j was
refused because it lacked the map's `GetChangingUnit` declaration. The compatible
candidate compiled; its host-free script probe then refused an unsupported
native. No gameplay started and no original bytes were saved or included here.

There is no production gameplay Host connecting this parsed context to player,
unit, handle and event state. Persistent VM ownership and source diagnostics
exist, but original initialization, game entities, scheduling, rendering and
campaign/community-map execution remain open.
