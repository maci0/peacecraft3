# Campaign AI construction

`SetProduce` on a structure kind (`common.ai` `StartUnit`, line 1292) is how the campaign AI
builds. The recreation answers false when nothing could build or upgrade the kind and refuses
every other structure request loudly. This note records what the Legacy executable
(SHA-256 `3f2ed012...`) and original-client probes show, and what is not decoded.

## Executable evidence

Disassembled with `objdump` and cross-checked with the Rizin Ghidra plugin. Addresses refer to
the selected Legacy image; local decompiler output stays under `.scratch/`.

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

- The driver `0xad4970` builds and scores a search object, invokes `0x68ea00` with the
  `0xad4c10` callback, copies that object using `0xad3c00`, then searches. `0x68ea00` enumerates
  objects and invokes the callback after its filters; it is not a sort or just cleanup
  registration (`0xad4a07` through `0xad4a60`, `0x68f599` through `0x68f61d`). `0xad3c00`
  copies both row-based byte grids, retained candidates, ring-buffer indices and coordinate
  arrays; `0xad4db0` copies each grid row. It does not order candidates (`0xad3c00` through
  `0xad3dbe`).
- The search object holds byte grids at `+0x24` and `+0x2ac4`, four retained x and y pairs at
  `+0x5580` and `+0x5594`, queue read and write indices at `+0x55a4` and `+0x55a8`, and
  parallel queued x and y arrays at `+0x55b0` and `+0xf518`. The rows include stride padding.
  The queue and state grid, not a separate candidate sort, determine visit order
  (`0xad3c00`, `0xad3de0`, `0xad41f0`).

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
  `0xad4ac0`; their structure selector and category rules are described below. Exact spatial
  boundary and dynamic-state behavior remain unqualified.
  The cliff lookup combines the terrain corner value with the maximum value from any matching
  separately stored spatial rectangles (`0x7044b0`, `0x7411b0`, `0x6fd140`). The diagnostic
  samples the admitted terrain domains; it does not yet establish whether the separate layer
  is populated in either original probe.
- The first search enables an additional check at `0x7fd6b0`. If no site is found and that
  check has failed for at least one candidate, `0xad4970` searches the copy again with that
  additional check disabled (`0xad4a38` through `0xad4aa2`). The result is a failure sentinel
  if neither immediate nor retained candidates succeed. The town selector's metric wrapper
  `0x7fd610` also calls `0x7fd6b0`; both lead to `0x8a2270`, which can return a value from a
  two-coordinate calculation, a result from `0x8a3130`, or -1 when its worklist is exhausted
  (`0x7fd68c`, `0x7fd8cb`, `0x8a22a3` through `0x8a2365`). The exact reachability rules,
  movement inputs and later AI-script retry timing remain unknown.

#### Town coordinates

The registered AI natives `TownHasHall` and `TownHasMine` identify the references at town
offsets `+0x48`, `+0x54` and `+0x60` (`0xab7b90`, `0xab7be0`, `0xac47a0`, `0xac4800`).
The primary mine reference at `+0x54` is assigned to an object whose ability lookup accepts the
gold-mine implementation class `Agmb` (`0xab4550`, `0xac7000`, `0xac7070`; see
[unit resource amount](unit-resource-amount.md)). `+0x60` is the second mine reference; its
selection and replacement rules need further tracing.

`0xac80a0` refreshes the search coordinates from the primary mine and the `+0x48` reference. If both references
resolve, it takes their positions and computes the midpoint with the executable's float helpers
and its half operand at `0x10db0a8`. If only one resolves, it copies that object's position. If
neither resolves, it leaves the previous coordinates in place. A newly allocated record is
initialized from supplied coordinates by `0xac7e90`; `CreateCaptains` can supply the selected
player start coordinates when it creates a record (`0xabd160` through `0xabd1e0`). Despite the
native's name, the first reference can resolve to an acolyte; see the runtime trace below.

The start-location path first calls `0xac8290` with no hall and the selected start coordinate.
That calls `0xac7450`, which considers existing global town records only when their primary
mine reference resolves and compares the start to that mine with `0x7fd610`. If one passes the
selection, it can be reused. Otherwise `0xac7e90` creates a record from the supplied coordinate
and initializes both mine references to invalid (`0xac8290` through `0xac82f7`, `0xac7f10`
through `0xac7f27`). A mine is associated separately: `0xab4550` checks the unit's mine ability
and calls `0xac7000`, which selects or creates a record and invokes `0xac7070`; the latter stores
the primary mine reference and calls refresh (`0xab45a2` through `0xab45c4`, `0xac7000` through
`0xac7057`, `0xac70d0` through `0xac70ee`). Thus a mine being nearby is not itself a reference
in a newly created start town. The unit registration path can register a mine separately
(`0x687f68`); this trace does not establish the order and chosen town in the original probe.

The worker search also resolves a reference held by the worker at `+0x510` using `0x6933c0`,
then resolves the reference at `+0x48` of that associated object (`0xac2bd8` through
`0xac2c04`). A missing reference skips that worker. For a nonnegative town argument,
`0xac2b1c` first resolves the indexed town entry, and the worker-associated town must match it
when the player-unit iterator is present (`0xac2c25` through `0xac2c3b`). One initialization
path allocates the worker-associated object with its town reference initially empty, then
attaches it to the unit (`0xabb34f` through `0xabb3ea`, setter `0x69dd10`). The removal path
clears that attachment (`0xac4c9a` through `0xac4ccf`). The selection path is described below;
the Human05 record's actual contents remain unobserved.

The selected `h05_orange.ai` calls `CampaignAI`, then sets captain homes and queues its first
Necropolis with `SetBuildUnit`. The selected `common.ai` implementation of `CampaignAI` calls
`CreateCaptains`; `SetBuildUnit` stores town -1, and `StartUnit` passes that town to
`SetProduce`. The Human05 map creates the four acolytes before it starts the orange script.
There is no explicit expansion request in that opening build list.

The worker-association initialization calls `0xabc630` and stores its returned town in the
associated object (`0xabb448` through `0xabb4b1`, `0xac3830`). With an existing first town,
`0xabc630` visits up to four town slots in order. For each resolved town it prefers its hall as
the reference object, otherwise its primary mine; a town with neither is skipped for the
comparison. It calls `0x7fd610` with the reference object and worker coordinates and associated
parameters, retaining the first result with the lowest unsigned returned value. If no town supplies a
reference object it returns the first resolved town instead (`0xabc65a` through `0xabc773`).
When the first town slot is empty it calls `0xac7120`, which invokes `0xac7550` over the global
town records. That routine prefers a primary mine and then a hall; without either it uses the
town coordinates, and passes the selected reference and worker to the same metric wrapper
(`0xac714d`, `0xac7550` through `0xac779a`). The metric is not yet qualified as a particular
path or distance, and this trace does not establish the actual contents of the Human05 town
list at the first build order.

