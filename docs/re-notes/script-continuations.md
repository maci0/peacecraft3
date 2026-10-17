# Cooperative script continuations

The existing `Vm` and `OwnedVm` can retain an unfinished call. A caller opts in
through `Host::suspend(name, args)`. Its default is false. A true response parks
that invocation without calling `Host::native` or claiming a successful return.
Implemented VM conversions still use their existing dispatch path.

`invoke` returns `Exec::Finished(value)` or `Exec::Suspended`. On suspension the
caller retains the VM and eventually calls `resume(value)` with that native's
result. A call statement discards the result. Expressions, arguments, indexes,
assignments, locals, returns and conditions use it in the saved expression.
Earlier effects are retained; short-circuit operands remain conditional.
Locals, globals, source spans, call depth and cumulative step work survive a park.
The existing synchronous `call` API returns an explicit error when it parks,
including the source and call span; it leaves the continuation available.
A second invocation while parked, idle resume and reentrant execution refuse.
Global initialization must finish and caches a suspension refusal like its other
initialization errors. It does not create a resumable startup thread.

The borrowed and owned localized Hosts forward suspension. The selected-map
neutral wrapper forwards only natives it does not own. The chapter trace wrapper
also forwards the hook; its synchronous chapter driver still refuses suspension.
The application startup factory, script session and current ObjectHost do not
admit a suspending timer native or drive a scheduler. `TimerStart` remains the
qualified Human01 refusal. No duration, expiry order, native thread policy or
`Sleep` behavior is supplied by this VM change.

Each activation retains one body snapshot. Returning or resuming uses that
snapshot, and repeated argument continuations share their remaining expressions.
Test-only deterministic counters guard linear body/argument copying. A separate
process checks a 4,096-argument call on a small native stack; WASI runs that input
directly. Synthetic control-flow comparisons check equal effects, globals,
returns and step work with and without suspension. These are implementation
checks, not original-client timing or performance parity. The VM still has its
existing harness step/depth guards and no aggregate heap bound.

Local review, failing regressions and validation receipts are retained under
`.scratch/vm-continuation-root-20261017/`. Original assets remain local.

Root validation passes 112 native and 111 actual WASI script unit tests, plus 14
script integration tests on each target. The play consumers pass 378 unit tests
and 65 selected integration tests on each target. Application checks pass 83
map-loading tests, 363 app tests, four library tests, strict native/browser Clippy,
a browser build, warning-denied docs and formatting. Explicit local-source and
GPU ignores remain. The same qualified Human01 production-factory replay passes
globals and config, retains 904 identities, 144 rectangles and 155 units, and
refuses `TimerStart` at common.j `61987..62103`, Blizzard.j `447183..447286`.
The first replay attempt stopped before execution because its historical
preparation receipt also named a rebuilt `scriptx` tool. The accepted replay
checks the unchanged native probe maps, source inputs and observation receipts;
that retired tool is recorded separately and is not re-executed.
