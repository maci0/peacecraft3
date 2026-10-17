# Trigger identities and retained event registrations

`ObjectHost::with_trigger_admission` admits the loaded `CreateTrigger` and
`TriggerRegisterUnitEvent` declarations on the existing handle arena. It checks
their signatures, type relationships and declaration source, including the
selected `ConvertUnitEvent` domain. The caller supplies trigger and registration
budgets; arena capacity is also checked before allocating a token.

A registration retains its trigger, executed unit, symbolic unit-event handle
and enumerand. Its returned `event` token comes from that same arena. It does not
run a callback. Trigger enable/disable, destruction, conditions and event delivery
remain unsupported. No enabled-state default is inferred from
creating a trigger or from a WTG record.

This is recreation state, not evidence of identical native storage or queues.
The application still needs its gameplay Host factory and event-delivery caller.

## Checks

Nine native and actual WASI tests exercise the owned map VM, distinct arena
families, source/signature refusals, record budgets and exhausted shared capacity.
Failures preserve the prior tokens and record counts. Full library/application
tests, strict native/WASM Clippy, rustdoc and the browser-target library build
pass. Evidence is retained in
`.scratch/cursor-gameplay-20261016/root-trigger-admission/`.

The same locally supplied Human01 program still completes global initialization
and `config`. Its `main` now creates three triggers and registers three unit events;
the captured identities and loaded enumerands match the retained records. It also
registers three actions and reaches 153 source-bound executed units. The
subsequent hero-request gate retains one executed `SetHeroLevel` assignment
and refuses `SetUnitState` at map span `67684..67742`. This does not establish callback execution,
native event behavior, a rendered world or campaign completion.

## Authored action registration

When declared, `TriggerAddAction` must have its loaded
`trigger, code -> triggeraction` signature and share the trigger declaration
source. Programs without that declaration retain creation/registration support.
The callback must be a loaded function taking and returning nothing. A successful
call retains its function id and owning trigger on one arena action token; the
read-only record obtains its source/span from the same immutable program.
Action budgets and arena capacity are checked before allocation.

Registration does not execute the function. Synthetic owned-VM tests distinguish
registration order from function declaration order, prove the callback stays
unexecuted until explicitly called, and check bad identities, signatures and
capacity without partial allocation. Raw function ids are meaningful within the
current program; deferred application callbacks still need the existing session
ticket rather than a bare id rebound after map replacement. The action integration
evidence is in `.scratch/cursor-gameplay-20261016/root-trigger-actions/`.

The later [Filter wrapper slice](script-filter-wrappers.md) retains boolean
callbacks on the same arena without evaluating them. This does not add trigger
conditions or predicate delivery; those remain separate consumers.

## Timer-expiry subscriptions

`ObjectHost::with_timer_events(max_records)` independently admits
`TriggerRegisterTimerExpireEvent` after trigger creation is admitted. Its loaded
non-constant contract must be `trigger, timer -> event`, share the source of
`CreateTrigger` and `CreateTimer`, and retain the authored timer ancestry from
that source. Missing or conflicting declarations, script overrides, a foreign
type source, repeated admission and impossible platform capacities refuse.
Programs that omit the opt-in keep this native unsupported.

A successful call requires an actual created trigger and a live created timer.
It allocates one event record on their existing bounded arena and retains those
two identities. Reserved objects with the same family names are insufficient.
`timer_event_registration` exposes the association; `retained_timer_events`
counts these records separately from unit events. Repeated registration returns
a distinct token under the same non-recycling recreation resource policy as the
existing unit-event records. Record limits and arena allocation fail before any
counter changes, preserving prior records.

These are retained subscriptions. Registration does not start the timer, infer a
trigger-enabled default or run its actions. This registration slice alone does
not admit `TimerStart`, time queries, expiry delivery or callback context. `DestroyTimer` retains
its existing identity-revocation behavior. Historical subscription metadata stays
inspectable, but no dead timer can be registered again; retaining that record is
not evidence of native unregister semantics or a deliverable event. A future
consumer must validate live dependencies and session tickets before delivery.
No independent clock, guessed timeout, tick conversion or callback queue is added.

Four additional synthetic tests cover owned-VM registration with unexecuted
callbacks, separate unit/timer record counts, exact association and event family,
repeated pairs, zero/exhausted record limits, shared-arena exhaustion, reserved,
unknown, malformed and destroyed identities, contract/source/ancestry admission,
and platform layout limits. All 361 play tests pass natively and through actual
WASI execution, with five explicit original-input ignores. Strict native and
browser-target Clippy checks pass.

