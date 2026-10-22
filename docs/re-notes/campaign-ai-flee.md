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
unsupported. The unseeded pair alone does not establish flag causality; the
seeded repeats below address reproducibility in this setup.

## Seeded repeats

Four fresh reference runs used the same setup with diagnostic `SetRandomSeed(1)`
as the first call in both map and AI `main`. Each flag was run twice in an
independent prepared directory. The only changes from the earlier maps were
these seed calls and diagnostic run identity. Map scripts match after label
normalization; AI scripts differ only by the requested flag and labels. Every
other packed member matches, and all four clients reported RoC.

Both true runs produced identical bytes for all 111 markers, including 94
samples. Both false runs produced identical bytes for all 110 markers, including
93 samples. Initial samples match across flags. The repeated results are:

| Measurement | Flee true, both runs | Flee false, both runs |
| --- | ---: | ---: |
| Terminal sample time | 9.438 | 9.313 |
| Terminal victim position | (-4428.578, -4688.027) | (-3863.524, -4688.868) |
| Terminal threat position | (-4290.801, -4688.020) | (-3748.344, -4695.082) |
| Terminal victim / threat life | 0 / 664 | 0 / 576 |

This establishes a reproducible movement difference for the policy in this fixed
seeded setup. A no-op implementation would not reproduce both outcomes. It does
not establish a general flee threshold, route, duration, all unit classes, or
which setup calls are prerequisites. Attack-event chronology and rendered
movement remain unqualified as in the earlier pair.

Seeded true map SHA-256:
`dadced501aa573e8b2ea0efcf6871ed438e339b1851942c6356e8ce734ae836e`;
seeded false map SHA-256:
`fae0200df2a1a911240a341b1358e28476f485ce890bf5dd6664ddb8f4f11a9c`.
All four owned clients are stopped and preserved. Source, staged-member checks,
launches, captures, raw markers and the full within-flag hash comparison remain
local under `.scratch/flee-seeded-runs/`. The collector rejects an unknown edition
and passed 24 offline checks before collection. No original assets are tracked.

## Selected-code consumer

The selected executable now provides a positive policy-consumer link. In
`0xba94a0`, a unit's indexed player resolves to its AI context. The non-hero
branch at `0xba9581` tests context `+0x2d0` bit `0x10`, the same bit set or
cleared by `SetUnitsFlee` at `0xab7665` / `0xab766e`. A zero bit returns false
from this decision function. True remains subject to further gates; its caller
can then reach the candidate action at `0xb67ce0`. Other caller paths can reach
that action after this decision returns false, so clearing the flag is not a
global prohibition on every retreat-like action. Earlier bounded searches that
reported no reader are superseded by this aligned instruction evidence.

The shipped `UnitData.canFlee` value also has a qualified read path. Its named
field descriptor passes through the integer accessor into cached unit-type
record `+0x58`; `0x67ec30` reads that record by type ID. The separate gate at
`0xb23550` obtains the type ID through a unit-attached behavior node. It also
checks node state and masked handle pairs, whose exact meanings remain open.
Its receiver is not the raw unit object.

The initial local loader trace was withdrawn: an incorrect PE-section conversion
mapped the string to `ToolTipDesc` and followed an unrelated font path. A section
table roundtrip identifies `canFlee` at VA `0xea0648`, with the descriptor setup
reference at `0x6b3eeb`. No gameplay rule follows from that discarded font trace.

The source-key audit, aligned windows and hashes remain local under
`.scratch/campaign-tactics-native/setunitsflee/`, including
`canflee-pe-address-audit.json`, `canflee-cached-record-consumer-review.md` and
`canflee-behavior-gate-review.md`. This establishes a consumer and a data input,
not the remaining decision thresholds, route or complete flee policy. Production
`SetUnitsFlee` behavior remains unsupported pending those consumers.

## Admitted policy operands

