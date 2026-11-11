# Wisp Harvest (`Awha`)

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Static disassembly only; no
original-client run. Implementation: `crates/pcx-play/src/wisp_harvest_runtime.rs`, rules in
`harvest_rules.rs` (`WispRules`). Peasant harvesting is in [worker-harvest-orders.md](worker-harvest-orders.md).

## Class and data (observed)

- `CAbilityWispHarvest`: vtable `0xf4c368` (RTTI), name string `0xf4c6b4`. Slot 7 `0xbbcfc0` returns
  the class code `Awha` (`0x41776861`). Both shipped rows, `Awha` and `Awh2` (the invulnerable
  variant), carry code `Awha`; the Wisp (`ewsp`) lists `Awha`.
- Data loader, slot 32 `0xbbd000`: after the base harvest loader `0xb75870` it reads `Rng` into
  `[+0x190]` (value at `+0x194`), `Dur` into `[+0x198]` (value at `+0x19c`), `DataA` into `[+0x1a8]` as an
  integer through `0x9a07b0` (a software float-to-integer truncation toward zero) and `DataC` into
  `[+0x1a0]`. No function in `0xbbcfa0..0xbbdc40` reads `DataB`.
- Shipped `Units\AbilityData.slk`: `Awha` has `Dur1` 8, `Rng1` 900, `DataA1` 5, `DataB1` 5, `DataC1`
  150. The metadata rows `Wha1..Wha3` are labelled `WESTRING_AEVAL_WHA1..3` in `War3Local.mpq`
  `UI\WorldEditStrings.txt`: Lumber per Interval, Intervals Before Changing Trees, Art Attachment
  Height. `DataC` is presentation, `DataB` has no reader in this class.

## Behavior (observed in code, event identities inferred)

- The ability listens on its owner unit for events `0xd0182`..`0xd0185` (registered at `0xbbd340`);
  `0xbbd480` dispatches `0xd0182`..`0xd0185`, `0xd01a0`, `0xd01c1` and `0xd01c5` to `0xbbe4b0`,
  `0xbbe640`, `0xbbe300`, `0xbbe560`, `0xbbe830`, `0xbbe710` and `0xbbdf70`.
- Interval handler `0xbbe710` (event `0xd01c1`): reads the owner player (`[unit+0xec]`, player
  table `0x4777d0`), sends the owner the harvest order notification `0xd0032` (852018, `harvest`),
  takes `[+0x1a8]` (`DataA`) and credits it with `0x4b5860(player, amount, 1, 1, 1)`. That routine
  runs `0x4b58a0` for resource kind 2 (lumber): it adds to the bank and the gathered counter and
  runs the ally-sharing loop. A credit above zero shows the lumber floating text (`0xb78c20`). Then
  it re-arms the timer (`0x7fbe90`) with the `Dur` value and event `0xd01c1`.
  There is no carried amount, no capacity, no depot and no tree damage call in this handler.
- Arrival handler `0xbbe300`: validates the target, moves the unit to the target's position vector
  (unit slot `0x180`), sets flags (`[unit+0x164] &= ~1`, `[unit+0x48c] |= 0x80`), records the tree,
  finds the attachment (`0xbbe090`) and starts the interval timer with `Dur`. The first credit is
  therefore one interval after arrival.
- Stop (`0xd01a0`, `0xbbe830` then `0xbbe1b0`): clears the flags and cancels the timer.
- `0xbbe560` (event `0xd0185`, taken as the tree ending): cancels the timer, then for a living unit
  whose owner is not player 25 or 27 calls `0xbbdda0`, which searches for another tree through
  `0xb79c00` with the `Rng` value as the radius, excluding the old target, and issues order
  `0xd0032` on it.
- `0xbbd460`: the target-order resolver answers `0x25` for order `0xd0003` (`smart`) and `0xdd`
  otherwise. The immediate `autoharvestlumber` order is not answered by this class.

## Recreation

- `harvest` on a harvestable tree for a unit listing `Awha`: accepted when the unit is shown, alive
  and unpaused; false otherwise. The unit moves until it is within `Rng1` of the tree
  (center distance, the reading used for `Ahar` `Rng1`; the original's order-range test for this
  ability is not decoded) and the first credit comes `Dur1` seconds later, then every `Dur1`.
- Each credit is `DataA1` truncated to an integer, through the same lumber bank and gathered credit
  as a depot deposit. The tree takes no damage and nothing is carried.
- Another order, removal, hiding or death ends the harvest. A tree that stops being harvestable
  hands the wisp to the nearest harvestable tree within `Rng1` of the wisp, or leaves it idle.
- `GetUnitCurrentOrder` answers the registry's `harvest` order while the wisp works.
- Wisps count against the harvest worker budget.

Refused or not applied:

- `autoharvestlumber` for a wisp refuses with "not decoded".
- The arrival repositioning onto the tree, the unit flags above, the lumber floating text and the
  player 25 and 27 exclusion are not applied.
- `DataB` ("Intervals Before Changing Trees") has no reader in the class and is ignored.
- The mapping of events `0xd0182`..`0xd0185` to arrival, tree ending and the like is inferred from
  the handler bodies, not traced to their publishers.
- A harvest order on an entangled gold mine (`egol`, wisp-only in `0xb946b0`) is not part of this
  class and stays on the gold path.
