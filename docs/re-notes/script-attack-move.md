# Attack-move and player attacked-event delivery

The unchanged local Human01 script supplies a concrete missing progression path:
`Camp_is_entered` registers `EVENT_PLAYER_UNIT_ATTACKED` for the bandit player,
checks the attacked unit's group and the attacker's owner, then sends the bandit
group an `attack` point order toward the attacker's location. Previously combat
only delivered individual-unit attacked registrations, and point orders only
accepted Move. Neither side could execute this authored response.

Combat now also queues matching player-unit attacked registrations at the same
swing boundary as individual-unit attacked events. It uses the target's current
owner and the enumerand resolved from the selected common source, retains each
registration's filter and event identity, and delivers the attacked unit and
attacker through the existing event context. Disabled and destroyed triggers are
still checked at delivery. Registration traversal order and deferred delivery are
existing recreation policies, not newly established native behavior.

Point attack orders use the attack identifier already admitted from the selected
client's order registry. All string/integer and coordinate/location forms reuse
the existing activity validation, copied locations, collision routing, movement,
source weapon acquisition range and damage consumer. The accepted route endpoint
survives target acquisition; when the target disappears or dies, travel resumes.
Reaching that endpoint without another target finishes the order. Unarmed units
finish travel without acquisition. Another accepted order cancels the endpoint;
dead units refuse movement orders. Invalid unit or location arguments identify
which argument failed and preserve the prior activity and combat state.

This supplies a faithful-recreation implementation path rather than proof of
native steering, target eligibility/priority, interrupt/resume timing or exact
attack-event nesting. General spell execution, hero experience, AI guarding and
chapter completion remain separate work. Synthetic tests cover matching owners,
filters and response context, pending disabled/destroyed listeners, attack-move
interruption/resumption/completion/cancellation, every point-order form, copied
location lifetime, unarmed travel and invalid/dead input. These tests require no
original assets and do not establish rendered or full-mission parity.


## Removed-unit order refusal

A frozen local source-backed Human01 replay with the cinematic skipped at 48
seconds reaches `IssuePointOrderLoc` at game time 52.057 seconds with a unit
argument whose canonical record is no longer live. `RemoveUnit` retains an
`Entry::RemovedUnit` tombstone in the same arena, still identified as family
`unit`, after removing occupancy, motion, group membership, range state and
attached effects. An order against that tombstone previously failed the live
record lookup as though it were a different handle family.

Supported immediate and point activity orders now return false for a tombstone
without reviving it or inspecting a replacement unit. This is an explicit
recreation lifetime policy, not a newly measured original-runtime return value.
Null orders already refuse the same way. Real wrong-family and unknown handles
still report errors; getters and unsupported order kinds retain their existing
behavior. Synthetic controls exercise all six supported immediate/point order
forms, wrong-family and unknown handles, unchanged surviving records and
allocation counts, and a later allocation that cannot revive the removed unit.
The retained local replay is
`.scratch/campaign-integration-20261020-v7/runtime-skip48/`; no original or
generated asset contents are part of this change.

After the fix, the stable-source v8 production-factory replay reaches 300 simulated
seconds (19,200 scheduler ticks) with no runtime error after either a 20-second or
48-second skip. The selected compatibility stock catalog is explicitly qualified;
this does not establish target stock-catalog parity or chapter completion. The
separate diagnostic main still refuses an unadmitted day/night model and is not
counted as a gameplay pass. Final build/replay records stay local under
`.scratch/campaign-integration-20261020-v8/` and
`.scratch/campaign-time-of-day-root-20261020/integrated-v8-skip48-20261020/`.
