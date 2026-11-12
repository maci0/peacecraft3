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

`GroupEnumUnitsInRange`, `GroupEnumUnitsOfType`, and the counted enum variants
have no host. `ForGroup` and `GetEnumUnit` are unchanged. A group native that
the loaded program does not declare stays unsupported.

The synthetic test is `target_orders_resolve_the_widget_before_the_group_and_reuse_follow`
in `crates/pcx-play/src/group_host_tests.rs`. It was not executed in this change.

## Range frontier from unchanged campaigns (2026-11-13)

The unchanged RoC Undead03 opening-skip/horizon probe reaches
`GroupEnumUnitsInRangeOfLoc` through `Scripts\\Blizzard.j:3437` and refuses:
neither range native has a host consumer. The existing shared continuation
engine in `unit_position_host.rs` supports rectangle and player queries only.
An implementation needs both candidate geometry and that existing same-VM
predicate path; a location-to-coordinate adapter alone cannot close the gap.

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
`/tmp/pc3-horizon-sweep/roc-Undead03.w3m/`. No native implementation or successful
Undead03 horizon is claimed by this note.