For `ugol`, the caller resolves one of the town's two mine references before searching; a successful order
passes that resolved handle to `0x69f760`, whereas an ordinary build order passes the selected
coordinates to `0x698f60` (`0xac2c3d` through `0xac2c89`, `0xac2d92` through `0xac2de1`).

#### Nearby-object score

The search invokes the unit enumerator with the town center, a radius taken from the same
1600-unit operand as the grid margin, and selector 2 (`0xad4b60`). The selector installs
predicate `0x696dd0` (`0x68ed7b`, `0x68f018`), which uses the same structure-class bit test as
`0x696b50`: the unit has bit `0x10000`, except the excluded `flags & 0x08000080 == 0x80`
combination. This is a structure-unit enumeration, not a separate enumeration of trees or
other destructables; mines participate when their unit state passes the same predicate. The
enumerator also applies its state mask (`0x696cf0`). Exact radius-boundary and dynamic-state
behavior remains to be checked against the recreation.

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

The facing for candidate snapping and the nearby-object texture walk comes from the selected
`Misc/BuildingAngle` configuration, normalized to a quarter turn by `0x46dea0`; `0xad4c10`
does not read the object's individual facing for this operation (`0xad41f0`, `0xad4cc4` through
`0xad4d6f`). The rectangle callback scales a world coordinate and the grid origin separately
before subtracting and truncating (`0xad4b90`); candidate indexing subtracts first and then scales
(`0xad438a` through `0xad43cb`). Their real-number operation ordering can matter for fractional
values.

#### Shared placement validator

Both AI candidate paths call `0x67ecd0` and accept only a zero result. The registered public
`IssueBuildOrderById` wrapper (`0x49a550`) calls the same routine before issuing its build order
through `0x698f60` (`0x49a5ab` through `0x49a5ea`). It passes a worker and different flags, and
continues for result 0 or `0x45`, subject to another check at `0x6a1790`. The AI call passes a
null worker and requires result 0. Counting arguments from one in call order, the worker is
argument 6; argument 10 is zero for the AI and one for the public order. Argument 7 comes from
the search object for the AI and from the selected worker for the public order. The meanings of
the flags remain unqualified (`0xad4471`, `0xad453b`, `0x49a576`). The recreation's existing
player-build placement rules implement the shared validator for this work, subject to the
specific limitations below.

`0x67ecd0` consults the building's pathing-texture object, transforms coordinates through
`0x4e00f0`, and uses masks and further callbacks. `0x4e00f0` depends on texture dimensions,
orientation and coordinate sign. It first clamps each coordinate to the map minimum and to the
map maximum minus 32 (`0x7f4a60`, initializer `0x4015b0`). For quarter turns 1 and 3 it swaps
the two texture dimensions. It truncates each clamped coordinate toward zero, removes its signed
remainder modulo 64, adds 32 toward that coordinate's sign when bit 1 of the corresponding
dimension is set, and adds 16 toward the sign when the dimension is odd (`0x4e013d` through
`0x4e028d`, initializer `0x403b40`). Zero uses the positive sign. Mapping the selected TGA's
axes to those native dimensions and the live construction facing remain unqualified. The
recreation's current `construction_position` explicitly uses a source-grid anchor as an
unqualified snap, and its construction host supports only the Human worker style and a subset of
placement tokens. Those limitations remain fidelity questions for this shared path.

## Recreation

`ai_placement.rs` implements the decoded score raster, clipping of texture-rectangle buffers,
queue order, candidate classes, tie-break and conditional second pass, town-coordinate refresh,
and the snap arithmetic. It reads grid, score and snap operands from the fingerprinted executable,
and uses the reviewed client real-number helpers for the snap's addition and subtraction.
Its caller must supply terrain samples, texture dimensions, complete texture-rectangle cells,
the extra check and the shared build validator; it propagates errors from those inputs. Synthetic
tests exercise these pieces. Production does not yet obtain those inputs or issue a build order.
The diagnostic below supplies provisional inputs from the local map. The caller must resolve the
special buffer-radius branch before scoring, and the `0x7fd6b0` reach check is not fully decoded.

The ignored local-input test `ai_placement_original.rs` reads the selected executable, the
Human05 terrain and pathing bounds, and the stock Necropolis texture from the selected install.
With the texture header dimensions supplied as native dimensions, the two recorded Necropolis
positions are unchanged by the snap. This is a snap compatibility check only: it does not apply
map object overrides, establish the native dimension mapping, or run the placement search.

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
| 8 to 9 s | The first acolyte reaches the rally rectangle `Acolyte_Move` and stops; a later sample shows its `unpl` build order. |
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
comparison from production orders is available.

### Order-time observations

One further isolated, muted and network-free reference run used a copy of the unblocked setup.
It registered player-unit order events and wrote their captured fields with the one-second samples;
scripts and receipts remain under `.scratch/ai-placement/order-probe/`. The map is instrumented,
and the added event trigger can affect handles, so this is a separate observation rather than
proof of identical internal town state. At elapsed 8.211 s, the `unpl` point order was issued to
a player 5 acolyte at (5467.445, 2808.107) for (6208, 2752), with no unit target. The 8 s
sample had four player 5 acolytes and no player 5 structures. The other workers' exact positions
at 8.211 s were not recorded. At 20.211 s, the `ugol` target order was issued to a player 5
acolyte at (5488, 2736), targeting the player 15 gold mine at (5696, 3456). The order events
for the first two ziggurats target (5664, 2912) and (5472, 2592). These API observations do
not expose the worker's native town association or either temporary pathing rectangle.

### Controlled origin experiments

Four additional network-isolated, muted reference runs copied the order-event probe. Each
changes one scenario input: the four acolytes' spawn location, the orange player's start
coordinate, or the original mine's creation position. Each still issues the same rally order,
records point-order events and samples once per game second. The per-run maps, preparation
receipts, input modifications and samples remain under `.scratch/ai-placement/origin-experiments/`.
The event instrumentation is common to the runs, but moving objects can change their subsequent
movement, occupancy, unit ordering or random state; the experiments do not isolate those effects.

