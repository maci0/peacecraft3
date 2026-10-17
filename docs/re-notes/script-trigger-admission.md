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
trigger-enabled default or run its actions. `TimerStart`, timeout/period queries,
expiry delivery and callback context remain unsupported. `DestroyTimer` retains
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
