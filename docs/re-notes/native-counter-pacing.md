# Application counter pacing

The selected local image is Legacy `1.29.2.9231`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
`native_turn_rules`, `native_game_frame` and `native_game_clock` bind the turn
quantum, speed table, hardware arithmetic, asynchronous conversion and queue
arithmetic from that image. The source scheduler at `0x5aefd0` delivers game
quanta before its asynchronous real-time event. Map sleep uses the Extended REAL
queue; AI sleep uses Extended GAME. These domains retain separate cursors.

`CampaignRuntime` submits one counter record per wall update, including its
application acceleration multiplier. It does not split that record into its own
presentation ticks. Explicit `advance_ticks` submits one counter interval per
requested application tick without acceleration, so deterministic headless
driving also advances REAL. Script game speed is consumed by the source turn
rules; it is separate from the application's acceleration control.

Application tick accounting uses committed counter duration relative to the
session binding. Retained frame duration remains pending until its REAL phase
finishes. Queue-selection budget refusals can resume; faults in selected world
projection, physics or VM execution remain sticky. This accounting does not
claim that application ticks equal native game ticks. Loading time and a retired
session's duration are excluded when a new session binds.

Safe native budget refusals also retain deferred blocked-path and follow
refreshes from successful mandatory projection. Retrying at the same queue point
does not repeat that projection, so discarding those continuations would lose a
replan. The paired-session regression compares both route families under a
one-callback allowance and an unlimited diagnostic allowance, including complete
activity state, callback observations and positions after the next frame.
Legacy error cleanup and hard-fault cleanup retain their existing behavior.

`campaign_startup::install_native_counter` selects the platform after host
creation and before VM globals, `config` and `main`. Generic `install` preserves
its existing Fresh/Legacy contract: ordinary `CampaignRuntime` uses the legacy
caller-time route unless the counter platform was explicitly selected. Merely
admitting source clock rules must not promote it to the diagnostic GAME-only
driver, whose callbacks cannot pump REAL map sleeps. The standalone
`advance_paced_tick` API remains an explicit GAME-only diagnostic caller.
This bounded counter entry point is not yet selected
by the normal GUI or the shared original-map diagnostic helper: their caller
migration and selected-source replays remain required before changing defaults.

The nonstock published-session regressions exercise wall-frame phase ordering,
REAL wake during script pause, budget retry, submillisecond transport, separate
speed domains, VM faults, replacement and global/config/main timer admission.
The curse-host regression also issues Slow and facing changes during those
three startup phases after preselection. These establish reconstruction behavior,
not a new original-client timing comparison.

Replay/network time-scale controllers, alternative QPC calibration, complete
cross-family tie ordering, AI-to-map observer delivery and full native tick
parity remain open. Actual autocast target acquisition and Heal channel
execution are not supplied by this application adapter.