| Scenario | Orange start | First `unpl` order time | Builder at order | Site | Mine / start / worker BFS rank |
| --- | --- | --- | --- | --- | --- |
| Baseline | (7488, 2752) | 8.211 s | (5467.445, 2808.107) | (6208, 2752) | 1760 / 5930 / 2076 |
| Four workers spawned at orange start | (7488, 2752) | 10.211 s | (5465.512, 2815.238) | (6336, 2752) | 1846 / 4762 / 2814 |
| Four workers spawned near mine | (7488, 2752) | 2.211 s | (5455.982, 2828.664) | (5248, 2944) | 872 / outside / 85 |
| Orange start changed | (6208, 2880) | 8.211 s | (5467.445, 2808.107) | (6208, 2752) | 1760 / 34 / 2076 |
| Mine creation requested at (1792, 3456) | (7488, 2752) | no event recorded through 28 s | no builder order recorded | no structure through 28 s | not applicable |

Ranks are one-based dequeues in the decoded grid traversal to the first occurrence of the
snapped observed point. The rank diagnostic reads the executable operands, selected Necropolis
texture and map bounds, exhausts the traversal, and rejects every candidate so no early result
distorts ranks. Its terrain scores and reach checks are deliberately not represented; these are
not ranks of accepted candidates. The event log for the initial simultaneous movement orders
is truncated, but later mine-moved samples show no structures or build orders. No subsequent
order event was recorded through 28 s. A separate repeat retained the mine handle for
observation: the requested (1792, 3456) position resolved to (2176, 2880), with neutral owner 15,
and that repeat also recorded no build order or structure through 28 s. The blocked-farm probe's 9 s sampled worker produced
mine / start / worker ranks 3822 / 8667 / 557; its exact order-time worker was not recorded.

The near-mine run's site is outside a grid centered on the assigned start. Native
`0xac2c9a` through `0xac2cc1` supplies the town coordinates to `0xad4970`, and `0xad3eab`
through `0xad3ede` initializes the raster origin by subtracting the search margin from those
coordinates, without shifting the entire grid to the map edge. The subsequent snap
(`0xad430e` through `0xad4367`, `0x4e00f0`) cannot move a cell from that start-centered grid
to (5248, 2944). Thus the assigned start coordinate is not the town search center in that run.
The unchanged result when only the start coordinate changes is consistent with a separately
associated town, but does not identify it. Mine-centered and worker-centered towns remain
possible; neither is an implemented explanation for all sites. Changing the mine's creation
coordinate produced no observed opening build in this window, but does not prove which AI gate or
town association caused that outcome. The 15-removal reach query and temporary rectangle clearing remain decoded
only in part; the probes do not establish that the mine is among the cleared rectangles.

### Local-input search diagnostic

The ignored `ai_placement_diagnostic.rs` test starts the selected Human05 map headlessly,
reads the selected texture headers and unit definitions, and runs the search with the existing
construction validator. It uses player 5's assigned start (7488, 2752), the map mine at
(5696, 3456), no hall, and the decoded refresh rule, giving a provisional center at the mine.
For the unblocked case it reads the builder's order-event position and the other three acolytes'
8 s sampled positions from the new probe. Creation in the recreation changes that builder's
position to (5467.25, 2808), and the other workers were moving between the sample and the order.
The blocked case uses its earlier sampled positions and creates the observed neutral farm.
Terrain domains and pathing come from the live placement cache; its static pathing sample does
not prove native dynamic pathing parity. Set `PCX_STARTUP_INSTALL`, `PCX_STARTUP_MAP`,
`PCX_HUMAN05_DECISION_LOG` and `PCX_HUMAN05_ORDER_PROBE` to the selected install, Human05 map,
recorded route log and new probe's `CustomMapData` directory, respectively, and run the ignored
test with `--features test-fixtures`. The route log was `.scratch/human05/placement-evidence-1.log`.
Set the optional `PCX_HUMAN05_VARIANTS` to the experiments directory to print the traversal
ranks in the table above.

The diagnostic now tries four origins: mine; assigned player start; the original-probe worker;
and the worker at the recreation's 1223 s refusal, read from the saved route log. The
last is a cross-scenario hypothesis, not an observed original town or probe worker coordinate.
For each it enumerates structures around that origin. Both texture-axis mappings give the same
results because these structures' textures are square. The table gives the chosen site when the
extra check is accepted or replaced by the worker-route surrogate. The one-based ranks count
dequeued grid points up to the first occurrence of the snapped site in a complete traversal;
they are not counts of valid cells. Both oracle and chosen scores are 5. The validator accepts
both oracle sites with the sampled worker positions. The first valid score-5 rank is also shown,
so no oracle is the first accepted site under these inputs. The diagnostic now reads the selected
building angle and uses the separate object-coordinate conversion described above. The ranks do not establish which
earlier candidates the native reach check would reject.

| Layout | Origin | Chosen point, cell | First valid score-5 rank | Oracle rank | Distance |
| --- | --- | --- | --- | --- | --- |
| No farm | Mine | (5184, 3008), (34, 36) | 867 | 1760 | 1055.515 |
| No farm | Start | (7168, 2432), (40, 40) | 307 | 5930 | 1011.929 |
| No farm | Probe worker | (5248, 2752), (43, 48) | 77 | 2076 | 960 |
| No farm | Route worker | (6272, 2304), (48, 49) | 1 | 705 | 452.548 |
| Farm | Mine | (5184, 3008), (34, 36) | 867 | 3822 | 960 |
| Farm | Start | (7168, 2432), (40, 40) | 307 | 8667 | 1216 |
| Farm | Probe worker | (5376, 2624), (43, 47) | 80 | 557 | 607.157 |
| Farm | Route worker | (6272, 2304), (48, 49) | 1 | 331 | 344.651 |

The town-origin route surrogate exhausts its eight-check work bound for mine, start and
probe-worker origins, and selects the same route-worker site shown above. Neither route
surrogate implements the native check. The mine-origin traversal also finds a valid retained
class candidate at rank 280; the later immediate score-5 candidate takes precedence.

The mine is present at the expected location, but the diagnostic does not issue or compare a
haunted-mine build. The two original ziggurat sites in each layout are recorded above; no
ziggurat comparison is available because the current construction host refuses its `blighted`
requirement. A first attempt using 8 s worker positions rejected the blocked oracle due to a
mobile occupancy cell. Using the 9 s samples instead admits it. This shows the importance of
the exact order-time state, not a proven validator error.

