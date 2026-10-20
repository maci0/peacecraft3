# Effective player resources

The selected 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Its registered `GetPlayerState` implementation at `0x494a80` indexes the same
state-property array that `SetPlayerState` (`0x4a97a0`, helper `0x4a9800`)
updates. The wrappers share their player lookup. Gold and lumber occupy the
source-decoded adjacent state range, use a source-decoded internal scale, and
follow the setter's unsigned comparison and signed clamp after wrapping integer
multiplication. Raw script requests remain separate from applied values.

The player state-schema initializer at `0x4b8d20` supplies a repeated template to
the property binder, including money and food state indices. Its template at
`0xe78730` supplies the counter kind pair. The reader follows the source binding,
then matches that kind pair to the allocation path at `0x89da93`. That allocator
writes the initial counter value into the same source-decoded object field read
by the getter and written by the setter. The initial amount comes from this
constructor operand, not from a copied balance, a template flag, an absent
handle or the null-player result. In particular, the template's creation
parameter is not mistaken for the initial amount.

The selected constructor supplies zero; a synthetic fixture instead supplies a
different initial value, scale, maximum and value-field displacement while its
template creation parameter differs again. Native registration signatures,
wrapper shapes, shared lookup, schema coverage, repeated clamp operand,
property stride, kind pair, getter/setter value-field relationship and
constructor bounds are checked. Input identities, image digest and ordered
profile sources remain attached to the rules.

The production startup can admit these rules before globals or assignments.
`GetPlayerState` and `ObjectHost::player_resource_value` then expose effective
gold, lumber, food used, food supplied and the food ceiling on the existing
player identity. An unadmitted projection stays unknown. Other player-state
settings keep their existing request-only behavior; their queries refuse.
Requests still use their symbolic handles and retain exact signed inputs, even
when the effective amount clamps. Failed signatures, identities, domains,
work allocations or record budgets preserve the previous effective amount.

Food is projected from each canonical living owned unit's metadata-associated
`UnitBalance.fused` and `fmade`, including map overrides, plus its admitted base
counter. Ownership transfer, removal and death consequently affect the same
projection used by scripts and the HUD. Explicit food assignments retain an
adjustment against the current unit sum, so subsequent unit changes remain
visible. The ceiling starts from the selected ordered `[Misc] FoodLimit` and
can be explicitly assigned. Supplied food is limited by that current ceiling.
Missing, ambiguous, nonnumeric, negative or overflowing unit food records report
errors instead of inventing a cost or supply amount.

This is a recreation of resource counters, not proof of the original's complete
resource lifecycle. Food assignment adjustments and ceiling application are
stated recreation choices; exact original death/ownership update ordering,
construction, resurrection, food disabling and original edge-case behavior
remain unverified. Harvesting, spending, upkeep deductions and player-state
change event registration/delivery remain separate consumers; no registered
player-state event is silently accepted here. Original/native/browser balance
and rendered parity are not established by parser or counter tests.

Synthetic tests exercise changed source operands and malformed source refusal,
initial balances, setter clamps with retained raw requests, per-player
independence, exhausted request budgets, authored food fields, ownership,
removal and explicit adjustment counters. The ignored original-image reader test
requires `PCX_PLAYER_RESOURCE_IMAGE`; its food profile is deliberately synthetic
and does not establish the original game's food ceiling. Private source analysis
and local validation receipts stay under `.scratch/player-resources-20261020/`.
