# Supplied declarations and script error scopes

The existing I2R/R2I conversion implementations require one scalar parameter and
their corresponding scalar result type. `NativeTable::call` now distinguishes a
supplied declaration that cannot describe that implementation using
`SignatureMismatch`. Execution refuses it with the declaration's source,
signature and byte span. This prevents zero-argument indexing and incompatible
results; compatible conversions and unknown-native Host dispatch retain their
existing paths. The contract comes from the implemented conversion bodies, not a
stock signature table or proof of native exceptional-input behavior.

A local array shadows a same-name global scalar. Reading its bare name reports
the existing array-needs-index error rather than returning the global value.
Indexed operations, scalar shadowing and source spans retain their existing
behavior.

A global declared as an array cannot carry a scalar initializer under the existing
VM array contract. Initialization now refuses that declaration before evaluating
its expression, matching the existing local-array refusal. The error retains its
declaration span/source and remains cached, so retries cannot repeat earlier Host
calls. Valid uninitialized arrays and scalar initializers retain their existing
paths. This is project validation; native initializer admission and ordering are
not established by these tests.

Only a literal without authored code-tag parts enters trigger-string lookup.
The decoded text representation omits those parts, so using it as a WTS identity
could previously fetch an unrelated entry and bypass missing-tag errors. Tagged
literals now keep the existing substitution/refusal path. Plain WTS references,
tags inside table bodies and opaque handle values retain their existing paths.

External calls with incorrect argument counts have no script call-site span.
Their diagnostics use the resolved function's declaration source/span. Calls
originating inside a script retain their caller's span. Registered callback tests
use caller-supplied synthetic Host delivery; they establish this diagnostic path,
not native event scheduling or a gameplay Host.

Public synthetic tests exercise actual MapScripts/VM consumers, compatible and
incompatible declarations, unknown Host calls, callback refusal/recovery and local
scope lookup. Compilation and interpreter checks do not establish original-game
native coverage, event ordering, array limits or campaign playthrough.
