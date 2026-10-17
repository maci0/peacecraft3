# Source-bounded session players

`ObjectHost::with_players(context, slot_rule, PlayerBudget)` explicitly admits
player identity and slot-setup operations. The Program must be the exact Arc
retained by that admitted map context. The slot rule must be the independently
approved `GetBJMaxPlayerSlots` rule; another getter is not an interchangeable
count. The existing `SelectedMapNeutralHost` checks the private admission witness,
map/W3I integrity, source receipts, header applicability and native signature.
Its selected result supplies the domain size. No conventional slot count,
neutral-player ID or fallback count is embedded in this path.

That source witness remains owned by the player state. The context and compiled
Program cannot be replaced behind the executing VM. Caller limits bound admitted
slots and aggregate retained player-name bytes; platform layout is checked before
state allocation. Zero source slots means an empty domain; negative or over-budget
counts refuse. Input-profile approval and selected-successful-W3I policy retain
the trust/qualification boundaries in
[selected-map-neutral-host.md](selected-map-neutral-host.md). This does not identify
the active native registration family or reproduce its cache/default paths.

`Player(number)` lazily interns one identity for each number in the admitted
zero-based range. `GetPlayerId` returns that number, not the opaque arena token.
Players, timers, groups and symbolic constants share the existing bounded arena;
no second allocator can collide with them. Duplicate lookups allocate nothing,
and capacity failure preserves existing state. Ordinary reserved objects named
`player` do not acquire player state. Tokens remain session-scoped, not global
cross-session authentication.

Executed setters retain team, start-location reference, color, race preference,
race selectability, controller, name and score-screen visibility. Matching
getters return assigned values; unset fields refuse instead of inventing lobby
names, colors, controllers, races or start points. Symbolic arguments must belong
to their exact admitted arena family. Names keep the VM's existing WTS resolution
and exact UTF-8 contents; replacement checks aggregate retained bytes before
allocation and preserves earlier names on failure. This is not a peak-heap bound.

Start references must already exist in executed map configuration. A force-start
command records the forced status and occupied start, as described by the supplied
common script. Later start assignments retain the force marker; this is a
recreation choice pending native late-override traces, not a claim about native
flag storage. Occupancy queries refuse if the player domain or start record is absent; they
do not silently report unknown data as unoccupied. No unit is moved and no
random-placement algorithm is invented.
Team values remain assigned nonnegative identifiers without inferring their range
from `SetTeams`. Invalid inputs refuse; native exceptional-input behavior is
unverified. Exact declarations/constant flags, type ancestry, declaration-source
ambiguity and function overrides are checked before admission.

Current slot state has a separate [explicit startup input](script-player-slots.md).
It does not turn omitted controller assignments into defaults or infer occupancy.
A separate [current-controller view](script-player-controllers.md) can supply
qualified current values while keeping executed controller assignments intact.

## Verification

Fourteen synthetic tests cover both source-rule branches and unsigned metadata,
bounds/zero/negative domains, shared identity interning and family isolation,
real getter/setter state, defined/forced starts, aggregate UTF-8 names, capacity
failure, source/context mutation, missing witness, wrong rule, exact signatures,
source ambiguity and script overrides. The retained owned-VM test initializes
player globals and aliases, executes localized name/team setup, drops input
owners and preserves the context across calls. The synthetic map fixture gained
an optional WTS member with matching input budgets; no production import policy
was weakened.

All 227 native tests pass with five explicit original-input ignores. Strict
native/WASM all-target/all-feature Clippy and WASM library generation pass. Five
actual 32-bit WASM checks pass (mask 31, zero imports): source-derived slot
branches, ownership/localization and layout refusal, typed settings/forced starts,
atomic aggregate-name failure, and cross-context/budget rejection. The first
WASM leaf compile omitted a transitive synthetic fixture helper; restoring the
complete existing helper fixed that harness-only error. Native verification also
caught an initially unused test import before the final run.

The same qualified Human01 context and separately approved rules now execute
player setup in authored `config`. Five player identities and 35 retained field
assignments match captured source calls; the approved rule supplies their domain.
Four timers, three groups and 432 symbolic identities remain intact (444 arena
entries total). The actual run makes 534 native calls including its next refusal:
`SetStartLocPrioCount` at map-script line 7765. Globals remain initialized without
new calls or replacement objects. Map, script, executable and profile hashes
remain unchanged. Actual commands, artifact/input hashes and logs are local in
`.scratch/player-arena-root/`; no original payload, profile, names or coordinates
are published.

