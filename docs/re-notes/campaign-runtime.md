# Campaign clock and initialization callbacks

## Ally-color display (2026-10-25)

The selected client binds `SetAllyColorFilterState(integer)` and
`GetAllyColorFilterState()` to the UI-option store, rather than unit ownership.
Read-only static windows from image
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
join the setter/getter (`4a6e70` / `490fd0`), option fallback table (`42b630`),
notification branch (`5ea740`) and unit palette (`5bd900`). The default, two
filter modes and neutral-owner classes are decoded from those inputs. Arbitrary
integer states round-trip; states outside the two filtered modes select
individual colors. These are static code findings, not a new original-client
runtime or rendered comparison. REA calls were unavailable under the session's
approval policy; the retained windows were mapped and hashed with local static
tools.

The host admits matching source identities and exact native declarations, keeps
the current raw state without allocating handles, and derives role colors from
ordered `UI/MiscData.txt` and map `war3mapMisc.txt` profiles. Model color indices
come from `TeamColorFilter`; minimap ARGB values come separately from `FogOfWar`.
The local player's own minimap color remains the player role in individual mode.
Passive alliances are directional and read from the canonical world. The
selected source's exempt neutral owner keeps its original model color.

Actors and portraits share the effective team-color query. A live filter or
alliance change rebuilds affected model instances using the existing skin's
TeamColor/TeamGlow paths, including retry after a previously unavailable palette.
Ordinary minimap markers use the skin's `MinimapSprite` model: dimensions,
authored offsets and alpha are parsed from its supported static quad, not copied
from the selected model. Individual player RGB comes from the first decoded
TeamColor texel, matching the selected palette initializer at `5d32e0` /
`5d36d1`. Markers honor canonical position, live owner color, shown/dead state,
typed `hideOnMinimap` cells/map overrides and the existing visibility raster.
Background RGBA is restored before every fog/marker update, preventing trails.
Count, allocation and pixel-work limits are application resource policies.

Nonstock profile, decoder and real-host tests cover separate palettes, map
overrides, raw-state round-trip, directional alliances, neutral roles and atomic
source/argument refusal. The explicit local-original rules and sprite gates pass.
The unchanged TFT entry passes this native and next refuses `Preloader` at
`Rise of the Naga:war3map.j:3545`; its preload behavior is not replaced with a no-op.

The mounted-menu offscreen RoC diagnostic reaches player control, freezes its
clock and toggles individual / filtered / individual through the actual native.
Rendered model instances match the host's effective indices; both their palette
map and the minimap RGBA buffer restore exactly on the final toggle. Captured
frames visibly change Thrall's team material and the minimap unit markers.
The strict overall render assertion still fails on the previously reported sky
illumination and terrain/tree sight occlusion. The color checks passing do not
convert that failure into an overall renderer pass. No original-client pixel
comparison or audio test was performed.

Remaining fidelity boundaries are observer team projection, hero/resource/camp
and remembered-building minimap layers, native subpixel coverage, and saving the
UI preference across map/application replacement. The current state belongs to
the admitted map host. Existing sky illumination, terrain/tree sight occlusion,
3D fog and water rendering gaps remain separate; a running script does not prove
rendered parity or a completed mission.

The launcher installs `campaign_runtime` beside campaign startup. It drives an
already published `MapScriptSession<MapStartupHost>`; it does not manufacture
missing startup inputs or turn successful map parsing into a running chapter.

The application uses a 1/64-second logical step, at most 64 steps per update and
4,096 timer expiries per step. These are recreation scheduling/work policies,
not values copied from game assets or a claim about the original engine tick.
The game-time origin comes from the admitted Host clock. A missing clock refuses.
`Time<Real>` only paces work: pending steps survive a slow frame and the per-update
budget. The runtime samples before startup publishes a replacement, then binds
that replacement on the following frame, excluding the loading-frame delta.
Explicit simulation pause and application-view suspension each exclude elapsed
time while retaining work already due. A successful
replacement resets the timeline by session identity; a refused replacement keeps
the prior session. Callback failure stops advancement without replaying effects
that completed before the failure.

The VM can now invoke Host-selected no-argument script callbacks on its existing
activation stack. Return values rejoin the original expression, and cooperative
suspension preserves that stack. Callback transitions consume the shared VM work
budget even for empty functions. Errors unwind Host callback contexts. The map
session's synchronous startup and timer caller still refuse yielding execution;
this does not add a multi-threaded mission scheduler.

With explicit trigger-runtime admission, present `TriggerEvaluate` and
`TriggerExecute` declarations must match their loaded signatures and the
`CreateTrigger` source. Evaluation combines registered boolean conditions;
execution runs registered actions. Both use retained arena order and the same VM
globals. Explicit calls validate the trigger independently of automatic-event
enablement. Nested calls restore the previous triggering trigger and inherit an
existing timer event context; a direct call outside an event has no event ID.
Callback-list changes apply on the next call. Native equivalence for mid-call
mutation, yielding and complete event/thread behavior remains open.

The reviewed `TriggerRegisterUnitInRange` path retains the actual trigger, unit,
finite range and optional filter, returning an event on the same arena. An omitted
budget preserves unsupported behavior. Registration neither evaluates the filter
nor changes placement. Accepted live positions and world-driven range delivery
remain separate work.

## Evidence

