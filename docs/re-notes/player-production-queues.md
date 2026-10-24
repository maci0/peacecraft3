# Player production queues: source and behavioral contract

This is the implementation contract for extending the existing replacement
production lifecycle. The shared-state implementation now admits queued player
training orders and explicit queue cancellation, with producer-death refunds.
Presentation, input delivery, removal and transfer remain unfinished. It uses
the canonical ObjectHost units, banks, food accounting, clock, placement and
event delivery, with an optional guard-post binding for replacement jobs.

## Qualified original behavior

Selected executable SHA-256:
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The runtime observations use the original client under Wine/software rendering,
with RoC explicitly selected. They do not establish Windows or browser parity.
Original assets, generated maps, captures and complete analysis records stay
local under `.scratch/player-production/`.

The bounded direct-order probe derives its producer and trainable type from the
retained source setup. It starts no AI and omits inherited unit creation and
initialization triggers. Twelve immediate training requests are issued through
`IssueImmediateOrderById` using the unit type's FourCC. The first seven accept;
the rest refuse without further spending. Every accepted request spends its
authored money cost immediately. Only the active job reserves food and emits
`EVENT_UNIT_TRAIN_START`. Pending entries neither reserve additional food nor
count as living units in `GetPlayerTechCount`. `GetUnitCurrentOrder` remains zero
during this production queue; it must not be used as the queue model.

The public string conversion `OrderId("cancel")` returns zero in this selected
client. Sending that string does not cancel a training job, emit a cancellation
event or refund money. A recreation must not invent a successful script order
for it. Queue-UI cancellation is a separate operation on the same production
state.

The repeat UI run uses the same frozen map bytes, a canonical local-player
producer and captured queue controls. Actions occur before any unit finishes:

| Action | Observed result |
|---|---|
| Enqueue the source queue | All accepted money costs are charged; one active food reservation and one start event. |
| Click the last pending entry | One refund and one cancellation event; active job and start count stay unchanged. |
| Click the active entry | One refund and one cancellation event; the next job starts and emits its start event. Food remains one active reservation. |
| Allow that job to complete | One living unit and finish event; the following job starts. Used food includes the completed unit plus the new active reservation. |

These observations qualify the ordinary single-type case. They do not prove
mixed-type callback ordering, explicit producer removal, ownership transfer,
revival or construction behavior. Separate food-blocked-head and producer-death
observations are recorded below. The earlier manual UI
rehearsal is retained separately; its late pending-entry click is not used as
the bounded cancellation-event proof.

Receipts:

- `queue-probe/native/result.json`: exact 49-marker direct-order inventory.
- `queue-probe-ui-r2/native/result.json`: exact 481-marker UI-repeat inventory.
- `queue-probe-ui-r2/native/actions-observed.json`: before/pending/active action
  rows, with separate input and screenshot receipts.

Both clients were stopped and preserved before collection. Maps/scripts and
staged executable identities are checked. The map and common preludes parse
and resolve; every packed map member round-trips. Collectors reject malformed
wrappers, identities, fields, nonfinite values, incomplete inventories and
invalid terminal sequences. The initial collector's assumption that a public
cancel order must exist was rejected by the actual zero conversion result; the
failed collector is retained rather than treating that request as a success.

## Source bindings for the implementation

Native inspection is retained with the selected image and Ghidra 12.1.3 analysis
identity. Pseudocode is recovered analysis, not original source. Critical
operand relationships also appear in the recorded instruction streams:

- `0x632190` resolves `SimpleBuildQueueBackdrop`, creates a separate active
  control and a bounded pending-control collection. Its pending allocation and
  loop agree. The selected pending count is six; it is source data to decode,
  not a production constant.
- The cancellation walkers at `0xb56f90` and `0xb57390` independently carry the
  selected slot count seven (`0xb56fbf`, `0xb573bd`). Read and cross-check these
  operands against the UI relationship and runtime boundary; do not copy seven
  into a queue limit.
- The ordinary cancellation branch reads `TrainRefundRate` through the source
  profile getter, separately from `ResearchRefundRate` and `ReviveRefundRate`.
  Ordered `Units/MiscGame.txt` and `war3mapMisc.txt` profiles supply this value.
  The implementation must preserve edition, source identity and map overrides.
- The ordinary refund path converts the unit money cost through `0x9a1620`,
  multiplies through `0x99ffe0`, converts the product through `0x9a07b0`, then
  calls the money-credit wrappers. The conversion's inspected mantissa/sign
  operations truncate finite in-range values toward zero. Apply conversion per
  cancelled entry, rather than multiplying a sum of queue costs. Reuse the
  selected numeric policy; do not substitute an assumed full refund or host
  float rounding. Fractional override runtime discrimination remains required.

