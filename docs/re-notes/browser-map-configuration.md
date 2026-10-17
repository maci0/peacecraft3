# Explicit browser map configuration

The browser can submit caller-owned metadata for the same bounded map binding and
admission path used by the application. This publishes parsed map data and scripts;
it does not initialize a gameplay Host, execute campaign scripts, or enable TFT.
The existing imported-menu edition restrictions and launcher selection remain
independent and unchanged.

## Local file controls

The demo's collapsed **Advanced map configuration** panel requires a local JSON
file and a caller-entered byte ceiling. The controller checks `File.size` before
`arrayBuffer`, validates the returned length, and decodes UTF-8 with fatal errors.
No source policy or metadata template is generated. Archive inspection uses the
entered zero-based ordinal and the current committed revision.

Only one controller is installed per form. It submits through the existing Rust
inbox, polls its own request identifier, frees temporary handles, and retains the
latest completed result until replacement or disposal. A failed JavaScript poll
retains the identifier for a later frame; it does not acknowledge completion.
Archive imports disable configuration controls. An import beginning during a
metadata read refuses that submission before it reaches Rust. Completion is
reported as map-data parsing, not playable gameplay.

## JavaScript boundary

The WASM module exports these functions:

| Function | Result |
|---|---|
| `inspect_map_archive(index, committed_revision)` | An owned `MapArchiveInspection`, containing selected archive metadata or its original typed failure |
| `submit_map_configuration(text, max_metadata_bytes)` | An owned `MapConfigurationResult`, containing a submitted request ID or its original typed argument/decoder/source/queue failure |
| `poll_map_configuration_result(id)` | Pending, an owned completed map outcome, or an explicit unknown/acknowledged request failure |

Numeric arguments require primitive finite nonnegative integers within the target
range. Identifiers additionally fit `u32`. Binding revisions and archive byte
lengths are exposed as decimal strings, preserving values beyond JavaScript's safe
integer range. Inspection exposes no archive bytes.

Call `status()`, `kind()` and `diagnostic()` on replies. A completed request can
have a refusal kind such as `map-binding`; only `map-admitted` denotes successful
admission. Completed polling acknowledges the inbox. The returned handle still
owns the original outcome afterwards; release handles with `free()` when finished.
Argument failures retain the existing JavaScript Error through `argument_error()`.
Diagnostics are views over retained causes, not substitutes for their Rust types.

## Mandatory metadata

The exact schema tag is `pcx-map-config-v1`. Every object rejects unknown and
duplicate keys; every field is required. No source priority, release identity,
edition, catalog field, codec input or allocation policy is inferred.

| Area | Explicit fields |
|---|---|
| Envelope | `schema`, `codec`, `mode`, `binding_revision`, `sources`, `map`, `common`, `blizzard`, `catalog`, `limits` |
| Source layer | Archive index and committed revision, unique priority, source info, exact logical/member mappings, packed and decoded extraction ceilings |
| Source info | Unique ID, kind `mpq`, exact inspected origin, version `{ "state": "unknown" }` or `{ "state": "known", "value": "..." }` |
| Each script/map role | Source-layer ID, logical path, diagnostic label, independent source/edition/version identity strings |
| Catalog | Identity, label, allowed source-layer IDs, explicitly selected `SlkId::ALL` table paths, raw four-byte field IDs represented by four `u8` values |
| Reader | Listed-path, per-asset and aggregate input ceilings |
| Overlay | Unit, item and aggregate input ceilings |
| Binding | Map-file and resolver-input ceilings |
| Admission | Terrain, info, pathing, units, aggregate input and required-field ceilings |
| Script import | Script, WTS, archive, archive-table, packed, decoded, prelude and aggregate input ceilings |

`mode` uses the existing four-axis resolver grammar. It is a caller policy, not
install detection. `codec` currently accepts only the exact `no-huffman` string;
table-backed browser configuration remains unimplemented. Unknown versions must
be declared explicitly. A role's provenance is checked against the actual resolver
winner during binding; a structurally valid source reference is not sufficient.

