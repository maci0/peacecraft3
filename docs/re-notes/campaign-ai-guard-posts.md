# Guard-post replacement behavior

`FillGuardPosts()` is the next unsupported native reached by the unchanged
Human02 opening-minute test after `AddDefenders`, at 8.312500 seconds. It must
use the same canonical units, owners, resources and production lifecycle as the
rest of the game. A successful empty implementation would omit its replacement
behavior.

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

## Implementation work remaining

The current host still refuses `FillGuardPosts`. The qualified next change is
shared guard-post state plus real pending unit production, not a second unit
world or instant respawn. The existing canonical guard home, unit catalog,
source-derived quantity queries, resource transaction preparation and unit
factory can supply the relevant state. Pending food, resource changes, completion,
guard binding, callbacks and failure atomicity need connected tests. The existing
live-guard list alone loses the vacant post on death and cannot supply that
replacement behavior.

Structure construction, hero replacement/revival, training event delivery,
multiple producers, cancellation and exact spawn/arrival behavior are not
qualified by this ordinary one-producer experiment. The game goal and the
unchanged campaign acceptance test remain incomplete.
