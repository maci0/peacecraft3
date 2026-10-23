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
loads and supply no gameplay evidence. These observations alone did not qualify
production `SetUnitsFlee` behavior. The unseeded pair does not establish flag causality; the
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

The initial indexed route input is now source-qualified. The selected
`rangeN1/2` descriptors feed cached unit-type fields `+0x100/+0x104`, a transient
initialization record, and the attack component's indexed values
`+0x25c/+0x264`. An indexed virtual setter hid this write from the earlier
direct-offset scans. The previously proposed cache offset `+0x108` belongs to a
different field and is withdrawn.

Ten mapped image-window hashes and the slot/stride arithmetic pass an independent
local check. This qualifies the initial authored weapon ranges on that path.
Later modifiers, selection of the alternate node value, and the decision's
update timing remain separate questions; the consumer must not substitute a
first-weapon range outside the qualified selection envelope. The aligned source
chain and checks remain local in `flee-range-live-baseline-join.md` and
`attack-initializer.json` under the retained flee evidence directory. A matched
flag-clear experiment supplies the bounded observations below.

The route position getter is also joined to the public `GetUnitX/Y` handlers.
On successful extraction it writes the same third component for actor and
target, so that component cancels in their difference; it does not supply
terrain or flight height. The length helper uses the selected software
multiply/add operations and internal square root in its reviewed order. The
existing numeric implementation can be reused, while the public `SquareRoot`
native wrapper's additional near-zero checks must not be inserted into this
internal call. Independent checks pin seven position windows and five length
and arithmetic windows. Failed position extraction and the zero-direction
orientation branch remain outside this qualified nonzero route path.

The private implementation now exposes that nonzero ordinary destination
calculation without issuing an order. It requires the flee and numeric policies
to bind the same image and requires the admitted software math implementation.
Synthetic checks cover translation, both axes, source margin changes, invalid
inputs and the distinction from the public square-root wrapper. Private
integration passes 1,530 core tests, the explicit selected-image route test,
strict native/WASM Clippy and formatting. This component does not qualify a
combat trigger, path execution or complete flee behavior.

The controller's route-request callback consumes a record containing a unit
pointer, flags and a scalar. Its scalar comparison rejects equality with the
source zero value; the intervening `LAHF`/`TEST` parity calculation must be
included when reading that branch. The callback's producer and event identity
remain unqualified. Neither a periodic update rule nor a landed-damage trigger
is established by this static evidence. The current low-life observations
therefore do not justify initiating retreat on every combat tick. A paused
threat comparison will separate acquisition and outgoing attacks from incoming
damage without changing the actor's initial life.

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

## Paused-threat comparison

A further matched true/false pair uses an unpaused `ogru` actor and a paused
`hfoo` threat, retaining the seed, geometry and diagnostic initial-life fraction.
The threat remains paused with constant position and order throughout both
20-second observations. Each run preserves 200 samples and twelve outgoing
attack events, with no incoming attack event and no source movement order.
The complete sample and attack sequences match between the flags.

The actor's sampled life rises from 102 to 106 in both runs. Consequently both
collectors reject the experiment's original constant-life premise; this result
must not be relabeled as a passed constant-life test. It neither identifies the
native callback nor proves that the flag is inert. A matched control changing
only the threat's unpause call is needed before attributing the difference from
the earlier two-active-unit experiments to incoming damage.

True map SHA-256:
`7782c6d875217ccf21ee0984817bf2c38e844ea8aae3d3fe3cb7a9cbed3760e8`;
false map SHA-256:
`6ee2187fd939ccdf954195243dc85c767e6c55033e2749aa8e7c3d198b670d3b`.
The corrected collector passes 52 offline tests, including literal JASS rawcode
byte order, unexpected marker identities, and approach movement that must not
be called retreat. Frozen source comparisons, staged-member roundtrips, raw
marker hashes and results remain local under `.scratch/flee-paused-threat-runs/`.

## Matched unpaused-threat control

The next true/false pair adds only the threat's unpause call after the actor's
unpause, plus diagnostic identities. Both preserve the initial paused identity
marker and the original object edits. Current map/AI parse and resolve checks
pass, all nineteen members round-trip with the independent archive tool, and
53 offline collector/source-diff tests pass.