`binding_revision` is canonical ASCII decimal `u64` text, with no leading zeros
except `0`. All other indices, limits, priorities and field bytes are JSON integer
numbers checked against their destination types. Strings, fractional/exponential
numbers, booleans, negative values and overflow do not coerce into those fields.
The decoder source defines the exact nested field names and types.

## Publication and limits

One request may be pending, in flight, or awaiting acknowledgement. Source work
runs outside the inbox mutex; source callbacks and retired request/catalog
destructors can reenter the inbox without that lock being held. Only successful
binding replaces the active `MapRequests` state. A refusal retains the previous
context, receipts, bindings and active outcome, while the separate reply owns the
new failure. This is normal-return behavior, not panic recovery or cancellation.

`SelectedMapSources` retains the owned configuration's resolver alongside the
exact published `BindingReceipts` arc. Runtime asset preparation can obtain a
reader through `for_receipts`; copied equal-content receipts and receipts from a
replaced configuration cannot select it. A failed configuration preserves the
previous reader, and reloading the same bindings keeps that receipt association.
Retired readers are dropped outside the inbox lock. Readers retained by an
existing consumer are not rewritten by later selections.

This ownership does not freeze external sources. In particular, a committed
browser archive still refuses reads after its generation changes. Read budgets,
archive policy and provenance remain on the existing resolver path. The borrowed
`MapRequests::replace_selected` API does not transfer resolver ownership; an owned
configuration or an explicitly retained caller reader is still required for
runtime reads. No Host construction or `config`/`main` execution is added here.

The UTF-8 text ceiling is checked before serde allocation, after wasm-bindgen has
already copied the JavaScript string into Rust. It is not a total-memory bound.
Mounted archive tables, inventory metadata, decoder/compiler working memory,
retained outcomes and JavaScript copies remain outside that ceiling. The selected
archive adapter retains its existing generation/metadata limitations described in
[browser asset reads](browser-asset-reads.md).

Actual Cargo-built application WASM with matching wasm-bindgen glue passes five
Node groups: archive selection, numeric guards, owned decoder/source errors,
real App publication/acknowledgement, and stale-source rollback. These exercise
the actual browser globals and existing consumer with synthetic encrypted MPQs.
Seven JSON and five public consumer groups also execute from the Cargo integration
test WASM, and two lock/exhaustion groups execute from the library unit-test WASM.
These distinguish metadata validation, wrong declared source identity, whole-state
rollback, source callback reentry and rejected-request cleanup. The native app
suite passes 200 tests, with seven original-input tests ignored by default.

The argument-driven Node runners are
`tests/support/browser_configuration_public.cjs` and
`tests/support/browser_configuration_private.cjs` under the app crate. Each takes
the path to matching wasm-bindgen-generated Node glue for its respective Cargo
test artifact. Captured root commands, artifact identities and exits are local in
`.scratch/browser-configuration-root-review/`.

Seven controller groups cover read limits, invalid UTF-8, handles and request/import
interleaving, including an actual Cargo-WASM App behind a fake DOM. Separately,
the optimized production page passes eight browser groups using synthetic local
MPQs: explicit archive inspection, pre-read byte refusal, typed metadata failures,
one submission through the running main App, stale-source refusal and result
cleanup. It uses no test-frame export. The previous menu selection and base-only
TFT restriction remain intact. Its 14 requests were local code/WASM GETs, with
no uploads, sockets or page errors. The proof is local in
`.scratch/browser-configuration-ui-root-review/functional-main-app-policy-baseline/`.

The retained earlier attempts expose two harness mistakes: filling a field before
opening its disclosure and assuming the preceding ISO import left no menu selected.
The final test opens the disclosure first and compares selection before/after
configuration. The published asset-free page subsequently passes the same eight
groups with 14 code/WASM GETs and no uploads, sockets or page errors; all 22 hosted
files match the checked package. Its private proof is in
`.scratch/browser-configuration-ui-root-review/live-functional-main-app/`.
These checks do not establish original-map execution, native behavior or full
rendering fidelity.
