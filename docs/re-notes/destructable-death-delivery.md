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
reads the selected client's near-zero threshold and minimum life from the
handler at `0x6d2a30` and its data operands. A dead target with `birth=false`
restores to its catalog maximum for a near-zero request; otherwise the request
is floored by that source minimum and capped at its catalog maximum. A live
target remains unchanged. Restoration rearms the next death event without
delivering one itself. `birth=true` still refuses because its presentation
behavior is unqualified. Local disassembly remains under the ignored
`.scratch/destructable-restore-native/` directory; the separate original RoC
amount probe is under `.scratch/destructable-restore-amount-probe/`.

A subsequent controlled run on the same reviewed client, with `-classic` and
explicit RoC readback, sampled nine restoration cases on one script-created tree.
Calling restore with half its maximum while alive left life unchanged. Killing
before each later call gave: zero request -> full life; half maximum -> half life;
twice maximum -> maximum; negative maximum -> 0.5; 0.0005 -> full life; 0.25 ->
0.5; and 0.75 -> 0.75. A half-maximum request with `birth=true` yielded half life
through getters. Each subsequent kill delivered another death callback.

All 12 markers qualified; four synthetic collector tests passed. The diagnostic
map hash is `07f0961538e6134401e350e3fff6c9a928922a0f9e2bf6b3daeecf80109c60b0`;
the script hash is `6b0e86f43b246cb5b05ea9d8a9d29b0d78769614ef922fcd76b1ec6044eca869`.
Raw markers, input receipts and the stopped run remain local under
`.scratch/destructable-restore-amount-probe/` and
`/home/maci/.cache/pc3-probe/drestore-261023a`. These observations corroborate the
native amount branches, but the production support boundary above is unchanged
until its rules are derived from image operands. Exact threshold equality,
rendered birth animation and collision behavior remain unqualified.


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

## Maximum life (`SetDestructableMaxLife`)

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Native
code evidence only (static disassembly); no original-client run of this native.
The name is at VA `0xe7188c`, the signature `(Hdestructable;R)V` at `0xe71850`,
and the registration row (push signature, name, handler, `call 0x8ccc80`) is at
`0x49d13c`. The handler is `0x4a7bc0`.

Steps, all observed in the disassembly:

1. The handle resolves to the destructable object through `0x481470`. A null
   handle writes nothing.
2. The old maximum is read through the object's virtual slot `+0x130`
   (`0x6cf000`, which returns object `+0x174`). The change is `new - old`
   (`0x9a00b0`, float subtraction).
3. The change goes to the shared pool routine `0x6be8c0` with mode 1, the same
   routine the unit maximum-life changes use (see `upgrade-effects.md`).
   - A zero change, tested before rounding, returns with no write (`0x6be8f0`).
   - The change rounds to `floor(x + 0.5)`: `0x9a1d70` adds the float at VA
     `0x10db0a8` (0.5) through `0x9a01c0`, then floors through `0x9a1520`.
   - The ratio is `life / maximum`, read through slot `+0x12c`, or 0 when the
     maximum is 0 (`0x635990`).
   - The new maximum is `old + rounded change` (`0x9a01c0`), written through
     slot `+0x144` (`0x6cf070`, which writes object `+0x174`).
   - The new life is `round(ratio * new maximum)` (`0x99ffe0`, `0x9a1d70`).
   - The life floor comes from slot `+0x13c` (`0x664bb0`, true when life is at
     most 0): a dead object floors at 0.0, a living object at the value in
     memory at VA `0x112d810`. That address is zero in the file image, so its
     runtime value is not read here. The living floor of 1 is the observed
     behavior of unit pools (see `passive-max-life.md`).
   - The life is `max(new life, floor)`, written through slot `+0x124`
     (`0x6cee60`).
   - Slot `+0x10` (`0x83edd0`) runs after the write. Its meaning is not decoded.

Rule in plain words: the maximum moves by the rounded change. A living
destructable keeps its life/maximum ratio, rounded half upward, and never drops
below one hit point. A dead destructable stays at zero life. A change that rounds
to zero still rewrites life, since only the unrounded zero check skips the write.

Inferred, not observed: the change queues no death event and leaves the collision
state alone, because a dead tree stays at zero and a living tree stays above zero.
The full body of `0x6cee60` (the life writer) is not decoded.

Refused, not decoded: a new maximum at or below zero. The native writes it with
no check, and the life clamp and death path for that case are unread. The
recreation returns a precise error before any write.

Implemented in `destructable_host.rs` (`set_destructable_max_life`) with
synthetic tests in `destructable_host_tests.rs`
(`set_destructable_max_life_*`). Chapters that call it (scan of every campaign
map's `war3map.j`): RoC `Orc03`, `NightElf03`, `Orc08`; TFT `HumanX03`,
`NightElfX08`, `OrcX02_02`.

## Evidence

Synthetic owned-VM tests in `death_event_tests.rs` cover synchronous setters,
kill and restore paths, event/widget/destructable responses, already-dead and
positive-rearm behavior, source-derived restore amounts and live no-op behavior,
engine-damage boundary delivery, both cross-family
nesting directions, false conditions, sleeping actions, callback errors, source
and platform admission, the prepare/commit seam, and the shared nesting budget.

- `cargo test -p pcx-play --lib --locked --offline`: 1429 passed, 42 ignored.
- `cargo test -p peacecraft3 --features test-fixtures --bin peacecraft3 --locked --offline`: 607 passed, 55 ignored.
- Native and WASM strict Clippy for `pcx-play` and `peacecraft3`: pass.
- `cargo fmt --all -- --check`: pass.

The selected Human01 chapter activation gate admits the reviewed executable
and publishes 152 units; this checks production reader/startup wiring, not
rendered restore presentation. Native registration order,
unregistration and duplicate-identity behavior, item widgets, death animation
and sound, rendered/collision parity and the 100-consecutive original baseline
remain open. The life-setter probe receipt does not by itself qualify native
internal or presentation behavior.