Further native tracing narrows the missing reach input. The construction caller passes the
selected builder to `0xad4970`, and `0xad41f0` uses references at builder offsets `+0x26c` and
`+0x13c` in its `0x7fd6b0` call (`0xac2cb0`, `0xad49dd`, `0xad42bf`, `0xad4442`). The
two coordinates supplied to that check are the town coordinate and the snapped candidate
(`0xad43ee` through `0xad4458`). Before calling `0x8a2270`, `0x7fd6b0` resolves each of those
two reference lists and changes associated spatial rectangles via `0x8b7b30` with argument 1;
afterward it calls the same routine with argument 0 (`0x7fd832` through `0x7fd8cb`,
`0x7fd95f` through `0x7fd9ab`). The meaning and contents of those worker-associated references,
the search's treatment of a town center inside the mine footprint and the state of the actual
worker references are not yet established. `0x8b7b30` clips each rectangle and dispatches updates
at several spatial levels. At the base level, `0x8b7740` clears byte 7 of every covered entry for
either argument. With argument 0 it also recomputes four two-bit fields from four underlying
spatial queries apiece (`0x8b781e` through `0x8b7877`, `0x8b78b0` through `0x8b7978`). Those
queries consult a masked spatial cell and, when present, a bounded object chain (`0x8be420`
through `0x8be50e`). Their full state and exclusion meanings are not yet mapped to the recreation.
The inner initializer `0x8a4a60` also has an early result when the two
coordinates map to the same cell or its starting-cell lookup returns -1; `0x8a2270` treats that
result as a finite coordinate-based value (`0x8a4b59` through `0x8a4b9e`, `0x8a22a3` through
`0x8a232d`). At the base spatial level `0x8a20a0` returns -1 for out-of-range cells or either of
two individual masked high-bit states (`0x8a20c4` through `0x8a211d`). Mapping those states and
the temporary rectangle operations to live placement data remains open, so ordinary reachability is not
an equivalent replacement. The recreation's route cannot substitute a stationary mine as its moving
unit: it refuses moving pathing textures. No production placement is admitted on these results.

### Reach search and temporary occupancy cache

The native search behind that check is a bounded, priority-ordered search over four levels of
spatial cells, not the recreation's unit route. The placement caller supplies the town coordinate
as its start, the snapped candidate as its goal, movement argument 1 and a work limit of 15
(`0xad43ee` through `0xad4458`, `0x7fd8a5` through `0x7fd8cb`). The search converts the
coordinates to its map-relative grid and floors them (`0x7fd6b0`, `0x8a4a60`, `0x9a1520`). It
uses a priority queue keyed by accumulated cost plus a goal estimate. The ordinary edge cost and
estimate use the client's integer square-root routine on coordinate differences scaled by 24
(`0x8a3d00`, `0x99b740`); the queue has its own equal-key ordering (`0x8a3090`, `0x8a2d20`).
Insertion promotes a new equal-key entry above its parent; removal favors the right child when
child keys are equal (`0x8a3090`, `0x8a2d20`). A lower-cost rediscovery increments the node's
generation and inserts another entry; a removal whose generation no longer matches consumes
work but does not expand (`0x8a3d00`, `0x8a3ff0`, `0x8a3bf0`). Equal or worse rediscoveries
do not replace the existing path. It allows at most 15 queue removals, including stale entries, and succeeds when the goal node is
removed (`0x8a3bf0`). This is a work limit, not a distance or path-length bound. The caller tests
only whether the result is different from -1 and does not compare the returned distance to a
threshold (`0xad4442` through `0xad4458`).

At the finest search level the routine considers cardinal and diagonal neighbors; diagonal
admission depends on the adjoining cardinal checks. The movement argument selects additional
two-cell-wide clearance checks (`0x8a4200`, `0x8a2e50`, `0x8a4680`). Larger cells use separate
edge and corner expansion routines and descend into mixed cells (`0x8a43c0`, `0x8a3830`,
`0x8a2ed0`). A path search over only the recreation's ordinary 32-unit cells therefore does not
reproduce the native work budget or necessarily the result. Exact edge expansion, integer cost
rounding, dynamic occupancy flags and worker-reference contents remain to be mapped before this
search can be implemented faithfully.

The lowest-level expansion can also follow an optional link identified by the cell's auxiliary
record. That transition adds one to the search cost instead of the ordinary distance; returned
path length handles it separately (`0x8a4200`, `0x8a4f70`, `0x8a3130`). A registration path
allocates an index, writes it across a clipped rectangle, and removes that index on teardown
(`0x7f6eb0`, `0x7f6d00`, `0x8b66c0`, `0xb2ef00`, `0xb33177`). The lookup checks an active
bit and a destination in the indexed record (`0x8a4371`, `0x8b73e0`). The index is stored in
byte 6 of each covered base cell; an otherwise open child with a nonzero index makes its parent
mixed, requiring finer search expansion (`0x8b6784`, `0x8b7990`, `0x8a3b50`). A nearby native class
identifier reports `Awrp`, which the selected ability table calls Warp; associating that class
with this registration is an inference until its vtable relationship is checked. The
recreation's movement router is a
separate eight-neighbor A* with its own octile heuristic, tie order and work accounting
(`unit_placement.rs`, `route_search`); its work policy and nearest-reachable behavior are not a
decoded implementation of this search ([collision replanning](unit-collision-replanning.md)).

The rectangle updates above have a more specific meaning than cache invalidation alone. Each
base entry contains four two-bit occupancy classifications in byte 7. A pre-search update
clears that entire byte for each covered cell, temporarily marking those classifications open;
it also recomputes the parent levels (`0x8b781e` through `0x8b7826`, `0x8b7c50`, `0x8b7990`).
Consequently this clears any overlapping blocker in those cache cells, not just the referenced
object. After the search, the base classifications are rebuilt by sampling each corresponding
two-by-two set of underlying spatial cells, then the parent levels are recomputed
(`0x8b7835` through `0x8b7877`, `0x8b78b0`, `0x8b7c50`). Zero blocked samples is open, four
is blocked, and an intermediate count is mixed. The underlying spatial query combines a mask
with a bounded object chain (`0x8be420`). Which placed structures, mines, trees, destructables
and mobile units contribute under the selected mask has not yet been reconciled with the
recreation's pathing layers. The selected first classification uses the image mask at
`0x10e92a8`: the spatial query rejects matching high-byte cell flags, then follows at most 49
object-chain links and applies object-state and lower-mask tests (`0x8b7835`, `0x8be420`).
Those native object-state bits do not by themselves identify the corresponding recreation
objects. The first builder reference is a position-bearing handle resolved
through `0x4707f0`; the second is a container resolved through `0x804ad0`. Their actual objects
and rectangles at the first Human05 order remain unobserved.

