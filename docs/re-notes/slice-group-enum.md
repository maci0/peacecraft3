# Group target orders

`GroupTargetOrder` and `GroupTargetOrderById` issue `IssueTargetOrder` and
`IssueTargetOrderById` for members already in the group.

## Source

Selected 1.29 image, the same one read by the point-order rules. Wrappers:

- `GroupTargetOrder` (`0x498700`) resolves the order string first. An unknown
  order returns false. It then resolves the widget (`0x483ba0`) and the group
  (`0x481f20`). A null result from either resolver returns false. A resolved
  pair calls `0x503130`.
- `GroupTargetOrderById` (`0x498780`) resolves the widget before the group. A
  null widget returns false without reading the order id or the group. It then
  calls `0x503130` with the integer id.

`0x503130` prepares with `push 12` and callback `0x502750`, then delivers with
`push 12` and callback `0x502c00`. Both calls go through `0x502360`. That is
the same member bound and preparation callback as point orders, so delivery
stops after `Rules::point_member_limit` members in retained order. The group
call returns true when any visited member accepts.

Per-unit delivery uses the existing target-order consumers in the same order as
`ObjectHost::native`: harvest, inventory, follow, construction, then combat. A
null unit is only the support probe. If none of those consumers answers, the
group order refuses and leaves membership unchanged.

## Not in this slice

`GroupEnumUnitsOfType` and the counted enum variants have no host.
The later range-enumeration section supersedes the former range-native refusal. `ForGroup` and `GetEnumUnit` are unchanged. A group native that
the loaded program does not declare stays unsupported.

The synthetic test is `target_orders_resolve_the_widget_before_the_group_and_reuse_follow`
in `crates/pcx-play/src/group_host_tests.rs`. It was not executed in this change.

## Canonical range enumeration (2026-11-13)

`GroupEnumUnitsInRange` and `GroupEnumUnitsInRangeOfLoc` now use canonical
accepted positions and the existing same-VM enumeration/predicate continuation.
Both declarations must match the admitted unit source. The location form copies
its point before enumeration; a null or retired location returns without
changing the destination. No rectangle-shaped approximation or candidate
collision radius is used.

The follow-up frontier recorded on main at `1225e137` is the unchanged RoC
**Undead03** call through `Blizzard.j:3437`, not Orc03. This implementation
must still be tested against that actual mission frontier; synthetic success
is not an unchanged-map horizon result.

### Native evidence and source bindings

Static bounded disassembly uses Legacy `1.29.2.9231`, executable SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Wrappers `498000`/`498080` join `502460`; the location wrapper reads its point
before resolving the group. `502460` calls `7f4cf0` with a zero fourth argument,
which selects `8094f0`. For the unlimited all-owner query, both owner masks
start empty and the latter wrapper selects **`807ad0`**. The earlier preliminary
`8077b0` window is a distinct owner-qualified branch, not the final all-owner
consumer.

The all-owner consumer subtracts the spatial origin from coordinates through
`9a00b0`, then normalizes coordinates and radius with matched bit/exponent
operations at `807b2e..807b99`. Radius admission loads its comparison threshold
at `807af7`; the selected pointer is in a PE-loader-zero-filled region. The
new decoder reads those operands from the same admitted placement image. It
checks the placement and query scales agree, retains provenance in the existing
placement binding, and defers unsupported/malformed range operands to an explicit
query error. Other placement operations remain usable.

`807d3c` squares the normalized radius through `99ffe0`. `807d71` calls
`9a1110`, which subtracts the two centre coordinates, squares each difference
and adds them with the same integer-backed real arithmetic already modeled by
`script_random::real`. `807d7e..807d82` compares that result with the squared
radius and skips only distances above it. The edge opcode is decoded rather
than assumed. There is no candidate collision-radius read on this final path.
Broad-phase conversion through `808f40`/`801930` floors cell bounds and increments
the upper bounds, so a zero-radius query still visits its containing cell.

