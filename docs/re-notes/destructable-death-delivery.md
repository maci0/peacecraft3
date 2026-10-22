# Destructable death delivery

## Event identity

`ObjectHost::with_death_events` derives `EVENT_WIDGET_DEATH` from the selected
`ConvertWidgetEvent` declaration and the source global with that name. The
conversion must share `TriggerRegisterDeathEvent`'s source, retain a non-empty
domain, and its `widgetevent -> eventid -> handle` ancestry is checked. Absence of
the global leaves the event identity unset instead of inventing a number.

`GetTriggerWidget` and `GetTriggerDestructable` are admitted only when declared,
with the same source and their `widget` / `destructable` result families.
Responses use the fired event's destructable subject; `GetTriggerWidget` also
keeps the existing unit subject path. Unit death subscriptions use the same
derived identity when it is available.

## Transition and queueing

`SetDestructableLife`, `KillDestructable` and `DestructableRestoreLife` share a
preflight that checks the selected signature, target family and resolved catalog
maximum life. A first alive-to-dead transition reserves one queue slot per
matching retained registration before writing life, dead state or collision
activation. The commit then pushes one event per registration with a destructable
subject and the derived event identity; the push cannot allocate. Engine damage
uses the same producer and leaves the events for boundary delivery.

Script death natives deliver matching callbacks synchronously before returning:
the host takes the reserved batch, installs the event context, and continues
through the shared synchronous frame machinery used by player-state events. A
positive life write rearms a killed tree without firing; writing life on an
already-dead tree, or killing it again, does not fire. Callbacks observe the
committed life and dead state; collision activation is updated before delivery.

Current recreation ordering follows arena registration order. A sleeping action
parks its thread and the remaining matching registration completes before the
native returns; the parked action resumes from its saved context later. Nested
death and player-state callbacks share the same frames and restore their caller
contexts. False conditions skip the action without queuing a second event.
Callback errors abort the synchronous delivery, thread contexts and native
callback frames. Runaway nesting stops at the VM's existing spawn-depth budget.

## Evidence

Synthetic owned-VM tests in `death_event_tests.rs` cover synchronous setters,
kill and restore paths, event/widget/destructable responses, already-dead and
positive-rearm behavior, engine-damage boundary delivery, both cross-family
nesting directions, false conditions, sleeping actions, callback errors, source
and platform admission, and the shared nesting budget.

- `cargo test -p pcx-play --lib -- --test-threads=1`: 1416 passed, 41 ignored.
- `cargo clippy -p pcx-play --all-targets -- -D warnings`: pass.
- `cargo fmt --all -- --check`: pass.
- `cargo check --workspace --all-targets`: pass.

The asset-backed campaign integration tests require their documented local
environment and were not run for this slice. Native registration order,
unregistration and duplicate-identity behavior, item widgets, death animation
and sound, rendered/collision parity and the 100-consecutive original baseline
remain open. The life-setter probe receipt does not by itself qualify native
internal or presentation behavior.
