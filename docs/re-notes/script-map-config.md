# Executed map configuration state

`ObjectHost::with_map_config(MapConfigBudget)` explicitly admits map metadata
operations from the same Program already owned by the session arena. The existing
`TimerHost` alias and timer-only constructor remain compatible. Configuration
starts absent; admission creates unset fields rather than invented game defaults.
`map_config()` exposes a shared read-only view of values actually assigned by
executed script calls.

The admitted interfaces are `SetMapName`, `SetMapDescription`, `SetPlayers`,
`SetTeams`, `GetPlayers`, `GetTeams`, `SetGamePlacement`, `GetGamePlacement`,
`DefineStartLocation`, `GetStartLocationX` and `GetStartLocationY`. Their exact
signatures and constant flags must match the supplied common script. Source
ambiguity and script-function overrides are refused. API identifiers are fixed
interface contracts; no authored name, description, count, coordinate, placement
value or timing is embedded in this implementation.

Text passes through the existing VM/WTS resolution path before the Host receives
it. Name and description retain the exact resulting UTF-8 contents. The explicit
budget bounds their combined retained text bytes, checked before replacement
allocation; failed calls preserve the previous fields. This is not a peak-heap
promise: argument owners, an old value during replacement and allocator overhead
are separate. Malformed present WTS is a contextual failure, not missing text or
a literal `TRIGSTR` fallback.

Player/team counts retain assigned nonnegative integers without guessing a game
limit, allocating player slots, or clamping. Getters refuse before assignment.
Placement retains an already admitted symbolic `placement` identity from the
same arena; timers and ordinary objects cannot impersonate it. Start locations
retain sparse, nonnegative script indices and exact finite float coordinates.
Integer coordinates use the VM's existing integer-to-real conversion. Updating
an existing index preserves record count; adding a distinct index checks the
explicit record limit before allocation. Unknown queries refuse. No world-grid
translation, implicit start point, player assignment or camera movement occurs.

These values belong to the candidate session Host. The existing
`MapScriptSession::replace` factory/entrypoint sequence publishes a new session
only on success. Individual config calls are atomic, but successful earlier
calls in a subsequently failing config function are not rolled back inside that
candidate. Callers must continue to isolate candidates from published state.
This slice does not install a default launcher Host or bypass that factory.

## Verification

Eleven synthetic tests cover explicit admission, unset reads, combined UTF-8
budgets, failed replacement, counts, sparse and repeated locations, float bits,
capacity and platform-layout boundaries, placement identity, exact signatures,
source ambiguity and function overrides. Actual owned-VM config tests retain the
same Program and WTS owner across calls and input-owner drops. A malformed WTS
entry preserves its source diagnostic and leaves the description unset.
The initial two WTS fixtures used unsupported LF framing; they were corrected
to the checked CRLF format before the final verification run.

All 213 native tests pass with five explicit original-input ignores, strict
native/WASM all-target/all-feature Clippy and WASM library generation. Five
actual 32-bit WASM checks pass (mask 31, zero imports), including pre-allocation
layout refusal, retained localization, atomic text/location limits, placement
family isolation and malformed WTS diagnostics. Local actual commands and
captured output are in `.scratch/map-config-root/`.

The same retained Human01 map, approved scalar inputs, source-selected enum
contracts and identified compatibility catalog from
[selected-map-neutral-host.md](selected-map-neutral-host.md) pass all globals,
then execute the authored map `config`. Five metadata fields and five start
locations match the actual intercepted script calls, including coordinate bits.
The run now refuses `Player` at map-script line 7706. It observes 459 native calls
including that refusal; four timers, three groups and 432 symbolic identities
remain intact. Reinitialization makes no calls or replacement objects. Original
map, script, executable and profile hashes remain unchanged; no original text,
coordinates, payload bytes or profile are published.

This is progress through configuration, not a completed player/lobby setup,
config/main execution, world simulation, campaign or native exceptional-input
parity. Player identity/state and explicit launcher Host/profile loading remain
open. No native game process was launched.


The later [source-bounded player setup](script-player-setup.md) executes those
player assignments in the same retained context. The next actual config boundary
is `SetStartLocPrioCount` at map-script line 7765. Completed placement, config/main
and launcher wiring remain open.


The [start-priority slice](script-start-priorities.md) retains declared lists and
explicit target/priority relationships without inventing unassigned entries.
Together with player setup, the same authored `config` now completes. `main`
starts and reaches unsupported `GetCameraMargin`; runtime placement remains open.