The spatial collector `809170` suppresses removed links and duplicate entries,
combines entry flags through `809030`, and rejects entries masked by the query
or marked unavailable. The final consumer rejects a retired backing record or
missing widget pointer before `5028e0` resolves a unit and runs the predicate.
Canonical queries omit removed and hidden records; the hidden spatial exclusion
has separate original observations in [unit visibility](script-unit-world.md).
The special entry-bit mapping is not fully modeled: known Locust candidates
refuse explicitly rather than guessing their eligibility. No owner, life,
pause or sleep filter is added to the ordinary canonical unit query.

### Execution and remaining qualifications

Successful null-filter queries replace group membership atomically after
allocation and membership-budget preflight. Bad geometry, unsupported rule
sources, wrong handle families and admission failures leave group membership
and the callback stack unchanged. Filtered queries retain the existing engine's
semantics: clear first, insert accepted members before the next predicate,
retain earlier script effects on a later callback error, and restore nested
`GetFilterUnit` state. A false predicate still runs in the same VM and can change
the actual script globals.

Candidate traversal is the existing creation-order snapshot. Native spatial
ordering, births/removals during callbacks, special eligibility flags and
original-client boundary/numeric comparisons remain qualified gaps; static code
is not an original runtime comparison. The tests cover nonstock normalization,
threshold and edge operands, source decode refusal, zero radius, boundary
inclusion, shown/removed identities, owner independence, location mutation,
nested predicates, preflight error atomicity and callback-error recovery.
`supplied_group_range_image_decodes_the_selected_inclusive_center_test` is an
explicit ignored gate requiring `PC3_RANGE_IMAGE`; it proves source binding and
reconstruction results, not native execution. Original disassembly stays local
under `/tmp/pc3-range-*`. No original asset bytes are committed.

Validation uses the isolated `/tmp/pc3-fidelity-sol61-08-range-target` with two
build jobs and verified private Xvfb/null audio. All sixteen enumeration tests
pass, including the five named `range_*` regressions. Both rule/refusal tests
pass, and the explicit selected-image gate runs separately and passes. Strict
native and WASM Clippy and both library builds pass. These nineteen test
executions include actual named regressions rather than a zero-test filtered
run. The unchanged Undead03 application horizon and original-client comparison
are not covered by these library checks.

## Type and counted enumeration (2026-11-13)

The additional entry points are `GroupEnumUnitsOfType`,
`GroupEnumUnitsOfTypeCounted`, `GroupEnumUnitsInRectCounted`,
`GroupEnumUnitsInRangeCounted`, and `GroupEnumUnitsInRangeOfLocCounted`.
They use the existing owned-VM continuation and canonical group membership.
Type queries share the lazy raw-name table with `UnitId`; admitting only a type
query does not admit an undeclared `UnitId`. The existing table's UnitUI-name
loader inference and ambiguous-registration-order refusal remain in force.
There is no rawcode-string fallback, query trimming, or WTS conversion in this
lookup. ASCII case normalization is shared with `UnitId`.

### Static native evidence

The selected executable identity is unchanged from the range section above.
Bounded local disassembly `/tmp/pc3-type-enum-wrappers.txt` identifies the type
wrappers at `498280` and `4982e0`. Both resolve a script string through `469010`
and look up the resulting raw text through `4770c0`, the same helper called by
`UnitId` at `4af290`. A null or unknown name returns before resolving the group,
filter, or count and before clearing destination membership. A known type
resolves the group, resolves its predicate, and calls `502580`; the counted
wrapper forwards its integer argument, whereas the uncounted wrapper reads a
signed immediate at `4982d1`.

`502580` initializes enumeration through `503310`, then scans the global unit
list through `7f4ba0`/`7f4c00`. That scan checks the backing record and widget,
resolves actual units, and excludes retired objects. This path does not use the
spatial shown-state eligibility check. Type queries therefore include hidden
extant canonical units. In `5028e0`, the type comparison at `50295a` reads the
unit's current type field, rather than a creation-request or base type. Canonical
queries compare `record.definition.rawcode`; removed-unit entries are excluded.
Life, owner, pause and sleep restrictions are not added to a type query.

