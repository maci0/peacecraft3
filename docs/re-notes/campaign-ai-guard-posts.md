# Guard-post replacement behavior

`FillGuardPosts()` uses the same canonical units, owners, resources and production
lifecycle as the rest of the game. The ordinary replacement consumer is now
implemented. The unchanged Human02 opening-minute test advances past this call
to `ReturnGuardPosts` at 8.312500 seconds and remains incomplete there.

## Selected-code evidence

The selected Legacy executable is SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Registration `0xab6420` binds `FillGuardPosts\0`, `()V\0` and `0xab4900`.
The handler resolves the owned AI context and dispatches to `0xabdda0` on both
reference paths. The generic stub is separate and is not edition evidence.

The consumer walks context `+0x1c8/+0x1cc` records. For each record with no
resolvable assigned unit, it compares record `+0x44` to the context's signed
replacement limit at `+0x1d4`. An eligible record requests its `+0x34` unit type
through `0xac2e00`, or `0xac3070` for the source-classified structure branch.
The unit path checks producer abilities and queue/eligibility callbacks before
submitting a request. The structure path has separate builder/placement logic.
Neither path justifies immediately creating an arbitrary same-owner unit.

Ordinary initial AI enrollment reaches `0xabf0c0`, which filters source state,
structure, hero and worker cases before calling `0xabae60`. That helper samples
the unit's position through the existing public-coordinate path and passes its
type and position to `0xababa0`, then binds the unit with `0xad3a50`.
`0xababa0` can reuse a vacant record with the same type and coordinates; a new
record starts its `+0x44` counter at zero. Binding increments that counter by
one. These are native storage observations, not a requirement to reproduce a
linked list or native wrapper class.

`RemoveGuardPosition` reaches `0xac1500`. It stops the unit, clears its matching
post reference and writes `99999` into that record's counter. This is a source
operand, not a production constant to copy. Ordinary release must not be modeled
as a dead unit that immediately qualifies for another replacement under a small
limit. The signed comparison also means this value is not a proof of permanent
ineligibility for every possible script-authored replacement limit.

REA observations retained locally:

- Handler: `ev_7e0c6f19f6d9a454fc3da87d8537469ac69a117057aa644b0fa052cec6e24889`.
- Fill consumer: `ev_d88e3ac279ce00e0b2c86406e8083ebf92717c4e6487705c934289c66e79b84b`.
- Initial enrollment filter: `ev_706afb18b7a76a282bdbe7c9bf26921883c4c55bf20536822623da112112f602`.
- Position/type factory: `ev_35154284ac0997fcca4e6bc7875f41277b666957ef77ea236ffdaaffb906a5a3`.
- Record insertion: `ev_3b302db580ca1f2a2512fb081e680d4b615f7beb67aa57527d8596036307f91b`.
- Binding/count update: `ev_1d05263ec702aa5e602f00f2a66c3198d10197b18e7fda7ed58807b1457d2c74`.
- Release: `ev_e3fa8689451457b2f29f4c6cc007dad12b9baad4bd4f4f375516d4f33d52d5b5`.

The owned REA session is closed. Several allocation-heavy decompilations have
explicit unreachable-block/null-wrapper limitations; their pseudocode does not
prove the full allocation or callback lifecycle. Source byte/assembly findings
and original-client observations must remain distinguished.

## Matched runtime experiment

The local diagnostic uses the authored Human02 grunt, barracks and fortress
creation statements, with diagnostic owner 9, an explicit resource allowance and
food cap. It creates those units before starting one AI program. The AI calls
`InitAI`, `CreateCaptains`, `SetCampaignAI` and `SetReplacementCount(1)`.
The map records a gold acknowledgment and continuing query sequence in a
different player's bank, kills the original grunt at sample 20, and collects
240 half-second samples. The three arms differ only in the fill call, the
release call, and diagnostic identities. No normal map AI or inherited unit
creation runs alongside the probe.

All three runs are stopped and collected, and reported RoC. With fill enabled, the
original unit dies at 15.063 seconds. At 15.563, gold is lower by 235 and food
is reserved while the live grunt count remains zero. At 45.563, a live grunt
is present again. The original handle remains dead. `GetUnitCurrentOrder` on
the producer remains zero, so the replacement queue must not be represented
as an invented reported order ID. The no-fill control spends no gold and has
no live grunt after the death through the terminal observation.

These values are observations for this selected row and setup. Production
costs, food and duration must come from the admitted object data, with the same
edition and map overrides; sampled timings must not become game constants.
The released-guard arm also spends no gold and produces no replacement through
the terminal observation. Its fill loop continues to make progress, so this is
not a stalled or unstarted AI control.

| Arm | Gold spent after death | Live replacement at terminal |
| --- | --- | --- |
| Fill enabled | 235 | 1 |
| No-fill control | 0 | 0 |
| Guard released, fill enabled | 0 | 0 |