This is player configuration state, not complete config/main, lobby membership,
unit ownership/spawning, resources, alliances, slot-state defaults, gameplay or
native handle parity. Start-priority relationships and explicit launcher
profile/Host wiring remain open.


The later [start-priority slice](script-start-priorities.md) executes the remaining
priority statements and completes the same map's authored `config`. The next
actual boundary is `GetCameraMargin` in `main`, at map-script line 7781. Placement
and gameplay remain unverified.

## Player technology maximums

The same retained Human01 Program completes globals and `config`, then stores
36 distinct executed `(player, techid, maximum)` records. Each source player,
signed id and signed maximum matches the actual captured call. Previously
accepted camera, rectangle and sound state remains intact. The next refusal is
`SetPlayerTechResearched` at map span `378455..378507`, declared in the selected
`common.j` at `113299..113406`. This remains script-state evidence; no training,
construction, upgrade effect or playable campaign is established.

`ObjectHost::with_player_tech_limits` admits `SetPlayerTechMaxAllowed` and
`GetPlayerTechMaxAllowed` on players already admitted in the shared arena. Both
declarations are the constant natives `takes player, integer, integer returns
nothing` and `takes player, integer returns integer`. The caller supplies
`TechLimitBudget.max_records` before any new `(player, techid)` record is
allocated. Replacing an existing key does not allocate and still works when the
bound is full. A zero bound refuses the first new key.

Each record keeps the executed signed `techid` and `maximum` on that player
identity. The integers are not treated as FourCC values, categories, or build
rules, and they are not clamped. An unset getter refuses; no default limit is
invented. Storing a maximum does not enforce training or construction. Refusing
a non-integer or wrong-arity call is this recreation's contract check. It is not
a claim that the shipped native rejects that input. Root still owns wiring this
admission into a map run.

## Player technology researched levels

`ObjectHost::with_player_tech_researched` admits `SetPlayerTechResearched` on
the same player rows. The selected `common.j` declaration is the constant
native `takes player, integer, integer returns nothing`, with the third
parameter named `setToLevel`. The selected `Blizzard.j` wrapper
`SetPlayerTechResearchedSwap` only reorders those arguments. Neither source
comment defines a tech category, a clamp, or an upgrade effect.

The caller supplies a separate `TechLimitBudget.max_records` before any new
`(player, techid)` research record is allocated. Replacing an existing key
updates that level in place. A zero bound refuses the first new key. The
executed signed id and level are stored together. They are not treated as
FourCC values.

`GetPlayerTechResearched` returns boolean and `GetPlayerTechCount` returns
integer. Both take `specificonly`. `GetPlayerTechCountSimple` passes `true`,
which does not establish whether that flag is an exact id, a category rollup,
or something else. `GetPlayerTechCount` and `AddPlayerTechResearched` stay
unsupported. The boolean getter can separately consume explicit current answers
as described below; it does not interpret these retained setter levels. An
absent researched pair is not reported as level 0.

The same retained Human01 Program completes globals and `config`, then stores
one researched-level record and 65 technology maxima on the existing source
players. Each retained signed id, level and maximum matches the captured
executed call, with no duplicate stored keys. The prior rectangle, camera and
sound checks still pass. The next refusal is `CreateDestructable` at map span
`58030..58093`, declared in the selected `common.j` at `85292..85437`.
This is script-state evidence, with the previously qualified compatibility
object catalog; it does not establish upgrade effects, destruction objects,
rendering or a playable chapter.

Verification: 377 native tests pass with six explicit ignores, five research
tests execute under WASI, and strict native/WASM Clippy and downstream app
checks pass. The actual map gate validates the retained state and stops at the
next unsupported call. No original data is published.

## Destructable creation records

`ObjectHost::with_destructables` admits `CreateDestructable` on the shared
arena. The selected `common.j` declaration takes `integer objectid`, four
reals (`x`, `y`, `face`, `scale`) and `integer variation`, and returns
`destructable`. That type extends `widget`. The caller supplies
`DestructableBudget.max_records` before a new record is allocated. Each record
keeps the executed integer id, finite reals and variation, plus the
declaration source and span. The id is not resolved through a destructable
catalog, and no model path, life, pathing or scale default is invented.

