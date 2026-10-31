# Terrain blight and regeneration

## Initial map state

W3E corner records carry blight in bit `0x20` of their texture/flags byte,
independently of the texture slot. `pcx-map` already preserves that byte as a
slot nibble and four flag bits; `Corner::is_blighted` names the corresponding
flag without changing file serialization.

Format references: the [mdx-m3-viewer corner reader](https://github.com/flowtsohg/mdx-m3-viewer/blob/master/src/parsers/w3x/w3e/corner.ts)
and [Warsmash corner reader](https://github.com/Retera/WarsmashModEngine/blob/main/core/src/com/etheller/warsmash/parsers/w3x/w3e/Corner.java)
both read that bit. These are format implementations, not observations of the
selected game's regeneration or proof of player-affinity semantics. No code or
original terrain contents are copied from them.

`InitialBlight` reads every flag from the admitted terrain, retaining its map
source identity. Its world origin comes from W3E. Grid spacing is the selected
client's already derived placement cell size multiplied by the admitted map's
terrain/pathing dimension ratio. There is no copied measured world-size literal
and no `Terrain.slk` class-letter substitution for missing state.

The nearest corner owns a half-open square footprint centred on that corner.
This footprint convention and the use of map-authored blight for all owners are
recreation choices pending a valid original-runtime eligibility comparison.
Outside those footprints there is no map-authored blight.

## Regeneration projection

The existing activity projection supplies its accepted straight motion spans.
Initial positions are captured before movement. Regeneration integrates standing
gaps and tails at their accepted position and splits each moving span at grid
crossings. Diagonal crossings visit a cell once. All visited cells consume the
caller's deterministic work budget; zero-length intervals do no work. Pools and
regeneration clocks are staged before any pool is written, and failed public
span output is discarded under the existing session-stop-on-error contract.
Motion effects themselves are not rolled back.

Only sessions with moving blight-regeneration units need the internal span
buffer. Callers already requesting spans share that buffer; no global movement
history is retained. A direct clock rebase with unprojected blight-unit motion
refuses rather than integrating the entire interval at its final position.
Relocation must be reconciled at a projection boundary; position writes and
relocating reveals/pathing changes refuse before mutation when that boundary is
missing. Continuous integration
and its existing f32 pool rounding do not establish native regeneration tick or
delay parity.

## Runtime script changes remain open

Selected source: Legacy 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Static primary registrations point to `SetBlight` handler `0x4a6f10`,
`SetBlightLoc` `0x4a6f70`, `SetBlightPoint` `0x4a6fd0` and `SetBlightRect`
`0x4a7030`. Each resolves its player and forwards a bitmask formed from the
player record. Circle/location call `0x6d98d0`; point calls `0x6d9ab0`.
Separate registrations use the known no-op stub family. These static paths do
not establish which registry is active in a runtime call.

The host admits circular `SetBlight` and `SetBlightLoc` additions and records
their player in the same state as structure growth. Removal requests refuse.
`SetBlightPoint` and `SetBlightRect`, the exact initial owner layers and
rendering remain open.

The previous phase-enabled native probe is invalid for eligibility: its unit
was misplaced and its records truncated. See [regeneration](unit-regeneration.md#phase-enabled-blight-probe-one-run-outcome-2026-10-20).

## Growth and placement investigation, 2026-10-30

The selected unit ability table assigns the Necropolis and Haunted Gold Mine a
blight-growth ability. The selected ability table supplies its `Area`, `Dur`,
`DataA` and `DataB` fields. The growth-ability initializer at `0xb8c3e0` reads
those fields and creates a separate effect at `0x6da390`. The effect stores a
target radius, update interval, radius increment and mode. Its start path at
`0x6daf10` schedules repeated updates and applies the first increment
immediately. The update at `0x6da100` multiplies the increment by the current
count, paints that radius and checks whether to stop. The interval is bounded
by an executable operand at `0x10dd068`. The effect's native lifecycle and
behavior when its source is removed still need qualification.

The shared placement validator at `0x6902d0` handles the blight requirement as
a special `0x20` branch. Depending on the caller it checks either the player's
mask or the terrain's global mask through `0x690d20` or `0x4e02a0`. The latter
includes the high bit of the terrain blight entry. On map initialization,
`0x4c40f0` scans groups of spatial pathing cells and seeds a separate blight
owner-mask grid; `0x4c5470` updates its high-bit layer. Runtime circle painting
at `0x6da510` changes that grid and notifies the spatial pathing system. Thus
the current nearest-corner regeneration layer is insufficient evidence for
the placement raster or for overlapping player-owned sources.

The larger-radius branch at `0x6da510` obtains row spans from `0x4c3e70`.
That function builds symmetric spans using the integer circle data generated
at `0x4c4e40`; the two smallest radius classes use separate raster cases.
On removal, `0x4c5160` clears the owner bits while preserving the high bit;
`0x6d98d0` also enumerates surviving effects through `0x6da2d0`. The ordering
and precise conditions for those effects to repaint remain to be integrated
and checked against overlapping-source observations.

The high-bit update at `0x4c5470` has two branches: one sets the high bit
from whether any owner bit remains in each cell, while the other updates only
cells selected by a separate mask layer. The ownership and lifetime of that
mask layer are not yet established. During a clear, `0x6da2d0` considers an
effect only if its active field and state bit are set and its expanded radius
intersects the cleared area; its two modes then use different replay paths.
The source-death call path and those two replay modes have not been qualified.

Human05 also registers a construct-start trigger whose action calls
`SetBlightRadiusLocBJ` for player 5. The host records that fixed circular patch
through `SetBlightLoc`. Construction completion and `CreateBlightedGoldmine`
start growth using the selected ability cells and the reviewed image operands.
Source death and overlapping dynamic patches refuse when their affected cells
are queried; they are not removed or merged by an approximation.

The selected ability table also has blight-dispel modes. At structure completion
the host records the source, clock and growing affected area from those ability
fields instead of refusing immediately. A construction footprint that requires
or prevents blight refuses when it reaches that area. Other blight reads still
use the pre-dispel state: the effect's removal and overlap raster is not
implemented, so those reads and blight regeneration inside that area are not
qualified. The pool's broad refresh similarly retains the prior flags; the
footprint validator performs the localized refusal. This avoids an unrelated
construction-completion failure but does not establish general dispel fidelity.

`IsPointBlighted` uses the spatial `0x20` flag (`0x499680`, `0x7f69d0`),
not the owner-mask query. A read-only debugger capture at `0x6da6d2` and
`0x6da6fd` confirmed that the two-cell-radius owner raster includes an edge
cell that the spatial rectangle excludes. The spatial rectangles and the
larger integer-circle spans reproduce all 600 recorded point samples in the
ignored local-input test, with the first sample used as the time origin. The
selected table supplies the interval and increment; the selected image supplies
the coarse grid spacing at `0x40a860`, the fine spatial grid spacing at
`0x4015b0`, the timer bound, effect class and stopping tolerance. The smallest
spatial rectangles use the fine grid even when the W3E origin is not aligned
to the coarse grid. The first-sample time origin does not establish the phase
between construction completion and that sample.

The shared construction validator checks the current spatial blight state at
each footprint cell. This uses the observed visible-cell behavior; the native
validator can instead consult its owner grid depending on the terrain visibility
test at `0x498f10`. That branch and the high-bit mask are not mapped to the
recreation's vision state. The pool rebuilds its spatial blight flag lazily when
an AI placement needs it; source-clock regeneration splits movement at grid and
growth boundaries. The initial W3E corner convention remains the approximation
described above. These choices do not establish unseen-cell placement parity.
Completed structures created directly through generic unit creation and runtime
changes to a source's growth ability are not yet enrolled by this path; the
completion and `CreateBlightedGoldmine` paths are the ones exercised here.

An isolated original-client run sampled `IsPointBlighted` once per game second
at the Necropolis center and along four axes. At sample-clock 12 seconds the
structure was under construction and the center was not blighted; the western
samples already included the separate scripted patch. The center first tested
blighted at 56 seconds, while construction was still in progress. At 72 seconds
the finish event had fired and additional samples tested blighted; by 73
seconds all four axes at the sampled 1024-unit offset tested blighted. Several
other structures existed by then, so these observations do not isolate the
source of the earlier center change or establish a single-source radius. The
sample clock starts three seconds after map initialization. The probe used a
private Xvfb display and null sink and was stopped after recording completion.

A second run isolated one Necropolis by disabling the script's blight trigger
and not starting the orange AI. Its build order was accepted and the first
sample showing a construction site was at two seconds. Every sampled point
remained unblighted through 61 seconds. The 62-second sample recorded both
construction completion and blight at the center, with all four axes blighted
at a 384-unit offset. At 63 seconds all four axes tested blighted at 1024 units;
at 1152 units only the positive-x and positive-y samples did. These are sampled
coverage points, not a fitted circle or a death/removal measurement. The same
three-second initialization offset and isolation applied.

A follow-up of the isolated run sampled at the native scheduler's nominal
5 ms substep for three seconds after the finish event. All 600 timestamped
samples were retained; the earlier attempt lost part of each output record to
the client's string limit and is not used for timing. At the four axes sampled
every 128 units, the first observed coverage was at 61.628 seconds on the probe
clock. The next outward coverage changes appeared at 61.788, 61.948, 62.108,
62.268, 62.427, 62.587 and 62.747 seconds. This measures coverage at those
points, not every underlying radius update. The 128-unit sample spacing skips
intermediate changes smaller than that distance.

## Verification scope

Synthetic tests exercise all flag combinations, source spacing/origin, budgets,
straight and diagonal crossings, reverse travel, chunk composition, standing
and moving life regeneration, independent mana, and refusal without pool writes.
Parser and host tests do not prove native or rendered fidelity. Original-map and
compile results are recorded only after their checks finish.

Private-target validation on this increment: `pcx-map` plus `pcx-play` package
tests pass 1,275 / 0 failed / 52 ignored (including ten focused blight-related
tests, one of which preserves the missing-state refusal). Strict all-target
clippy on both crates, the WASM playback library check, and native
`peacecraft3 --all-targets --features test-fixtures` check pass. The selected
Human01 startup/order/profile and Human03 patrol-initialization gates pass.

The selected unchanged Human03 ROC startup/order gate now reaches tick 155,
where its movement work budget exhausts. The previous first-tick missing-blight
refusal is gone. The broader gate still exits 101, so this is progress through
the recreation, not playable campaign acceptance or native eligibility evidence.
