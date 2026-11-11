# `SetUnitUseFood`

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Static disassembly only.
Shipped use: `SetUnitUseFoodBJ` (`Blizzard.j:3861`) calls `SetUnitUseFood(whichUnit, enable)`; TFT
`NightElfX04` uses it.

## Handler `0x4abc70` (native table row at `0x49d2d5`)

- Resolves the unit handle (`0x483940`); a null result returns with no effect. The owner is
  `0x692340` (the unit's `+0xec` slot, looked up in the player table).
- Flag word `[unit+0x16c]`, bit `0x200` is "this unit's food use is counted".
- Enable (`[ebp+0xc]` nonzero) with the bit clear: calls `0x4b57d0(owner, fused)` and sets
  `0x300`. Enable with the bit set does nothing.
- Disable with the bit set: calls `0x4b57d0(owner, -fused)` and clears `0x300` (`and
  0xfffffcff`). Disable with the bit clear does nothing.
- `fused` is `0x6902a0(unit type)`, the type record's field `+0x5c` (the pair `fmade` is `+0x60`,
  read through `0x690270`).
- `0x4b57d0`: adds the amount to the player counter object at `player+0x90` (`0x7fb2b0` read,
  `0x7fb380` write) and raises the recorded maximum at `player+0x2ac` when the new value exceeds
  it. Inferred from the pairing with `+0x60` and the unit lifecycle below: the counter is food
  used.

## Initial state and lifecycle

- The unit factory `0x687ca0` takes a creation flag word (argument 6). With `0x100` set it adds
  `fused` through `0x4b57d0` and sets `0x300` (`0x688028`, `0x68802d`); with `0x400` it adds
  `fmade` through `0x67a210` and sets `0xc00`. `CreateUnit` reaches it through `0x663ca0` with
  `0x502`, so a script-created unit starts with the bit set. Two other factory callers
  (`0xb585ad`, `0xb5bb21`) pass `0x404` and set `0x300` after creation (`0xb585bf`,
  `0xb5bb2f`).
- Three teardown paths test the bit first: `0x4aed72` and `0x6831c2` and `0x69d33e`
  subtract `fused` only when `0x200` is set and clear it. A unit whose bit was cleared by
  `SetUnitUseFood(false)` therefore never subtracts, and one re-enabled does.
- Morph (`0xb736cf`, re-add at `0xb737bc`): the old type's `fused` is subtracted only when the bit
  is set, and the new type's `fused` is re-added only for those units, so a cleared bit survives
  a morph.
- The food-made counter is untouched by this native.

## Recreation (`unit_food_use_host.rs`)

The host derives food used from the live units (`live_food`), so the state is the set of units
whose bit was cleared: `fused` is left out of the owner's total for those units, `fmade` is not.
Enabling removes the unit from the set. The native is admitted when the selected source declares
it. A null or removed unit does nothing (removal is inferred from the failing handle lookup).
A dead, illusion or under-construction unit refuses: how the bit starts on those is not decoded.
The counter's maximum at `player+0x2ac` is not modeled.
