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

`campaign_startup::install_native_counter` selects the platform after host
creation and before VM globals, `config` and `main`. Generic `install` preserves
its existing Fresh/Legacy contract. This bounded entry point is not yet selected
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
