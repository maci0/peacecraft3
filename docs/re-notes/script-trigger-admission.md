# Trigger identities and retained event registrations

`ObjectHost::with_trigger_admission` admits the loaded `CreateTrigger` and
`TriggerRegisterUnitEvent` declarations on the existing handle arena. It checks
their signatures, type relationships and declaration source, including the
selected `ConvertUnitEvent` domain. The caller supplies trigger and registration
budgets; arena capacity is also checked before allocating a token.

A registration retains its trigger, executed unit, symbolic unit-event handle
and enumerand. Its returned `event` token comes from that same arena. It does not
run a callback. Explicit enable/disable assignments are retained as described
below. Destruction, condition evaluation and event delivery require a separate
consumer. The later [timed-trigger consumer](legacy-timer-start.md#owned-timed-trigger-consumer-2026-10-18)
admits positive-timeout delivery on this same arena and published VM; other event
families still have no world-driven delivery.
No enabled-state default is inferred from
creating a trigger or from a WTG record.

This is recreation state, not evidence of identical native storage or queues.
The application still needs its gameplay world and event-delivery caller.

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


## Player-unit event subscriptions

`ObjectHost::with_player_unit_events(max_records)` admits the selected
`TriggerRegisterPlayerUnitEvent(trigger, player, playerunitevent, boolexpr) -> event`
interface after trigger admission. The registration, `Player`,
`ConvertPlayerUnitEvent` and `CreateTrigger` declarations must share a source.
The selected conversion supplies the allowed enumerands; no event number or
player slot is copied into the implementation. The authored parameter ancestry
and the existing shared contract checks remain mandatory.

A call requires a created trigger, an existing player, a selected symbolic event,
and either null or an actual retained `Filter` or `Condition` wrapper from this
ObjectHost. The two registration consumers share the same wrapper validation;
a reserved boolean-expression token is insufficient.
A successful call allocates one distinct event in the existing arena and retains
those identities, the source enumerand and the optional filter identity.
`player_unit_event_registration` exposes that association, and
`retained_player_unit_events` counts it independently of other subscriptions.
Invalid arguments, the caller's record limit and shared arena exhaustion refuse
without changing prior records. No second arena or copied predicate is added.

`MapStartupBudgets.player_unit_events` is an optional factory admission. `None`
keeps the native unsupported. Registration does not evaluate the filter, infer
trigger-enabled state, enumerate units or deliver an action. Event
contexts, destruction/unregistration rules and world-driven event delivery remain
open. The retained subscription alone is not evidence of native event behavior.

At the earlier player-unit slice, the unchanged retained Human01 Program passed
this registration through the
production startup factory. Each retained identity and enumerand matches the
executed call. The verifier supplies an explicit synthetic game-time origin and
does not advance timers. Two executed `TimerStart` calls succeed; their timeouts
come from the selected scripts. Globals and `config` finish. `main` next refuses
`Condition`, at map-script span `117660..117717`, with common declaration
`71743..71804`. At that frontier the same arena has 922 identities, 155 units,
12 triggers and one player-unit subscription. Earlier source, localization,
player, unit, sound, force, action and timer checks still pass.

This verifier retains its explicit compatibility object catalog and qualified
startup observations; it does not establish target-catalog fidelity, complete
chapter execution, timer timing or the 100-consecutive unmodified-map baseline.
Commands, original-input fingerprints and actual refusal output remain local in
`.scratch/player-unit-event-root-20261017/`. No original runtime, new render,
public site or original payload upload was performed for this slice.


## Condition wrappers and trigger associations

`ObjectHost::with_conditions(ConditionBudget)` admits the loaded non-constant
`Condition(code) -> conditionfunc` interface. The native and the complete authored
return ancestry must share the source of `CreateTimer`. Only loaded functions
taking nothing and returning scalar boolean are supported. Wrapping stores the
function id in the existing arena and does not enter its body.
`condition_callback` borrows the id, name, source and span from that same Program.
Reserved handles and Filter entries are not adopted as Condition wrappers.

`ObjectHost::with_trigger_conditions(max_records)` independently admits the
selected `TriggerAddCondition(trigger, boolexpr) -> triggercondition` contract,
its type ancestry and trigger declaration source. A call requires an actual
created trigger and an actual retained Filter or Condition wrapper. A distinct
arena registration retains those two identities. `trigger_condition_registration`
returns that association; `retained_trigger_conditions` counts it separately
from wrappers, actions and events. Registration does not evaluate the predicate
or infer an enabled-state default. Record limits, shared arena allocation,
source/signature failures and malformed identities preserve prior records.

The factory takes two independent optional budgets: `conditions` and
`trigger_conditions`. `None` keeps the corresponding operation unsupported.
The shared boolean-wrapper validator also lets player-unit registrations retain
a Condition identity as their filter argument. Boolean operators, wrapper or
association destruction, predicate evaluation and trigger-event dispatch remain
unsupported. Repeated successful creations/registrations receive distinct tokens
under the recreation arena policy; native repeated-code interning is unmeasured.

At the condition-association slice, the unchanged Human01 Program passed two
Condition constructors and two
TriggerAddCondition calls through the production factory. Every wrapper matches
its executed loaded function, declaration source and span; every association
matches its actual trigger and wrapper. Globals and `config` finish. With the
same explicit synthetic game-time origin and no timer advance, `main` next
refuses `DisableTrigger`, map span `127485..127527`, common declaration
`70027..70093`. The same arena holds 932 identities, 155 units, 15 triggers,
two condition wrappers and two associations. Earlier source, context, player,
unit, sound, force, action, registration and timer checks still pass.

The compatibility catalog and qualified startup observations remain explicit
conditions of that verifier. This is recreated retention and script-execution
evidence, not native predicate behavior, timer timing, a rendered chapter or the
100-consecutive original baseline gate. Local commands, source/artifact hashes
and original-input receipts are in `.scratch/trigger-condition-root-20261017/`.
No original client, new render or site publication was run for this slice.

## Explicit trigger enabled state

The existing trigger admission also checks any loaded `EnableTrigger`,
`DisableTrigger` and `IsTriggerEnabled` declarations. Each must be non-constant,
take one scalar trigger, return the matching `nothing` or `boolean` type, have
no script override and share the selected `CreateTrigger` source. An absent
declaration stays unsupported. No extra factory option or state budget is added.

Each created trigger stores an optional enabled value on its existing arena
entry. Creation leaves it unknown. A successful `EnableTrigger` or
`DisableTrigger` sets only that trigger's value, and a repeated assignment
allocates nothing. `trigger_enabled` exposes the optional value. The native
query returns a boolean only after an explicit assignment; an unknown initial
state refuses instead of becoming a guessed default. A reserved identity whose
family name is `trigger` is insufficient. Invalid arguments and handles preserve
all previously assigned state and retained registrations.

The owned-VM checks cover both assignments, repeated calls, independent triggers,
unknown state, malformed/reserved handles, optional and conflicting contracts,
and unchanged unit-event/action registrations. Neither assignment evaluates a
condition nor runs an action. Event delivery and destruction remain unsupported;
this state alone does not make a subscription deliverable.

The explicit-state slice used the unchanged Human01 Program, production factory,
compatibility catalog, qualified startup inputs and explicit verifier time
origin. One `DisableTrigger` succeeds. Its retained value matches the executed request;
every other created trigger remains unknown. Globals and `config` finish.
`main` next refuses `TriggerRegisterPlayerEvent`, Blizzard span `80853..80926`,
common declaration `75025..75150`. The same arena still holds 932 identities,
155 units, 15 triggers, two condition wrappers and two condition associations.

This is reconstructed state retention and script-execution evidence. Initial
native state, event behavior, timing, rendered gameplay, chapter completion and
the 100-consecutive original baseline remain open. Local commands, immutable
source/input hashes and results are in `.scratch/trigger-state-root-20261017/`.
No original client, new render or site publication was run for this slice.

## Player-event subscriptions

`ObjectHost::with_player_events(max_records)` independently admits the loaded
non-constant `TriggerRegisterPlayerEvent(trigger, player, playerevent) -> event`
interface after trigger admission. The selected `Player` and `ConvertPlayerEvent`
contracts, declaration sources, type ancestry and selected conversion domain
must match. The factory's `MapStartupBudgets.player_events` is optional; `None`
keeps registration unsupported. No event number or player slot is copied from a
capture into the implementation.

A successful call requires an actual created trigger, an existing player and a
selected symbolic player-event value. One returned event token on the existing
arena retains those identities and the loaded enumerand.
`player_event_registration` exposes that record; `retained_player_events` counts
it separately from unit, player-unit, timer and game-state subscriptions.
Repeated pairs receive distinct tokens under the existing recreation resource
policy. The caller's record limit and shared arena capacity are checked before
allocation and counter changes. Invalid arguments, reserved identities, unknown
handles and wrong families preserve prior records.

Registration neither changes enabled state nor executes an action. A disabled
trigger can retain a subscription. Native duplicate-registration identity,
unregistration, event context, dispatch and event ordering remain unverified or
unsupported. This record alone does not make the event deliverable.

Synthetic owned-VM tests cover distinct pairs and repeated pairs, disabled-state
preservation, unexecuted actions, source/signature/domain refusals, separate
budgets and exhausted shared capacity. The application regression drives actual
map delivery through the production factory and published session. A missing
opt-in or exhausted record budget on a replacement keeps the previous session,
its context, lifetime witness and retained event records.

The unchanged Human01 Program now passes one player-event registration through
that factory. The event retains its executed trigger, player and selected
enumerand, and the earlier explicit disabled state stays intact. Globals and
`config` finish. `main` next refuses `CreateRegion`, Blizzard span
`81939..81953`, common declaration `67878..67938`. The same arena holds 945
identities, 155 units, 20 triggers, three condition wrappers/associations and one
player-event subscription. Earlier source, timer and startup checks still pass.

The retained compatibility catalog, qualified startup inputs and explicit
verifier clock remain conditions of that run. It does not prove native event
behavior, rendered gameplay, chapter completion or the 100-consecutive original
baseline. Local commands, immutable input/source hashes and results are in
`.scratch/player-event-root-20261017/`. No original client, new render or site
publication was run for this slice.

## Widget death subscriptions

`ObjectHost::with_death_events(max_records)` admits the selected non-constant
`TriggerRegisterDeathEvent(trigger, widget) -> event` contract after trigger
admission. The selected creation declarations and widget/unit/destructable type
ancestry must share its source. Missing or incompatible contracts, script
overrides, duplicate admission and impossible platform capacities refuse.
`MapStartupBudgets.death_events` is optional; absence keeps the native unsupported.

A successful call requires an actual created trigger and an existing unit or
destructable entry. Reserved identities with a matching family name, nulls,
unknown identities and other families refuse. One event token on the existing
arena retains the exact trigger and widget. `death_event_registration` exposes
that pair; `retained_death_events` counts these records independently of unit,
player, timer and other subscriptions. Repeated pairs get distinct tokens under
the same recreation resource policy as other retained subscriptions. Record
limits and arena capacity are checked before counters change, preserving prior
records on failure. The path introduces no second arena, world, VM or clock.

Registration does not change explicit trigger-enabled state, infer a life total,
kill or resurrect a widget, evaluate conditions or run actions. A destructable's
creation-time dead-form flag is retained separately and is not interpreted as a
new death transition. Item widgets are unsupported because this arena has no
created item entries. Destructable death detection, already-dead behavior and
synchronous action delivery are now implemented; native duplicate identity,
unregistration, native event ordering, rendered behavior and the original
baseline remain open. See
[destructable death delivery](destructable-death-delivery.md).

Synthetic owned-VM checks cover actual unit and destructable associations,
repeated pairs, disabled state and unexecuted actions. Invalid-input checks leave
capacity available so exhaustion cannot conceal missing handle validation.
Separate record/arena limits, source/type/signature checks and optional admission
are exercised. The application regression delivers a real synthetic map through
the production factory; replacement without opt-in or with an exhausted limit
keeps the previously published context, lifetime witness and event record.

The unchanged Human01 Program now passes three death subscriptions through the
production factory. Each returned event retains its actual existing trigger and
destructable identity; all earlier source, startup, timer and region checks still
pass. Globals and `config` finish. `main` next refuses `GetRandomReal` at map span
`186558..186584`, common declaration `134223..134292`. The same arena holds
1016 identities, 144 rectangles, 155 units and 36 triggers. The explicit verifier
clock remains unchanged. The retained compatibility catalog and qualified startup
observations are conditions of this check, not inferred original defaults.

Commands, preserved failed fixture runs and successful source/input/artifact
hashes are local in `.scratch/death-event-root-20261017/`. No new original-client,
render or public-site run was performed. Original random-stream behavior, event
delivery, the application world/clock, full chapter completion and the
100-consecutive original baseline remain open.
