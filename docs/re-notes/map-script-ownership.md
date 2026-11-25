# Retained map script execution

The borrowed `Vm::new(&Program, host)` and `MapScripts::vm(host)` APIs retain
their existing lifetimes. An application session can instead own
`OwnedVm<H>`, constructed with `OwnedVm::new_owned(Arc<Program>, host)`.
Both forms share the same initialization and interpreter implementation.

`Arc<MapScripts>::owned_vm(host)` retains the resolved program, source text,
source identities, digests and raw WTS table. Its `OwnedLocalizedHost` keeps
localization and diagnostic rendering attached to that same map after the
importer's temporary owners have gone. Repeated calls use one VM's globals
and Host. Separate VMs sharing a compiled context have independent state.

The owned program reference is bounded by the VM borrow; the borrowed VM's
`program()` reference still has the original program lifetime. Session
replacement drops its previous owners normally. External `Arc` owners may
deliberately keep contexts alive. No self-reference, leaked owner or new
unsafe operation is needed.

Synthetic public tests compare borrowed and owned execution, retained
callbacks, WTS lookup and source diagnostics. They also check sticky failed
initialization, owner release and independent VM state. The same five groups
execute in 32-bit WASM. These tests establish ownership and the project's
interpreter behavior, not native event ordering or handle allocation.

This ownership API leaves installation and gameplay to the caller.
[Map script sessions](map-script-session.md) use tickets to associate callbacks
with the correct map across replacement. The tests here cover VM ownership.
They leave total session memory, initialization cost and native scheduling
unmeasured.