The blocked-start early result is an acceptance, not a route: when an in-bounds starting-cell
lookup returns -1, the initializer returns its immediate-result status and `0x8a2270` returns a
finite coordinate-based value (`0x8a4b59` through `0x8a4b9e`, `0x8a22a3` through `0x8a232d`).
The same immediate status is used for matching start and goal cells or matching resolved
hierarchy nodes. This cannot be replaced by a conventional reachability query. The exact
order-time state of the cache, including the temporary rectangles, is needed to determine whether
this branch applies to the mine-centered town.

### Live town and validator trace, 2026-10-28

The isolated Wine reference image has `winedbg` and needs no extra packages to debug its own
child under the existing capability and network restrictions. A synthetic 32-bit program first
verified conditional breakpoints and output capture. The reference session used the original
private Xvfb, null sink and software renderer. The executable hash was the pinned 1.29 image.
The copied harness and raw logs are local to `.scratch/ai-placement/debug-harness/`.

At the first Necropolis placement call, `0xac2c9a` through `0xac2cc1` pass the selected town's
stored coordinate to `0xad4970`. A breakpoint at that call and the following search recorded:

| Scenario | Town coordinate | First reach candidate | Chosen site | Native queue removals at choice |
| --- | --- | --- | --- | --- |
| Baseline | (6264, 2776) | (6208, 2752) | (6208, 2752) | 1 |
| Workers spawned at start | (6632, 3112) | (6592, 3072) | (6336, 2752) | 355 |
| Workers spawned near mine | (5592, 3336) | (5184, 3328) | (5248, 2944) | 419 |
| Start coordinate changed | (6264, 2776) | (6208, 2752) | (6208, 2752) | 1 |
| Neutral farm at baseline site | (6264, 2776) | (6464, 2752) | (5952, 2432) | 280 |

In the baseline and farm traces the resolved `+0x48` object is an orange `uaco`, and `+0x54` resolves to
the neutral gold mine. The initial samples show an acolyte at (6832, 2096); its midpoint
with the mine at (5696, 3456) is the captured town coordinate. The same calculation with
initial acolyte positions (7568, 2768) and (5488, 3216) gives the captured worker-spawn
variant coordinates. This matches the existing midpoint code. The selection trace below narrows
which acolyte is associated. The result corrects
the earlier inference that the absence of a completed hall implies a mine-centered town.

The first reach call in all five traces used a
nonempty handle at builder `+0x26c` and an empty reference at `+0x13c`. The handle resolved
to a spatial object with a rectangle, which `0x7fd84b` through `0x7fd85d` clears before the
query. In native rectangle coordinates the baseline and farm captured (398, 505, 400, 507),
while the near-mine trace captured (399, 505, 401, 507). Its exact object identity and mapping
to recreation occupancy are still open. For the first candidates the inner query returned
0, 0, 12, 0 and 8 respectively; these finite results
pass the native reach test. They do not prove that all later candidates pass it.

With the captured baseline origin supplied through `PCX_HUMAN05_TRACE_ORIGIN`, the ignored
diagnostic selects (6208, 2752) at traversal rank 1 in both texture-axis variants. In the farm
layout it selects (6464, 2752), rank 189, instead of (5952, 2432), rank 280. Both cells score
5 and the diagnostic accepts both; the town-origin route surrogate exhausts its own work bound.
The native farm trace accepts the earlier candidate's reach query, then the shared validator
returns `0x44` (`0xad4458`, `0xad449d`, `0xad44a2`). The validator's texture-cell callback
queries pathing at (6224, 2800) with mask `0x08` and reports a blocked cell (`0x690386` through
`0x69039f`, `0x7f69d0`). The diagnostic reports an occupied structure footprint at that exact
cell, but its construction validator still accepts the candidate. A further trace identifies
the native texture index as (6, 0), with texture flag `0x08`; that selected Necropolis pixel is
blue-only. The construction raster at the time retained red pixels only, so this border cell was
absent from its check. The subsequent channel decode and diagnostic are below.

The native hierarchical queue and its occupancy mapping remain incomplete. The diagnostic
surrogate is not that search, and the strict production refusal remains. Human05 was not rerun
because both layouts are not reproduced by the recreation.

### Texture channels, town selection and reach traces, 2026-10-28

The pathing-texture constructor at `0x4de9f0` tests the low bit of the red channel and adds
native flags `0xd2` when set; any nonzero green adds `0x04`; the low bit of blue adds `0x08`.
The constructor does not inspect alpha. Its source indexing and the texture traversal at
`0x4dfb40` account for the texture axes and quarter turns. The shared construction callback
`0x6902d0` tests the resulting flags against `0x4a` before its per-cell pathing and required-mask
checks. Thus red and blue pixels can participate in this construction check, while green alone
does not. The check at `0x690386` uses the supplied prevented mask; the required-mask and special
`0x20` branches have further conditions, including visibility and spatial rectangles. Their full
blight, water and object-state behavior is not established by the channel mapping alone.

The shared recreation construction raster now derives the channel operands and mask from reviewed
windows of the fingerprinted executable and includes all cells selected by that mask. Movement
occupancy continues to use its existing red-channel footprint. Non-binary movement red values,
odd texture dimensions and unqualified facing still refuse rather than gaining invented behavior.
Synthetic nonstock channel and rotated-footprint tests exercise the new raster. Human03 and
Human04 route checkpoints and RNG fingerprints match their `g1` logs, including victories at
279.02 s and 449.02 s respectively.

The selector at `0xac3f50` walks the player's native unit list, skips entries with the tested
state bit at `+0x20`, and immediately chooses the first eligible entry with bit `0x10` at
`+0x454`. Otherwise it remembers the last structure and last other eligible unit, preferring
the structure after the list ends. It passes the chosen unit to `0xac8290`; `0xac8010` assigns
the town reference and calls `0xac80a0` to refresh the coordinates. Breakpoints in baseline,
workers-at-start and workers-near-mine runs show the first visited orange unit is an acolyte
with that preferred bit and is the chosen object. Its creation positions are respectively
(6832, 2096), (7568, 2768) and (5488, 3216), the fourth created acolyte in each scenario.
The selected reference is not the worker that issues the first order. Native list ordering and
the meaning of the state bit have not been mapped to recreation units; generalizing this to
fourth-created workers would be an unsupported rule. Other refresh triggers also remain open.

The isolated bounded breakpoint logs capture every dequeue through the selected candidate and
the result of every reach query that actually occurred. Empty reach fields in the local trace
mean the candidate did not reach that check, not that it failed it.

