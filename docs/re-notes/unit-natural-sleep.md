# Natural unit sleep

## Source boundary

Selected executable: Legacy 1.29.2.9231 (plan 1.29b), SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The selected `common.j` declares `UnitAddSleep`, `UnitCanSleep`,
`UnitIsSleeping` and `UnitWakeUp` as non-constant unit natives. Human03's
`SetupCreeps` enumerates neutral-aggressive units and disables natural sleep
through `UnitSetCanSleepBJ`, which forwards to `UnitAddSleep`.

Primary registrations install handlers `0x4aec50`, `0x4aedc0`, `0x4af3c0` and
`0x4afab0`, respectively. The broad stub family also registers these names;
runtime selection remains unqualified. The primary setter looks up a component,
adds it only if absent, and removes it only if present. Its getter reports
component presence. The corresponding type getter at `0xb3ba00` returns the
natural-sleep ability identity. The permanent-sleep API uses a different getter
at `0xb3ba30`. `UnitIsSleeping` reads a live flag after a lookup using the latter
identity and different arguments; their full meaning is not established here.
`UnitWakeUp` calls a virtual method on that component.
These are static code reads, not an original-client runtime experiment.

`UnitMetaData.slk` maps the editable boolean to `UnitData.canSleep`. The native
class loader names that column at `0x6b3d6f`. Runtime preparation reads the
metadata-backed field from each resolved unit, preserving map overrides.
Missing, ambiguous, malformed or non-boolean fields refuse. No ability rawcode,
native mask or address is a production constant in this implementation.

## Current recreation behavior

Each executed unit retains natural-sleep permission separately from its live
sleeping flag. The setter is idempotent; disabling permission immediately wakes
the unit. The permission query reads permission, while `UnitIsSleeping` and
`IsUnitType(UNIT_TYPE_SLEEPING)` read the live flag. Null/removed handles return
false or do nothing; malformed arguments and other families refuse.

The existing activity pass puts eligible idle living units to sleep at night.
Its night window comes from the admitted clock and the selected script's
`bj_TOD_DAWN`/`bj_TOD_DUSK` constants. Missing clock/window data refuses when
sleep eligibility needs it. Moving units or units with a combat target/attack
destination remain awake. Sleeping units do not acquire targets or advance
motion; regeneration continues. No independent timer or simulation loop is added.

Accepted movement/attack/pickup/Defend orders, a successful Holy Light order,
successful item use and positive damage wake the affected unit. Explicit wake
does likewise. A pending wake allows one activity/combat pass, so a damaged unit
can retaliate before idle sleep is reconsidered. Dawn and death clear sleep.
Explicit pause preserves sleep state until activity resumes. Each visited sleep
record charges the existing activity work budget.

These scheduling and interaction rules are recreation choices. Native wake
proximity, resleep delay, owner eligibility, clock-edge/tick ordering and all
invalid/dead-handle cases are not qualified. The implementation adds no guessed
wake radius or duration. Sleep animation, indicators/effects and audio are not
projected. `UnitAddSleepPerm`/`UnitCanSleepPerm`, authored permanent-sleep ability
effects and spell-induced Sleep remain unsupported; the natural flag does not
stand in for those effects. Initial ability-level queries for implicit components
retain their existing refusal boundary. Unit-cache restoration currently resets
this runtime state from source rather than persisting explicit sleep overrides.

Synthetic VM tests cover permission, night/day transitions, wake independence,
acquisition suppression, damage/retaliation, death, invalid inputs and deterministic
work exhaustion. Pure tests validate boolean cells and overrides. These tests
establish the recreation contract, not native behavior or rendered fidelity.

## Current gates

Worktree-private, locked/offline checks pass: 1,162 playback package tests,
0 failures, 45 ignored local-data gates; strict all-target clippy; WASM library
check; app all-target compile with `test-fixtures`. Its renderer fixture now
explicitly carries unadmitted sleep state. The package supplies the existing
Human01 campaign-drop fixtures explicitly. Both selected Human01 headless gates
pass: repeated startup/ordered-versus-control movement and profile identity/isolation.
Source-field census and selected executable/script identities remain
in ignored local receipts; original asset bytes are not committed.

The unchanged Human03 ROC startup with seed one and an explicit fresh profile
passes `SetupCreeps` and later refuses `IssuePointOrderLoc` through
`Blizzard.j:7800`. Its setup contains a `patrol` location order at
`Human03.j:1997`; patrol is not implemented by the current activity host.
The startup gate exits 101. It is neither campaign acceptance nor a native sleep
comparison. Permanent/spell sleep, presentation and native wake rules remain open.
