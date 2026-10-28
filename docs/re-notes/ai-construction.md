# Campaign AI construction

`SetProduce` on a structure kind (`common.ai` `StartUnit`, line 1292) is how the campaign AI
builds. The recreation answers false when nothing could build or upgrade the kind and refuses
every other structure request loudly. This note records what the Legacy executable
(SHA-256 `3f2ed012...`) and original-client probes show, and what is not decoded.

## Executable evidence

Disassembled with `objdump` (virtual addresses of the Legacy image). The Kuna decompiler did not
build an architecture for this PE image here, so no decompiler output is used.

| Fact | Routine |
| --- | --- |
| `SetProduce(qty, id, town)` accepts 1 to 50, calls the AI player's method `0xac2880(id, 0, town)` once per unit and stops at the first failure, returning false; it returns true only when all succeed. The recreation's existing quantity loop matches. | wrapper `0xab7370` |
| With a zero second argument, `0xac2880` classifies the id (`0x5cc9f0`) and calls the structure method `0xac3070` or the unit method `0xac2e00`. | `0xac2880` |
| `0xac3070(id, town)` answers false, with nothing ordered, when: `0xac0be0` is nonzero (the shape of the check: some unit with an ability object in state 2 whose order target lacks flags `0x30000000`); a float at `+0x188` of the AI player is not zero; the player's tech check (`0x4c1020`, mode `0x12`) or the count limit (`0x4bd8d0`) fails. Otherwise it searches for a builder and a site with `0xac2b00`. | `0xac3070` |
| `0xac2b00` walks the player's units. A unit is skipped when flag `0x20` bit 0 is set, when its state (`+0x3a0`) is neither 0, 5, 6 (state 3 is accepted only for types other than `hpea` and `nhew`), or when its ability object (`+0x400`) does not accept the id (vtable slot `0x270`). Candidates also appear to need a town object (the town argument, or one reached through the worker's order target). For `ugol` (haunted mine) it uses two handles in the town object. The site comes from `0xad4970`; a negative result means no site. With a site the order is built by `0x698f60` (`0x69f760` for `ugol`) and issued to the worker with `0x690590`, and the method returns true. With no builder it returns 0. | `0xac2b00` |
| `0xad4970` initializes a search object, visits nearby objects through `0x68ea00` and `0xad4c10`, copies the object, and invokes `0xad41f0`. The caller supplies two coordinates from offsets `+0x3c` and `+0x44` of the selected town object; the refresh rules are described below. | `0xac2c9a`, `0xad4970` |
| When `0xac2b00` finds no builder and the request is not `ugol`, `0xac3070` falls back to an idle unit with the upgrade ability class (`0xb883e0`, counters `+0x3c` and `+0x40` zero, check slot `0x328` accepting) and issues the upgrade order (`0x698910`), returning true. | `0xac3070` |

Consequence: with no unit of the player that builds the kind and no idle unit that upgrades
into it, `SetProduce` returns false and orders nothing.

### Placement search, partial decode

These are native-code observations for the fingerprinted executable, not a recreation or a
claim that the unknown checks have been matched:

- The search constructor (`0xad3de0`) initializes two 101 by 101 byte grids. Their origin is
  each supplied town coordinate minus 1600, with a 32-unit step on each axis. The constants are
  initialized at `0x4161a0` and `0x40a910`; the reciprocal step is initialized at `0x416170`.
  The grid spans 3200 units on each axis. The caller's coordinates are not necessarily the
  eventual building position: each dequeued point also passes through `0x4e00f0` before its
  corresponding grid cell is inspected.
- The second grid starts with value 5 in every cell. A FIFO queue starts at grid index (50, 50).
  `0xad41f0` removes one point at a time and enqueues unvisited neighbors in this order:
  west, southwest, northwest, east, southeast, northeast, south, north, where increasing grid
  indices are called east and north here. Each enqueued neighbor is marked 6. This fixes the
  visitation order, but not the final placement by itself (`0xad42d1` through `0xad4867`).
- For a candidate whose first-grid byte is 5, the routine can return immediately after its
  checks pass. For first-grid bytes 0 through 3 it retains the first accepted position of each
  class; if no immediate candidate succeeds, it chooses the highest available class, from 3
  down to 0. Other byte values do not enter these candidate paths. In both paths, value 7 in
  the second grid excludes the inspected cell, and `0x67ecd0` must return zero. The shared check
  and the full meaning of the class bytes remain partly decoded (`0xad4424` through `0xad458a`,
  `0xad4879` through `0xad4964`).
