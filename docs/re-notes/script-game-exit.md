# Executed game exit requests

The selected `common.j` declares `EndGame(boolean doScoreScreen) returns nothing`.
The selected `Blizzard.j` defeat dialog registers `CustomDefeatQuitBJ` as the
action of a normal `DialogAddButton` (`GAMEOVER_QUIT_MISSION`). That action runs
`PauseGame(false)` when single-player, `SetGameDifficulty(GetDefaultDifficulty())`,
then `EndGame(true)`. The quit button of `DialogAddQuitButton` is a separate path
(`dialog_host.rs`), and `EndGameBJ` and other `EndGame` calls are reached from
other BJ functions.

Read-only original-byte inspection of the locally supplied Legacy executable
(version `1.29.2.9231`) locates the native wrapper at `0x48fe20`. It stores the
boolean argument and clears a second field, calls `0x475930`, and returns to its
caller. The calling thread continues; the request does not clear the executing VM.
`RestartGame` (wrapper `0x4a5620`, which calls `0x47c740`) is a different native
and is not covered here.

`ObjectHost::with_game_exit(max_pending)` validates the declaration and its source
against the checked host's `CreateTimer` source, as `with_game_pause` does. Missing
declarations leave the capability unadmitted; incompatible or foreign-source
declarations refuse admission. `game_exit_native` appends a `GameExitRequest` to a
bounded queue and returns `nothing`. Invalid arguments and a full queue fail without
changing recorded requests. `take_game_exit_requests` drains the queue in call order.

The campaign runtime (`crates/peacecraft3/src/campaign_runtime.rs`,
`take_game_exit`) runs after `advance_campaign`, the path that delivers queued
callbacks (`advance_queued_events`). A request recorded by a delivered callback is
taken in that same update. A dialog click only queues its callback during input;
delivery, and therefore the request, happens in the later update whose
`advance_campaign` drains it. The queue persists until taken. On a request, the
runtime clears the session and suspends the campaign view when one is present. A
failed callback still ends the session when it recorded a request before failing.

The score screen is not implemented. `score_screen` is carried through the
request and a warning is logged when it is set; the session still ends. This
means `EndGame(true)` does not reproduce the original score presentation.

Synthetic VM tests (`crates/pcx-play/src/game_exit_tests.rs`) cover admission,
recording through a VM call with the calling thread continuing, drain order, argument
and overflow failures, and signature and source refusal. Parser and host tests do
not prove the rendered defeat dialog, the score screen, or native session teardown
parity. The live Human01 route that reaches `CustomDefeatQuitBJ` remains the runtime
evidence to collect.
