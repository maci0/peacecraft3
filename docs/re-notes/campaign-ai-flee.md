# Campaign unit-flee observations

Two diagnostic maps ran on the selected Legacy 1.29.2.9231 executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
with `-classic`; both map and AI markers reported RoC. Each program called
`SetTargetHeroes(false)`, `SetUnitsFlee(flag)`, `InitAI()` and `CreateCaptains()`
in that order, wrote a bank watermark, then slept. These setup calls were shared
between the runs; the experiment does not establish which are prerequisites.

Each map created one player-zero `ogru` and one hostile player-three `ogru`,
paused both, registered attacked-event observers for both owners, and set the
player-zero unit to the diagnostic fraction 0.15 of its source-derived maximum
life. After synchronous AI loading, both units were unpaused without issuing
movement or attack orders. The threat began east of the low-life unit. Samples
recorded positions, current orders and life at bounded timer intervals; the
observer recorded attacker/target identities separately.

| Measurement | Flee true | Flee false |
| --- | ---: | ---: |
| First sampled life / maximum | 102 / 680 | 102 / 680 |
| First sampled victim position | (-4000, -4700) | (-4000, -4700) |
| Terminal sample time | 9.438 | 9.313 |
| Terminal victim position | (-4428.856, -4685.892) | (-3863.524, -4688.868) |
| Terminal victim life | 0 | 0 |
| Terminal threat life | 663 | 571 |
| Victim / threat attack events | 6 / 6 | 6 / 6 |
| Samples / total marker files | 94 / 111 | 93 / 110 |

The paired sources differ only by the flee flag and diagnostic labels. All
other packed members match. Initial samples match exactly, both banks begin at
zero, and the AI-owned player-zero bank reaches 17 before the after-load marker;
the other bank stays zero. The load marker retains its empty `GetAiPlayer` text
without inventing a numeric value. Positions show a westward terminal displacement
with the flag enabled and an eastward terminal displacement with it disabled.
The disabled run also leaves less life on the threat. Both low-life units die.

This is an observed movement difference, not a recovered flee algorithm or
threshold. No explicit combat RNG seed was set, so damage totals and death times
are single-run measurements. Attack events lack timestamps and do not establish
landed hits; the collector reports the threat-attack and positive-life observations
separately and leaves survival-after-a-swing unqualified. Current-order samples
all read zero and are not evidence that internal movement was absent. The camera
remained over unexplored terrain, so rendered movement was not compared.

True map SHA-256:
`ffe8c23c82426f3dca0a8dcc97b97edf8d15d62a133504ac7c3c45ffe6775315`;
false map SHA-256:
`704a9b6095f766359ee1327a61bb35e20be11f0baf113d2fb54f175764440116`.
All 19 members were extracted from each staged map and compared with its frozen
build. The collector checks actual Preload wrappers, phases, file/payload bounds,
edition, flag and bank witnesses, reciprocal event identities and raw hashes.
Both owned clients are stopped; their sources, captures, raw markers and audit
receipts remain local under `.scratch/flee-original-runs/r6/`. Earlier drafts
with invalid version helpers or duplicate observer globals are preserved as failed
loads and supply no gameplay evidence. Production `SetUnitsFlee` behavior remains
unsupported; these observations rule out treating it as a harmless no-op here.
