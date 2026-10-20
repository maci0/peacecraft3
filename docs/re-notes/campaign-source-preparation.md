# Campaign source preparation

The normal native/browser application now prepares startup inputs after a map
delivery and before `CampaignStartupSet`. It uses the resolver published with
that delivery's exact receipt arc. Preparation runs once per context/revision;
an already supplied matching input is respected. Failure retains the previous
published session, reports the preparation error and permits matching corrected
inputs to recover.

The reusable `prepare` API supplies source rules only. The normal application
calls `prepare_new_game`, which additionally supplies the fresh-game state below.

The client reader supports the fingerprinted Legacy 1.29.2.9231 executable.
Reviewed instruction locations identify existing camera, numeric, random,
startup-clock, XP-handicap and hero readers. Values, lookup names and arithmetic
tables are read from the selected bytes. A different image refuses; no measured
rule values are embedded as replacements.

Stock tables and unit/item/ability/upgrade profiles come from the retained
resolver. Hero progression also reads the optional map misc and ability edits.
Conversion contracts come from the compiled source's constant conversion calls;
the existing Host still validates declarations, ancestry, source and domains.
The supported Defend alias is selected from the authored order pair rather than
assuming an alias. Input counts, bytes, inventory visits and Host storage have
explicit implementation bounds.

Sound stores start with empty, source-backed constructor banks. The existing
executed-path loaders supply actual resources when the script requests them.
This adds no placeholder SLK or audio payload. The elapsed timer epoch is a
recreation choice; time of day remains the separately decoded startup snapshot.

The local Human01 audit prepared 75 source members (6,501,648 bytes), selected
43 conversion contracts, constructed the production Host and completed globals
and the authored `config` on that same Program/context. It injected neither an
engine seed nor current runtime observations. The executable is qualified by its
fingerprint; the audit's compatibility stock archive release remains separately
unestablished. This is not proof that the entire source set belongs to one build.

Native and WASI executions of the selected client reader produce identical
camera/clock/XP words and 128 explicit-seed integer/real draw pairs. The original
client was not run for this check. It does not qualify the original engine's
initial seed, full gameplay call order or three-target simulation parity.

General research effects, normal placement/world integration and live orders remain open.
The factory's numbered slot input now covers later lazy player creation without
minting players early. A separate comparison of all sixteen retained original
Human01 slot states passes on the source-prepared Host, including eleven players
absent after `config`; this is explicit test input, not a production default.
The source-only audit stops after `config`. The later fresh-game audit attempts
unchanged `main` through the real application startup systems, as described below.
Neither publishes a playable mission or verifies rendering or the campaign UI.
The input task remains active. See [campaign runtime](campaign-runtime.md),
[startup settings](script-startup-settings.md) and
[map player roles](map-player-roles.md).

## Fresh-game state (2026-10-20)

The fresh single-player policy selects the sole authored human slot as local and
includes authored computer slots as participants. Multiple human choices refuse
until the caller can select one explicitly. After `config`, the same Host keeps
executed controller assignments, supplies source `MAP_CONTROL_NONE` for unassigned
normal slots and source `MAP_CONTROL_CREEP` for engine-neutral slots. The two slot
counts come from the selected native rules. Selected participants and neutral
slots are PLAYING; other slots are EMPTY. These symbolic values resolve through
the same compiled common source. This is a declared new-game reconstruction
policy, not occupancy inferred for an existing lobby or a general lobby model.
Only occupied participants can require player creation during setup; other player
identities remain lazy. Explicit current snapshots cannot be mixed with this mode.

The fresh speed uses the selected executable's `gamespeed` preference fallback,
passed through its decoded speed clamp. The two independent fog flags come from
the fog manager constructor fields read by the corresponding native getters.
Complete client identity and supported instruction shapes are checked. The
speed's symbolic global is selected from the loaded source conversion constant;
the frontend's explicit edition selects the corresponding loaded version global.
Saved preferences and saved-game restoration are not implemented by this path.
These inputs do not qualify simulation speed ratios or implement fog-of-war.