`campaign_flee_policy::FleePolicy::read` now binds the reviewed image and
decodes 26 bounded instruction/data windows. It derives the context flag and
default, current-speed minimum, ordinary life cutoff, six life bands, route
default distance and margin, near-zero distance, and route movement order.
The first four life bands remain separate from the final two because the native
decision conditionally skips the first group. The movement order is checked
against the admitted order registry and unit-activity movement identity; the
neighboring attack-order operand is not used for ordinary AI retreat.

Synthetic tests vary operands, check exact aggregate window budgets, and refuse
malformed relationships and sign-extended flag masks. The selected-image test
also passes. Private integration passes 1,526 core tests (45 ignored), strict
native/WASM Clippy, and formatting. These checks establish source decoding,
not a working flee consumer or rendered parity.

The remaining route-score relationship is explicit: the client caches authored
`rangeN1/2`, and a later `ratr` upgrade updates indexed node values, but the
ordinary cache-to-node initialization and alternate node value are not yet
qualified. The host's first-weapon range cannot silently replace that value.
A matched flag-clear experiment supplies the bounded observations below.

## Clearing before retreat initiation

Two further seeded runs both start with the flee flag true, sleep for one
second, and then write a second bank watermark. One retains the flag; the other
clears it immediately before that write. The map scripts differ only in
diagnostic filenames and the selected AI member. The AI programs additionally
differ by the clear call (and a control comment). The staged members, executable
and all retained marker hashes were checked.

Both runs first observe the second bank watermark at 1.125 seconds and have
identical samples through 1.219 seconds. At 1.313 seconds, the retaining control
reports the source-registered movement order while the clearing run reports
zero. Their positions still match at that sample. The control later retreats
west and reaches the same terminal position as the earlier seeded true runs;
the clearing run reaches the same terminal position as the earlier false runs.
Both clients report RoC and are stopped.

This timing clears the flag before the observed retreat order. It establishes
an effect on subsequent initiation, not cancellation or retention of a route
already in motion. Attack markers have no timestamps; no landed-hit chronology
is inferred. The camera remains over unexplored terrain, so this is sampled
world-state evidence rather than rendered movement comparison.

Retaining map SHA-256:
`75229ab49767f202dde9f0557480284f13ba69275f79d822143dad4e9be0ea84`;
clearing map SHA-256:
`62ac13d3a7bea76452567ab541d3d9c6262207e8a9133576a0513a31176a8fd2`.
The 111/110 raw markers, 94/93 samples, staged-member receipts, source comparison
and first-divergence summary remain local under `.scratch/flee-flag-clear-runs/`.
The actual-wrapper collector passes ten offline tests, including malformed bank
transitions, identities, reciprocal attack records and terminal-witness refusals.

## Clearing during the first retreat

A second matched pair changes only the diagnostic identity and the AI sleep
duration to two seconds. The retaining control reproduces the prior seeded
trajectory. Both runs first observe the second bank watermark at 2.125 seconds,
while both units still report the movement order and continue west. They reach
the same first retreat endpoint at 2.25 seconds and have identical samples
through 2.938 seconds. At 3.0 seconds the retaining control starts another
movement order; the clearing run remains in combat with order zero.

In this ordinary-unit setup, clearing the flag therefore permits the current
retreat to continue and prevents the later observed initiation. It does not
establish this rule for every unit class, guard state or explicit order. The
bank watermark brackets delivery rather than exposing an exact native-call
timestamp, and the camera still does not compare rendered movement.

Both runs preserve 111 markers and 94 samples. Retaining map SHA-256:
`237f9ea7d356d80365e860bcb1d99958c27c128ce212068c8eb51f649449df8a`;
clearing map SHA-256:
`9f11cf77bba029b2abdf53614971623f746d2c5c9a6a9ff2c7b134be73851e84`.
The eleven offline collector/build checks pass. All-member staged roundtrips,
raw marker hashes, captures and the source/first-divergence summary remain
local under `.scratch/flee-midroute-clear-runs/`. Both clients are stopped.
