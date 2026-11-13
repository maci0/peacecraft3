# Counter pacing checkpoint status

Work stopped for wrap-up. Normal GUI startup remains on its accepted generic
Fresh/Legacy route. Explicit counter startup remains opt-in. No new feature,
original-client run, test or build was started during wrap-up.

## Preserved startup candidate

Historical commit `b2fa5be6` preserves the unaccepted four-file normal-startup
candidate. Its child restores those four files exactly to
`88628015214f78c5646ee253531e02cc5805ed1c`; do not select the historical commit as
a release tip. The candidate changes GUI/original-map startup and migrates
opening/score diagnostic callers to counter driving. It is not accepted.

Both former stash tips contain identical candidate content:
`b368c5b691c69ebbc26acc1388af1975e12790c3` (base `9cf6888d`) and
`c3b0954065393aff868a45c065972e1c205d98c1` (base `2c612baa`). Their exact patches
have SHA-256 `7a649c21dc7ccc9592ea2a0832663754f04e912b087cccdcb740129a04bc822c`
and length 11050 bytes. Both are represented by the historical commit; no stash
was dropped. Other shared stashes are outside this checkpoint.

## Intake and validation

Released main `04c1dafb2c22124706955963767e2bdff4029612` was merged at `88628015`.
It already includes the opt-in clock/counter/physics adapter, route-continuation
budget fix `963c944c`, and generic runtime dispatch restoration `535a85f6`.
Do not duplicate these picks. The branch additionally includes short timed-facing
`72062f3b` (equivalent to `8af16cf0`), negative-facing `4b6bf8c4` (equivalent to
`1628d464`), and lifecycle evidence note `9cf6888d`.

Retained local receipts:

- `/tmp/pc3-sol09-app-counter-handoff.json`: 12 successful groups for opt-in adapter.
- `/tmp/pc3-sol09-route-retry-handoff.json`: 15 successful groups, unchanged source,
  named RED/GREEN regression and independent review. In
  `crates/peacecraft3/tests/support/native_clock_cases.rs`,
  `paired_timer_budget_retry_preserves_blocked_and_follow_route_continuations`
  and `app_counter_long_wall_frame_budget_retry_preserves_pause_real_wake_and_next_grant`
  actually ran and passed.
- `/tmp/pc3-sol09-generic-runtime-handoff.json`: seven successful groups, unchanged
  source, review, 72 timer/runtime cases and both original-map audio gates
  `human01_thematic_ex_seek_restart_and_retirement_reach_audio_entities` and
  `human01_opening_audio_reaches_entities`. These use generic Legacy driving.
- `/tmp/pc3-sol04-timed-facing-validation-1794559224667871464/report.json` and
  `/tmp/pc3-sol04-negative-facing-validation-1794560271249514917/report.json`:
  eight successful groups each, including source/native witnesses and strict
  native/WASM checks and libraries. These are component receipts, not a new
  combined validation of this wrap-up tip.
- `/tmp/pc3-native-clock-main-validation/acceptance.json`: released-main combined
  acceptance, 3668 package tests, 33 explicit source/witness gates, 449 application
  executions and strict native/WASM checks/builds.

No builds remain running. Managed terminals `391fc931-041c-4d7b-b905-e84920e99db5`
and `b62b3708-697e-4cbd-979a-8df3a17a0885` are idle. At handoff, no owned compiler,
linker, validation runner, original client or reverse-engineering session remained.
Idle shell PIDs were 1176967 and 1908396. Infrastructure process 874642
(`blender-mcp`) had this working directory; it is not a validation job and was not
terminated. The private build cache is `/tmp/pc3-fidelity-sol61-09-target`.

## Remaining limits

The last normal-counter Human01 replay reached startup with 152 units and skipped
opening control at GAME `0.4999995529651642`, then failed on a timed-facing duration
above the admitted instant threshold. It is a failed horizon, not normal adoption
clearance: `/tmp/pc3-sol09-normal-counter-human01-negative-timed.log`.
Undead02 likewise reached 123 units then refused the long timed-facing branch:
`/tmp/pc3-sol09-normal-counter-undead02-short-timed.log`.

Long timed-facing, normal counter startup/audio acceptance, actual common
autocast retry/acquisition and Heal channel execution remain incomplete. Autocast
suspension/reactivation has additional source evidence in
`ability-autocast-tick.md`; an empty cooldown map does not prove eligibility.
No full mission, native tick-parity or full-save completion is claimed. Separate
bridge/save and regular-music work requires its own intake and validation.