| Worker layout | Dequeues | Reach queries | Finite | Failure sentinel | Chosen point, reach value, validator |
| --- | ---: | ---: | ---: | ---: | --- |
| At start | 355 | 100 | 62 | 38 | (6336, 2752), 12, 0 |
| Near mine | 419 | 12 | 12 | 0 | (5248, 2944), 15, 0 |

For example, at-start dequeue 1 returns 0 then validator `0x44`; dequeue 35 at (6528, 3200)
returns the failure sentinel and does not enter the validator. Near-mine dequeue 406 returns 12
then validator `0x44`. These are ground-truth inputs and outputs, not validation of
an implemented native pathfinder. The hierarchy's edge expansion, queue tie behavior and mapping
of spatial object-state masks remain unresolved, so the recreation does not yet reproduce those
100 and 12 reach decisions.

An additional breakpoint trace of the at-start dequeue 35 shows 15 queue removals before
the failure sentinel. The removed nodes include all four hierarchy levels. Removal 10 is a
stale entry: its recorded generation is 1 while the node's current generation is 5. It still
consumes a removal, as the control flow at `0x8a3c10` through `0x8a3c4b` predicts. The trace
does not establish the missing edge expansion or occupancy mapping, and it has not been
reproduced by a recreation search.

With the captured baseline center and the updated construction raster, the diagnostic still
chooses the unblocked site (6208, 2752), rank 1. In the farm layout it rejects the formerly
accepted (6464, 2752), but chooses (6080, 2432), rank 282, instead of native (5952, 2432),
rank 280, a difference of 128 units. Both axis readings give the same result. The diagnostic
reports mobile occupancy at cells around (5728, 2560) and (5760, 2592), and rejects the oracle
site using its sampled worker state. The sample is at whole seconds rather than the native
order instant, and the client's treatment of mobile occupancy in this check is not yet mapped;
the evidence does not distinguish those causes. The route surrogate also exhausts its own work
bound for the farm case. Neither observation licenses changing the result to fit the oracle.

### Farm validator and exact order-time workers, 2026-10-28

An isolated farm-layout breakpoint capture records all 280 dequeues through the chosen site.
It records 77 nonzero prevented-mask cell queries, with no required-mask, special-mask or
visibility rejection captured. The first rejected cells include the neutral farm's footprint;
the mine footprint also produces rejected cells. The trace's immediate-candidate validator
returns `0x44` on 13 captured returns. The retained-candidate return site was not separately
instrumented, so that count does not cover every rejected candidate.

The query at `0x7f69d0` expands the requested low-byte mask into both the upper and lower
bytes of a spatial mask. `0x8bdbe0` first rejects a matching upper-byte cell flag. Otherwise it
walks eligible spatial entries: an entry must be active and have a valid visit marker, its state
at `+0x44` must have no bits in `0x8fffffff`, and its low pathing mask at `+0x34` must intersect
the requested lower mask to block the cell. `0x7f6890` temporarily enables the query option that
otherwise filters state bits `0x60000000`. These checks describe the native state, not yet a
mapping from every recreation unit and destructable to that state.

At cell (5776, 2512), covered by an acolyte during the first search, the captured spatial entry
has state `0x20000001` and a low pathing mask intersecting `0x08`. The state test skips it.
At farm cell (6160, 2800), the captured entry has state `0x10000000` and a low pathing mask
intersecting `0x08`; the state test admits it and the cell blocks. The former entry's linked
native object was captured. Its overlapping rectangle and the order-time unit positions are
runtime observations; its unit type was not established by that capture.

The spatial-entry constructor at `0x8c1950` initializes the state word to zero. The wrapper
at `0x8c19e0` sets `0x10000000` on the new entry. Two call sites use that wrapper, at
`0x8013a3` and `0x805343`; the latter creates a collection of entries and assigns their lower
pathing masks. The movement-object constructor at `0x8baf10` instead creates two entries with
the zero-initializing constructor, keeping them at offsets `+0x94` and `+0x98` and setting the
active flag on the second. This does not establish that every farm, mine or destructable uses
one particular constructor.

The movement update at `0x8bca70` changes `0x20000000` on the second entry: it combines two
stored components with an input, applies a magnitude threshold, and sets or clears that bit
according to a stored positive scalar. Another operation sets `0x40000000` temporarily at
`0x8ac327` and clears it at `0x8ac378`.

An isolated Wine debugger run placed a hardware watchpoint on the state word of the first
newly constructed movement entry after `0x8bafd6`. The watchpoint was accepted, but did not
fire during the bounded observation window. That entry was not identified as the captured
acolyte-overlapping entry; this negative result does not establish its later state. The
original-client run used a private Xvfb display and null audio sink.

### Placement exclusion count, 2026-10-28

The low portion of the spatial state is a nested exclusion count, not a mobile-unit class bit.
`0x7ffd00` resolves the supplied object's movement entry and increments the word at `+0x44`
for a nonzero argument (`0x7ffd2f`), or decrements it for zero (`0x7ffd36`). The placement
validator `0x697940` can enumerate objects in its candidate rectangle using `0x7f4d30`,
conditionally increments selected objects through `0x6bde70`, checks the footprint, then
decrements the saved list at `0x697f0d`. The predicate includes native object-type, state,
player and virtual-method checks (`0x697a83` through `0x697c1a`); their complete mapping to
recreation objects is not established. The supplied object at context `+0x4c` is also
temporarily excluded. The cell-query helper `0x7f6890` independently brackets its optional
object with the same increment and decrement (`0x7f6962`, `0x7f69a4`). The movement query at
`0x8b03c0` also temporarily increments and then decrements its own entry.

In an isolated farm-layout capture, a watchpoint installed on the entry actually reached by
the (5776, 2512) validator query observed `0x20000001` become `0x20000000` at `0x7ffd36`.
The stack passes through `0x697f0d` after the footprint check, confirming that the observed
count belongs to the validator's temporary exclusion list. The native object passed to the
decrement is different from the builder pointer captured at the placement driver in that same
run. A subsequent read-only capture of that same query and decrement identifies the object as
an orange acolyte: its unit rawcode is `uaco`, its owner is player 5, and its vtable is the
reviewed CUnit table at `0xe9c504`. A fixed address observed in three earlier captures shifted
in a later run, so the watchpoint was attached to the entry resolved at the query in the same run.

### Placement exclusion selection, 2026-10-28