`NewGameInput.seed` initializes the source-bound script RNG in the factory before
globals execute. The normal application records its chosen seed in `CampaignSeed`
and its diagnostic log. A requested seed controls subsequently prepared games;
changing it does not reseed an existing session. Without an override, the app
requests platform entropy through [getrandom](https://docs.rs/getrandom/latest/getrandom/),
using its browser feature only on the browser target. No wall-clock value or
loading duration supplies the seed. This is an explicit recreation game input,
not the original engine's still-unqualified implicit seed-mixing policy. Existing
source-only callers remain unseeded, and authored reseeds retain their usual effect.

With an explicit test seed, the source-prepared unchanged Human01 Host matches
all sixteen retained original controller and slot-state identities, plus speed,
fog flags and the selected edition, without reading those observations as provider
inputs. The factory has the exact seeded state before globals. Native and actual
WASI executions of the fresh-settings reader agree. All 587 Host, 23 library,
151 map/application and 419 binary tests pass, along with strict native/browser
checks, both full builds and formatting. Rejected fixture attempts and the initial
short fog-getter window remain alongside the accepted evidence.

The real application startup audit uses the retained source reader, normal lazy
asset loaders and unchanged chapter script. A compatibility archive input lacks
the PCM ambient table and correctly refuses `LordaeronSummerDay`. Repeating with
the supplied Legacy archive inventory loads the ambient source and reaches
`GetPlayerTechResearched` in `InitSummonableCaps`. No current research provider is
installed, so the candidate is refused and no session is published. Normal source
lookup was not weakened to bypass the missing table. Archive release and native
archive priority remain distinct from executable identity; the private audit
declares its archive-set ordering explicitly. There is no new original runtime,
audible/GPU check or playable-game claim. Evidence is local under
`.scratch/campaign-bootstrap-root-20261020/`, particularly `fresh-game-6/`,
`fresh-reader-1/` and `fresh-application-3/`.

## Fresh upgrade counters (2026-10-20)

Normal new-game preparation now admits upgrade counters before globals, on the
same Host that executes the chapter. The selected client supplies the boolean
getter's minimum and the count helper's empty accumulator. Upgrade level caps
come from `UpgradeData` and the selected `UpgradeMetaData` field mapping, with
optional `war3map.w3q` overrides. Missing, ambiguous, indexed or malformed cap
inputs refuse. Unit/item identity collisions and custom base chains also refuse.
No captured research answers are installed as defaults.

The new-game policy starts known upgrade counters at the decoded empty sum.
This is a reconstruction of fresh research state, not a saved-game initializer
or proof of every original engine initialization step. Offline decoding of the
fingerprinted client shows that the setter subtracts the current count with
signed wrapping arithmetic, then adds or removes levels. Upgrade addition stops
at its limit and removal stops at zero. The recreation applies that result
without iterating once per requested level. Connecting that native limit to the
authored upgrade level-count column is an implementation choice supported by the
table schema; the entire native property lookup has not been reconstructed.

Executed setter requests remain separate from applied levels. The admitted
getter and optional count native read applied levels, as does the existing
Defend requirement. Invalidating the model prevents both queries and further
assignments without falling back to raw requests. Explicit observation mode
remains available separately and cannot be mixed with the fresh model. General
unit/item technology relationships, `AddPlayerTechResearched`, stat effects and
unit transformations are not implemented by these counters.

All 592 Host, 23 library, 151 map/application and 419 binary tests pass. Eight
targeted tests execute under WASI, including the direct Defend regression.
Strict native/browser checks, both full builds, formatting and source guards
pass. The unchanged Human01 script through normal application startup gets past
`InitSummonableCaps` and now refuses `TriggerRegisterTimerEvent` at Blizzard.j
line 1660. No session is published. The source-only diagnostic without application
asset loaders still refuses its earlier day/night call; that is a separate path.
There is no new original runtime, rendered/audio comparison or playable-mission
claim. Local evidence is `research-model-4/` and `runtime-state/research-*.json`
under the same private evidence directory. Earlier failed attempts are retained.

## Fresh timed-trigger input (2026-10-20)

Normal new-game preparation now supplies the existing bounded trigger runtime.
The fresh policy selects the enabled state, supported by the retained original
lifecycle probe in [the timer notes](legacy-timer-start.md#timed-trigger-lifecycle-reference-2026-10-18).
Its value comes from the selected `EnableTrigger` operation. The decoder checks
that enable, disable and the enabled getter address the same property and that
the two state operands differ. This does not reconstruct the original constructor
and property-initialization machinery; clearing raw constructor proxy fields
does not establish the final observable enabled state.

The timed-registration helper's event code and the trigger handler's event
normalization supply the event identity. Preparation finds its unique constant
`ConvertGameEvent` global in the same compiled common source, with matching type
and source. Speed now uses that same conversion-global lookup. Missing or
ambiguous constants refuse. No measured event ordinal or global name is copied
into the producer. The implementation's retained-event bound is explicit and is
not presented as an original limit.

This activates the existing arena, clock, callback tickets and session scheduler;
it creates no alternate VM. Source-only preparation still leaves the runtime
unset. Existing lifecycle limits, including yielding callbacks and callback-list
edits during a firing, remain open.

All 593 Host, 23 library, 151 map/application and 419 binary tests pass, with two
source-reader and four trigger-runtime tests executed under WASI. An initial
incorrect WASI filter selected zero cases and is excluded from coverage; the
corrected command explicitly checks four executed cases. Strict native/browser
checks, both full builds, formatting and source guards pass. Unchanged Human01
through normal application startup now reaches `TriggerRegisterUnitInRange` at
Blizzard.j line 1814. The candidate is refused and remains unpublished. The
registration-only range API is not enabled as a substitute for transitions.
There is no new original runtime or rendered/audio verification, and the mission
remains unplayable. Evidence is local under `trigger-supply-1/`,
`trigger-host-wasi-1/` and `runtime-state/` in the private bootstrap evidence root.
The selected-image fresh-settings/trigger reader also produces identical native
and actual WASI output; that comparison is retained in `trigger-reader-1/`.

## Normal creation-placement supply (2026-10-20)

Fresh preparation now retains the selected client's placement rules. The normal
application builds the existing collision cache immediately before its first
executed `CreateUnit`, after earlier stationary creations. The cache reads the
same admitted map's terrain, pathing, doodads and object edits, plus the actual
destructable records on the owned arena. It shares the already loaded stock
tables with the hero consumer and retains the same receipt-bound source reader.
No authored placement is aliased to an executed unit identity.

Stationary pathing fields follow their metadata table/column mapping and custom
base edits. Scalar column metadata supports whole-cell and first-slot indexes;
ambiguous fields, other indexes and unsupported edit selectors refuse. An actual
source run exposed the earlier synthetic fixture's incorrect assumption that
these SLK fields always use the whole-cell index. That refused attempt is retained.
Executed building footprint paths load lazily through the retained reader, with
byte, texture-count and aggregate cell bounds. A failed load does not stamp unit
occupancy. Terrain-domain sampling and quarter-turn stationary rasterization
remain reconstruction policies; arbitrary-angle geometry, moving units and
dynamic stationary updates are not qualified by this initialization slice.

All 594 Host, 26 library, 151 map/application and 419 binary tests pass, including
five core placement cases and three source-parser cases executed under WASI.
Strict native/browser lint, both full
builds, formatting and source guards pass. Unchanged Human01 through the actual
application again reaches `TriggerRegisterUnitInRange` at Blizzard.j line 1814.
Its candidate remains unpublished; the registration-only range API stays disabled.

A separate unsigned diagnostic map returns just after the authored unit-creation
call, leaving its helper bodies unchanged. That diagnostic publishes through the
normal application loader. All 155 executed requests and accepted `GetUnitX` and
`GetUnitY` bits match the retained original observations. Those observations are
test expectations only, not placement inputs. This is a native comparison with
the retained creation stream, not a new original run, full WASM mission comparison
or gameplay/rendering pass. Checks are local in `placement-supply-3/` and
`placement-creation-cut-3/` under the private bootstrap evidence root; failed
attempts remain separate. Range transitions, main scheduling, campaign delivery
and live world/orders still prevent playing the mission.

The parser WASI run compiles the production placement module and its exact
source-identity helper separately from the renderer. An attempted full application
WASI build fails in `wgpu-hal` before tests because its GLES buffer does not meet
the selected target's `Send`/`Sync` bounds; it contributes no coverage. The first
standalone runner also lacked the host procedural-macro search path and did not
compile. Both failures are preserved; the corrected runner explicitly executes
all three cases in `placement-source-wasi-3/`. This is CPU parser coverage, not a
rendered browser or full-application WASI pass.

## New-game range runtime (2026-10-20)

New-game inputs now bind the selected common's unit-range event and activate
the existing registration budget together with the position-driven consumer.
Loading and registration do not invoke callbacks. The application dispatcher
polls accepted unit records on logical simulation ticks, then runs their
filters, registered conditions and actions in the same owned VM. An initial
completed-`main` flush was rejected because it could dispatch at zero game time;
it is absent from the accepted caller.

The [range consumer notes](script-unit-world.md#deferred-range-consumer-2026-10-20)
describe context, ownership, work bounds and remaining geometry/lifetime limits.
All 604 Host, 26 library, 154 map/application and 419 binary tests pass. Fourteen
core tests execute under WASI, and eighteen application regression cases execute
on the actual browser-target WASM module under Node. Strict native/browser lint,
both full builds and source guards pass; this does not test browser rendering.
The placement diagnostic still matches all 155 retained original requests and
accepted position bits.

Unchanged Human01 through the actual application now gets past range registration
and reaches unsupported `IssueImmediateOrder` in Blizzard.j line 7817. Its
initialization issues `holdposition`, which needs an actual unit activity/order
consumer beyond the existing Defend-only branch. The candidate remains
unpublished and the mission unplayable. There is no new original runtime,
rendered/audio comparison or complete original/native/WASM simulation claim.
Evidence and rejected attempts remain local in
`.scratch/campaign-range-root-20261020/`.
