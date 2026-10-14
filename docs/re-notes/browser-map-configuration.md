# Explicit browser map configuration

The browser can submit caller-owned metadata for the same bounded map binding and
admission path used by the application. This publishes parsed map data and scripts;
it does not initialize a gameplay Host, execute campaign scripts, or enable TFT.
The existing imported-menu edition restrictions and launcher selection remain
independent and unchanged.

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

These gates do not establish live browser controls, original-map execution,
native behavior, rendering, or a refreshed public demo.