`0x697940` conditionally enumerates objects in the candidate rectangle with the selected-image
mask and callback (`0x697a3a`, `0x7f4d30`, `0x698520`). In the applicable enumeration branch,
`0x808480` requires the object's center to be strictly inside the rectangle and checks the
underlying object's state before passing its reference to the callback. The validator checks the
object's native class and resolved state before either exclusion branch (`0x697a83` to
`0x697adf`). This is a candidate-rectangle selection, not a rule that ignores all mobile units.

For the observed AI placement context, `+0` is nonzero, `+0xc` is zero, and `+0x58` is
nonzero. The relevant branch obtains the candidate unit's owner through CUnit vtable slot
`+0xec` (`0x6736a0`), tests a directed player mask (`0x47f9f0`), and requires a non-null
unit component at `+0x3f8` whose call to `0xb66f10(0, 0)` succeeds (`0x697b91` to
`0x697bc7`). The component is the unit's movement ability: its captured class identifier
agrees with the native lookup at `0x6d6570`. One captured player 5 acolyte passed the player
test and returned true from that component check. Its movement-component counters at `+0x3c`
and `+0x80` were zero. The branch then temporarily excludes the selected unit at `0x697c1a`.

The movement check is not determined solely by movement type or an active move order. At
`0xb66f10`, a positive component counter at `+0x80` rejects; a positive count at `+0x3c`
is adjusted using two ability or buff lookups, unit flags at `+0x164`, and a unit value at
`+0x15c` before acceptance. The recreation has ownership, source definitions, activity,
pause and some buff state, but no established mapping from those fields to both native
counters and all of their adjustments. In particular, an absent recreation pause is not proof
that those native counters are zero. The directed player-mask category and the counter writers
are qualified further below.

Another context branch uses CUnit vtable slot `+0xfc` (`0x66d280`) for a player-dependent
result and slot `+0xf0` (`0x666e90`) for a unit-flag predicate. The former reaches native
player and visibility checks (`0x475150`, `0x4758d0`, `0x4757a0`); its complete mapping to
the recreation's vision and detection state is not established. The captured acolyte's
`+0x164` value makes the latter predicate false. These unresolved branches prevent replacing
the diagnostic's mobile-occupancy switch with the full decoded rule.

An order-event probe records all four acolyte positions at 8.211 s, when the site is issued.
Two acolytes occupy cells selected by the Necropolis construction texture. The ignored
local-input diagnostic can load that local event record. With the recorded town coordinate and
those exact positions, the ordinary recreation validator selects (6144, 2432), rank 283, in
the farm layout. An experimental run that disables the recreation's mobile occupancy
selects (6208, 2752) at rank 1 without the farm and (5952, 2432) at rank 280 with the farm,
for both tested texture-axis readings. The diagnostic asserted those results. It bypassed the
native reach check and disables all four mobile stamps rather than deriving each object's native
state, so it is not a production implementation or validation of the complete search.

### Movement and alliance inputs, 2026-10-28

The directed mask is the unit owner's shared-control relationship toward the querying player.
The `GetPlayerAlliance` implementation at `0x4944c0` indexes the same player record by an
alliance-type value with a 16-byte stride, starting at `+0x38`. The placement helper at
`0x47fa31` uses the record at `+0x98`, corresponding to the shared-control value in the selected
script declarations. `SetPlayerAlliance` writes the same directed record (`0x4a94a0`,
`0x4c1f70`, `0x4c21e0`). Player initialization enables shared control from each player to
itself (`0x47ce30` through `0x47ceab`). The observed own-player acolyte passes the mask. For
other players the answer depends on the directed shared-control state, not simply on passive
alliance, enemy status or neutrality. The recreation retains explicit directed assignments, but
its startup-alliance reader currently binds the passive category, not this self default. It
cannot yet supply the full shared-control answer without extending the selected-image rules.

The movement ability initializes `+0x3c` in its shared base constructor (`0x46d05c`) and
`+0x80` in its own constructor (`0xb663b8`). The shared ability-disable paths increment or
add to `+0x3c` at `0x46ca26` and `0x46e0d9`, and subtract at `0x46e651`; their movement
ability virtual check admits this class (`0x46a490`). The movement-specific path increments
`+0x80` at `0xb666bc` and decrements it at `0xb665f9`. Effect code can choose between the
two counters based on its own state: examples include `0xb8ec16`/`0xb8ec2b` and
`0xb9d839`/`0xb9d844`, with corresponding removal paths. The native effect classes identified
by `Bens` and `BEer` also call the movement-specific increment and decrement paths
(`0xc1a019`, `0xc1a656`, `0xc4866e`, `0xc48873`). These examples do not establish an
exhaustive mapping of every caller or effect transition.

For the placement call's two zero arguments, `0xb66f10` rejects a positive `+0x80`. If
`+0x3c` is positive, it subtracts at most one when either the sleep buff is present or the
sleep-always ability has its `0x80` state bit, then subtracts at most one when any of the two
unit flags or the positive unit counter tested at `0xb66fb5` through `0xb66fd4` is present.
The resulting positive count rejects. The selected ability data identifies the first two
lookups as sleep-related; the two native unit flags and counter have not been fully mapped to
recreation state. A second isolated farm capture reads the movement component of the exact
non-builder acolyte whose spatial exclusion is restored after the (5776, 2512) query. Both
component counters were zero at that instant. This observation does not establish the absence
of every possible effect in other units or at other times.

The recreation records natural sleep, explicit pause and a bounded collection of active buffs,
but it does not maintain the native movement-disable reference counts and their effect-specific
transitions. Its buff records contain removal traits, not the native movement counter
contributions. Supporting the general predicate needs source-backed apply/remove handling for
the relevant effects and other remaining callers, the two unit flags and unit counter, and the
shared-control startup state. The `+0xc` visibility branch also still needs the native
player-dependent fog, concealment and detection conditions mapped to available live vision
state; a shown unit or a vision source alone does not establish that result. At this stage the
diagnostic used its experimental mobile switch, and production still refused.

### Candidate exclusion preconditions, 2026-10-28

The validator builds its enumeration rectangle from the selected texture dimensions and
candidate position (`0x67f2e6`, `0x4dffc0`), then expands each edge by the product of two
client-initialized operands (`0x67f2eb` through `0x67f3bb`, `0x40a860`, `0x401610`). The
texture scale is initialized at `0x403b40`; a quarter turn swaps the axes. A read-only isolated
farm capture observed a 16-by-16 texture and the resulting rectangle at the same validation
call. The local-input test compares the decoded calculation against that captured rectangle.
The recreation admits integral candidate coordinates only while every resulting integer is
exactly representable; other coordinate cases refuse pending the client's real arithmetic.

