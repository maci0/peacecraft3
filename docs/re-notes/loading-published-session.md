# Loading `published` session shortcut and stale-screen dismissal

Follow-up to [campaign-loading-screen](campaign-loading-screen.md). Evidence
classes are labelled: **runtime** (recreation observations and committed
recreation code), **inference**, **open**. No new shipped-byte or native-code
evidence; nothing below describes original behavior.

## The `published` renewal proxy and its false negative

**runtime.** `campaign_loading.rs` `signals()` derives `published` as `prepared
&& (session_current || witness-renewed)`, where the witness branch counts the
load done only when the live `session_witness()` no longer pointer-matches the
`Weak` captured at `begin` — i.e. a post-begin session replacement. This is a
renewal proxy, not an observed native rule.

**runtime.** `begin(..., pre_admitted)` records `begun_admitted`; chapter
launches begin before admission (`false`) so the first stage waits for new
bindings, while continuation/browser launches begin after (`true`) so current
bindings count. `ensure_loading` re-beginning with `begun_admitted=true`
captures the already-live witness, so the witness branch can never fire for
that screen. Observed DIAG on the mounted Human01 re-launch path:
`{admitted:true, prepared:true, published:false}` with the clock frozen at 0.0.

**runtime.** Fix: the `session_current` shortcut in `advance_loading` counts
the admitted live session for the screen's own bindings as published when the
bindings revision matches the screen revision and `session.context()`
pointer-matches (`Arc::ptr_eq`) `MapRequests.admission.shared_context()`. A
stale screen (revision mismatch) still waits for the renewal its revision
implies.

## Stale-`AwaitKey` dismissal

**runtime.** `advance_loading` cancels a `Phase::AwaitKey` screen whose
`session.context().is_none()`. Meaning: reaching `AwaitKey` required all four
readiness signals (admitted, prepared, published, view ready), so a vanished
session means the load it waited for was exited (quit to menu), not pending.

**runtime.** Explicitly not covered: `Loading`-phase screens still wait for
admission (no session-vanish detection there); the only vanish signal is
`context().is_none()`, with no outcome/revision check beyond it.

## The input-blocking chain it unblocks

**runtime.** Trigger: the mounted Human01 re-launch gate timed out
("replacement campaign did not complete") while a stuck `shown()` screen
remained up. Chain: `loading.shown()` sets `blocked`
(`campaign_presentation.rs:189-194`) → `input_allowed` false
(`button_actions.rs`) → the second MissionButton click is discarded → no inbox
request → no admission → no new witness → the stuck screen's `published` never
resolves. The dismissal breaks the loop by dropping a finished-but-dead
screen so menu clicks register again.

## Open questions

- **open.** Whether `take_game_exit` should cancel the screen directly:
  alternative placement, same observable effect as the `AwaitKey`-vanish
  check. Not decided here.
- **open.** F10/menu-key press-clear ordering vs `advance_loading` is a
  related in-flight fix (press consumed while the screen is up, before/after
  the screen observes it). Described as related, not decided here.
