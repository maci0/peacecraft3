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
visible. The ceiling reads the selected ordered `[Misc] FoodCeiling` override and applies
its source-decoded limits. With no authored override it uses the selected
client's source table, indexed by the same global that its registered `VersionGet`
reads. The recreation chooses that table entry from the canonical startup version
at query time; it refuses before that version is supplied. Explicit script
assignments remain authoritative. Supplied food is limited by that current ceiling.
Missing, ambiguous, nonnumeric, negative or overflowing unit food records report
errors instead of inventing a cost or supply amount.

This is a recreation of resource counters, not proof of the original's complete
resource lifecycle. Food assignment adjustments and ceiling application are
stated recreation choices; exact original death/ownership update ordering,
construction, resurrection, food disabling and original edge-case behavior
remain unverified. Harvesting, spending and upkeep deductions remain separate
consumers. Original/native/browser balance and rendered parity are not
established by parser or counter tests.

## Player-state change events

`ObjectHost::with_player_state_events` admits the loaded
`TriggerRegisterPlayerStateEvent(trigger, player, playerstate, limitop, real)`
contract on the existing trigger arena. The signature, `CreateTrigger`,
`Player`, `ConvertPlayerState` and `ConvertLimitOp` must share their selected
declaration source. The six `limitop` names (`LESS_THAN`,
`LESS_THAN_OR_EQUAL`, `EQUAL`, `GREATER_THAN_OR_EQUAL`, `GREATER_THAN`,
`NOT_EQUAL`) resolve their values through the admitted `ConvertLimitOp` domain
rather than embedded opcode numbers; missing, duplicate or out-of-domain
operations refuse. `playerstate` and `limitop` must descend to `handle` through
source-matching declarations. `GetEventPlayerState` is admitted when present.

Registration validates the trigger and player families, the symbolic state and
limitop handles, the finite real threshold, and the aggregate caller bound. It
seeds the observed value from the effective player state without firing. The
central mutation path used by `SetPlayerState` and gold debits compares each
intermediate effective value before it is overwritten. Every changed value that
satisfies the selected comparison queues its own event, so a write that passes
through a qualifying value and back still fires; equal writes and changes that
do not satisfy the comparison do not. A script-boundary poll catches effective
changes computed outside that path, such as live food, and records the last
observed value. Delivery uses the existing event context, with
`GetEventPlayerState` returning the registered state handle. States outside the
effective resource projection refuse instead of inventing a value.

`SetPlayerState` is the mutation with synchronous delivery: when player
state events are admitted,
its matching filters, conditions and actions run through the normal
event-delivery context before the native returns; no later tick is required.
Other event families still defer to the next script boundary. A
`TriggerSleepAction` inside an action parks that action and lets the remaining
matching registrations and the enclosing script continue. `SetPlayerState`
calls inside a trigger or another state event also deliver synchronously and
reentrantly. Nested frames restore the caller's trigger and response context;
a parked action retains its event context when resumed. The controlled native
ordering and sleep observations are recorded in
`player-state-event-observation.md`. Other event families and external effective
food changes retain boundary delivery as a recreation policy.

The shipped witness is `Human02.j:3506` and `:3520`, registering gold below
1000 and lumber below 300, with the declaration at `common.j:1284` and the
`EVENT_PLAYER_STATE_LIMIT` response at `:1287`. Synthetic tests cover baseline
seeding, changed and unchanged values, comparisons from selected constants,
response context, budgets and missing operation declarations. The selected
Human02 admission passes through this source contract. The integrated continuation
still requires a new execution gate after the runtime changes; admission alone
does not establish that map's startup or mission completion.

Synthetic tests exercise changed source operands and malformed source refusal,
initial balances, setter clamps with retained raw requests, per-player
independence, exhausted request budgets, authored food fields, ownership,
removal and explicit adjustment counters. The ignored original-image reader test
requires `PCX_PLAYER_RESOURCE_IMAGE`; its food profile is deliberately synthetic
and does not establish the original game's food ceiling. Private source analysis
and local validation receipts stay under `.scratch/player-resources-20261020/`.


## Original normal-profile regression

The selected stock `Units/MiscData.txt` and `Units/MiscGame.txt` do not supply
`FoodLimit`; the metadata's `fcap` record names `Misc/FoodCeiling`. The normal
stock profile also omits that optional setting. The earlier synthetic-only
profile test hid this missing native fallback and normal preparation refused.
The selected food-ceiling function at `0x4300a0` reads `FoodCeiling` when present,
checks its limits, and otherwise selects a value from its source table at
`0xe69ae8`. Its selector global is the same one read by registered `VersionGet`.
The reader checks that relationship, repeated key/section/cache addresses,
repeated clamp operands and table bounds before admitting the defaults. No
numeric fallback or edition mapping is embedded in production code.

An additional ignored gate requires `PCX_PLAYER_RESOURCE_IMAGE` and
`PCX_PLAYER_RESOURCE_PROFILE` for the actual selected executable and normal
stock profile; optional `PCX_PLAYER_RESOURCE_MAP_PROFILE` adds the map override.
It exercises the original missing-setting path with the decoded source table,
without a synthetic ceiling. Synthetic tests separately change both default
table entries and clamp operands, refuse a mismatched version selector, and
cover absent canonical version, profile override and explicit setter priority.

## Authored food markers

The actual Human01 resource probe reached authored `Hart` food cells containing
`-`, and `hfoo` food-made containing whitespace-padded ` - `. The object catalog
correctly retains those string cells. The former strict floating parser rejected
them before it could report any normal food total.

On the selected image, the `fmade` and `fused` descriptor bindings at
`0x6b3fff`/`0x6b4017` match integer accessors `0x6aee20`/`0x6aee40`. Both reach
`0x6afae0`, whose stock-cell path calls `0x95d830`. That reader returns an integer
cell, truncates a floating cell, and routes string cells to `XOR EAX,EAX` at
`0x95d858`. This establishes zero from instruction semantics, rather than an
invented profile default. These are static shipped-byte observations; no original
execution was performed. A cached analysis snapshot failed integrity validation,
so the bounded checked PE ranges were decoded directly; private receipts retain
that limitation.

Resource admission now checks both source keys, matching descriptor offsets,
their shared accessor and integer/string dispatch before retaining that marker
result. Only the resource food projection opts into it through the shared balance
reader. Existing strict callers retain marker refusal. Numeric text is trimmed;
recognized `-`/`_` markers use the admitted result, while arbitrary malformed,
nonfinite, negative, missing or ambiguous food data still refuses. Limiting the
string conversion to recognized markers is an explicit strict recreation policy;
the native reader's string branch is broader.

Synthetic cases cover whitespace, markers, malformed-value refusal and corrupted
source descriptor/conversion refusal. The ignored
`supplied_resolved_human_units_support_normal_food_queries` gate requires
`PCX_PLAYER_RESOURCE_CATALOG` naming an explicit retained source-copy directory,
plus the image/profile variables above. It resolves original `Hart`/`hfoo` through
actual unit metadata and balance tables, creates both on one player, and queries
effective food using canonical version admission. This fixture uses stock object
records; the full actual Human01 startup/runtime replay remains a separate gate.
