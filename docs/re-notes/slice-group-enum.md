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
The later range-enumeration section supersedes the former range-native refusal.
`ForGroup` and `GetEnumUnit` are unchanged. A group native that the loaded
program does not declare stays unsupported.

The synthetic test is `target_orders_resolve_the_widget_before_the_group_and_reuse_follow`
in `crates/pcx-play/src/group_host_tests.rs`. It was not executed in this change.

## Historical range frontier from unchanged campaigns (2026-11-13)

At the baseline, the unchanged RoC Undead03 opening-skip/horizon probe reached
`GroupEnumUnitsInRangeOfLoc` through `Scripts\\Blizzard.j:3437` and refused:
neither range native had a host consumer. The shared continuation engine then
supported rectangle and player queries only. The later implementation below
adds candidate geometry and reuses that same-VM predicate path.

Bounded GNU objdump observations on selected executable 1.29.2.9231,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`:

- Registration `49cdd0..49cdda` binds `GroupEnumUnitsInRange` to `498000`.
  It resolves the group, resolves the predicate, and calls `502460` with the
  supplied coordinate/radius pointers and an unlimited count argument.
- Registration `49cde4..49cdee` binds `GroupEnumUnitsInRangeOfLoc` to `498080`.
  It first resolves the location through `4823e0`, copies fields `24` and `28`
  into locals, then resolves group/predicate and calls the same `502460`.
  An absent location returns before resolving or changing the group.
- `502460` prepares enumeration through `503310`, then calls `7f4cf0` with
  callback `5028e0`. Its zero fourth-argument branch reaches `8094f0`, which
  dispatches to spatial scan implementations including `8077b0`. Native scan
  geometry, quantization, eligibility and ordering still require qualification;
  the rectangle consumer's creation-order policy does not prove range parity.

These are static native-code observations and a recreation script refusal,
not an original-client comparison. Bounded disassemblies and the failed probe
remain local under `/tmp/pc3-range-enum-*` and
`/tmp/pc3-horizon-sweep/roc-Undead03.w3m/`. That baseline did not include range
support or a successful Undead03 horizon.

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

Independent main integration runs the full gameplay package: 3,169 passed,
zero failed, 144 explicit ignores. All ten named new range/membership tests
execute, and the selected-image range decoder gate runs separately and passes.
Strict native/WASM all-target Clippy, both libraries, formatting and the
application test-binary rebuild pass with source hashes unchanged. Receipts
are retained in `/tmp/pc3-group-range-main-validation/`.

The unchanged Undead03 replay before range intake publishes 185 units but
already stops at the default `ACsw` acquisition deadline,
`0.019999999552965164`; after integration it stops at that same frontier.
The before/after receipts are `/tmp/pc3-undead03-before-range/` and
`/tmp/pc3-group-range-horizon-main/`. Neither run reaches the historical range
call or completes the horizon. No automatic-cast work is skipped to reach it.
