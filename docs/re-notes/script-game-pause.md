# Executed game pause requests

The selected `common.j` declares `PauseGame(boolean flag) returns nothing`.
The selected `Blizzard.j` victory and defeat dialog functions execute
`PauseGame(true)` in the single-player local branch, then execute `EnableUserUI`,
`DialogDisplay`, volume and sound calls in that same JASS thread. Their later
button actions execute `PauseGame(false)`. These shipped-script relationships
require the pause call to return so the current thread can finish its dialog work.
They do not establish an authored fresh-game pause flag.

Read-only original-byte inspection of the locally supplied Legacy executable
(version `1.29.2.9231`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`)
locates the native wrapper at `0x4a3e90` and its request path at `0x5f1460`.
That path has additional player/request checks; this slice does not claim to
reproduce native network scheduling, pause ownership or multiplayer policy.
No measured numeric default or executable operand is needed to retain an
explicit script boolean.

`ObjectHost::with_game_pause` validates the declaration and its source against
the checked host's `CreateTimer` source. Missing declarations leave the optional
capability unadmitted; incompatible or foreign-source declarations refuse it.
`game_pause_admitted` distinguishes that case from successful admission with
no executed call. `game_paused` returns only the last executed boolean, retaining
`None` before a call. Invalid arguments preserve state. Calls return `nothing`
synchronously; they neither end nor park the running JASS thread.

The parent runtime owns subsequent simulation/timer delivery gating and resume
integration. The stored request alone does not prove timers, movement, combat,
UI input, audio or animation stop in the original order. Same-thread completion
is supported by shipped scripts; exceptional timing around simultaneous queued
callbacks remains a recreation choice until original-runtime evidence qualifies
it. The host capability does not independently advance time or consume callbacks.

Synthetic VM tests cover current-thread completion after pause, idempotent
requests, later resume, unavailable admission, argument failures, signatures and
source mismatch. The ignored original declaration gate requires an explicit
`PCX_GAME_PAUSE_COMMON` path and reads that local file without committing it.
Parser/host tests do not prove a rendered defeat dialog or native pause parity.
