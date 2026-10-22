# Placed destructable admission and script creation

## Source and controlled runtime observations

Selected Human02 SHA-256 is
`d606be8dd85bfd04fb7701dffbb4ea31cc748466889ad2a94d621f6223dfca04`.
Its `war3map.doo` contains `LTcr` creation numbers 2866 and 2868 at the
positions used by `CreateAllDestructables`. Both records have flags 0 and
life 100. The selected destructable catalog classifies `LTcr` as debris.
Admitting every `.doo` destructable and then allocating the script calls
therefore creates duplicate recreation identities.

Two controlled RoC runs use the reviewed client SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
They omit authored destructable/unit/trigger initialization and retain four
isolated original `LTlt` records. Their life bytes stay 100. Each location
is enumerated before and after a matching `CreateDestructable`; the markers
also compare every enumerated handle with the returned script handle.

| Raw flag stimulus | Before script creation | After script creation |
| --- | ---: | ---: |
| 0 | 0 | 1 |
| 2 | 1 | 2 |
| 3 | 0 | 1 |
| 6 | 1 | 2 |

No preexisting handle equals the script-created handle. The second run
swaps flags 2 and 3 between the same two poses: the inside position changes
from one preexisting object to zero, while the map-edge position changes
from zero to one. That discriminates flag-driven admission from the
initial location/bounds hypothesis. Both runs report RoC and all six
expected bounded markers validate.

R1 map SHA-256 is
`a1f34a8b6ecffa1f4f6756f3030afd454b977071b02eeb6f3e92ea0711b7449b`;
R2 map SHA-256 is
`afcbb920961c5198b93a045cbfd54398bf22d08134dee7ed7a7d02027e413d8d`.
R2 script SHA-256 is
`07e25444acf2063398f9d2683447add116176d205e83023a81309dc237832e78`.
Exact source-record bytes, mutation offsets, packed-member hashes and raw
results remain local under `.scratch/placed-tree-flags-probe/` and its
`r2/` subdirectory in the main checkout. The owned runs are stopped.

## Recreation boundary

The runtime arena and collision cache must agree on which source records
are instantiated. Skipping an arena record but still stamping its static
footprint leaves phantom collision. A later script creation is a new
identity, rather than an alias selected only by matching position.

The observed mapping covers the flag set present in this selected map.
Other flag values, decorative doodad behavior, rendering/visibility effects,
native Windows, and late scripted creation after the collision cache is
built remain separate qualification work. This does not prove an arbitrary
bit-mask rule or a full harvest cycle.

## Script life writes and repeated death deliveries

A separate controlled run of the same reviewed client reports RoC and creates
one scripted `LTlt`, with no placed instances. The script saves its observed
maximum life, then separates each action and observation by a timer callback.
All ten bounded markers validate. The observed maximum is 50 throughout:

| Action | Observed life | Cumulative death callbacks |
| --- | ---: | ---: |
| Create | 50 | 0 |
| Set life to twice the saved maximum | 50 | 0 |
| Set life to half the saved maximum | 25 | 0 |
| Set life to negative saved maximum | 0 | 1 |
| Set life to saved maximum after death | 50 | 1 |
| Kill after the positive write | 0 | 2 |
| Explicitly restore life to saved maximum | 50 | 2 |
| Kill after explicit restoration | 0 | 3 |

The repeated callback after the positive write supports observable death
rearming; this is stronger than a positive getter alone. These observations
support clamped life writes and repeated death delivery for this tested tree.
They do not establish collision, animation, other destructable classes, or
native internal state. Values in this table are observations, not runtime
constants; the recreation obtains maximum life from the selected catalog.

Diagnostic map SHA-256 is
`663feaaa938bea40df5f6be7f06bb283b22e303b4f50d7c5fd0dfbb9b6d5594d`;
script SHA-256 is
`5909fb815a6a9a2803e4f18ada22bfac9f6afbab502d88762e2408711a9ca497`.
Raw markers, source receipts and `result.json` remain local under
`.scratch/destructable-life-setter-probe/` in the main checkout. The owned
reference run is stopped. Destructable death-event delivery in the recreation
still needs its own implementation and tests; the native receipt does not
qualify that host path.