A non-finite real is refused as this recreation's policy. That is not a claim
that the shipped native rejects it. `CreateDestructableZ` and
`CreateDeadDestructable` stay unsupported. The record is creation state for a
later world or render consumer. It is not a mesh or a placed world object.

The same Human01 Program retains four executed `CreateDestructable` records.
Each integer id and variation, real coordinate/facing/scale bit pattern, source
declaration and shared handle identity matches the captured call. The preceding
camera, rectangle, research and sound checks still pass. The next refusal is
`CreateDeadDestructableZ` at map span `58398..58471`, declared in the selected
`common.j` at `85741..85894`. No destructable catalog lookup, terrain-height
selection, life total or world/render consumer is established by that run.

## Destructable create variants

The same creation record now serves `CreateDestructable`,
`CreateDestructableZ`, `CreateDeadDestructable`, and
`CreateDeadDestructableZ` through one allocation path. `CreateDestructable`
remains required. Each other native is admitted only when the program declares
it, with the same source and the declared parameter list. A small program that
omits a variant still admits the base native, and calls to the omitted native
stay unsupported.

`z` is `None` when the creating native has no `z` parameter, and the executed
finite real when it does. That absence is not a terrain height. `dead` records
that the call used a `CreateDead*` native. It is not a life total, a kill, or
a pathing change. `RemoveDestructable` and `KillDestructable` stay unsupported.
The record is still not a mesh or a placed world object.

The same Human01 Program now retains five creation records, including the
executed dead form with its supplied `z`. All integer values, real bit patterns,
variant declarations and shared identities match the captured calls. Globals
and `config` still complete; `main` next refuses `CreateUnit` at map span
`69421..69469`, declared in the selected `common.j` at `90363..90474`.
The existing camera, rectangle, technology and sound checks still pass.

Verification: 385 native tests pass with six explicit ignores; all five
destructable tests execute under WASI. Strict native/WASM Clippy, downstream
application checks, rustdoc and formatting pass. This establishes executed
creation state, not world placement, rendering or a playable chapter.

## Executed unit creation

`ObjectHost::with_unit_catalog` admits `CreateUnit` against the caller's selected
catalog `Arc`. `MapBindings::admit_executed_units` passes that existing catalog
and the same source identity used for map admission. It does not load another
catalog or infer an installation from a unit id. Record and handle limits are
checked before catalog resolution. Separate retained-field and modification
limits are checked on the returned definition; they do not bound the catalog's
internal allocation or work.

A successful call retains the checked definition, executed player number,
the existing player handle, source identity, and finite x/y/facing values on
the shared handle arena. Integer rawcodes pass through as four big-endian bytes.
Catalog failures keep their typed reason and declaration context. Finite-real
refusal is a recreation policy. No life, stats, model, movement-world id or
rendering defaults are assigned.

Both constant and non-constant loaded declarations are admitted for these read-only
getters; parameter/result kinds, family and source still have to match.
When declared on `CreateUnit`'s source, `GetUnitTypeId` returns that definition's
rawcode as an integer and `GetOwningPlayer` returns the retained player handle.
Neither call allocates, resolves the catalog again, or reads a requested level,
state, position, facing, or order. A reserved unit token and a handle of another
family are refused. Programs that omit either declaration leave that getter
unsupported. Owner changes remain unsupported: there is no admitted setter that
replaces the retained handle.

At the initial creation-only checkpoint, the retained Human01 program created
53 unit records. Their definitions
matched the same selected catalog, and their owners and coordinate/facing bits
matched the executed calls. Globals and `config` completed. The next refusal
was `SetUnitAcquireRange`, map span `68392..68428`, declared in the selected
`common.j` at `92262..92356`. Previous camera, rectangle, technology,
destructable and sound checks remain intact. This uses the explicitly qualified
compatibility catalog from the preceding probes, not established native Legacy
archive priority.

Strict native/WASM Clippy and rustdoc, the play library and application tests,
nine unit-creation cases executed under WASI, and an actual WASM application
catalog-binding case passed at that checkpoint, before the application gameplay
Host factory was added. Those results established executed creation records,
not spawned game entities, rendering or a playable campaign.

