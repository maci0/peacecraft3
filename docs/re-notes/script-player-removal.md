# Executed player removal and campaign results

The selected Human01 defeat action reaches `CustomDefeatBJ` in the supplied
Blizzard script. After checking the cheat policy, that helper calls `RemovePlayer`
and then checks the same player's controller before building its defeat dialog.
Destroying the player handle, changing its controller to an invented value, or
clearing the session at removal would prevent the authored continuation.

`player_removal_rules::Rules::reviewed_legacy` qualifies the selected executable
against the reviewed legacy image digest and retains its origin and SHA-256.
The real native registration binds `RemovePlayer` to `0x4a51c0`, which resolves
the player and calls `0x4bfe20`; the alternate stub registration is not behavioral
evidence. The slot-state getter at `0x494994` reads the same field compared at
`0x4bfe40` and written at `0x4bfeab`. The reader checks that relationship and
reads the PLAYING/LEFT operands. A zero-result test skips the LEFT write for
victory. The result selector chain and the direct event path have separately
checked matching operands. A comparison against the initial result gates later
result assignment.

The direct defeat/victory routines (`0x4b9450`, `0x4bcc30`) carry event operands.
The register-player-event routine adds its namespace operand at `0x4add28`;
subtracting that decoded operand yields the selected script enumerands. No
result, slot-state or event number is copied into production constants. Admission
cross-checks the image-derived values against the exact selected common-source
constant/conversion declarations, their handle ancestry and declaration sources.
The checked paths establish defeat/victory events; this support does not invent a
leave event for `RemovePlayer`. Network departure is a separate unimplemented
producer.

`ObjectHost::with_player_removal` requires admitted player rows and player events.
Missing removal declarations remain unsupported. It stores at most the caller's
`max_players` result records; `max_event_deliveries` bounds cumulative removal
callbacks for the session. These are application resource policy. Queue and record
capacity are reserved before publishing a transition. Refused arguments, unknown
current slot state, mismatched sources and exhausted limits preserve prior slots,
results and queued callbacks.

An explicit PLAYING slot can receive its first result. Victory retains PLAYING;
the other admitted results set that player's effective current slot to the source
LEFT handle, including its retained numbered participation entry. The player
identity, controller, setup, units and owning VM remain available. Subsequent
removal requests retain the first result and do not repeat result events.
Non-PLAYING slots and a null player are no-ops in this bounded path. Matching
player-event registrations queue in retained arena order with the actual player
and typed event response; the existing event delivery owns response lifetime.
Queuing at the next script boundary is the existing recreation scheduling policy,
not proof of identical native callback nesting.

The public state exposes the player, executed game-result identity and whether it
left. Application gameplay suppression can consume `left` without ending the
session or disabling modal controls. Victory is distinct from departure so the
script's continue-game choice can remain meaningful.

Synthetic tests use changed slot/result/event operands and symbolic globals.
They cover state/identity/controller retention, sticky results, all admitted
outcomes, matching event context, ordinary leave-event exclusion, callback/record
budget atomicity, malformed arguments, absent admission and source/contract
refusals. An owned VM check executes a defeat continuation past removal. A
separate ignored gate requires both `PEACECRAFT3_PLAYER_REMOVAL_IMAGE` and
`PEACECRAFT3_PLAYER_REMOVAL_COMMON`, checking the real image operands against the
supplied common-source constants. Parser/VM gates do not prove an original-game
runtime comparison or rendered Human01 defeat UI.

The REA snapshot refused with an integrity mismatch; the bounded PE/Capstone
fallback and its source identity remain private under
`.scratch/player-removal-host/`. No original bytes are bundled or committed.
Native network broadcasting, departure cleanup, alliance/vision/AI handoff,
score screens, restart/load/end-game actions and complete campaign progression
remain outside this slice. This implementation preserves authored continuation;
it does not silently suppress defeat or claim those later actions are complete.
