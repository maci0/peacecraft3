# Player-state changes inside one script callback

An isolated Human02-derived diagnostic ran on the reviewed Legacy 1.29.2.9231
executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Its map SHA-256 is
`63a2e8ee69b3624e88ef168f695b5af609ca679e49fec433e174b1493a212fad`.
The staged map and extracted script matched the build receipt. Standard
initialization was retained; placed units and authored initialization triggers
were omitted. This is a controlled native observation, not mission acceptance.

At game time 5.000, one callback registered two gold-state subscriptions on
player zero: `EQUAL 10` and `GREATER_THAN_OR_EQUAL 10`. Gold initially held zero.
The callback then performed the following writes without yielding:

| Write | Equal-action count afterward | Greater-or-equal count afterward |
| --- | --- | --- |
| 10 | 1 | 1 |
| 20 | 1 | 2 |
| 20 again | 1 | 2 |
| 0 | 1 | 2 |

The action counters were already updated in the checkpoint immediately after
each setter returned. The equal callback observed gold 10, player zero through
`GetTriggerPlayer`, and the gold state through `GetEventPlayerState`. The
greater-or-equal callbacks observed 10 and 20 respectively. At 5.125 and 5.250,
the counters remained 1 and 2, with gold zero.

This establishes per-change comparison and synchronous observable action
delivery for these setter calls. A poll after the entire script callback would
see only the unchanged final zero and incorrectly lose all three notifications.
Repeated writes of the same value did not notify in this experiment.

The initial-condition subscription in this run attached its action after
registration. Its lack of observed actions therefore cannot exclude a
synchronous notification before the action was attached. Other player states,
all six comparison operators, nested delivery and native Windows execution
remain separate gates.

A follow-up changed only that action-registration order and its diagnostic run
identifier. Its map SHA-256 is
`09259f599252dcc1e123a07db6d8357f0236d088572513c0f0b47090bcac96e9`.
With the action attached first, registration at an already-equal gold value did
not execute the action, nor did repeating the same value. Both subsequent
checkpoints still recorded zero initial-condition actions. The per-change
notifications above repeated unchanged. This qualifies silent initial seeding
for the tested gold/equality case; other states and operators remain open.

All 15 checkpoints and their hashes are retained locally in ignored
`.scratch/player-state-probe/result.json`; generated map/script bytes remain
local. The private original-client container was stopped after collection.
The follow-up's 15 checkpoints are in the ignored
`.scratch/player-state-initial-probe/result.json`, and its container was also
stopped after collection.

## Nested mutation and condition evaluation

A separate diagnostic used the same reviewed executable and an explicit
`-classic` launch. `VersionGet` reported `VERSION_REIGN_OF_CHAOS` at game time
5.000. Map SHA-256:
`2f439ddb8157833f6ec9d498412622486133e5f9062cd4c540f3f79490da0def`.
The staged map and extracted script matched the build receipt. Human02's
information and unit overrides were retained; its authored units and triggers
were omitted. All 21 records arrived, including completion at 5.125.

One trigger action, entered through `TriggerExecute`, changed gold from zero to
10. A gold `GREATER_THAN_OR_EQUAL 10` event had a condition that called
`TriggerEvaluate` on another trigger and an action that changed gold from 10 to
20. The ordered records at 5.000 were:

1. Caller action before its setter, with caller trigger context and gold zero.
2. Event condition before nested evaluation, with event context and gold 10.
3. Nested condition, with nested trigger context and gold 10.
4. Event condition after evaluation, with event context restored and gold 10.
5. Event action before its setter, with event context and gold 10.
6. The condition/evaluation/action sequence repeated for gold 20, finishing its
   inner action before the outer action's setter returned.
7. Outer event action after its setter and then its return, still with event
   context and gold 20.
8. Caller action after its setter, with caller context restored and gold 20,
   followed by the return from `TriggerExecute`.

This establishes synchronous, reentrant delivery for these nested gold writes
and restoration of the outer trigger context around `TriggerEvaluate` and event
delivery. Deferring those writes until the outer trigger returns would disagree
with the observation. The next timer callback at 5.125 observed gold 20 with no
trigger context. Suspended actions, arbitrary recursion depth, other player
states and other operators remain separate qualification cases.

Ordered records, hashes and launch arguments are retained locally in ignored
`.scratch/player-state-nested-probe/result.json`. This Wine/software reference
is not native Windows or whole-mission acceptance evidence.