The unit identity follow-up (2026-10-18) passes 29 unit-creation tests, 423 native
and 423 actual WASI play tests (six explicit ignores per target), and 95 map-loading
tests. Strict native/browser Clippy, the browser application build, rustdoc and
formatting pass. The original Human01 production factory admits the selected
`common.j`'s constant getters. Both getters return the exact executed rawcode and
original owner handle for all 155 created units, without issuing another identity.
Globals and `config` complete; `main` still refuses `GetRandomReal` at
`186558..186584`. This uses the retained compatibility catalog and explicit startup
observations, with no original-client launch or automatic clock advance. Native
Legacy archive priority, world association and gameplay/render parity remain open.
Evidence: `.scratch/unit-identities-root-20261018/review-1/acceptance.json`.

## Executed unit acquire range

An admitted `SetUnitAcquireRange` writes its executed finite real onto the
existing unit record. The optional declaration must share `CreateUnit`'s source
and have the loaded `unit, real -> nothing` contract. Units without an assignment
retain `None`; an executed zero is `Some(0.0)`. The setter does not resolve the
catalog, allocate another record, change the definition or implement aggro.
Finite negative values are retained, with no guessed clamp or default range.

The existing `executed_unit` accessor exposes this state. Wrong-family, unknown
and non-finite calls preserve the prior value. Mutable arena access constructs
an error only on failure. `GetUnitAcquireRange` remains unsupported.

The same Human01 program retains one range assignment with exactly the captured
real bits and 53 unit creation records. Globals and `config` still complete.
`main` next refuses `CreateTrigger` at map span `68442..68459`, declared in the
selected `common.j` at `69768..69821`. The earlier sound, camera, rectangle,
technology and destructable checks still pass. This is script-state evidence;
there is no active gameplay scene or aggro behavior from this change.

Verification covers the current play library and application tests, 12 executed
WASI unit cases, strict native/WASM Clippy, rustdoc and the actual retained-map
run. No original game bytes are published.

## Executed hero-level request

When declared with the loaded `unit, integer, boolean -> nothing` contract
and the same source as `CreateUnit`, `SetHeroLevel` retains the executed signed
level and eye-candy flag on that existing unit. Both fields stay absent until
that call runs; zero and false remain explicit assignments. Wrong-family and
unknown handles refuse without mutation. Programs without the declaration
keep unit creation support. No catalog lookup or allocation occurs in the setter.

This records a request. It does not derive experience, hero stats or visual
effects, classify heroes from rawcode spelling, or supply `GetHeroLevel`.

The same Human01 probe retains one request matching the captured call, alongside
153 unit records and three trigger actions. Globals and `config` still complete.
The next refusal is `SetUnitState`, map span `67684..67742`, declared by the
selected `common.j` at `91204..91315`. No gameplay scene or playable chapter
is established. Sixteen unit cases run under WASI; the full play and application
tests, strict native/WASM Clippy, rustdoc and browser-target library build pass.
Root evidence is in `.scratch/cursor-gameplay-20261016/root-hero-level/`.

## Executed unit-state assignment

`ObjectHost::with_unit_states` admits the selected `SetUnitState` contract and
`ConvertUnitState` domain on the existing unit arena. Each assigned pair keeps
the symbolic handle and finite real. An integer argument uses the VM's existing
real conversion. Zero stays distinct from an absent assignment; negative finite
inputs are retained without a guessed clamp or maximum. The caller bounds distinct
pairs before allocation, while replacing a retained pair updates it in place.
Programs that omit this opt-in keep unit creation. Wrong-family, unknown and
non-finite calls leave prior assignments unchanged. No state ordinal table is
embedded, and no `GetUnitState`, resource spending or combat consumer is added.

The same retained Human01 Program completes globals and `config`, retains one
assignment matching the captured domain handle and real bits, and reaches 154
source-bound unit records. The next refusal is `IssueImmediateOrder`, map span
`67827..67884`, declared in the selected `common.j` at `106038..106124`.
The compatibility catalog remains explicitly selected; native Legacy archive
priority is unproved. The previous sound, camera, rectangle, technology,
destructable, hero-request and trigger-registration checks still pass.