Native tests exercise the production factory, application schedule and published
VM. A 10,000-second simulated loading delta fails with the clock ordered after
startup and passes when ordered before startup. Steady, delayed and uneven frame
delivery reach identical logical ticks and script results after pending work is
drained. Nested direct callbacks, context cleanup, declaration refusal, failure
effects, cooperative VM resume and bounded empty callback chains are covered.
Actual Cargo WebAssembly executes the application/runtime and trigger gates.
These are execution checks, not rendered gameplay or original-game parity.

A local Human01 candidate completes globals/config, retains 155 `CreateUnit`
calls and reaches its map-initialization action through the new trigger dispatch.
Its next refusal is `SetCampaignMenuRace`. The probe uses an explicit recreation
seed of 1, a separately labelled compatibility catalog, retained local startup
observations and opt-in range registration. It does not prove the original engine
seed, prior RNG consumers, accepted unit placement or a published playable session.
The original game was not launched for this check. Command receipts and outputs
are local in `.scratch/campaign-runtime-root-20261019/`.

## Legacy campaign-menu call in Frozen Throne mode

The selected 1.29b image's functional `SetCampaignMenuRace` registration points
to a routine that compares the same mode slot returned by `VersionGet`. A nonzero
mode returns before reading the race or calling the campaign selector. The loaded
common source defines the Reign of Chaos and Frozen Throne version constants;
this establishes the ignored legacy call for the explicit Frozen Throne input.
The image also contains a separate generic fallback registration. Both sites
are retained in the private analysis; an arbitrary unique-registration assumption
was rejected. This is static code evidence, not a new original-game observation.

The existing Host now accepts that ignored call only with an admitted runtime
version matching the loaded, source-qualified `VERSION_FROZEN_THRONE` constant.
No numeric version or race mapping is copied into production. Native signatures,
source ownership and typed arguments remain checked. Missing or invalidated
version input, another version, missing/foreign declarations and the extended
`SetCampaignMenuRaceEx` call still refuse. No campaign selection is manufactured.
The Reign of Chaos path calls a real selector and remains unsupported.

All 489 native Host cases and six version cases under actual WASI pass, including
owned-VM continuation, no allocation, invalidation and declaration/argument
refusals. Strict native/WASM Clippy and both application builds pass. A local
production-factory run of the unchanged Human01 scripts now passes the legacy
call and next refuses `InitGameCache`, retaining the same 448 global calls,
538 cumulative config calls and 155 unit-creation requests. It uses the prior
explicit version observation, recreation seed and compatibility catalog; it is
still neither a published playable session nor a full original-game comparison.
Evidence stays local in `.scratch/campaign-menu-state-root-20261019/`.

The subsequent [primitive game-cache implementation](script-game-caches.md)
and application storage adapter now pass the executed `InitGameCache` call on
an isolated fresh profile. Blizzard.j retains its real arena handle; the same
Human01 prefix next refuses `EnableUserUI(false)`. Hero transfer, original W3V
import and complete chapter playability remain open.

Automatic Host-input preparation, live scene/unit/control wiring, source-qualified
speed and pause behavior, mission-native coverage and complete campaign playability
remain open. No menu appearance or public browser package is changed here.

The launcher also installs the [executed camera consumer](script-camera-setups.md#application-camera-consumer).
It binds an instant script setup to a real Bevy perspective camera over matching
admitted terrain, with a selected-source clip receipt and isolated render layer.
This supplies a live scene caller for supported published sessions; it does not
remove the missing automatic startup-input or mission-native gates for Human01.

An explicit post-config local-player selection now supports `GetLocalPlayer`
on the existing player arena. The application camera test follows the same
identity branch used by the loaded Blizzard camera helper. No local slot is
chosen automatically; see [player setup](script-player-setup.md#explicit-local-client-player).

Startup also installs a bounded lighting loader on the candidate Host using the
reader attached to its binding receipts. Executed day/night model arguments now
load their own source assets. The rest of automatic Host-input preparation and
the remaining Human01 mission natives still block playability. See
[day/night source preparation](script-day-night-models.md#application-source-preparation).

## Extended campaign menu selection (2026-10-25)

The unchanged TFT first chapter reaches `SetCampaignMenuRaceEx` through
`SetCampaignMenuRaceBJ` in the selected Blizzard.j. In the reviewed Legacy image,
registration `0x49ea38` binds the extended call to `0x4a7440`, which forwards its
integer to `0x47e490`. The selector calls `0x5901d0`, retains the integer unchanged,
and updates pending glue state. The latter stores signed values before its
negative-index branch; this is not a race-enumerand conversion or a copied TFT
campaign offset.

The host checks the owned common declaration and exact integer-to-nothing ABI,
then publishes the pending index. The application resolves it against the
original `CampaignList` positions, retaining blank slots rather than confusing
source indices with the compact displayed campaign list. Invalid or blank
positions select no manufactured campaign. Exit capture preserves a request
before dropping the VM, and the menu consumes it before the score consumer takes
the snapshot. The menu's existing selection/background path owns the result;
this does not launch a chapter or unlock one.

Synthetic checks cover nonstock signed indices, malformed/foreign declarations,
arguments, blank source slots and single consumption. The unchanged TFT startup
now passes this call and next refuses `SetAllyColorFilterState`. The RoC prologue
still reaches player control and advances through delayed setup. No new original
client menu-return comparison was performed.