The complete evidence records are `rea-session-queue-strings.json`,
`rea-queue-ui-function.json`, `rea-refund-function.json`,
`rea-single-cancel-function.json` and `rea-refund-integer-conversion.json`.
No native wrapper layout or linked list needs to be reproduced to implement
the observed queue faithfully.

## Food-blocked pending head observation

A separate preserved `queue-probe-food/native` run lowers the food cap to the
current food used after all seven requests have been accepted. The original
client completes the active unit, keeps the next entry pending, and emits no
second start while at the cap. Restoring the cap starts the next head and adds
its food reservation without another money debit. The strict collector retained
481 markers; `food-transition.json` checks the bounded wait and restore against
those rows and pins the result hash. The run was stopped after collection.
This tests a previously paid pending head, not an initial request made at a full
food cap. Same-callback activation timing and producer interruption still need
separate connected recreation checks.

## Producer death observation

A separate `queue-probe-death/native` run kills the producer after seven paid
requests and before the first finish. Money returns to its initial value, active
food is released, and no train-cancel or finish event is emitted. The bounded
80-tick run retained 161 markers; `death-transition.json` pins the result and
checks the post-death state through the terminal sample. This does not qualify
explicit removal, owner transfer or a fractional refund override. The host now
prepares this distinct death refund before death mutations and
clears production without replaying UI cancellation events. Explicit removal
and owner transfer still need their own qualified lifecycle handling.

## Fractional producer-death refund observation

The separate `queue-probe-death-refund/native` run adds an explicitly experimental
map override, `[Misc] TrainRefundRate=0.37`, while retaining the producer and unit
source setup. Killing the producer refunds 602 gold across seven entries;
truncating the combined cost would yield 608. Food returns to zero and no cancel
event fires. `fractional-refund-transition.json` pins the strict 161-marker
result, derives each cost from the observed debits, and discriminates per-entry
conversion from aggregate conversion. The generated map and original inputs
remain local. This qualifies the positive fractional death-refund case; negative
rates, overflow classes and UI cancellation with this override were not tested.

The host's `cancel_unit_training(producer, slot)` prepares the source refund,
food release, successor activation and combined event capacity before mutation.
Pending cancellation leaves the active deadline unchanged. Active cancellation
starts the successor only when food permits. Invalid slots and repeats return
false. The operation is not exposed as an invented script order. Tests use
nonstock fractional rates and costs, exercise missing numeric-policy refusal,
and distinguish counted living units from pending entries. Hero training refuses
through the source cost classifier until token transitions are implemented.

## Connected implementation and acceptance

1. Generalize the existing training state into a per-producer FIFO with a
   separate pending/active phase and optional guard binding. Keep one unit and
   player store. Decode the source queue bound; retain application work and
   allocation budgets as separate limits.
2. Admit a request through the producer's resolved `Trains` list, unit
   prerequisites, current owner limits and money balance. Spend money once at
   enqueue. Activate only the head, with its food reservation, source `bldtm`,
   game-clock deadline and start event. Do not reserve food for every queued
   entry. Qualify the food-blocked-head case before choosing that transition.
3. Complete through existing canonical unit creation/placement and train-finish
   delivery; activate the next head without publishing duplicate debit or
   callbacks. Use the existing pause and clock boundaries, with interruption
   regressions. Explicit producer-removal/transfer refusal remains an open lifecycle
   gap, not a completed cancellation implementation.
4. Add queue cancellation by index as a shared-world operation used by UI and
   input/replay delivery. Prepare the source-derived per-entry refund, food,
   queue/event capacity and replacement-head work before mutating state.
   Pending cancellation must not restart the active job. Do not map it onto the
   unavailable public `"cancel"` string order.
5. Add command-card train buttons from resolved unit metadata (`Art`,
   `Buttonpos`, tooltips and hotkeys), resource/prerequisite availability and
   the producer's `Trains` relationships. Render the canonical queue and progress
   through source frame geometry, textures and bindings. Keep training's
   reported current-order behavior separate. Building work animation and
   progress presentation need rendered comparison, not parser-only claims.
6. Cover nonstock metadata/cost/time/slot/refund values; queue-full and
   insufficient-resource refusal; callbacks and retries; pending and active
   cancellation; pause; blocked food; placement; producer death/removal and
   transfer. Use real public training commands and UI dispatch in tests. Finish
   with a connected unchanged-map gather/train/cancel/fight scenario and the
   existing main-menu/campaign regression.

Hero training/revival, research, construction and race-specific workers remain
the subsequent parts of the economy stage. This contract does not close that
stage or the first-mission playthrough requirement.