Both runs first sample a life reduction from 102 to 91 at 1.5 seconds. At that
same sample the true arm reports the registered movement order; the false arm
reports zero and never reports that movement order during the run. The true
arm's first observed leg ends at 2.313 seconds, with separation from the threat
increasing from about 115.35 to 220.61 map units. Both directions of attack are
observed. The true arm records 148 samples through death at 14.813 seconds; the
false arm records 137 through death at 13.75 seconds. Both satisfy this
unpaused control's premise, which permits changing life and positions.

Together with the paused pair, this supports evaluating an ordinary retreat
after positive incoming damage as a recreation choice for the tested setup.
It does not prove the native callback's identity or its only possible trigger:
unpausing also permits threat movement and attacks, sample clocks bracket
state changes, and attack markers have no timestamps. Route endpoints remain
sampled world state, not a rendered path comparison. No incoming-damage hook
is implemented by the pure destination component.

True map SHA-256:
`1916b9700b85b9bb8bae7783fb135c4e142ddbd3f0d4d0255fcfbb9a950a385a`;
false map SHA-256:
`ea6b22e58738808a36e5a26e1169d1557b3c1d2446ce0142b98955f0ccf8baaa`.
All staged-member identities, source comparisons, loader captures, raw markers
and summaries remain local under `.scratch/flee-unpaused-threat-runs/`. Both
clients are stopped.

## Shared state and startup admission

Campaign preparation now decodes the flee policy from the selected image and
admits it into the published map host. `SetUnitsFlee` validates its owned
AI-common boolean-to-nothing declaration and updates the same canonical player
registry as targeting. Two programs in a published map session prove isolated
flags and changes after sleep. Failed startup rolls back newly created state;
a failed wake retires the discarded program's flags without removing its peer.
The latter is failure cleanup, not a native error-behavior claim.

Private verification passes 1,543 core tests and 610 application tests, strict
native/WASM Clippy, formatting and diff checks. Explicit selected Human01 startup
and Continue tests each run and pass; startup also checks that the admitted flee
policy binds the selected recipe image. Receipts remain local under
`.scratch/campaign-flee-integration/`. These gates establish dispatch, shared
state and production input wiring. Incoming-damage retreat execution remains a
separate integration gate.

The unchanged selected Human01-to-Human02 opening-minute check now passes
`SetUnitsFlee` and `StopGathering` and refuses at `CreateCaptains`, still at
7.218750 seconds in Human02's `StartCampaignAI` call. This is the next exact
script frontier, not a completed opening minute or a flee-movement result.
The earlier failure remains in `state-frontier.log`; the gathering advancement
is in `.scratch/campaign-flee-integration/gathering-frontier.log`.

## Ordinary damage consumer

The private consumer evaluates positive incoming melee hits and missile impacts
as a bounded recreation trigger. Generic and spell damage do not start this
path. It checks the admitted player flag, source `canFlee`, current movement
speed and post-damage life. The ordinary cutoff is strict; native life-band
bounds include both endpoints. Conditional score bands and earlier range or
ability branches outside the qualified low-life leaf refuse with unit context.
They do not silently become a supported no-retreat result.

The supported incoming weapon is an initial single ordinary weapon without
effect history or active listed research. Its admitted authored range supplies
the initial indexed route operand; later range modifiers, alternate selection,
hero attackers and effect history refuse. The destination uses the selected
software arithmetic and internal square root. Unqualified zero-direction
orientation also refuses. Native trigger identity and full scorer behavior
remain open.

Route preparation precedes damage mutation; only surviving hits commit motion.
Melee refusals preserve the primary random state and pending swing. Failed
missile processing retains unprocessed impacts, retaining the current impact
only when damage was not applied. Existing activity routing owns travel and
arrival. Explicit orders, removal, death and a replacing guard return release
retreat ownership; a guard wait preserves it. Clearing the flag does not cancel
an accepted route, matching the bounded original-client clear experiment.

Real melee-launch and missile-launch regressions exercise incoming hits and
single damage traces. Separate direct-damage and queued-missile tests cover
life boundaries, route refusal and pending-impact recovery. Combined startup,
application and native/WASM verification of this consumer is still pending;
rendered path parity has not been checked.