Verification: 305 play tests pass with three explicit ignores; 417 application
tests pass with fifteen explicit ignores. All 22 unit cases execute under WASI.
Strict native/WASM Clippy, rustdoc and browser-target library build pass.
This establishes executed state, not a gameplay scene or playable chapter.
Root evidence is in `.scratch/cursor-gameplay-20261016/root-unit-states/`.

The subsequent [immediate-order slice](script-immediate-orders.md) joins the unit
to its authored ability. Original-client controls establish observed on/off and
repeat results at two actual resolutions. Explicit Defend admission now retains
activation state and advances the unchanged-map VM to `SetPlayerAlliance`;
combat modifiers and general order behavior remain open.

## Executed directed alliance assignments

`ObjectHost::with_player_alliances(PlayerAllianceBudget)` admits only the loaded
non-constant `SetPlayerAlliance(player, player, alliancetype, boolean)` contract
on existing players. The setter and selected constant `ConvertAllianceType`
must share `Player`'s declaration source. The symbolic family and its nonempty
source-derived domain are checked; no alliance ordinal table is embedded.

Each source player's existing `PlayerSettings` stores a sparse tuple of target
identity, setting handle and executed boolean. The target and setting are shared
arena identities. Explicit false differs from an absent tuple. Directions are
independent, including when players share a team; no reverse assignment or default
matrix is inferred. The source BJ wrapper's self guard is not imposed on direct
native calls. The aggregate caller bound is checked before allocating a new key;
updates at capacity reuse the retained key. Invalid arguments, wrong or unknown
handles, out-of-domain settings and capacity failures preserve earlier records.
No new arena identity is allocated by the setter.

The unchanged Human01 completes globals and `config`, then retains 26 directed
alliance tuples matching its executed calls. Previous state checks still pass:
155 units, 142 rectangles, three unit events and seven stacked sound requests on
the same Host/context. The arena contains 843 identities. The next refusal is
`SetPlayerState` in `Blizzard.j` at `225072..225136`, selected `common.j` declaration
`113953..114062`. The explicit compatibility catalog still does not establish
native Legacy archive priority. Local commands and source/input stability receipts
are in `.scratch/alliance-root-20261017/`.

The owned-VM regression drops input owners before execution and checks alias
identity, separate directions/settings, explicit false, same-team independence,
updates at capacity and atomic failure of a new tuple. Host-boundary cases cover
zero budget, malformed arguments, unknown handles and wrong families. This is executed assignment state;
`GetPlayerAlliance`, initial-state policy, shared vision/control, targeting rules,
multiplayer enforcement and the application Host factory remain open.

## Executed player-state requests

`ObjectHost::with_player_state_requests(PlayerStateRequestBudget)` admits the
loaded non-constant `SetPlayerState(player, playerstate, integer)` contract on
existing player identities. The setter and selected constant `ConvertPlayerState`
share `Player`'s declaration source. Symbolic kind and source-derived domain
membership are checked; state ordinals and starting balances are not embedded.

Each player row retains the last executed signed integer request for each setting
handle. A stored zero is an executed input; absence means no request. Distinct
players and settings stay independent. New pairs respect the aggregate caller
bound before allocation, while updates at capacity reuse the existing pair.
Malformed calls, wrong/unknown handles and settings outside the selected domain
leave prior records unchanged. The setter allocates no arena identity.

These are requests, not effective balances. State-specific clamps, resource
limits, bounty effects and initial values remain unimplemented. `GetPlayerState`
stays unsupported rather than returning a request as an effective value. Its
declaration is not required by this setter-only admission. The earlier broad
assignment/query planning lease is not evidence that those behaviors work.

The unchanged Human01 retains its one bounty-setting request with the exact
executed integer, along with all 26 directed alliance tuples. Globals and `config`
complete. Previous checks still retain 843 identities, 155 units, 142 rectangles,
three unit events and seven stacked sound requests on the same Host/context.
The next refusal is `Filter` in `Blizzard.j` at `436855..436900`, with selected
`common.j` declaration `71869..71927`. Local source/input stability receipts and
commands are in `.scratch/player-state-root-20261017/`. This remains the explicit
compatibility catalog, not proof of native Legacy archive priority or a playable
chapter.

The owned-VM regression drops input owners and checks zero versus absence, signed
request preservation, alias identity, independent rows, updates at capacity and
atomic new-key refusal. Host-boundary cases distinguish a state handle from a
color with the same enumerand, reject another conversion's out-of-domain state,
and preserve records on malformed/unknown handles. Native and actual WASI tests
exercise the same request path. Application Host construction and the effective
resource consumer remain open.