## Yield inside an event action

A paired diagnostic added `TriggerSleepAction(0.125)` immediately before the
outer event action's gold-20 write and extended the final timer checkpoint to
two seconds after dispatch. Map SHA-256:
`c28f821841362f7daa8a548963b1c279da64831bf676eb979da006d2bcd39045`.
The explicit `-classic` run again reported `VERSION_REIGN_OF_CHAOS`; all 23
records arrived.

At 5.000, the caller's gold-10 setter entered the event condition and action as
before. After the action's before-sleep checkpoint, the caller's setter returned
with gold 10 and caller trigger context; `TriggerExecute` then returned. At
5.250, the parked action resumed with event trigger context and gold 10. Its
gold-20 setter synchronously ran the nested condition/evaluation/action sequence
before returning to the resumed outer action. The final 7.000 checkpoint saw
gold 20 with no trigger context.

This qualifies synchronous delivery until an action yields, caller continuation
at that yield, and event-context retention across the observed resumption. The
requested delay and observed 0.250 interval do not establish a scheduling quantum
to copy into the host. Other wait forms, multiple parked events and arbitrary
recursion remain separate cases. Local receipts are in ignored
`.scratch/player-state-sleep-probe/result.json`.

## Condition-list mutations during synchronous gold events

A later Human01-derived probe used the same selected executable hash above.
Original map initialization was retained, and one self-authored timer callback
ran 13 serial gold-event cases. Each case reset player zero's gold to zero,
attached the action before registering `GREATER_THAN 1`, and set gold to two.
The callback recorded condition trace and action count immediately after
`SetPlayerState` returned and again at the next controller-timer checkpoint.

All 52 ordered records completed. Every immediate pair equaled its delayed pair,
and all 13 pairs equaled the earlier one-shot timer-event cohort described in
[timer condition traversal](script-trigger-admission.md#timer-condition-traversal-and-action-selection).
A false condition followed by a true condition ran both and suppressed actions.
Clearing, self/pending/later removal, appending, clear-then-append and nested
explicit evaluation matched the previously observed registration traversal.
Clearing actions inside a true predicate suppressed the original action;
appending an action ran it in the same synchronous event.

The old synchronous path returned immediately on a false condition and retained
its action list before evaluating predicates. A regression reproduced the first
mismatch: the false-then-true trace ended after the false callback. Synchronous
delivery now uses the same live condition cursor as explicit and timer evaluation,
folds every visited predicate and selects current actions only after conditions
finish. The event's optional filter remains a separate gate: rejecting it skips
both conditions and actions. A passing filter starts condition selection after
that filter returns, so its list mutations are visible.

The shared timer/state fixtures keep the 13 synthetic predicate setups and
expected trace/action pairs in one test source. The player-state test checks the
values inside the setter's caller, then verifies an empty queue, no retained
synchronous frame and restored trigger context. A separate synthetic test injects
a filter into an issued event to check both rejecting and passing gates. This
injection tests shared delivery control flow; player-state registration has no
filter parameter and this is not an additional native ABI claim. Native
filter-time list mutation was not tested by this gold-state cohort.

Probe inputs, modified map copies, output hashes and isolation receipts remain
local in `.scratch/goal-native-state-conditions-6/`. The run used private Xvfb,
one null audio sink and no network or host audio devices. Its stopped container
was checked for process ID zero. This is controlled Wine observation, not native
Windows, unchanged-campaign completion or full tick parity. Callback-time trigger
destruction, subscription-list edits and other resource states remain separate
qualification work. The common synchronous engine also serves other event
families; those inherit the cursor change without acquiring independent native
acceptance from this gold-state probe. Published queued boundary/range/region paths now share the same cursor; their
independent native spatial-event qualification remains incomplete.

Validation passed 3,342 `pcx-play` library tests with 163 explicit ignores, strict
library/test Clippy, formatting, whitespace checks, and native and
`wasm32-unknown-unknown` production library builds. Rust/Cargo 1.100.0 used
`RUSTFLAGS=-Dwarnings` and a checkout-specific target directory. The private
`.scratch/goal-state-condition-traversal/` receipts retain the reproduced failure,
focused checks and final dependency/source/artifact closure. The final checks
include the source comment updates that distinguish live and snapshot paths.
