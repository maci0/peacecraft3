# Unit technology providers and production requirements

Production prerequisites consume the resolved `Requires` and `Requiresamount`
profile fields. Upgrade requirements use canonical applied research levels;
unit requirements use canonical owned, living units. The source `Upgrade`
relationship supplies upgraded providers for nonspecific counts. The required
type's `DependencyOr` list supplies alternatives for both specific and
nonspecific counts. These relationships come from UnitMetaData/profile values
and map overrides, not a table of stock buildings or named missions.

`GetPlayerTechCount` and `GetPlayerTechResearched` use the same unit provider
query when the admitted technology is a unit. Hidden and paused living units
still count. Ownership changes and death affect the next query immediately.
Pending training jobs are not living providers. All prerequisite data is checked
before a replacement job spends resources, reserves food or publishes a start
event; an unmet requirement waits for a later fill request.

The provider graph forms a bounded union, counting each canonical unit once.
Traversal refuses malformed, unresolved or mismatched relationships and capacity
failures. Cyclic/overlapping graph deduplication is a recreation choice, tested
synthetically; the original observations below qualify ordinary upgrade chains
and one alternative, not every possible custom graph.

## Original runtime observations

The selected executable SHA-256 is
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The runs use its original runtime under Wine with software rendering, not a
Windows hardware or REA Evidence capture. Source archives were hash-checked
against the selected installation. Maps, scripts, markers and executable bytes
remain local and are not part of the repository.

The first matched pair derives a three-type upgrade chain from the selected
edition's HumanUnitFunc profile. One arm preserves the source relationship;
the other adds one metadata-derived `DependencyOr` object override. Public
queries are recorded before and after creation, transfer, death and removal.
Upgraded descendants satisfy earlier types only for nonspecific counts. The
authored alternative satisfies the required type in both count modes; the
relationship is directional. Two providers count twice. A hidden unit killed
by `KillUnit` stops counting immediately, so the host death operation must not
reuse combat targeting's visible-unit filter.

The training pair changes only diagnostic identity and the actor's authored
requirement amount between one and two. It issues a real training order on a
fresh producer at each step and records acceptance, resource debit and food
reservation before removing that producer. Both arms reject with no debit when
the current owned provider count is below the authored amount; both accept and
reserve food when it is met. Upgraded providers satisfy this requirement too.
The comparison checks the observed matrix against the actual counter values,
with positive debit/reservation controls. It does not use an accepted boolean
alone as evidence of production. Local receipts:

- `.scratch/core-mechanics/unit-requirements-probe-r2/{stock,dependency}/result.json`
- `.scratch/core-mechanics/unit-requirements-probe-r4/{one,two}/result.json`
- `.scratch/core-mechanics/unit-requirements-probe-r4/comparison.json`

Every qualified arm has a terminal witness, bounded exact marker inventory,
source identities and a stopped, preserved runtime container. Map scripts parse
and resolve and every packed archive member round-trips. The unqualified r3
diagnostic is preserved: its collector rejected a truncated row. Shorter r4
diagnostic IDs fix the marker envelope without changing the experimental
requirements or accepting partial records.

## Removal and remaining boundaries

Original removal retains the counter and unit readbacks during the same
callback; by the next sampled timer they retire. The host reuses its existing
same-game-clock removed-unit snapshot for technology counts. This is a
recreation of deferred retirement, not proof of an identical native reclamation
phase. No sampled interval is embedded as a game rule. Same-callback removed
handle type/life/owner getters have separate remaining fidelity debt.

Synthetic regressions cover renamed metadata, changed relationships, explicit
quantity, owner/life/removal changes, malformed later requirements and refusal
before spending. These checks do not complete player production queues,
cancellation, construction, research jobs, hero requirements or item technology
categories. They also do not prove an unchanged campaign playthrough.

The integrated source passes 1,764 core tests, 711 application tests, strict
native/WASM Clippy, the desktop build and the existing Human02 opening-minute
regression. The latter still starts via the earlier explicit victory stimulus.
