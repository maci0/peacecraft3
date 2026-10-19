# Campaign clock and initialization callbacks

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

Automatic Host-input preparation, live scene/unit/control wiring, source-qualified
speed and pause behavior, mission-native coverage and complete campaign playability
remain open. No menu appearance or public browser package is changed here.

The launcher also installs the [executed camera consumer](script-camera-setups.md#application-camera-consumer).
It binds an instant script setup to a real Bevy perspective camera over matching
admitted terrain, with a selected-source clip receipt and isolated render layer.
This supplies a live scene caller for supported published sessions; it does not
remove the missing automatic startup-input or mission-native gates for Human01.