The startup-alliance reader now derives the self shared-control kind from the selected client
call at `0x47cea6`. Explicit directed assignments take precedence. The diagnostic uses the
expanded rectangle, the directed relation and the decoded visibility branch to qualify each
unit. It temporarily excludes the builder and the qualified units, then restores their collision
stamps. It no longer provides the blanket mobile-occupancy switch used for the historical
experiment above. The predicate requires the observed AI context with nonzero `+0` and `+0x58`;
an unqualified context refuses instead of using the separate fallback branch.

This is a preconditioned diagnostic, not an admitted production search. For a unit inside the
rectangle with shared control, live explicit pause, sleep or an active buff cannot yet be mapped
to the native movement counters and cause a typed refusal naming the unit and source. Even when
those records are absent, the host does not track all writers of the counters, so it refuses
with `unit-wide ability-disable counts and native effect writers`. The known write paths are `0x46ca26`,
`0x46e0d9`, `0x46e651`, `0xb666bc` and `0xb665f9`; identified effect calls include
`0xb8ec16`, `0xb8ec2b`, `0xb8f2e6`, `0xb8f2fb`, `0xb9d839`, `0xb9d844`, `0xb9dc0c`,
`0xb9dc17`, `0xb9dd66`, `0xb9dd71`, `0xba0f79`, `0xba0f84`, `0xc1a019`, `0xc1a656`,
`0xc4866e` and `0xc48873`. These are not an exhaustive classification of all possible
callers. The other visibility branch refuses if its player-dependent result is unavailable.
An empty retained buff list alone is not evidence that all native counter writers were absent.
With the recorded town origin and order-time farm workers, the qualified local-input diagnostic
refuses at the unblocked oracle candidate (6208, 2752), dequeue 1, and at (6272, 2752),
dequeue 5, in the farm layout. Both refusals identify a unit with untracked movement-counter
writers. Thus neither selected site has been reproduced under the qualified rule.
The complete hierarchy edge expansion and its occupancy mapping remain open for the two
recorded reach tables; neither the recreation's ordinary router nor the historical no-reach
diagnostic is a validation of those tables.

### Movement-counter callers, 2026-10-28

The common ability attachment path `0x6d4560` records the movement component at unit offset
`+0x3f8` and applies existing unit-wide disable counts to the attached ability. The unit-wide
update at `0x6d60f0` maintains six counts at offsets `+0x3c4` through `+0x3d8` and walks
the unit's ability list, incrementing or decrementing eligible abilities. Its callers must be
qualified as well as direct movement-effect callers. The `PauseUnit` registration at `0x49d8f1`
selects `0x4a3f00`, which reaches that update through `0x69c6e0` and `0x69c750` under the
native flag conditions. The recreation records the explicit pause interval, but that alone does
not represent the other unit-wide disable counts.

The native virtual tables at `0xf3c528` and `0xf3d130` identify two effect classes as `Bprg`
and `Bchd` via their class getters `0xb9d5e0` and `0xb9e8c0`. Both tables use the methods
containing the calls at `0xb8ec16`, `0xb9d839` and `0xb9dc0c`. The selected ability table
associates `Bprg` with the `Aprg` Purge record, as well as other records; this establishes
another effect family to investigate, not that a particular Purge occurred in the probe.
The class identity alone does not establish all callers or when those methods run.

A scan of direct calls and jumps in the retained disassembly found five calls each to the
movement-specific increment and decrement helpers, and 107 and 99 calls to the generic add
and subtract helpers. There are also 90 direct calls to the unit-wide update. The generic
helpers also operate on ability classes other than movement, so these counts are a work
inventory, not a count of established movement triggers. Indirect calls and unit-wide dispatcher
callers still require classification. The probe retains the
original map initialization and other players; its added script creates four orange acolytes
and the farm and starts the orange AI. The map also contains a global pause in an ending
trigger, which does not establish that it ran before this order. The selected unit-ability table
gives `ACen` to `nmrr`, and the original map creates two neutral-hostile units of that type.
Their creation locations are far from the workers, but their entire movement and targeting
history has not been bounded for the order time. The orange AI script does not by itself rule
out an effect from another unit or a unit-wide engine transition. Consequently neither this
script search nor the absence of a recreation buff proves zero for every writer at the order
instant. The previously captured zero values remain observations for the sampled native unit only.

The hierarchy lookup at `0x8a20a0` distinguishes blocked cells from mixed coarse cells:
the former returns -1, while a mixed cell above the base level returns -2. For an open
cell it climbs through open parents before resolving or creating the search node. The edge
walkers at `0x8a3830` and `0x8a2ed0` descend into a mixed neighbor, splitting its edge and
recursing on the two portions before adding traversable nodes. This narrows the remaining edge
work, but does not by itself map the occupancy classifications or establish the clearance
and corner helpers needed to replay the recorded reach decisions.
At a coarse node, `0x8a43c0` visits the negative-y, positive-x, positive-y and negative-x
edges in that order, then tests the four corners in the corresponding order. The edge walkers
can emit multiple lower-level nodes when a neighboring cell is mixed, so a single coarse
neighbor cannot be substituted for this traversal.
For internal movement class 2, the finest-level positive-x check also requires the cells one
step farther in positive x, with the second at positive y; the positive-y check similarly
requires the next row at the current and positive-x columns (`0x8a2e50`, `0x8a4680`). The
relationship between this internal class and every recreation movement state remains open.
The distance helper `0x99b740` chooses an initial integer estimate from three input ranges,
then repeatedly averages the estimate with integer division of the input by that estimate.
It stops when half the signed difference is zero. A replacement must preserve that termination
and integer arithmetic, rather than assume a host floating-point square root has identical
rounding.

## Not established

- The recreation mapping of the native acolyte list and state bit, other refresh triggers, and
  the route metric used to select among towns;
  terrain sampling at grid edges and the enumerator's boundary/state behavior;
  live texture dimension mapping, blight and the shared validator's detailed
  checks; and the two-pass check's meaning. The decoded buffer layer alone cannot reproduce a site.
- Worker choice beyond "an idle acolyte in the rally area": the state filter of `0xac2b00` and
  the arena or list order among equals.
- Cost charging for AI construction (the human construction path charges when the structure is
  placed), refund on cancel, and retry cadence when a site or the builder is not ready.
- The early false answers listed above, and the upgrade fallback.

What would settle placement: map the remaining validator and town state and the hierarchical
reach search, then rerun the layouts at exact order-time state. Until then
Human05's orange AI refuses at its first structure, which is the 1223 s stop.