The same unchanged Human01 Program, source-bound resources and explicit startup
inputs now pass the timer-expiry registration. The returned event points at its
actual existing trigger and live timer; the following `TriggerAddAction` retains
its loaded callback. No timer start or event delivery occurs in this slice. The
same arena contains 882 identities, 155 units, four triggers, three unit-event
registrations and one timer subscription. Existing context, source, player,
unit, sound and force checks still pass. The next unsupported call is `VersionGet`,
Blizzard span `440139..440151`, common declaration `116044..116091`. No version
is inferred from an archive name, neighboring image or skin selection.

Commands, preserved failed fixture checks, stable source/artifact fingerprints
and the successful selected-program gate are local in
`.scratch/timer-event-root-20261017/`. This is recreated registration state and
script execution evidence. Original callback timing, native lifetime behavior,
application Host construction, scheduler delivery, rendered gameplay and chapter
completion remain open. No original-client run or original payload publication
was performed for this change.

## Game-state subscriptions

`ObjectHost::with_game_state_events(max_records)` separately admits the loaded
non-constant `TriggerRegisterGameStateEvent(trigger, gamestate, limitop, real) -> event`
contract after trigger creation is admitted. It requires the selected
`ConvertIGameState`, `ConvertFGameState` and `ConvertLimitOp` contracts, their
source-defined domains and the trigger declaration source. The authored state
ancestry must reach `gamestate`; all state/comparison ancestry is walked to
`handle` with source checks. No state number, comparison number, threshold or
parent chain is copied from a measured input into production.

The call requires an actual created trigger, a symbolic integer or float state
in its corresponding selected domain, a symbolic comparison in the selected
limitop domain, and a finite real threshold. Integer threshold arguments use the
existing VM's `i32 as f32` promotion. Each successful call allocates one event on
the existing bounded arena and retains these identities and threshold bits.
Repeated calls receive distinct records. Independent record limits and shared
arena capacity are checked without changing prior records on failure.

`game_state_event_registration` exposes the trigger, state identity, borrowed
state type, comparison identity and real threshold. Its `state_value` and
`limitop_value` are source enumerands, not sampled game-state values or an
interpreted operator. The selected program and enum arena remain their authority.
`retained_game_state_events` counts these records independently of unit and timer
events. Nulls, unknown/wrong families, unselected/out-of-domain values, nonfinite
thresholds, incompatible declarations and conflicting sources refuse.

Registration does not query or change a game state, evaluate a comparison, invent
an initial condition, advance time or deliver an action. The immutable startup
clock is unchanged. Condition evaluation, crossing/initial-notification rules,
clock integration, trigger-enabled semantics and callback context/delivery remain
open; a future consumer must use the actual session and its callback tickets.
No second clock, VM, handle pool or scheduler is added.

Three new synthetic groups exercise owned-VM integer/float subscriptions,
loaded enum identities, exact negative/signed-zero threshold bits and integer
promotion, independent counters, unexecuted actions, interface/source/ancestry
checks, zero/full record policies and arena exhaustion. Invalid-input tests keep
one record slot available so exhaustion cannot mask missing validation. All 367
play tests pass natively and through actual WASI execution, with five explicit
original-input ignores. Strict native/browser Clippy checks pass.

The unchanged Human01 Program, resources and qualified startup inputs now pass
six game-state registrations. Every retained trigger/state/comparison identity,
source enumerand and threshold bit matches the executed calls. Previous source,
context, player, unit, force, sound, action, version and timer checks still pass.
The same arena contains 901 identities, 155 units, nine triggers, three unit
events, one timer subscription and six game-state subscriptions. Its next
unsupported call is `GetCameraBoundMinX`, Blizzard span `440924..440944`, common
declaration `154188..154249`. No camera-bound getter value is fabricated.

The missing-API baseline, initial and strengthened tests, exact commands, stable
source/artifact hashes and successful selected-program verifier stay local in
`.scratch/game-state-event-root-20261017/`. This is retained registration and
recreated script execution evidence, not proof of original event delivery or a
playable chapter. No new original-client, browser, render or site run occurred.


The later [positive-timeout timer consumer](legacy-timer-start.md#bounded-recreation-timer-consumer)
admits non-yielding direct handlers on the same arena and published VM. An expiry
with any matching retained subscription refuses before consuming that deadline;
its direct handler is not run in place of the unresolved trigger actions.
