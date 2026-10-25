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

## Rectangle item and destructable callbacks (2026-10-25)

The shared host now implements `EnumItemsInRect`, `GetFilterItem`, `GetEnumItem`
and the corresponding destructable enumeration/responses. Optional declarations
are checked with their creation family's common-source ownership and exact ABI
before catalog admission. The action must be a loaded no-argument, no-result
function; the optional filter must be an admitted filter/condition wrapper.
Queries and callbacks reuse the existing arena and VM activation stack.

Ground items use canonical coordinates and exclude carried, consumed and removed
records. Destructable enumeration includes hidden and dead records so library
restore/show actions can address them. The current query snapshots inclusive
centre containment in creation order, defers new births to the next query, and
skips records removed or carried before their turn. Those edge/order choices are
explicit recreations; native spatial-boundary and mutable-iteration parity
remain unmeasured. No extra handle or parallel world is allocated.

Filters and actions retain separate response contexts. Nested queries restore
outer item/destructable contexts, and a callback failure unwinds responses through
the existing callback cleanup. Tests execute nested real JASS filters, hide the
canonical destructables, discriminate boundary/outside positions, exercise marked
carried/removed records and deferred births, and retry after callback errors.
The unchanged TFT intro now executes its item-hiding callback and next refuses
`SetItemVisible`; enumeration success is not item-visibility or full cinematic
support. No original-client enumeration-order comparison was run.

## Item visibility (2026-10-25)

`SetItemVisible(item, boolean)` and `IsItemVisible(item)` retain one visibility
state on the canonical item record. Optional declarations must have the exact
nonconstant ABI and the creation declaration's source. Visibility rules are
admitted before item creation, with the selected player/client identity checked.
Missing rules leave the initial state unknown, rather than guessing a default;
the production input reader always supplies them.

Local static PE evidence uses selected image SHA
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The getter at `4991a0` resolves an item and complements/masks its flag word. The
item constructor `6c3320` calls widget constructor `676940`, whose flag initializer
is decoded together with the getter mask. The typed item vtable connects the
show/hide methods; the hide OR and show complement-AND must agree with that mask.
Nonstock synthetic flag/default fixtures and truncated/malformed relationships
exercise the decoder; an explicit local-original test executes the real reader.
These are local static observations, not an original-client runtime comparison.

The selected setter `4a91c0` skips a resolved held association. The host therefore
ignores held setters and reports held items invisible to the world while retaining
their inventory icons. Removed/consumed/null items read false; null/removed setters
are no-ops, and wrong-family or malformed calls refuse. A drop returns the item to
the source-derived initial ground state. That drop-reset choice, hidden-target
pickup refusal and cancellation on hiding a pending pickup remain recreation
choices pending original pickup/drop edge comparisons.

Hidden items remain in rectangle enumeration, allowing the original intro's
show callback to restore them. The renderer hides an existing actor and retains
its source animation clock; even a hidden newborn admits its model clock before
culling and waits to spawn a draw until revealed. Picking reads the
current canonical visibility before testing actor bounds, including a visibility
change before the next render update. Held/removed actors still retire normally.

Real-VM rectangle callbacks test hide/show, nulls and wrong arguments. Inventory
tests exercise a hidden script-inserted item, suppressed held setters, drop and
removal; delivered-map startup proves the same canonical state is published.
Core, application, integration and strict native/WASM checks pass. An isolated
software-Vulkan RoC run checks an actual source ground item's actor visibility
and actual ray picking before/after projection, including an exact hide/show
round trip. Captures and a machine-readable receipt are retained locally; the
broader render gate remains red on sky illumination and terrain/tree sight
occlusion. No original-client visibility comparison was run. The unchanged TFT
chapter gets through this frontier and returns control; its subsequent gameplay
horizon still refuses `CreateCorpse`, so the full entry test remains red.
