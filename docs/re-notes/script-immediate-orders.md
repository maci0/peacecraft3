# Immediate order behavior

## Selected client lookup and activity-order frontier (2026-10-20)

Fresh-game preparation now binds `OrderId` and `OrderId2String` to the selected
Legacy client's order-registration initializer and its preceding bootstrap registration.
The shared reader follows
the initializer's name pointers, integer operands and common call target, checks
initialized PE ranges and explicit storage/count budgets, and preserves the one
identical repeated registration. Names and IDs are not copied into a table in the
source. Lookup uses the original ASCII case-insensitive comparison and C-string
termination; a missing name returns zero and a missing inverse returns null.
Both loaded constant native contracts must share their declaration source;
registration alone does not admit order execution.

The private original-client probe under
`.scratch/campaign-unit-orders-root-20261020/probe-run-2/` uses two original
Human01 units immediately after `CreateAllUnits`, with an explicit early return
before the rest of `main`. Both accept Hold Position, its repeated request, an
uppercase spelling and its numeric form. The invalid string and null-unit
request return false. `GetUnitCurrentOrder` remains zero immediately and at the
three later timer observations. The bandit moves between Stop and the next Hold
Position request, then retains that position through the last observation. This
qualifies a limited activity distinction, not a general movement or AI rule.
The earlier run produced no records because it remained on the original loading
screen; its screenshot and failure are preserved. Both containers are stopped.

Static dispatch associates the no-target Hold Position admission with the move
ability class. Full unit eligibility, persistent activity, acquisition, movement,
collision-cache updates and current-order lifetime still need a shared world
consumer. **Do not implement `GetUnitCurrentOrder` as the last accepted request.**
The unchanged application remains unsupported at its first Hold Position call;
no playable campaign, rendering or three-target gameplay parity is claimed.

`ObjectHost::with_defend_ability` admits the Defend activation state for one
caller-selected ability alias. Programs without that admission keep the explicit
unsupported order path. The selected alias, code, on/off strings, requirement and
cost/cooldown cells come from the caller's `DefaultTables`; no order table is
embedded in the host.