## Explicit current boolean research answers

`ObjectHost::admit_current_research` accepts caller-qualified
`ResearchObservation` records on the exact admitted `ParsedMapContext` Arc.
Each record contains a source player number, signed tech id, `specific_only`
flag and boolean answer. The complete query triple is the key. A true answer
for one flag does not provide an answer for the other flag. No numeric level
predicate, category equivalence, clamp, default or upgrade effect is inferred.

Admission checks the exact constant boolean getter and constant research setter
contracts, rejects script overrides and ambiguous source declarations, and
requires their source to match `Player` and `CreateTimer`. The explicit
`TechLimitBudget.max_records` bounds current query records independently of
retained setter records. Source player-domain, duplicate-key and capacity checks
precede publication. The owned vector is sorted without another allocation.
Invalid admission leaves the previous answers intact; admission does not create
player identities and can provide answers for later lazy `Player` calls.

Missing queries refuse, including after an explicitly empty admission.
`invalidate_current_research` clears answers without falling back to setter
inputs. A successful `SetPlayerTechResearched` keeps its existing signed input
record and invalidates all current query answers for that player. Other players'
answers survive. The whole player is retired because cross-tech/category rules
remain unknown. Failed setters preserve current answers. Refresh and invalidation
are the caller's responsibility for external runtime changes; context identity
does not authenticate an observation. Without this opt-in the getter remains
unsupported. Counts, additional research, simulation effects and a live provider
remain unsupported.

### Qualified original startup observation

A bounded local Wine/Xvfb probe uses the same qualified Legacy executable
`1.29.2.9231` (SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`),
Human01, common and Blizzard inputs retained by the campaign gate. Its readonly
queries come from the selected `InitSummonableCaps` body, and its player range
comes from the source's `bj_MAX_PLAYERS`; there is no measured query/player list
in production code. Instrumentation adds calls before and after the original
`InitBlizzard`, preserving the complete original main/config and all other map
members. The early authored research assignment has already run at this point.

The complete output contains 50 probe records: begin, 24 queries before
initialization, the same 24 after initialization, and end. All answers are false
for these two exact technology queries across these 12 source players. Their
before/after equality is an observation for this run, not a false default or
proof about other technology ids, flags, clients or saved state. No setter,
level conversion, cost or equivalence behavior was measured by this probe.
The private reference container was stopped and preserved; no assets were
uploaded.

The unchanged Human01 VM refreshes this source-bound input once at the first
research query, after its actual early research setter. Refresh allocates no
identities and is followed by the ordinary production Host dispatch, with no
fabricated native success or second VM. All 24 executed query keys and booleans
match the original observation. The loop completes and cumulative
`SetPlayerTechMaxAllowed` calls reach 101. The prior eight bound getter results,
both BJ rectangles and all earlier shared-arena assertions remain intact:
903 identities, 144 rectangles, 155 units, nine triggers, three unit events,
one timer subscription and six game-state subscriptions. The next refusal is
`SetAllItemTypeSlots`, Blizzard span `446992..447041`, common declaration
`109983..110054`, with one executed integer argument. No item-stock state is
invented.

Three new synthetic regression groups cover owned-VM reads and lazy players,
independent booleans/flags and signed ids, no handle allocation, retained setter
levels, empty/invalidation behavior, malformed/wrong-family/unknown handles,
atomic admission/budget/context/source failures, and successful versus failed
setter invalidation. All 373 native and 373 actual WASI play tests pass, with
five explicit original-input ignores; strict native/browser Clippy passes.
Evidence and command receipts remain local under
`.scratch/player-current-research-root-20261017/`. This does not provide an
application Host factory, live research simulation, item-stock behavior or a
playable campaign.

The research gate above stops before shop stock. Executed slot configuration
is the following slice: `script-stock-slots.md`. The BJ slot integers are
not production constants.

The following same-context continuation retains both stock slot assignments
and reaches `TimerStart` in Blizzard.j (`447183..447286`; common.j
`61987..62103`). The newly executed stock timer shares the existing arena.
This is assignment retention, not stock inventory or restock execution;
see [the stock slice](script-stock-slots.md).
