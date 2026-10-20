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
unqualified. Walk playback scaling to actual velocity, autonomous acquisition,
combat/death, speed effects, other movement classes, queued orders and full
original/native/WASM trajectory parity remain open. This candidate is not a playable first mission or a completed Hold/Stop
behavior implementation. Validation receipts must be consulted separately; the
preceding completed registry checks do not validate this new consumer.

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
