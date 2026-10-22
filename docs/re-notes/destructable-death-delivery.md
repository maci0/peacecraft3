# Destructable death delivery

## Event identity

`ObjectHost::with_death_events` derives `EVENT_WIDGET_DEATH` from the selected
`ConvertWidgetEvent` declaration and the source global with that name. The
conversion must share `TriggerRegisterDeathEvent`'s source, retain a non-empty
domain, and its `widgetevent -> eventid -> handle` ancestry is checked. Starting
death subscriptions requires that global and named initializer; a missing or
foreign global, another initializer and an enumerand outside the admitted domain
are refused rather than producing a null identity. Combat admission derives the
same identity opportunistically: it stays unset when the global is absent and is
validated when declared. Unit death subscriptions use that identity when it is
available.

`GetTriggerWidget` and `GetTriggerDestructable` are admitted only when declared,
with the same source and their `widget` / `destructable` result families. Their
responses also require the admitted death-event source and identity, so the
dispatchers stay unsupported before death subscriptions are admitted and cannot
serve a foreign declaration afterwards. Responses use the fired event's
destructable subject; `GetTriggerWidget` also keeps the existing unit subject
path.

## Transition and queueing

`SetDestructableLife`, `KillDestructable` and `DestructableRestoreLife` share a
preflight that checks the selected signature, target family and resolved catalog
maximum life. A first alive-to-dead transition reserves one queue slot per
matching retained registration before writing life, dead state or collision
activation. The commit then pushes one event per registration with a destructable
subject and the derived event identity; the push cannot allocate. Engine damage
uses the same producer and leaves the events for boundary delivery. The producer
is split into a fallible `prepare_destructable_damage` preflight, which reserves
event capacity without changing life or collision, and an infallible
`commit_destructable_damage`, which writes the planned state and queues the
reserved events. Harvest can prepare before updating carry, then commit after
those writes. Destination toggles for stationary collision layers are likewise
infallible at commit because their layers are already retained.

The selected `common.j` declares restoration as `(destructable, real life,
boolean birth)`. The initial synthetic fixture incorrectly omitted `life`;
the selected Human01 factory gate exposed that mismatch. The corrected path
supports the measured dead-target, full-catalog-life, `birth=false` case.
Other amounts, live targets and birth animation refuse before mutation until
their remaining behavior is admitted. The reviewed handler `0x48f6f0` forwards
both arguments to `0x6d2a30`; that path also has a special near-zero amount and
a lower-bound branch. Those values must be derived from image operands before
extending the support boundary. Local disassembly remains under the ignored
`.scratch/destructable-restore-native/` directory.

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
and platform admission, the prepare/commit seam, and the shared nesting budget.

- `cargo test -p pcx-play --lib -- --test-threads=1`: 1422 passed, 41 ignored.
- `cargo clippy -p pcx-play --all-targets -- -D warnings`: pass.
- `cargo fmt --all -- --check`: pass.
- `cargo check --workspace --all-targets`: pass.

The asset-backed campaign integration tests require their documented local
environment and were not run for this slice. Native registration order,
unregistration and duplicate-identity behavior, item widgets, death animation
and sound, rendered/collision parity and the 100-consecutive original baseline
remain open. The life-setter probe receipt does not by itself qualify native
internal or presentation behavior.
