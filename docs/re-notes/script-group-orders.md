# Script group orders

Covers `GroupImmediateOrder`, `GroupImmediateOrderById`, `GroupPointOrder`,
`GroupPointOrderById`, `GroupPointOrderLoc` and `GroupPointOrderByIdLoc`.

## Source evidence

- Native entries and signatures: `.scratch/group-orders-20261020/native.txt`.
  Each group native's target has the group, unit-null and order-id shape that
  the host admits. Source: shipped `Warcraft III.exe`, reviewed image SHA-256
  matched by `menu_lighting::REVIEWED_IMAGE`.
- Enumeration callsites: `.scratch/group-orders-20261020/consumer.txt`.
  - Immediate: `0x502da2` pushes the bound `push eax; push 0x0c; push 0x502750`
    (12, callback `0x502750`) before the enumeration call at `0x502360`.
    The delivery window at `0x502dd0` pushes `-1` (`6a ff`) and calls `0x502360`
    with callback `0x5029f0`. The `-1` is the format's no-cap sentinel.
  - Point: `0x5030c2` pushes the same bound and callback, with two `mov`
    instructions (`89 4d e0`, `8b ca`) between them. Delivery at `0x5030ee`
    pushes 12 and calls `0x502360` with callback `0x502b30`.
- Callback behavior: the per-unit order consumers only defer events and cannot
  run group-mutating callbacks (`callbacks.txt`), so the host reads retained
  membership directly.

## Behavior

- Point delivery is capped at the native bound. The host visits at most that
  many members of the group in retained order and stops. Immediate delivery has
  no cap and visits every member.
- A group order returns true when any visited member accepts the order (OR).
  An empty or null group returns false. An unknown order name or id returns
  false. An invalid location (null or removed) returns false. Wrong-type
  arguments and non-finite coordinates refuse with an error.
- Per-unit support is checked once with a null unit before any member is
  visited. Unsupported per-unit orders refuse without touching members.

## Decoder

`group_order_rules::Rules::reviewed_legacy` reads four reviewed windows from
the selected executable and checks:

- Each bounded push is `push eax; push n; <intervening>; push callback`. The
  intervening bytes must match exactly at their authored offset. A `0x68`
  elsewhere in the window is not accepted. The point preparation's intervening
  moves are `POINT_PREPARE_MOVES`.
- Immediate preparation, point preparation and point delivery share the bound.
  Immediate and point preparation share the callback.
- Immediate delivery uses `push -1` and the same enumeration target as point
  delivery.

The bound is decoded from the supplied executable at runtime. Nothing from the
original binary is copied into tests or source. Tests build synthetic windows
that use the same instruction shapes, and they cover moved intervening bytes,
truncated windows, missing callback pushes, out-of-range bounds and mismatched
bounds.

## Limitations

- Only the reviewed image digest is accepted. Other builds refuse at startup.
- `GroupPointOrderLoc` and `GroupPointOrderByIdLoc` resolve the location through
  the existing location host before the group is read, so a removed location
  refuses.
- Campaign startup admits group orders only after group membership and unit
  activity are admitted, and only once per host.
- Rendered or live-route behavior is not covered by this note. The route
  evidence lives in `.scratch/ledger-route-20261020/`.
