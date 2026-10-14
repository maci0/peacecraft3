# Bounded differential seed minimization

`pcx_play::minimize` reduces a reproducing seed over an explicit candidate corpus.
It supports the Tier C harness in PLAN.md §3.5; it does not establish native RNG,
combat ordering, a native baseline, or a global minimum over all u64 values.

## Fresh execution and trace identity

`minimize_differential_seed(config, oracle)` calls `oracle(seed)` to execute both
implementations freshly. Original verification must reproduce the requested
failure before the candidate iterator advances. A passing original, oracle error,
stale seed, incompatible identity or malformed trace returns a typed error.

The pair validator requires the supported trace version, canonical names and
unique ascending fix rows, matching scenario/map/asset mode/fix rows/tick count,
consecutive tick labels, ascending unit IDs within each tick, ascending RNG and
timer ordinals across the whole run, and canonical timer names. Candidate identity
must also match the original identity, excluding the deliberately varied seed.
Header differences cannot count as a reproduced gameplay failure.

`FailureSignature` offers `SameField`, `SameTickAndField` and `Exact`. A custom
`DivergencePredicate` may select a specific failure; unrelated earlier differences
do not replace the requested signature.

## Budgets and corpus scope

`MinimizationConfig::new(original, corpus, max_oracles)` uses the same positive
limit for inspections and oracle calls. `with_max_inspections` and
`with_max_oracle_evaluations` set the limits independently.

Original verification costs one inspection and one oracle call. Each yielded
candidate costs one inspection, including duplicate seeds. Only previously
unevaluated candidates invoke the oracle. Both ceilings are checked before
`Iterator::next`; inspection exhaustion wins if both limits are reached together.
The deduplication set contains only evaluated seeds and is bounded by oracle calls.

`CorpusExhausted` means the iterator actually returned None. At an exact budget
boundary, the result conservatively reports exhaustion rather than probing the
iterator beyond the ceiling. `minimum_seed` is the smallest reproducing seed among
checked candidates and the original, independent of caller order or monotonicity.

These counts bound invocations, not the internal work of a caller's iterator,
predicate or oracle. Blocking callbacks require caller-controlled termination.

`search_scope` provides finite bit-clearing, halving, sequential-small and combined
scopes. The caller may also supply an infinite iterator; inspection limits make
infinite duplicate streams terminate when each callback itself terminates.

## Command line

The existing trace comparison command is unchanged. The new command executes two
synthetic `.rpl` variants at each checked seed:

```text
pcx-diff minimize left.rpl right.rpl ORIGINAL MAX_ORACLES MAX_INSPECTIONS [CANDIDATE ...]
```

The two files must identify the same scenario, map and configuration, with their
authored action differences describing the failure. Both are parsed with the
existing strict RPL parser and run with their expectations preserved. The explicit
seed replaces each file's initial seed for every invocation. An expectation or
runner failure is an oracle error, not a reproduction.

Candidates are checked in argument order, deduplicated and reduced numerically.
The command reports original/minimum seeds, oracle/inspection counts, exhaustion
status and the minimum's first divergence. A successful bounded search exits zero;
invalid arguments, failed reproduction, incompatible traces or oracle failure
exit two without a success result. Default matching preserves the original field.
The library supports custom predicates and caller-supplied native execution oracles;
this CLI runs synthetic RPLs and does not capture original game behavior.

## Verification

`crates/pcx-play/tests/minimize.rs` checks signatures, errors, canonical headers,
inspection-before-next, fresh synthetic dual runs and deduplication.
`minimize_validation.rs` adds discriminating record/identity corruptions, infinite
duplicates, zero/exact budgets and an unsorted nonmonotone corpus. CLI tests drive
actual files through fresh RPL executions and preserve errors and existing trace
comparison behavior. These tests prove harness policy, not native fidelity.
