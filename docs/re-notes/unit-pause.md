# Explicit unit pause

Target: Legacy executable 1.29.2.9231 (plan 1.29b), SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.

## Source evidence

The selected `common.j` declares non-constant `PauseUnit(unit, boolean)` and
`IsUnitPaused(unit)`. `Blizzard.j`'s `PauseUnitBJ` forwards its arguments, and
`PauseAllUnitsBJ` enumerates units and pauses them individually. Human03 startup
reaches this path.

In the selected executable, the `PauseUnit` string at `0xe72228` has references
at `0x49d8f1` and `0xcb8bd9` (fresh Ghidra analysis and objdump agree). The primary
registrar installs `0x4a3f00`; the other installs the broad no-op stub
`0xcbda30`. Runtime selection of these registries is not established here.
The primary handler resolves the unit, returns on a missing unit, and branches
on the boolean at `0x4a3f3d`. True calls `0x69c6e0`; false calls `0x69c750`.
The former tests a unit flag before setting it and invoking its helper. The
latter tests the same flag before decrementing an internal suspension count
and clearing the flag. Repeated explicit true/false calls are therefore
idempotent on this path; the internal count does not make repeated
`PauseUnit(true)` calls stack. No addresses or flag masks are runtime constants.

## Recreation behavior

Each executed unit retains an optional pause-start time. A pause preserves its
route, order, turn, target and pending pickup or spell. Movement, turning,
automatic acquisition, attack progress, pending spell/pickup execution and
life/mana regeneration stop. Resume shifts movement and attack epochs by the
paused interval, preventing accumulated movement or windup from executing at
once. Repeated pause does not reset the start time. Paused units remain visible,
occupy their cells, can be targeted and can take damage. Already launched
missiles advance independently. Null and removed units are harmless queries or
no-ops; wrong families and malformed arguments refuse.

New supported movement, combat, Defend, pickup and item-use commands refuse
while paused, preserving the prior activity. Explicit script relocation, life
writes, ownership changes, visibility and removal keep their own semantics.
Animation and spell/item cooldown clocks are unchanged. These are recreation
choices, not a native comparison of every paused subsystem. In particular,
cooldown behavior, command refusal, regeneration and animation still need a
focused original-client experiment.

The production script session projects activity to the current clock before
delivering a native. Direct callers must do this before changing pause state;
an unprojected transition refuses without changing state. This keeps a pause
boundary from silently discarding an earlier unprojected interval. No new
projection, timer or event-delivery loop is introduced by the setter.

## Validation boundary

Synthetic tests check preservation, idempotence, resume without catch-up,
argument validation and lifecycle. They establish the recreation contract only.
The selected Human03 startup now passes its `PauseUnit` calls and refuses later
at `RestoreUnit` in `Blizzard.j:7480`. This is a startup-frontier result, not
unchanged campaign acceptance or an original-client comparison. Results belong
to the exact checked tree.

Worktree-private checks on this increment:

- `pcx-play` full package tests: 1,146 passed, 0 failed, 44 ignored. The four
  required campaign-drop tests used the explicitly supplied local Human01
  script, unit placements, `common.j` and `Blizzard.j` fixtures.
- `pcx-play` all-target clippy with warnings denied and the WASM library check:
  both exit 0.
- `peacecraft3 --all-targets --features test-fixtures` compile check: exit 0.
  This checks app consumers and fixtures; it does not execute or render the app.
