# Trigger identity and unit-event registration

`ObjectHost::with_trigger_admission` admits the loaded `CreateTrigger` and
`TriggerRegisterUnitEvent` declarations on the existing handle arena. It checks
their signatures, type relationships and declaration source, including the
selected `ConvertUnitEvent` domain. The caller supplies trigger and registration
budgets; arena capacity is also checked before allocating a token.

A registration retains its trigger, executed unit, symbolic unit-event handle
and enumerand. Its returned `event` token comes from that same arena. It does not
run a callback. Trigger enable/disable, destruction, conditions, actions and
event delivery remain unsupported. No enabled-state default is inferred from
creating a trigger or from a WTG record.

This is recreation state, not evidence of identical native storage or queues.
The application still needs its gameplay Host factory and event-delivery caller.

## Checks

Five native and actual WASI tests exercise the owned map VM, distinct arena
families, source/signature refusals, record budgets and exhausted shared capacity.
Failures preserve the prior tokens and record counts. Full library/application
tests, strict native/WASM Clippy, rustdoc and the browser-target library build
pass. Evidence is retained in
`.scratch/cursor-gameplay-20261016/root-trigger-admission/`.

The same locally supplied Human01 program still completes global initialization
and `config`. Its `main` now creates one trigger and registers one unit event;
the captured identities and loaded enumerand match the retained records. It
then refuses `TriggerAddAction` at map span `68526..68583`. The selected declaration
returns `triggeraction`, not `nothing`. This does not establish callback execution,
native event behavior, a rendered world or campaign completion.
