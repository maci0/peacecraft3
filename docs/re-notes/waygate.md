# Waygate natives and transfer

Executable: `Warcraft III.exe`, 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, ImageBase
`0x400000`. Addresses are virtual addresses in that image. Evidence labels:
**observed** is read from the disassembly, **inferred** is a reading of observed
code, **open** is not decoded.

Chapters that failed startup with `native WaygateSetDestination is unsupported`
(RoC): NightElf02, NightElf05, Orc07, Undead04, Human09. Every waygate in those
scripts is a `nwgt` unit, and every call pairs `WaygateSetDestination` with
`WaygateActivate(gate, true)`.

## Registration (observed)

The native table pushes `(signature, name, handler)` and calls `0x8ccc80`. The
five waygate entries are consecutive at `0x49e103` to `0x49e162`:

| Native | Name string | Handler |
|---|---|---|
| `WaygateGetDestinationX` | `0xe72d44` | `0x4afc60` |
| `WaygateGetDestinationY` | `0xe72d5c` | `0x4afcb0` |
| `WaygateSetDestination` | `0xe72d74` | `0x4afd40` |
| `WaygateActivate` | `0xe72d8c` | `0x4afc20` |
| `WaygateIsActive` | `0xe72d9c` | `0x4afd00` |

## Native handlers (observed)

Each handler converts its unit argument with `0x483940` (null gives null, see
`removed-unit-locations.md`), then looks up the owned ability with `0x6d6570`
using code `0x41777270` (`'Awrp'`, returned by `0xb32600`). The lookup pushes
flags `1, 1, 0, 0` before the code, as the resource-amount lookup does
(`unit-resource-amount.md`).

- No unit, or no owned `Awrp`: the getters return `0.0` (X, Y) or `0` (IsActive),
  and the setters do nothing. The IsActive handler returns `0` in that path.
- `0x4afc60` and `0x4afcb0` call `0xb32590`, which copies the destination pair from
  the ability state at `+0x70` into an output buffer. X is the first float, Y the
  second.
- `0x4afd40` calls `0xb32c40(ability, x, y)`, which calls `0x7f6ef0` on the state
  at `+0x70`.
- `0x4afc20` calls `0xb33040(ability, flag)`, which calls `0x7f6ba0(flag, state)`.
  It then reads the ability's `+0x30` object (created by `0x7f2610` when null) and
  calls `0x67a900`. The set branch passes `0xb`; the clear branch argument and the
  effect of the call: open.
- `0x4afd00` tail-jumps to `0xb330a0`, which calls `0x7f6c10(state)`.

## State storage (observed, layout inferred)

The state at `+0x70` is a slot index. The accessors reject a slot of zero or
above `0xff`. The global object at `0x115e9ec` holds a 2-float row at `+0x74`
per slot (written by `0x7f6ef0`, read by `0x7f6c40`). `0x7f6ba0` sets or clears
a bit in the global bitset at `0x1161d90` and adjusts the counter at `+0x58` of
the global object when the bit changes. `0x7f6c10` tests the bit. Table and bit
defaults for a fresh slot: open. The recreation refuses reads of a field never
written, rather than assuming a default.

## Transfer on a move order (partly observed)

- `0xb6a610` tests the dword at `+0x34` of the handle object against `0xd0012`.
  The Move class takes `0xd0012` (`input-smart-order.md`), so the test is inferred
  to be the current order. The routine then reads the target through `0xb20b30`
  and `0x6b8740`, looks up `Awrp` on that target with `0x6d6570`, and calls
  `0xb330a0` (IsActive). When the target is an active waygate it reads the
  destination with `0xb32590` and calls `0x699860` with `0xd016b` (observed call
  arguments; the meaning of that call is inferred). The follow-up order and the
  arrival rule are open.
- `0xb686a0` (callers `0xb654d1`, `0xb654e4`, `0xb69167`, `0xb698c3`) is a larger
  transfer routine. It uses order and event IDs `0xd0012`, `0xd0014`, `0xd0148`,
  `0xd014a`, `0xd0161`, `0xd0165`, `0xd0166`, `0xd016e`. Its position and
  effect steps are open.
- `0xb66ff0` returns `0` when bit `0x400` of the dword at `+0x454` or bit
  `0x8000000` of the dword at `+0x164` is set, when the dword at `+0x3f8` is zero,
  when two helpers (`0x7fb2b0` and `0x6967a0`) return nonzero, and when its owned
  `Awrp` is active. Otherwise it returns `1`. The flag and helper meanings are
  open. Callers `0xb64861` and `0xb69152`.

Open, not decoded: which units transfer (ownership, alliance, size), when the
transfer happens (arrival, collision, or order completion), where the unit lands
(destination exact point or nearest placement), what happens to the unit's order
after the transfer, the `Ambt` check in `0xb6a610`, the effect call `0x67a900`,
and what an unset destination means.

Data (observed): `Units\UnitAbilities.slk` row `nwgt` has `abilList` `Awrp,Avul`.
`Units\AbilityData.slk` row `Awrp` is named `Warp` with `DataA1` and `DataB1` set
to `400`. The meaning of those fields in the transfer is not decoded.

## Recreation behaviour

- `WaygateSetDestination`, `WaygateActivate`, `WaygateGetDestinationX/Y` and
  `WaygateIsActive` store and return the state on a unit whose `abilList` names
  `Awrp`, in `crates/pcx-play/src/unit_waygate_host.rs`. Setters on other units
  do nothing, and getters return the missing-ability zero.
- A destination or flag read before its write refuses with `waygate ... was never
  written`. A non-finite destination refuses.
- A move order onto an active waygate refuses in `follow_target_native`
  (`unit_follow_host.rs`) with `destination transfer is not decoded`, before any
  state changes. The transfer is not implemented.
- Tests: `unit_waygate_tests.rs` (state, missing-ability path, null handle,
  refusals) and `unit_follow_tests.rs` (move onto an active waygate refuses; the
  same order onto an inactive waygate follows).