Stopped result SHA-256 identities are
`9f8d81e5c4f88b1bbd4426030c1aa869c51f499944701f5227878d4eafdceac0`
(fill), `dfb6330e19607ff533094cae6197938e124f7959199acdbb8264a6e2ad994072`
(control), and `1c5875b7498d3ef170d955ea083b1699eb949f57a2ee964c548550779f6572ea`
(released). Maps, scripts, raw markers, collectors, source/build/launch receipts
and matched results stay local and ignored under `.scratch/fill-guard-posts/`.
All map/AI sources resolved and every packed member round-tripped. Normalization
checks the allowed arm differences; comparison reparses the retained raw marker
bytes and verifies their recorded hashes before reporting outcomes.

The collector validates stopped-container identity, frozen source/map hashes,
exact bounded marker inventory, wrappers, numeric fields, acknowledged startup,
query progress, producer identity, the explicit death and terminal equality.
Its synthetic tests accept both replacement and no-replacement outcomes and
reject malformed, stale and truncated data. An initial collector assumption
expected the diagnostic `-1` lumber write to read back negative; the selected
client instead reports zero. The corrected check preserves the maps and raw
markers and uses the independent gold acknowledgment for startup qualification.

## Pending counts and training callbacks

A fourth stopped run repeats the fill arm with map-side training event observers
and numeric AI query results transferred through diagnostic player resources.
Its map SHA-256 is
`2f3d229ed4adfcdbd05b1a7dfb78cd357454c02ca80405b608d99c940df5e2a4`;
the collected result is
`392b103caf24f8fa9c14e4435896e9482adf0647f0fe32dd71d6e006c20f728c`.
There are 240 query samples and two training callbacks. At the start callback,
15.500 seconds, the producer is the barracks, `GetTrainedUnit()` is null,
`GetTrainedUnitType()` is the grunt type, gold is already debited, food is
reserved, and the live grunt roster is empty. At the finish callback, 45.563,
`GetTrainedUnit()` identifies a grunt distinct from the dead original, the
declared type still reports grunt, and the live roster contains one.

During pending work, `GetUnitCount` and `GetPlayerUnitTypeCount` both report one,
while `GetUnitCountDone` reports zero. After completion all three report one.
These AI observations are sampled through the resource handoff, so the death
sample can still contain the prior AI query and the done-query transition can
lag the finish callback by one sample. This does not establish exact scheduler
ordering between the two VMs. No cancellation was requested or observed.

The initial observer draft attempted preload text generation inside the AI VM.
It produced malformed query filenames and the original client's error dialog
after sample 20. That run is stopped, preserved, and explicitly unqualified;
its partial callback is not the basis of the contract above. The successful
revision keeps preload output and string formatting in the map VM. Its map/AI
sources resolve, every packed member round-trips, and the four collector tests
cover numeric, wrapper, identity, progress and truncated-data refusal. All
artifacts remain ignored under `.scratch/fill-guard-posts/`.

## Runtime implementation and remaining work

The existing per-player AI registry now retains ordinary guard posts after death.
Fill checks the source-derived signed assignment limit and submits one pending
job to an eligible idle producer from its authored `Trains` profile. Source
costs, build duration and food usage drive shared player resources and pending
food. Repeated fill calls do not duplicate the job. Training start and finish
callbacks observe the same canonical producer and newly created unit; pending
count queries include the job while completed queries exclude it.

Completion uses the normal unit factory and prepared placement/activity path,
binds a vacant matching post and preserves its home and attack-point behavior.
A replacement that cannot start moving can still complete production; route-work
or capacity errors retain pending work before unit/event publication. Resource
shortage or an authored unit cap does not spend resources. Paused production
preserves pending food and extends its deadline without timer-partition drift.
Unsupported prerequisites and producer loss/transfer refuse explicitly; no
cancellation refund is invented. These choices reuse the game's canonical state
without reproducing native linked lists, record layout or queue wrappers.

`CampaignModePolicy` now derives the initial and release counters from selected
instructions, checks the binding increment and signed-limit branch, and binds
the `FillGuardPosts` registration/forwarding to those fields. Nonstock synthetic
operands and malformed/truncated relationships are tested; the actual selected
image decodes. The full library suite passes 1,748 tests; app library/binary
suites, published-session integration, selected startup and Continue, strict
native/WASM Clippy, formatting and the default game build pass. Synthetic tests
exercise nonstock costs/duration/counters, callbacks, pending counts, release,
food and authored unit limits, malformed profiles, route refusal/retry,
no-motion completion and partitioned clocks.

The recreation currently completes work at the catalog deadline. In the stopped
callback run the start-to-finish observation is 30.063 seconds versus the authored
30-second duration. Exact native polling/subtick cadence remains unqualified;
the measured offset is not copied into a constant.

Structure construction, hero replacement/revival, other training event paths,
multiple producers, cancellation and exact spawn/arrival behavior are not
qualified by this ordinary one-producer experiment. The game goal and the
unchanged campaign acceptance test remain incomplete.
