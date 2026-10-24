# Captain combat queries

`CaptainInCombat(boolean attack_captain)` reads the selected captain's current
target state. It does not mean that any unit belonging to the player is
fighting. The true argument selects the first captain; false selects the second.
The existing canonical captain registry retains those roles separately.

## Selected-code evidence

The reviewed Legacy 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The primary registration at `0xab65e4` binds the NUL-terminated name,
`(B)B` signature and handler `0xab3e40`. The handler resolves the owned AI
context and forwards its caller's boolean to `0xabbd90`.

That helper chooses the same first/second references used by captain creation
and home assignment. It resolves the selected controller, then returns true
when either of its two target references resolves. It does not read the
captain member counter or directly scan every owned unit. The creation path
initializes both target-reference pairs to the invalid-reference sentinel.
The policy reader checks the registration, handler, selector relationship,
short-circuit boolean result and initial target pairs against the existing
source-derived captain slots. Synthetic target offsets vary coherently with
their initialization operands; broken relationships and truncated windows refuse.

Retained REA byte observations:

- Registration: `ev_71969b4a4d5be37d63e247c71dbc7703193c8b5ff48a331dd6ca308a46e61640`.
- Handler: `ev_1ea614f89249f576415dc74e8858f1dd53545412561a773167c3e88df4f46851`.
- Query: `ev_2333bdeb633ecc48377cbc68512235142b386b7aad5af101a93bd8f6bafb405f`.
- Initial targets: `ev_80c4be705d93f813675c81128bbc882390a3e824c952397019847330fef6d750`.

The installed REA Kuna route refused this PE target: that provider admits ELF
and thin Mach-O artifacts, not this Windows executable. The Ghidra route's
checked memory reads and bounded disassembly supplied the static evidence.
No provider configuration changed; the owned analysis session is closed.

## Matched original-client observation

Two local Human02-derived diagnostics ran with the selected executable under
the isolated Wine/Xvfb/software-Mesa reference. Both reported RoC. They used
the same seed, source members, unit setup and timing; only `GroupTimedLife`
and diagnostic identities differed. Script resolution and every packed member
roundtrip passed, and the normalized pair comparison checked those differences.

Both programs created captains and continuously queried group size and both
combat selectors. The map created one ordinary footman, applied the checked
native-factory timed-life key, then moved a separate-owner enemy beside it,
paused the enemy and issued an explicit attack. The enemy was later killed.
The native factory key remains distinct from an editor buff-table row.
Gold acknowledged AI startup; lumber packed a bounded query sequence, count
and both booleans. All 100 samples and the tail qualified in both runs.

| Arm | First-captain count during effect | Attack query while fighting | Defensive query |
| --- | --- | --- | --- |
| Grouping enabled | 1 | became true, then false after the enemy died | false |
| Grouping disabled | 0 | false throughout | false throughout |

The enemy lost life in both arms, independently qualifying actual fighting.
Thus the ungrouped negative is not merely an unaccepted or idle attack.
In the grouped arm the order was accepted at 7.063 seconds, the attack query
first reported true at 7.688, and damage was first observed at 7.875. Killing
the enemy at 9.063 was followed by a false query at 9.188. Those are sampled
observations, not production delay constants or exact callback-order proof.

Map SHA-256 identities:

- Grouped: `21a6de6ac12d860d5a0ffe9499b68eac94d9bd5d712b7fb9cad85605d032096b`.
- Ungrouped: `84a270bcbce90c534deb7f443ac0e3c610928178590428158c2edc575f290a79`.

The clients are stopped and the raw files were revalidated after stopping.
Source/build/launch receipts, collector checks and observations remain ignored
under `.scratch/captain-in-combat-probe/` and
`.scratch/captain-in-combat-defense-probe/`; the checked pair summary is under
`.scratch/captain-in-combat/`. These observations qualify this ordinary-unit
case, not every controller, roster, effect or native Windows rendering path.

## Recreation and remaining limits

The existing registry now has two canonical member lists and two optional
target handles. Grouped timed life still attaches to the first captain;
ordinary ungrouped fighters do not acquire captain membership. Combat steps
publish targets from registered members, and the query reads that published
state without allocating units, handles or orders. It does not flip merely
because a script submitted an attack order. Removed targets invalidate their
references; combat updates retire dead or lost targets. Membership and target
state remain separate from the no-argument first-captain count/empty queries.

Native priority selection and controller cadence are recreation limits: the
host publishes at its existing combat step rather than copying the native
callback system. The measured delay is not fitted into a timer. Defensive
worker assignment and other captain command producers remain unimplemented;
their absence is not replaced by counting every guard or same-owner unit.
An unqualified non-unit target class refuses explicitly.

Tests exercise accepted target orders and real combat stepping, ungrouped
negative results, independent selector/owner state, target death/removal,
uncreated contexts, retirement, malformed source ABI/arguments and calls after
sleep. The published map-session fixture checks both empty captain queries
before and after sleeping. Full campaign strategy and exact controller timing
remain unfinished.

## Integration checks

The frozen source passes 1,711 core tests, 55 app-library tests and 638
app-binary tests, the published two-program session, the actual selected-image
reader, Human01 startup, victory Continue into Human02, and strict native/WASM
Clippy. The unchanged Human02 opening-minute test now passes this query and
refuses `HarvestGold` at 8.312500 seconds. It remains a failing campaign
acceptance test; no native call or authored map was replaced to bypass it.
The earlier published-fixture failure is retained locally: its query ran before
captain creation. The corrected fixture checks both selectors after creation
and after sleeping.
