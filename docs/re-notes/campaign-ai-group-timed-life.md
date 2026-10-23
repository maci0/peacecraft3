# GroupTimedLife and captain membership

The selected Legacy executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Unchanged Human02 currently refuses `GroupTimedLife` at 7.218750 seconds.
The policy reader is implemented; host dispatch and lifecycle behavior are not.

## Selected-code evidence

The native boolean setter changes an owned AI-context flag. Its consumer can
detach one unit association or attach that unit to the first captain reference.
The attachment and detachment paths update a signed counter. Public
`CaptainGroupSize` reads that same first-reference counter, while
`CaptainIsEmpty` compares it with zero. `CaptainPolicy` validates the query
registration/name/signature, first-reference and result-counter relationships;
this does not by itself define a roster of all units sharing an owner.

The public `UnitApplyTimedLife` factory has a fallback key derived from a
selected-image getter. The checked class constructor/vtable binds that stock
key to the initializer containing the association bridge. The selected
`AbilityBuffData.slk` members contain no matching alias or code row. This is a
native factory identity, not an invented editor-buff record.
`GroupTimedLifePolicy.timed_life_factory` now derives this identity from the
getter and validates its agreement with the class getter, constructor vtable,
fallback call and virtual initializer. Nonstock synthetic identities,
relationship mutants and every truncated window are checked alongside an
explicit actual-image decode. No constructor default is inferred for the flag.

## Original-client observation

A matched diagnostic pair created one player-9 town hall before starting AI,
then one ordinary player-9 footman after an acknowledged AI startup. The AI
called `InitAI`, `CreateCaptains`, and `GroupTimedLife(false/true)`. It queried
public captain size repeatedly, recording both count and a progressing sequence
in a bounded diagnostic bank encoding. The map applied public timed life with
the image-derived factory key and a diagnostic duration of six seconds. A
different-owner unordered control and the anchor remained unchanged.

Both arms passed the collector's complete bounded-observation checks. The
false arm reported count zero throughout. In the true arm, count was zero
through application at game time 6.063, one at samples 11–70 (6.188–12.063),
and zero from sample 71 (12.188) through the tail. Both actors retained life
420 through sample 70 and reached zero at sample 71. Their handles still
resolved afterward; this is evidence of life reaching zero, not unit removal.
Query sequences reached 120 in both arms, so unchanged readings were not a
stopped query loop.

The before-application actor observations match in both arms. Immediately
afterward, the false arm reports no order and the true arm reports a movement
order. Later positions also differ. The experiment did not set an explicit
random seed; these observations do not establish a complete movement rule or justify
fitting a route, stop distance or timing constant to the samples. The native
association path's additional effects remain a separate implementation boundary.

The false/true packed-map hashes are respectively
`85de2fa0fbb42dbdc69d1a5b1d6131886f49555cdb7e86d03bc572789a8b4a70`
and `545ac4df91650f9c10a3ed1ffadc25621cbad96188bd7e6ff60dbdedb51a5949`.
Their scripts differ only in diagnostic identities and the boolean flag.
Original member bytes, generated maps, raw markers and source hashes remain
local under `.scratch/group-timed-life-probe-r2/`. The compact runtime receipt
has SHA-256 `9ad8194d33b46e08f9f0d23552cbe8130daa019bd685401fcfe74a24c6599728`.
It supersedes the preserved first receipt, which incorrectly described the
order difference as preceding application.
Both original-client containers were stopped and preserved after collection.

## Remaining implementation

Use canonical player and unit identities for membership and existing death,
removal and timed-life machinery for lifecycle updates. Qualify attachment
events and movement behavior before claiming the full consumer. The observation
does not establish every timed-life effect, owner transition, repeated
application, captain recreation or roster operation. A passive flag setter or
an all-owner unit count would omit the demonstrated behavior.