The shared state initializer reads the caller limit into the group field at
`50336b..50336e` and initializes the insertion counter with an immediate at
`50337b`. At `50292a..502934`, TEST/JE bypasses the quota check for a zero limit;
CMP/JAE compares the insertion counter and limit as **unsigned** values.
Consequently zero bypasses the limit, while negative script integers represent
large unsigned limits. There is no invented count cap or negative-to-zero clamp.
After predicate acceptance, `5029a9..5029b0` calls group insertion and increments
the counter only if that insertion succeeds as new. A predicate that manually
adds its candidate can therefore add membership without consuming an enumeration
insertion. Rejected predicates do not consume the quota. Once the quota is full,
remaining predicates do not run.

`unit_group_count.rs` reads default/reset operands from the admitted native PE
and validates the bounded zero-bypass, unsigned-comparison and insertion-only
instruction grammar. The uncounted type default is checked against the existing
rectangle, range, location-range and player wrapper operands. All read bytes
are charged to the placement profile budget. An unqualified image or changed
instruction grammar remains an explicit query error; ordinary placement remains
available. No native image operands are copied into production rule constants.

### Execution and limits

Counted queries reuse their existing shape/eligibility calculations. Null-filter
queries cap candidates before membership-budget preflight and replace the group
atomically. Filtered queries clear first and increment a frame-local quota only
for actual new insertions; nested calls preserve the outer quota and
`GetFilterUnit`. Existing callback errors retain prior script effects and unwind
the frame. Early null/unknown-name and null-location returns preserve destination
membership even if later arguments would be invalid. The counted rectangle form
also returns early for a null group or rectangle.

The synthetic checks exercise raw host strings, compiled function string
arguments and computed `TRIGSTR` prefixes; the old branch's global literal-WTS
rewrite is a separate migration. Native traversal order and mutation during
iteration remain qualified: these queries retain the canonical creation-order
snapshot. The counted rectangle path also retains the existing centre-containment
and shown-state qualifications. Static decoding and synthetic executions do not
establish original-client runtime parity or advance an unchanged-map horizon.

Validation on this branch used verified private Xvfb/null audio and the isolated
`/tmp/pc3-fidelity-sol61-08-range-target`, with two Cargo jobs. All nine driver
stages pass: 22 enumeration tests, two range-rule/refusal tests, the new nonstock
count-rule test, six UnitId tests, the explicitly executed selected-image gate,
strict native Clippy including tests, strict WASM Clippy, and both library builds.
The broad `group_count` filter additionally ran one unrelated control-group test;
there were 33 successful test executions in total, including that incidental
match. The selected-image gate remains ignored in the ordinary test sweep and
was separately executed successfully with its required local image path.

Named new regressions are
`type_enumeration_uses_raw_names_and_current_type_including_hidden_not_removed`,
`unknown_type_and_null_subject_short_circuit_before_group_filter_and_count`,
`all_counted_queries_treat_zero_as_unlimited_and_negative_as_unsigned`,
`counted_predicate_counts_only_new_insertions_and_restores_nested_filter_context`,
`counted_early_exit_stops_predicate_effects_and_preflight_uses_capped_membership`,
`counted_callback_failures_unwind_and_ambiguous_names_preserve_destination`,
`type_only_lookup_does_not_admit_undeclared_unit_id`, and
`nonstock_group_count_operands_unsigned_zero_and_decode_refusals`.
The existing selected-image gate now also checks the count operands and grammar.
Source hashes, per-stage command/exit receipts and complete output remain local
at `/tmp/pc3-sol61-08-type-{sources.sha256,validation-receipt.json,verified.log}`.
The literal-WTS migration, full application rebuild and unchanged-map horizons
are outside these branch checks.
