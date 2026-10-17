# Explicit startup speed, fog and mask state

`ObjectHost::admit_startup_settings` installs caller-qualified current settings
on the exact map-context Arc already admitted for players. It retains the actual
symbolic speed identity and two independent booleans. Absence is unsupported,
including when an explicit admitted boolean would be false. No startup default,
speed ordinal or simulation-rate mapping is supplied.

Admission validates the loaded signatures, constant flags, absence of function
overrides and unambiguous declaration sources for `GetGameSpeed`,
`IsFogEnabled`, `IsFogMaskEnabled`, `FogEnable`, `FogMaskEnable` and the explicitly
selected `ConvertGameSpeed`. They must share the admitted Player/CreateTimer
source. The speed type's ancestry is walked from the loaded declarations to
`handle`, with source checks; no measured parent chain is copied. The speed must
be an actual symbolic arena entry in that selected conversion's domain. A
same-family identity outside that domain cannot supply current speed.

The three getters return the retained values. The two fog setters validate one
boolean and change only their own field. These operations allocate no identities.
Malformed calls and failed re-admission leave previous state intact. Successful
re-admission replaces the settings. The caller must refresh or call
`invalidate_startup_settings` when external settings change. Context identity
binds the map; it does not authenticate runtime provenance or synchronize a live
settings provider. Boolean retention does not implement fog rendering, visibility,
exploration or fog modifiers. Speed retention does not change a simulation clock.

## Original-client observation

One fresh private Wine/software-renderer run used the supplied executable with
VERSIONINFO `1.29.2.9231`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, at
1280×960 and 96 DPI. The selected Human01 source map has SHA-256
`f5b155615dbce93b88fff80214a413b137c6fbf8c12abf357976a6ff42040ee9`.
Executable identity is separate from archive/map release and runtime edition.
The run reports `VERSION_FROZEN_THRONE`; the chapter filename does not establish
edition. Selected compile-prelude receipts do not prove runtime archive winners.

All 17 retained map members were independently re-extracted; only `war3map.j`
changed, and reversing the edits restores its original bytes. The unsigned
experiment omits attributes/signature metadata. Its helper derives speed names
from the selected common script's conversion globals, saves current settings,
tries every source speed and each boolean value, then restores the settings
before the original `InitBlizzard` call. The original configuration is unchanged.
The resulting preload text is read as data, never executed. A running chapter
capture follows the completed loading screen; the container is stopped and
preserved. Harness stop is not proof of a clean game exit.

The 52 nonce-qualified records include 15 settings records and a before/after
player census. Before and after initialization, current settings read
`MAP_SPEED_NORMAL`, fog true and mask true. These are inputs for this one launch,
not universal defaults. Slowest, Slow and Normal requests read back their selected
source values. Fast and Fastest requests both read back Normal. The observation
does not distinguish clamping, context restrictions or ignored requests;
`SetGameSpeed` therefore remains unsupported. No speed-to-time relationship is
inferred; the separate [simulation clock investigation](legacy-simulation-clock.md)
remains partial.

Fog and mask each read back false and true as requested while the other flag
stays unchanged. Restoration and the before/after initialization snapshots agree.
These getter observations support the retained boolean state, not rendered fog
parity. The census and null-force members belong to that same startup context;
controller/slot-state eligibility and live participation remain separate gates.

Local preparation, command logs, observation hashes, reviewed captures and stop
receipts are in `.scratch/game-settings-root-20261017/native/`. No original assets,
modified maps or generated records are committed or uploaded. One instrumented
run does not meet the consecutive original-baseline gate.

## Unchanged Human01 continuation

The existing private Host verifier loads the preserved observation, resolves its
speed global through the same initialized VM and supplies the booleans and force
participants after `config`. It does not embed observed enum values in production.
All three `bj_cineModePrior*` globals match that input. Prior shared-arena records
remain checked: 868 identities, 155 units, seven filter wrappers, 17 forces and
17 explicit memberships. The next refusal is `GetPlayerSlotState` in selected
`Blizzard.j` at `438504..438537`, declared in common at `61390..61470`.

Synthetic owned-VM tests save, toggle and restore state, cover every fog/mask
combination, retain exact speed identity and verify no handle growth. Refusal
checks cover absent/invalidated state, malformed calls, wrong map context,
missing selected conversion, wrong-family/unknown/out-of-domain speed tokens,
source/type/interface changes and speed writes. Native and actual WASI execute
the same tests. Command and source-stability receipts remain in the local root
evidence directory.

The later [explicit slot-state input](script-player-slots.md) passes that slot
getter without providing defaults or a live player provider. The application
Host factory, current settings provider, speed writes/rates and rendered fog
remain open. This continuation is not a playable chapter or campaign completion
claim.