- `0xad3de0` samples `0x7044b0` at the supplied center and each grid point. This is also the
  routine used by the registered `GetTerrainCliffLevel` native (`0x496710`). A different result
  assigns the cell -98. Otherwise it calls `0x7f6a00` with mask 2 and assigns -100 when the
  result is nonzero, or 5 when it is zero. `IsTerrainPathable` (`0x4998a0`) calls that same
  function after translating its pathing type; the selected image maps the walkability type to
  mask 2. The exact edge normalization and dynamic pathing inputs still need qualification.
  Nearby objects modify the first grid through `0xad4c10`, `0x4dfb40`, `0xad4b90` and
  `0xad4ac0`; their category and radius rules are described below. The spatial enumeration
  filter is not decoded.
- The first search enables an additional check at `0x7fd6b0`. If no site is found and that
  check has failed for at least one candidate, `0xad4970` searches the copy again with that
  additional check disabled (`0xad4a38` through `0xad4aa2`). The result is a failure sentinel
  if neither immediate nor retained candidates succeed. The check's meaning and later AI-script
  retry timing remain unknown.

#### Town coordinates

The registered AI natives `TownHasHall` and `TownHasMine` identify the references at town
offsets `+0x48`, `+0x54` and `+0x60` (`0xab7b90`, `0xab7be0`, `0xac47a0`, `0xac4800`).
The primary mine reference at `+0x54` is assigned to an object whose ability lookup accepts the
gold-mine implementation class `Agmb` (`0xab4550`, `0xac7000`, `0xac7070`; see
[unit resource amount](unit-resource-amount.md)). `+0x60` is the second mine reference; its
selection and replacement rules need further tracing.

`0xac80a0` refreshes the search coordinates from the primary mine and hall. If both references
resolve, it takes their positions and computes the midpoint with the executable's float helpers
and its half operand at `0x10db0a8`. If only one resolves, it copies that object's position. If
neither resolves, it leaves the previous coordinates in place. A newly allocated record is
initialized from supplied coordinates by `0xac7e90`; `CreateCaptains` can supply the selected
player start coordinates when it creates a record (`0xabd160` through `0xabd1e0`). These paths
do not prove which record the Human05 builder selects, or when its mine and hall are associated.

For `ugol`, the caller resolves one of the town's two mine references before searching; a successful order
passes that resolved handle to `0x69f760`, whereas an ordinary build order passes the selected
coordinates to `0x698f60` (`0xac2c3d` through `0xac2c89`, `0xac2d92` through `0xac2de1`).

#### Nearby-object score

The client's unit-data constructor binds `buffType` and `buffRadius` to the fields later read by
`0x67c350` and `0x67c310` (`0x6b3e7f`, `0x6b3e9a`, `0x67cf1e` through `0x67cf3e`).
The `buffType` decoder maps `townhall`, `resource`, `factory` and `buffer` to distinct values
(`0x6accf0`). For each object returned by the region enumeration, the callback `0xad4c10` maps
these types to score caps 0, 1, 2 and 3 respectively. Other types use cap 3 and a fallback radius
of 6 grid cells. The four named types use the object's integer-converted `buffRadius`, except
that buffer objects use radius 5 or 15 when the requested building is `hwtw` or `uzig`; which
one applies depends on whether the sum computed at `0xad49c4` is at least 2. The semantics of
both counts in that sum are not yet established.

The callback selects the object's pathing texture and facing, then `0x4dfb40` visits the texture
rectangle and invokes `0xad4b90` for each cell. That callback does not inspect the passed texture
byte. For each converted cell, `0xad4ac0` limits its score to at most -99 and limits the square
of the selected radius in cells around it to at most the category cap. It clips the writes to the
search grid. Thus these scores are an AI-specific preference and exclusion layer around texture
rectangles; they do not by themselves establish the general construction validity test.

#### Shared placement validator

Both AI candidate paths call `0x67ecd0` and accept only a zero result. The registered public
`IssueBuildOrderById` wrapper (`0x49a550`) calls the same routine before issuing its build order
through `0x698f60` (`0x49a5ab` through `0x49a5ea`). It passes a worker and different flags, and
continues for result 0 or `0x45`, subject to another check at `0x6a1790`. The AI call passes a
null worker and requires result 0. This establishes a shared validator, not equivalent inputs or
complete parity with the recreation's player preview.

`0x67ecd0` consults the building's pathing-texture object, transforms coordinates through
`0x4e00f0`, and uses masks and further callbacks. `0x4e00f0` depends on texture dimensions,
orientation and coordinate sign, including 64-unit alignment and parity adjustments. The
recreation's current `construction_position` explicitly uses a source-grid anchor as an
unqualified snap, and its construction host supports only the Human worker style and a subset of
placement tokens. Those limitations preclude treating it as a complete port of `0x67ecd0`.

