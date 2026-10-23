# Campaign gathering context

The selected Legacy 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Its owned `common.ai` declares `StopGathering` with no arguments and no result.
`InitAI` calls it before campaign build and captain setup. The unchanged
Human01-to-Human02 opening-minute gate currently reaches this call and refuses
it at 7.218750 seconds; this is not completed campaign execution.

## Selected-code effects

The primary handler at `0xab78b0` resolves the current program's player context
and calls `0xac3f50`. That function clears a context flag. If the first occupied
association slot already exists, it returns. Otherwise it scans linked units,
skipping one unit-state flag. A separate classification flag selects a unit
immediately. Among remaining units it retains the last eligible structure as a
preferred candidate and the last nonmatching unit as fallback. It associates
one preferred candidate, or the fallback if no preferred candidate exists.
The flag and predicate operands must come from the selected image.

The structure predicate calls the same source-backed classifier used by
`UNIT_TYPE_STRUCTURE`. Its carrier in the recreation is the resolved,
metadata-qualified `UnitBalance/isbldg` field. The predicate also excludes a
particular combination of two dynamic native state bits; their gameplay
identities remain unqualified.

The immediate classification has an initialization-only source join. The unit
type-mask table at `0xe6eae0` maps the same mask to the source-declared
`UNIT_TYPE_PEON`, while the townhall mask is different. The cached unit-balance
flags pass through `0x688730` into an initialization descriptor and then into
the live unit field read by `StopGathering`. This joins the initialized peon
case to the resolved `UnitBalance/type` classification list. It does not prove
that no later native writer changes the live flags.

Association changes remove a keyed listener from the old unit, subscribe it to
the new unit, and refresh position-derived link state. The selected order
registry contains no order matching that callback key. These are not evidence
for issuing a worker stop order, cancelling every worker, or changing an
explicit map harvest order. A later unit-admission branch also reads the cleared
context flag, so a context-free successful no-op would lose state.

## Recreation boundary

The implementation under integration uses the existing canonical player and
unit arena, one association per program owner, and current unit positions
resolved on demand. It does not need a second unit store or the native cached
listener representation. Initial units can be classified from the same source
fields as public unit-type queries. Canonical creation order is a recreation
choice for enumeration; native linked-list membership and ordering remain
unqualified. Dynamic states outside the supported initial-unit envelope must
remain explicit rather than becoming invented eligibility values.

Constructor flag initialization is conditional on a context-kind argument.
The policy reader preserves that condition; the program-start mapping must not
guess a kind or turn it into an unconditional gathering default. A previously
uncalled gathering state can therefore remain unknown until `StopGathering`
establishes its cleared value. An association getter must resolve ownership,
life and position against current shared units instead of publishing a stale
foreign or removed unit.

The host, startup wiring and policy reader are not yet validated together.
Native callback delivery, later classification changes, AI production and
strategy, rendered behavior and chapter completion remain open. The retained
local evidence is under `.scratch/campaign-basics-native/`, including the
selection, callback and initialized type-mask lineage notes and their mapped
window hashes. No original assets are tracked.
