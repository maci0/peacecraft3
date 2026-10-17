# Filter wrappers on the script arena

`ObjectHost::with_filters(FilterBudget)` admits the loaded non-constant
`Filter(code) -> filterfunc` interface. The result must be a boolean-expression
agent type. Its complete ancestry is read from the Program, not copied from a
measured parent table; each declaration and the native must share the source of
the already-admitted `CreateTimer`. Script overrides, ambiguous native sources,
wrong signatures and foreign-source type overrides refuse admission.

A successful call stores the loaded function id in the existing `EnumHost`
arena. Only zero-argument functions returning scalar boolean are supported.
Callback validation, the caller wrapper bound and shared arena capacity precede
allocation. Failed calls leave prior records and tokens unchanged. Each successful
call currently issues one token; native repeated-code identity/caching remains an
explicit fidelity question. Wrappers live until Host/session drop. Destruction
and other boolean-expression constructors/operators remain unsupported.

Creation does not run the predicate. `filter_callback` borrows its id, name,
source and span from the same immutable Program. Reported kind comes from the
loaded native's return declaration. An ordinary reserved object with the same
kind name is not a wrapper and cannot be adopted by the accessor.

The caller can explicitly invoke a retained function on the same owned VM;
false remains false. Deferred application delivery must use the existing
[session callback ticket](map-script-session.md), minted when binding from the
active session. A bare id must not be rebound after replacement. No second VM,
handle pool, copied function body or predicate-execution API was added.

The unchanged Human01 now completes globals and `config`, creates seven wrappers
in `InitBlizzardGlobals`, and reaches `CreateForce` in `Blizzard.j` at
`437580..437593`, declared in selected `common.j` at `65813..65872`. Each wrapper
matches its executed code argument and loaded declaration. The same Host/context
retains 850 identities, 155 units, 142 rectangles, 26 directed alliance tuples,
one player-state request and the earlier trigger/sound state. Wrapper creation does not
invoke those predicates. The explicit compatibility catalog does
not establish native Legacy archive priority. Local stable-source commands and
receipts are in `.scratch/filter-root-20261017/`.

Synthetic owned-VM tests construct wrappers in reverse declaration order through
a `boolexpr` global, prove creation leaves callback side effects untouched, and
explicitly invoke both boolean results. Other cases cover caller/shared capacity,
reserved-kind refusal, malformed callbacks and source/type checks, including an
extra authored ancestry edge. Exceptional native inputs are not established by
these contract tests. Native and actual WASI run the same tests.

Predicate candidate context, `GetFilterUnit`, enumeration and trigger-condition
delivery remain open. The first later source consumer is
`GroupEnumUnitsInRangeOfLoc` in `IssueHauntOrderAtLocBJ`; its predicate also needs
`GetUnitTypeId`. Wrapper retention does not implement those operations or the
application gameplay Host factory.

The subsequent [explicit force slice](script-force-memberships.md) now passes
that constructor boundary. Predicate contexts remain unsupported.
