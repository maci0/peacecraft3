# Authored UnitWeapons row identity

The campaign objective replay found every local party unit with no active weapon.
The selected original UnitWeapons table carried weapon-enable cells, but its
identity header differed from the expected format header `unitWeapID`. Editor
built an empty row index, so the metadata-backed `uaen` lookup could not reach the
source cell. DefaultTables independently rejected that header and omitted those
weapon rows. Loading the map and moving units had not exercised this join.

Both consumers now use one private identity resolver. A unique standard identity
header remains authoritative, with all keys checked for four ASCII bytes and
case-folded uniqueness. When that header is absent, the resolver reads the
canonical identity columns from the supplied UnitData and/or UnitBalance tables.
It then considers every authored named weapon column and accepts exactly one
whose unique four-byte keys cover every supplied canonical identity. Source row
and column order do not participate in the decision. No spelling of the
unexpected header is an alias in the implementation.

The selected weapon table includes an additional authored identity absent from
both canonical tables. Requiring every weapon row to occur canonically would
therefore reject the real source. The proof instead requires complete canonical
coverage, while validating every weapon row, including additional rows, for
well-formed and unique identities. Additional weapon records remain source rows;
Editor does not thereby invent missing canonical UnitBalance definitions.

Missing canonical tables, empty/malformed canonical key sets, duplicate headers,
case-colliding or malformed identities, incomplete joins and multiple matching
columns refuse with source/member context. A malformed standard-header weapon
table also refuses rather than retaining the first duplicate. Editor loading
returns a contextual error; DefaultTables supplied-byte loading propagates that
refusal. Its internal assembly uses the same resolver and existing unresolved-row
channel rather than a second fallback policy.

Five synthetic integration tests exercise changed labels, reordered columns and
rows, source rows beyond canonical coverage, case-folded joins, one/two canonical
anchors, ambiguous candidates, missing/malformed/duplicate keys, partial joins,
and standard-header compatibility. They check both Editor field lookup and
DefaultTables cell lookup/provenance. The explicit local-original gate compares
`uaen` cells for the campaign party rawcodes through both consumers to the actual
selected source cell and checks weapon-row coverage for all supplied UnitData
identities. It passed with the local Legacy install on 2026-10-20:

```sh
PEACECRAFT3_LEGACY_DIR='<local original install>' cargo test -p pcx-objects --test weapon_identity local_original_weapon_fields_reach_editor_and_default_tables -- --ignored --nocapture
```

`cargo test -p pcx-objects` also passed: 82 tests, with nine explicitly ignored
local-asset gates. Strict Clippy passed for all native crate targets and the
browser-WASM library target. No original-game runtime victory or damage claim is
made by those checks.

The install path is explicit, and DefaultTables retains independent member
fingerprints and caller-supplied edition/version provenance. This establishes a
shipped-byte row join and source field availability. Combat consumption, damage,
orders, objective progression and native gameplay parity require their own
runtime checks. Original assets and generated copies remain local.
