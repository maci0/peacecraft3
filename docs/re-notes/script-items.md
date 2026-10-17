# Script-created item records

`ObjectHost::with_item_catalog` admits `CreateItem` against the selected Program's
positional `integer, real, real -> item` declaration. Admission checks declaration
source ambiguity, constant status and item ancestry through `widget` and
`handle`, using the existing object contract checker. Missing or incompatible
declarations refuse. Formal parameter names do not define the ABI.

The production startup factory opts in through `MapStartupBudgets::items`.
`None` preserves unsupported behavior without requiring an item declaration.
An admitted factory reuses the same selected catalog Arc and map Source used
for unit creation, the same VM and the same EnumHost handle allocator.

A successful call retains the resolved Definition, explicit install/edition/map
Source and executed x/y arguments in an `ExecutedItem`. Catalog resolution checks
both the requested rawcode and item family. The definition keeps its existing
metadata results and authored modifications; unsupported fields stay explicit.
Integer arguments in real positions use the VM's existing numeric promotion.
Real signed zero is preserved. Non-finite coordinates refuse as recreation
policy; no native runtime behavior is asserted for those inputs.

Caller budgets bound creation records, retained fields and modifications; the
shared handle limit also applies. Argument, record and handle capacity failures
refuse before catalog resolution. Catalog, family and retention failures allocate
no handle and spend no creation record. The existing allocator uses fallible
reservation. These budgets are resource policy, not original game limits.

`executed_item` reads only a typed item entry. Timer identities and generic
objects are not adopted as items, and timer destruction refuses item handles.
Tokens remain local to the published session. No initial owner, life, inventory,
world entity or render state is inferred. Other item natives, death events,
world publication, picking and rendering remain unsupported by this slice.

## Verification boundary

Synthetic core tests exercise numeric arguments, signed zero, catalog/family
refusals, record/handle/field/modification budgets and shared timer allocation.
Delivered-map App tests parse an actual synthetic ItemData SYLK table through
the selected resolver and binding pipeline, then execute globals/config/main
through the production factory. The retained definition equals the bound
catalog's resolution. Candidate refusals preserve the previous published
session; a valid replacement retires it.

Native tests, WASI execution and the actual browser-target Cargo test artifact
are separate gates recorded in the root review evidence. These checks establish
admission and executed state, not original item behavior, rendered items or a
playable campaign. No supplied original asset contents are included in this
implementation or its fixtures.
