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
