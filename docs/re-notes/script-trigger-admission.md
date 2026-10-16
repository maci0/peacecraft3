# Trigger identity and unit-event registration

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
