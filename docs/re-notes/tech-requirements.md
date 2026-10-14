# Caller-selected authored field catalog

`pcx_objects::requirements::RequirementsCatalog` batches exact metadata fourccs
selected by the caller across the objects in `DefaultTables`. It does not select
an editor category or establish a field's prerequisite role. Each definition is
queried only for families that use its metadata table; this ownership rule does
not establish native applicability.

`RequirementsCatalog::new(tables, fields)` returns `Result<_, CatalogError>`.
Every identifier is checked before collecting outcomes, even when there are no
objects. An absent definition returns `CatalogError::UnknownField`, instead of an
empty catalog. Caller selection order and duplicates are retained within each
object; objects follow the underlying family/rawcode order.

Each `RequirementEntry` retains its authored rawcode, family, definition and one
`Object::field` result. Successful mappings borrow their metadata, source cell,
lexical value and identity. Missing cells, conflicting profile assignments,
malformed values and unverified selectors remain `Refusal` outcomes. Source
loading and excluded-row issues stay separately available in
`catalog.tables().unresolved()`; the catalog does not create absent objects.

The constructor now reports errors, so callers must handle its result. The unused
`TechnologyRequirements` alias was removed before public module integration.
No dependency count defaults to one, no all/any or `DependencyOr` semantics are
implemented, and no availability, native technology graph or gameplay parity is
claimed. This is a checked batch of the existing asset joins.

## Verification

Three active synthetic tests use renamed field identifiers and categories. They
check successful values and cell/metadata provenance, missing and malformed
cells, conflicting profiles, selector refusal, unknown selections with empty and
populated tables, metadata-family filtering, and selection order/duplicates.
They run against the actual crate module using the existing test-only table
fixture builder. There is no source-leaf self-import or blanket lint suppression.

The public API original-asset gate requires both an explicit install directory
and caller-selected identifiers:

```sh
PEACECRAFT3_LEGACY_DIR=/local/install \
PEACECRAFT3_REQUIREMENT_FIELDS=ABCD,EFGH \
cargo test -p pcx-objects --locked --test requirements -- --ignored --nocapture
```

`ABCD,EFGH` above are placeholders, not asserted game fields. Select actual
identifiers from the supplied metadata. The gate requires at least one successful
join, compares each mapped cell/value with a direct lookup, checks independent
source identity, and reports refusals separately. It deliberately labels archive
edition/version as unverified; nearby executable versions do not identify the
archives. Originals and generated content remain local.

The explicit local gate on `wc3/game/app`, selecting `ureq,urqa` as caller input,
verified 100 successful mappings and retained 2,120 lookup refusals separately
from 14 underlying source issues. These counts describe this supplied archive
set, not a release identification or native technology behavior. The crate's
native suite passed 46 tests with six original-asset tests ignored by default;
the selected-field original gate passed when explicitly enabled. Strict Clippy,
format checks and the warning-denied WASM library check passed.
