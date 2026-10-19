# Explicit startup settings and runtime version

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
rate-changing `SetGameSpeed` requests therefore remain unsupported. No speed-to-time relationship is
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
source/type/interface changes and rate-changing speed writes. Native and actual WASI execute
the same tests. Command and source-stability receipts remain in the local root
evidence directory.

The later [explicit slot-state input](script-player-slots.md) passes that slot
getter without providing defaults or a live player provider. The application
Host factory, current settings provider, speed writes/rates and rendered fog
remain open. This continuation is not a playable chapter or campaign completion
claim.

## Explicit runtime version identity

`ObjectHost::admit_startup_version(context, version)` separately supplies the
current `VersionGet` result for the exact context already admitted for players.
The input is an actual symbolic arena identity in the selected `ConvertVersion`
domain. The loaded non-constant `VersionGet` and constant `ConvertVersion`
signatures, declaration sources, absence of overrides and authored version type
ancestry are checked against the Player/CreateTimer source. Unknown, wrong-family,
out-of-domain and wrong-context inputs refuse without changing a prior value.

The getter returns that same identity without allocating or translating it into
an executable version number. There is no default and no archive-name, display
skin or expansion-availability inference. Successful re-admission replaces the
input; `invalidate_startup_version` withdraws it. Version input and speed/fog/mask
input have independent lifetimes. The context check binds the map but does not
authenticate the caller's observation or install a live settings provider.
`VersionSupported`, `VersionCompatible` and other version natives are untouched.

The preserved original-client settings probe records the same
`VERSION_FROZEN_THRONE` result immediately before and after initialization. The
current private qualifier verifies both records, their generated-file hash and
the observation's map/common/BJ/executable inputs before resolving the recorded
name through the same initialized Program. The Host stores its loaded identity;
no measured enum integer is copied into production. This observation belongs to
that explicit launch/profile, not every map, archive set or client release.
No new original-client run was needed or performed.

The unchanged Human01 gate now passes its one executed `VersionGet`, creates two
more triggers and reaches 888 shared-arena identities, including 155 units and
six triggers. Prior source/context, player, force, sound, action and timer-event
checks remain intact. The next unsupported call is
`TriggerRegisterGameStateEvent`, Blizzard span `435358..435454`, common declaration
`72797..72927`. No game-state event or callback delivery is fabricated.

Three synthetic groups exercise actual owned-VM reads and source-global equality,
no identity growth, replacement and independent invalidation, malformed queries,
wrong family/domain/context, missing selected conversion, declarations, overrides
and authored ancestry. All 364 play tests pass natively and through actual WASI
execution, with five explicit original-input ignores. Strict native/browser
Clippy checks pass. The missing-API baseline, exact commands, source/artifact
hashes and source-observation-qualified gate stay local in
`.scratch/startup-version-root-20261017/`. The application provider and composed
Host factory, remaining event/scheduler semantics, rendered gameplay and campaign
completion stay open. No original asset or generated payload is published.

## Current-speed requests (2026-10-19)

An optional `SetGameSpeed(gamespeed)` declaration is now admitted with the exact
non-constant signature and the same source as the selected startup settings.
Missing declarations remain unsupported, including direct Host dispatch. A request
for the exact already-current identity returns nothing and preserves the retained
settings, arena and logical clock. Without an explicit speed lock, every other identity, null, malformed argument
list or request after invalidation refuses without changing state. No enum number
or speed-to-time factor is copied into production.

This supports the unchanged Human01 cinematic request for its already-admitted
speed. It does not implement rate changes. The retained original runtime probe
above shows Normal requested while Normal remains current. Additional bounded
static reads of that same executable locate the functional setter at `0x4a8790`:
it tests a context restriction bit, then delegates to `0x4c2450`. That callee
clamps the input byte, stores the current-speed byte and calls `0x5ae760`, which
sets a scheduler index and derives its rate from image data. The getter at
`0x492806` reads the stored byte. Those instructions explain the retained clamp
observation; they do not qualify the live restriction bit, all pacing inputs or
networked speed-change behavior. The reconstruction therefore acknowledges only
the already-current request. Full rate changes and original/native/WASM clock
parity remain open. Private static reads and verification receipts are in
`.scratch/campaign-speed-root-20261019/`; no original runtime was launched.

The unchanged private Human01 prefix now passes that current-speed request and
next refuses `SetMapFlag(MAP_LOCK_SPEED, true)` in Blizzard at `292599..292636`.
The explicit recreation seed and retained observation/profile limitations still
apply; this is not normal application bootstrap or a playable chapter. Native
Host/application/integration tests, strict native/WASM Clippy, eleven actual WASI
settings/version tests and thirty-six executable Cargo-WASM factory gate cases
pass. No new browser render or original-game runtime comparison was performed.

## Executed map speed lock (2026-10-19)

`SetMapFlag` and `IsMapFlagSet` now support the loaded `MAP_LOCK_SPEED` constant.
Its value is derived from its authored initializer and the selected conversion
domain. The exact native declarations, source, type ancestry and constant source
are checked. No flag value is copied from the original observation or executable.
Other map flags and combined masks remain unsupported until their consumers exist.

The map configuration retains no initial lock default. A query before an explicit
write refuses; a successful true or false write replaces only the speed lock.
While explicitly locked, a valid selected speed request returns nothing without
changing current settings, clock or identities. This also works without current
settings input: the lock does not invent a value for `GetGameSpeed`. After unlock,
only the already-current speed remains supported; actual rate changes still refuse.
Malformed calls, unknown/wrong-family tokens, outside-domain speed tokens and
foreign declarations cannot mutate the state.

Bounded static reads of the selected `1.29.2.9231` executable locate `SetMapFlag`
at `0x4a9320`: true ORs the input into context `+0x38`, and false ANDs its inverse.
`IsMapFlagSet` at `0x49932a` tests those same bits. The preceding speed investigation
shows `SetGameSpeed` testing that field before delegating to the speed setter.
The selected script's `MAP_LOCK_SPEED` initializer identifies that tested bit.
This supports the lock consumer; it does not establish other map-flag effects,
initial map flags, networked mutation or a speed-to-time mapping. Static reads and
verification receipts stay local in `.scratch/campaign-mapflag-root-20261019/`.

The unchanged private Human01 prefix now passes its source speed-lock write and
both following fog-state writes. The next refusal is
`EnableWorldFogBoundary(false)`, Blizzard `292712..292746`, common
`137947..138014`. All 1,247 native test executions, fifteen actual WASI
settings/version/lock tests, thirty-seven executable Cargo-WASM factory gate
cases, strict native/WASM Clippy and formatting checks pass. This slice performs
no new original-game runtime or browser rendering comparison. Rate changes,
normal application bootstrap, rendered fog and first-chapter playability remain
open.