## Recreation

`set_ai_produce` (`unit_training_host.rs`) for a structure kind:

- a live unit of the player lists the kind in `Builds`: refuse (`AI construction placement is
  not admitted`, naming the builder type and position);
- a live unit lists the kind in `Upgrade`: refuse (`AI upgrade is not admitted`);
- otherwise return false, charge nothing, order nothing.

"A unit builds the kind" is the `Builds` profile list standing for the ability check at slot
`0x270`, and "upgrades into" is the `Upgrade` list standing for the class check at `0xb883e0`;
neither mapping is decoded. The other early false answers (state-2 worker, float at `+0x188`, tech
and count checks) are not modelled; a request that reaches the refusal may have been false in the
original.

## Original-client probe (orange AI)

One isolated run (Legacy 1.29.2.9231, `-classic`, private Docker, no network, null audio, via
`tools/original-game-reference/reference.py`; scripts and receipts in `.scratch/depot-probe/`,
`prepare3.py`, `probe3.j`, `analyze3.py`, results in `result-orange/`). Human05's map was patched
to, three seconds after initialization, create the four acolytes of the shipped
`Trig_Corruption04` for player 5, set its resources as `Trig_ResourcesOrange` does and run
`Trig_OrangeTown` (which starts `h05_orange.ai`). The script then sampled player 5's structures and
acolytes once per game second for 190 s. Player 5 had no structure and no other builder at the
start. Map SHA-256 `7513e385...`.

| Game time | Observation |
| --- | --- |
| 8 to 9 s | The first acolyte reaches the rally rectangle `Acolyte_Move` and stops; one second later its order is the build order for `unpl`. |
| 12 s | Necropolis `unpl` at (6208, 2752), life 166 (under construction), built by the acolyte that was at (6064, 2800) at that sample. |
| 15 to 18 s | Two ziggurats `uzig` at (5664, 2912) and (5472, 2592) by two other acolytes. |
| 24 s | Haunted gold mine `ugol` at (5696, 3456), the map's gold mine position. |
| 31, 36, 40, 47 s | Altar of Darkness `uaod` (4832, 2784), graveyard `ugrv` (5184, 1984), crypts `usep` (5568, 1984) and (5376, 2944). |
| 79 to 108 s | Six more ziggurats, then a sacrificial pit `usap` (181 s) and a temple of the damned `utod` (188 s). |

All sites are multiples of 32 world units. Player 5's start location (`DefineStartLocation` 4) is
(7488, 2752), start location 0 is (6208, 2880); the Necropolis stands 128 from the latter on the
y axis, which is a coincidence until a second sample says otherwise. Up to two structures were under
construction at the same sample (15 to 18 s). The probe did not record the cost charge (the map keeps player 5's bank near 5000), the
refusal of a builder that is busy, or retries.

### Blocker experiment

Two more isolated runs use the same scenario, with a neutral `hhou` created before the acolytes
at the original Necropolis position. The second run explicitly records the blocker's actual
position as (6208, 2752). It sampled once per game second until 43 s; the first run used the
same setup without the position recording. Scripts, map, run receipts and samples remain under
`.scratch/ai-placement/`; the position-recording map SHA-256 is `04cd922a...`. Both runs place:

| Game time | Structure and position |
| --- | --- |
| 12 s | Necropolis `unpl` (5952, 2432) |
| 15, 17 s | Ziggurats `uzig` (5408, 2912), (5408, 2592) |
| 22 s | Haunted gold mine `ugol` (5696, 3456) |
| 31, 36, 40 s | Altar `uaod` (5152, 1952), graveyard `ugrv` (4800, 2752), crypt `usep` (4672, 2304) |

This differs from the unblocked probe, while the mine stays at the same coordinates. Creating a
unit can also affect handles or random state, so the comparison does not isolate a footprint or
prove a specific blocking rule. The recreation refuses before selecting a site, so no placement
comparison against it is available.

## Not established

- Town association and the search record selected by the Human05 worker; the initial grid's
  edge normalization and object enumeration; candidate snapping, blight and the shared validator's detailed
  checks; and the two-pass check's meaning. The decoded buffer layer alone cannot reproduce a site.
- Worker choice beyond "an idle acolyte in the rally area": the state filter of `0xac2b00` and
  the arena or list order among equals.
- Cost charging for AI construction (the human construction path charges when the structure is
  placed), refund on cancel, and retry cadence when a site or the builder is not ready.
- The early false answers listed above, and the upgrade fallback.

What would settle placement: decode the remaining calls and the town-object initialization,
then compare against several original-client layouts. Until then Human05's orange AI refuses
at its first structure, which is the 1223 s stop.
