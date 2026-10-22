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

All 15 checkpoints and their hashes are retained locally in ignored
`.scratch/player-state-probe/result.json`; generated map/script bytes remain
local. The private original-client container was stopped after collection.