With this admission, the unchanged selected Human01 completes globals and `config`
and executes both fresh-unit off requests as `false`. It reaches
`SetPlayerAlliance` in `Blizzard.j` at `224476..224550` (selected `common.j` declaration `60256..60399`). It retains 155
source-bound units on the same arena, context and localization. This advances the
previous `IssueImmediateOrder` boundary at `67827..67884`. The object catalog is
still the explicitly selected compatibility catalog; native Legacy archive
priority is unproved. No playable chapter or rendered combat is claimed.
The subsequent [directed-alliance slice](script-player-setup.md#executed-directed-alliance-assignments)
advances the same map to `SetPlayerState`; alliance gameplay remains open.

The recovered baseline gate and the integration evidence remain local under
`.scratch/campaign-host-recovery-20261017/` and
`.scratch/defend-order-root-20261017/`. The earlier advanced private harness
path below is historical and no longer available. Original-client captures
remain separate evidence from executing this map in the Rust VM.

## Source association

The refused unit already retains its resolved `Definition`, including map unit
overrides. Its ability list is metadata field `uabi`, declared `abilityList`,
from `UnitAbilities/abilList`. Read that executed unit's definition rather than
assuming its base unit's abilities or copying rawcodes into an order table.

`Editor` does not load the ability-function profiles. The existing
`DefaultTables::object(Family::Ability, ...).profile(...)` does, preserving every
assignment and its provenance. For this call, exactly one associated ability's
`Unorder` assignment matches the executed string. Its selected member is
`Units\HumanAbilityFunc.txt` from compatibility `War3Patch.mpq`, FNV-1a64
`c4b72f8dcc1cb49e`, profile line 293. The earlier base `war3.mpq` member hash
`698a0abcb11cc53db9896fe6477848cbd0e6be256ce56ebccc61bb5cd9542931`
identifies a different archive read; it is not the selected member's identity.
This catalog policy does not establish native Legacy archive priority.

## Original-client control

An isolated Wine run of the supplied `1.29.2.9231` executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
loaded an instrumented copy of that Human01 map through Custom Game. All 17
retained members were re-read; only `war3map.j` changed. The experiment archive
omits the original attributes member and opaque signature trailer. It makes no
authenticity claim and does not change the source map.

The probe creates two units from the captured rawcode, player and coordinates.
It reads the on/off strings and research requirements from the selected ability
profile. Research level one and a script seed are explicit experiment stimuli,
not inferred defaults. Both units report ability level one. Booleans are encoded
by the probe's explicit true/false branches.

| Sequence | Observed return |
| --- | --- |
| Initial off on each fresh unit, before research stimulus | false |
| On on unit A, after research stimulus | true |
| Repeat on on A | false |
| Off on A | true |
| Repeat off on A | false |
| Deliberately invalid order on A | false |
| On, then off on untouched unit B | true, true |

Two independent runs use actual 3840×2160 and 1280×960 framebuffers/clients at
96 DPI, with fresh private profiles and the identical probe map. Both in-game
marker captures and generated preload files contain these results. The complete
2168-byte output files have the same SHA-256,
`7babfa3325ca938d46ed10e23f912734f78e5be6941d039a1f07231bb1fb1fd0`.
Probe map SHA-256 is
`fa1a7f8bb008073f81a37e72fbd3b63aa39a042ef915d1d5954ea09698c25177`.
Local preparation and source association are retained under
`.scratch/cursor-gameplay-20261016/root-immediate-orders/`; the runtime receipts,
input journal and unchanged captures are under
`.scratch/original-game-vm/orders-control-4k-1/` and
`.scratch/original-game-vm/orders-control-1280-1/`. Both containers were stopped
after capture; their files remain local.

This observes one ability and one sequence under Wine, not general order,
requirement, damage, animation, clock or network behavior. In particular, the
pre-research off results do not isolate the effect of the research requirement.
The output is read as data; the generated preload script is never executed.

## Implementation and remaining behavior

Admission is explicit before any unit creation. It checks the loaded
non-constant `unit, string -> boolean` contract against `CreateUnit`'s source,
then reads one alias row and its exact code. The supported shape is a single-level,
non-hero, non-item ability with authored zero cost, cooldown, required hero level
and level skip, and dependency checking enabled. Missing, blank, marker and
unsupported numeric cells refuse. `Requires` must name one upgrade with exactly
one authored `maxlevel`; an absent or blank `Requiresamount` is supported for
that shape. The required level is read from the upgrade row. Other requirement
shapes remain unsupported. This does not copy an implicit level from a different
parser or infer a missing cost/cooldown as zero.

Creation binds the alias through that unit's metadata-associated `abilList`,
including string map overrides. Alias spelling stays exact. The code is retained
separately from the alias. A unit without that association receives no Defend
state. Each associated unit starts inactive, matching the fresh-unit control.
An inactive off request returns false without a write. For the other supported
transitions, the owner's executed research pair must equal the source-bound
single level. On changes inactive to active; off changes active to inactive.
Those transitions return true. Repeating the current state returns false.
Missing research stays unset, and unsupported levels refuse without a write.
Units remain independent and orders do not resolve or copy the unit catalog.

Unmatched strings stay unsupported until the order registry is bound. In
particular, the original control's invalid-string false result is still an open
part of the general order native. A different valid order must not silently be
classified as a failed Defend request. Damage, speed, animation, mana spending,
cooldown progression, dead-unit admission and event delivery remain open; the
activation bit alone does not apply those effects.

`DefaultTables::from_bytes` supplies the same table/profile reader for explicitly
provided bytes, with caller count, per-input and aggregate limits checked before
parsing/copying. It preserves source identity, member fingerprints and profile
occurrences. Archive priority remains caller policy. Synthetic tests use their
own alias, class, requirement and order strings. They exercise script functions
in an owned VM, with separate malformed-call checks at the host boundary.

The pinned reconstruction source remains a separate implementation reference:
[Warsmash Jass2](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/parsers/jass/Jass2.java)
checks unit abilities for use and no-target admission before dispatching an
immediate order. Its
[active toggle](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/w3x/simulation/abilitybuilder/ability/CAbilityAbilityBuilderActiveToggle.java)
also checks active state, mana and cooldown. These are reconstruction-source
references, not native-code evidence or proof that every profile uses that class.

## Explicit movement and activity consumer (2026-10-20, candidate)

The candidate supplies Hold, Stop and point Move on the existing executed-unit
arena. It decodes their identifiers from three checked comparisons in the same
selected 1.29 client as the order registry. It does not embed an order table.
Unit movement class and base speed come from the metadata-associated source
fields, including map overrides. Unsupported movement classes and active ability
speed modifiers refuse; missing source fields do not acquire defaults.
Nonmoving records do not require a numeric speed to be created. A speed query
still reads their source field and refuses a marker instead of inventing zero.

A point order creates a bounded route against the admitted map pathing and live
occupancy. Movement writes the canonical accepted unit position and replaces only
that unit's collision stamp; static pathing and other units' occupancy remain.
The renderer and range consumer read that record. A location order copies its
finite destination from a location in the same arena. Removing or moving that
location does not retarget the order. No separate world or handle aliases exist.
Hold cancels motion and retains a held state; Stop cancels and releases it. These
operations preserve an idle scripted animation. Move selects the source Walk
sequence, and completion or cancellation selects Stand when a model is present.

The application projects movement at the selected logical timer deadline before
running its callbacks, and through the final requested game time afterward.
Absolute distance from the order's logical start avoids repeated frame-delta
addition. Arrival animation starts at the computed arrival time. Projection
requires the current monotonic game clock and repeated projection at the same
time does no work. The caller stops on a projection error; already executed
unit effects are not rolled back.

A separate bounded original-client experiment used one original Human01 unit
and explicit Move/Hold/Stop requests after the original `CreateAllUnits` call.
The unsigned experiment archive was re-read: only `war3map.j` changed among the
17 retained members, and the insertion/early return can be reversed to the source
script. Its preparation, input journal, 1280×960 captures and fourteen output
records remain local under `.scratch/campaign-unit-activity-root-20261020/` and
`.scratch/original-game-vm/unit-motion-1280-1/`. The private container is stopped.
The output is read as data, never executed.

In this original observation, Move succeeds and immediately becomes the current
order. Hold and Stop succeed, clear that query to zero, and prevent subsequent
position change. A later Move succeeds and becomes current again. The reported
speed remains the selected unit's nominal speed. The original keeps turning
briefly after Hold even though its position has stopped. This is runtime evidence
for that sequence under Wine, not a general native movement specification.

The candidate's instantaneous steering and constant speed do **not** match that
turning/acceleration observation. Its bounded grid route and stable tie policy
are recreation choices; native path choice, local avoidance and replanning are
unqualified. Walk playback rate is the candidate below, not a native fact; autonomous acquisition,
combat/death, speed effects, other movement classes, queued orders and full
original/native/WASM trajectory parity remain open. This candidate is not a playable first mission or a completed Hold/Stop
behavior implementation. Validation receipts must be consulted separately; the
preceding completed registry checks do not validate this new consumer.

### Walk playback rate (candidate, unqualified)

Candidate: a unit moving under a point order plays Walk at `speed / move_speed`.
`speed` is the resolved UnitBalance `spd` that drives displacement. `move_speed`
is the Walk sequence's SEQS ground speed (`pcx_mdl::Sequence::move_speed`). Stand,
Attack, Death and script `SetUnitAnimation` keep the authored clock. A Walk with
zero SEQS move speed keeps the authored clock; movement is still accepted. Nonzero
negative or nonfinite move speed, and nonpositive unit speed, refuse. Walk-to-Stand
arrival selects Stand at the computed arrival time. Reselecting Walk restarts it at
its new rate.

Static facts, not pacing proof:
- The selected Legacy 1.29.2 footman (`Warcraft III.exe` SHA256 3f2ed012...570eed,
  sibling archives read-only) has UnitBalance `spd` 270, SEQS Walk 215 over 800 ms,
  and UnitUI `walk` 210. The footman alone does not separate the candidates.
- Across 421 joined unit-model Walk records (local archive scan and unitUI.slk),
  UnitUI `walk` differs from SEQS Walk `move_speed` in 263. Shipped Walk sequences
  include zero (Murgul BloodGill, Murgul Slave). UnitUI `walk` and `run` are real engine
  columns: their names sit in the executable's unit-field table beside `modelScale`.
- The executable names `SetUnitTimeScale` and `BlzSetSpecialEffectTimeScale`.
  This repo implements neither, so a script time scale has no interaction here.

Candidate probes (not yet captured, plan under local ignored .scratch/walk-capture-plan-1):
- Moon Priestess (Emoo, spd 320, one Walk 634 ms, SEQS 200, UnitUI walk 300):
  predicted period 634 (authored), 396 (SEQS), 594 (UnitUI walk) ms.
- Watcher (nwat, spd 280, one Walk 900 ms, SEQS 250, UnitUI walk 290):
  predicted 900, 804, 932 ms.
The Pit Lord shows SEQS Walk 125 equal to UnitUI walk and Walk Fast 350 equal to
UnitUI run, so SEQS may be the reference that the walk/run fields select among.
The Demon Hunter has a second Walk variant, so it is not a clean probe.

Original-runtime capture (2026-10-20, Legacy 1.29.2.9231, 1280x960 at 96 DPI, one
private Wine run, stopped afterwards). An instrumented copy of the Human01 map
created Emoo (spd 320, scale 2.25) and nwat (spd 280) under explicit point moves,
with a fixed-target camera. Fog and the black mask were disabled so the unit is
drawn. The game records trace identically across two independent runs.

- Emoo was accepted at game clock 1.0 s. It advanced 16 units per 50 ms of game
  clock, consistent with spd 320. The displayed clock moves in 62.5 ms steps.
- Its route was blocked near (-2417, 1794). It stopped at about 3.25 s with the
  move order still current until about 5.25 s.
- The Watcher was accepted at 4.0 s and moved on a detour route until about 8.0 s.
- Frame capture (crop at the viewport centre, grayscale, about 40 fps) shows the
  game redrawing about every 2 to 4 captures. Identical consecutive frames are
  not evidence of stillness.
- The camera-follow background shift is not a pure translation at this crop.
  Phase correlation across the periphery returned inconsistent shifts. Translation
  per walk cycle therefore could not be measured from these frames.

Result: the candidate Walk rate (SEQS move speed, UnitUI walk, or the authored
clock) is not separated by this capture. Its discriminating prediction depends on
a measured screen speed that these frames do not yield. The next capture needs a
static background (for example, a non-following camera on a flat, textured area)
and a wall-to-game clock calibration from frame events.

Obstruction, from the same run's records. Emoo reached (-2417.4, 1794.3) at game
clock 3.25 s and stopped. Its move order stayed current until 5.25 s, then cleared
with no script call in between: the probe issues orders only at 1.0 s and 4.0 s,
and the 4.0 s order belongs to the other unit. Whether the clear was a route
timeout, a path failure or another native cause is not established.

Displacement against `GetUnitMoveSpeed` (spd). The 0.76 ratio in an earlier draft
is withdrawn. It divided endpoint or window-wide distance by the quantized
`TimerGetElapsed` clock over a span that included startup and stops. Two errors
in it: the endpoint is not a path length, and the clock is quantized to 62.5 ms.
The timer period (0.05 s) is the reliable time base: 169 callbacks read 8.5 s.
Using the callback count as time and accepted steps only:

- Emoo moving steps, callbacks 20 to 55: 533.3 units over 35 moving steps and 1.75 s,
  305 units/s (spd 320, ratio 0.95). Stationary callbacks are excluded.
- Emoo, all callbacks 20 to 104: 292 units/s, because the blocked stop (45
  stationary callbacks) is included in the time.
- Watcher, callbacks 80 to 169: 271 units/s over its moving callbacks (spd 280,
  ratio 0.97). The path includes a detour; each step is a chord, so turns shorten
  it slightly.

The per-step chord was 16.0 units early in the walk (320 units/s at 0.05 s) and
about 15.2 later, where the path turns. The 244.5 units/s figure in the earlier
note is the same records divided by the clock span from 1.0 to 2.75 s. That span
includes startup and stopped callbacks, so it is not a moving speed. Speed
comparisons must use moving intervals. Endpoint or whole-window speed cannot
refute spd while stops and detours are in the window.

A static-camera diagnostic (`probe-diag.j`, Human01 source ground, runtime-chosen
clear strip checked against `IsTerrainPathable`, fast and slow units in parallel
lanes, game-clock record every tick) builds and resolves. It is not yet run. The
period estimator on game-clock time recovers the three candidate periods on a
synthetic signal to within 6 ms. That check does not measure real uncertainty.

Sampled Walk poses (scratch harness over `pcx_mdl::bone_matrices` and the runtime's
single-matrix-per-group skin; not a public command, selected Legacy archives,
10 ms steps). Skinned vertices count as moving when any axis differs between
sampled times. Exact equality is the only motion test; no threshold is applied.

- Moon Priestess Walk (634 ms): 471 of 529 vertices move. The contact region is the
  lowest 10% of moving rest vertices, driven by named foot and leg joints. Its mean
  contact height has half-cycle autocorrelation −0.835 and full-cycle 0.962, so it
  repeats once per Walk.
- Watcher Walk (900 ms): 59 of 596 vertices move. Their lowest drivers are `Plane05`,
  `Plane04` and `Object06`, not the named leg bones (`LegLeftLow`, `LegRightLow`),
  which drive no moving vertex. The contact mean height ranges about 10 units over
  the cycle. Half-cycle autocorrelation is 0.290, so footfall multiplicity is not
  resolved for this observable. Joint names are labels, not proof of foot role.

Earlier per-bone foot tracks (BONE_Foot* heights, step 10 ms) are antiperiodic at
half cycle, −0.18 to −0.88. Foot-height tracks built for each candidate rate recover
the candidate periods to within 5 ms. At 2P, the UnitUI-walk candidate scores 0.971,
above its P score of 0.931. This is retained evidence, not a selection rule: a 2P
rival can outscore the candidate lag. Pose and period results are model-plane and
game-clock quantities, not projected pixels. The sampled cycle is the interval
length; the cyclic wrap to the first row is approximate because the end pose differs.

Chord-versus-path on the Emoo's 35 moving callbacks: chord sum 533.3 units, maximum
turn between adjacent chords 51.5 degrees, arc-model shortfall about 0.8 units.
That model does not bound corners between samples, so the corner remainder stays
unresolved.

Static-camera diagnostic, one original run (Legacy 1.29.2.9231, 1280x960 at 96 DPI,
stopped afterwards). The first launch refused: the fixed slow lane was not walkable,
so no units spawned. Its records existed about 45 s after launch, before any key press.
The second run's records appeared only after a key press on the chapter card. The cause
of that difference is unexplained. The begin record and an on-screen message (`PCXDIAG start`) confirm the
probe ran. Fog records changed from enabled and masked to disabled. Both orders were
accepted at game clock 1.0 s. The clock file was rewritten each tick and held 400
tick records by the end.

Not established: unit visibility and the camera. Units were not identified in frame
differences. The camera target read back as the start point, not the requested
midpoint, so the request may not have applied. The camera fields are in radians:
1.222, 5.306 and 1.571 are 70.0, 304.0 and 90.0 degrees. The projection conversion
was corrected. No track or candidate discrimination follows from this run.

The ratio source is therefore undecided: SEQS `move_speed`, UnitUI `walk`, or
another authored field. No native code or original-client capture checks any of
them. The Hive Workshop forum thread
(https://www.hiveworkshop.com/threads/art-animation-walk-speed-and-run-speed.352848)
describes a walk/run comparison but is not evidence. Acceleration, turn rate and
footstep timing remain unqualified.

The bounded consumer's final verification is retained in
`.scratch/campaign-unit-activity-root-20261020/verification-1.json`: 1,378 native
test executions, twenty actual WASI cases, nineteen cases on the actual Cargo
browser-target WASM application under Node, strict checks and both builds pass.
The normal placement producer still matches all 155 original creation requests
and accepted position pairs. The original probe unit's source-backed nominal
speed and explicit Move/Hold/repeat-Hold/Stop controls agree without minting new
handles. This does not compare movement trajectories. The complete unchanged
Human01 application now passes Hold and refuses `ShowUnit` at Blizzard.j:3412;
it publishes no session. The separate standalone diagnostic lacks application
asset loaders and its earlier day/night-model refusal is not the mission frontier.


## Scripted relocation and cinematic cancellation (2026-10-20)

`SetUnitPosition` and `SetUnitPositionLoc` now update the existing accepted unit
record. Location arguments resolve on the same arena; finite coordinates are
required. Visible units with pathing enabled use the existing bounded placement
search, excluding their own occupancy. Preparation leaves the old stamp intact
until success, so a refused placement cannot release collision occupancy. Hidden
or pathing-disabled units retain the requested point. Moving units rebase their
existing route at the current logical clock.

Reusing reveal placement and retaining motion are recreation choices, not newly
measured native teleport rules. Route replanning, native edge placement and all
combat interactions remain unqualified. Synthetic coverage checks collision
avoidance, location resolution, invalid-coordinate refusal, subsequent valid
placement and pathing-disabled exact positioning.

Escape queues registrations of the selected constant
`EVENT_PLAYER_END_CINEMATIC` for the admitted local player. The existing owned
VM event consumer runs their conditions/actions and context. It remains available
when cinematic scripts disable ordinary unit control. The launcher suspension
key is read from the selected `UI\FrameDef\GlobalStrings.fdf` `KEY_MENU`, with
its byte receipt retained; missing/unsupported key text is reported. The old
Escape suspension binding no longer consumes the map's skip event. The camera
integration fixture uses a different function key to prevent a copied binding
from passing the test.

Selected circles consume the existing source selection bands, unit scale and
relation colors, with local-to-owner passive alliances shared with combat. The
renderer uses a flat alpha-blended ground quad with an explicit depth-bias
policy. Diameter interpretation, slope clipping, extension selection and rendered
native parity remain unqualified. Selection settings and material caches retire
with their owning session; failures appear in campaign diagnostics. No original
assets or derived copies enter version control.


Verification for this slice passes 643 playback cases natively and under actual
WASI, 768 application/library/map/audio-session cases natively, and 32 exported
integration cases from the actual Cargo browser-WASM module under Node. Strict
native/browser Clippy and both complete application builds pass. The final
stable-source driver queues four matching local registrations at 20 seconds and
advances Human01 to 300 seconds (19,200 logical ticks) without a runtime error.
The separately labeled compatibility-catalog and diagnostic-main limitations in
its receipt remain; this does not establish chapter completion. A fresh native
Xvfb/Vulkan software-rendered smoke also skips the cinematic into the mission
view/HUD. No fresh original-client or browser rendering comparison was performed.
Receipts remain in `.scratch/campaign-controls-20261020/` and
`.scratch/campaign-time-of-day-root-20261020/controls-confirmed-20261020/`.
