# Returning ordinary campaign guards to their posts

`ReturnGuardPosts` now returns eligible ordinary guards through the shared
canonical post and activity state. It does not create units or treat every
same-owner unit as a guard. The stopped observations below distinguish explicit
return requests from other movement and from released posts.

## Selected-code evidence

The selected Legacy executable is SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Registration `0xab6434` binds `ReturnGuardPosts\0`, `()V\0`, handler `0xab6ca0`
and registrar `0x8ccc80`. The handler resolves the owned AI context and forwards
both reference paths to `0xac1b20`.

That consumer walks the same context `+0x1c8/+0x1cc` post list used by fill.
It resolves each assigned unit through post `+0x38/+0x3c`, requires unit `+0x3a0`
to equal zero, then calls `0xad3b50` with the post and unit. An earlier inferred
`0xad3b60` label was an interior address, not the action entry; retained branch
arithmetic and REA's returned function body correct that label.

The action reads the post's `+0x28/+0x30` pair, performs a position-related check,
then checks a unit component at `+0x3f8` and its `+0x3c` value. On its admitted
branch it builds a point-order packet using source operand `0xd000f`, dispatches
it through `0x690590`, then calls the existing activity-refresh helper `0x699d90`.
The observed order below agrees with that operand. The gameplay meaning of the
`+0x3a0` and component gates is not established here. The position check includes
numeric wrappers and a runtime object; it does not justify copying an observed
arrival distance into a constant.

REA references retained locally:

- Registration: `ev_c34144e58c8e1bbad3ef4c40fffae78262a814202edc1b77386327195d64102f`.
- Handler: `ev_4673ee6e6f5c9f7139416d420ffad2df6e0267f9a3e1a66c03ec750b8a986a68`.
- Post consumer: `ev_bb021574f9dd289a9a912218ebdc278e0f04053006e3d37ccae73dd087b38fc3`.
- Action: `ev_6c815064baf3cd54503a7e5fcfb1357a21ac14eaf9d900a71f7da1fbf715ce5a`.
- Position-related helper: `ev_cc9973f8b18e0c89a17dd145f581e0e2dae53e5bbe08cff0b7e58e5d06daf877`.

The named local inspector checks retained REA bytes against PE section mapping,
hashes each window, and derives the registration, forwarding, action-call and
order relationships. Initial analysis at the handler/consumer entries reported
missing procedure boundaries; those refusals are retained. Action analysis
returned the containing function with assembly and pseudocode. The owned REA
session is closed.

## Matched original-client observation

Three stopped RoC runs use the same authored Human02 grunt, barracks and fortress
creation statements. The diagnostic AI calls `InitAI`, `CreateCaptains` and
`SetCampaignAI`. A resource acknowledgment must arrive before the map displaces
the grunt by a diagnostic +256 X translation. That translation is an experiment
condition, not game data. At sample 10 the map requests one AI-side return call;
the AI acknowledges completion and continues a bounded sequence counter.

The no-return arm omits only that call. The released arm retains the call but
uses `RemoveGuardPosition` before displacement. Map/AI normalization checks the
allowed call and diagnostic identity differences. Each arm samples the same
canonical actor's type, owner, life, order and coordinates, alongside edition,
clock and acknowledgment/query progress.

| Arm | Observed result |
| --- | --- |
| Return enabled | Order changes from zero to 851983, the guard moves toward its original post, then reports zero order again. |
| No-return control | Remains at the accepted displaced position with order zero through the terminal sample. |
| Guard released, return enabled | Also remains displaced with order zero while the AI call and sequence progress complete. |

In the return arm the accepted displaced position is `(-857, 1790)`, versus the
authored original `(-1113, 1790)`. The order is observed at 6.188 seconds; the
terminal settled position is `(-1099.554, 1789.905)`. These coordinates and sampled
times describe this run only. They do not establish a universal arrival extent,
exact polling cadence, or native route algorithm. The actor stays alive with
unchanged owner and life in all arms.

Stopped result SHA-256 identities:

- Return: `75bb24e0d34613a8192a185c8ae357ad479ba5b6bca044d307af56e43545fc59`.
- No return: `ac183acb10c3102bfa951a2319e4974d0d7423c8c95fc3559e9b4180e5cfb02b`.
- Released: `dc0b0fa34695674c928a5064d7d6166d891f85ffdda7c9804cf02ef2a523f02f`.

Maps, scripts, source/build/launch receipts, captures, raw markers and results
remain local and ignored under `.scratch/return-guard-posts/`. All map and AI
sources parse and resolve; packed members round-trip byte-for-byte. The collector
requires the exact bounded marker inventory, stopped source identity, startup and
completion acknowledgment, continuing AI query progress, unchanged actor identity
and life, finite numeric fields and terminal equality. Three synthetic tests,
including malformed-marker and sequence mutants, pass. The first synthetic
positive used an incorrect pre-start sequence value; that fixture was corrected.
The map loading-screen prompt was captured and advanced explicitly in each run.
No screenshot matching or full campaign acceptance is claimed.

## Active-order controls

Six additional stopped RoC runs retain the idle probe's setup, seed and sampling
protocol. They add a source-named `move`, `holdposition` or `attack` call before
the return request, with a matched no-return arm for each. The point-order target
is another diagnostic +1024 X translation, not game data. The source map contains
all three order names. Map and AI sources resolve, archive members round-trip,
and normalized sources retain only the declared order, return-call and diagnostic
identity differences.

The comparison reparses every retained marker and verifies its hash before
comparing outcomes. Move and attack-point arms have identical actor records in
all 100 samples with or without the return call. The move completes and remains
at its accepted destination through the terminal sample. Both attack-point arms
later return toward the post: the matched control prevents assigning that later
movement to `ReturnGuardPosts`.

The hold-call pair reports current order zero before the request. The return arm
then follows the same return leg as the idle guard; its control remains displaced.
The stimulus does not record the boolean result of the hold call, so that result
is not independently qualified by these markers. The collector's initial
assumption that every explicit command must report a nonzero current order was
wrong for this case. Its corrected validation preserves the emitted maps and raw
markers; move and attack-point runs still require continuing nonzero orders
through the request boundary.

| Pair | Return result SHA-256 | Control result SHA-256 |
| --- | --- | --- |
| Move | `d227436fb3efbf38fac611058f8936345d8a38568a7b1b517b339771eb0fad1a` | `affcabfb894fc6332297ffef7ea5b73020fa7fd7733447624a3752a3a18936e2` |
| Hold call | `cf4d116f3bc53d0b83aef7ab365a974d8090d908463f9ac3ae8274d179bbd625` | `9621f5bdbe3dda1e371c1eabb0441c9e41e7dfda6a9daada22db23712480cb54` |
| Attack point | `da26a08b3ffbe0ae4e02acf88ce6948ef9fdc282174ab7d9b2b64b8ed17157aa` | `b7e9573d1fd05f1f46305cf7bca6b8176a8e3228c1242a487a4e881198192b13` |

The three original collector tests and two active-order tests pass, including
retained-marker reparsing and request-boundary mutants. All reference containers
are stopped and preserved.

## Runtime implementation and remaining fidelity

`CampaignModePolicy` now binds the return registration, NUL-terminated name,
owned-context forwarding and action-order operand. It joins the return list and
post-reference fields to those decoded for fill. Nonstock synthetic order data,
relationship/opcode/branch mutants and every truncated new window are tested;
the actual selected image decodes. No measured order or distance is copied into
a production constant.

AI-owned declaration checks admit exactly `ReturnGuardPosts() -> nothing` before
globals or world lending. Dispatch uses the existing active per-player registry
and persistent posts. The runtime checks source-image and attack-order agreement,
ignores released, dead, removed and foreign-owned references, and preserves units
with an existing reported order. An affected idle unit must have admitted ordinary
creation, visibility, pause, ability and effect state. Unsupported affected states
refuse before route publication. All selected routes are prepared and reserved
before committing the batch. The ordinary point-order path supplies motion,
animation, attack-point reporting and normal combat behavior; there is no second
unit world or native packet-layout clone.

The controls also exposed a distinction in background guard activity. A completed
explicit move now settles at its accepted endpoint, while an attack-point leg
can naturally return. A held unit is not automatically pulled home by a normal
guard tick. Explicit return can replace that held idle state. Internal prepared
stops clear held state before a later guard refresh; connected timed-life tests
cover that shared stop path. These use the existing canonical activity and guard
state, without copying native controller storage.

The full core suite passes 1,754 tests, with the actual-image gate run separately.
Coverage includes two-player AI ownership/ABI checks, idle/held return and arrival,
active move and attack-point preservation, explicit-move settlement, release/death/removal/owner
isolation, source mismatch and route-work refusal/retry. These tests do not prove
rendered or exact native movement parity.

The unchanged selected-source application Continue test now admits Human02 and
advances the shared chapter session through 60 game seconds without a script
refusal. It invokes the selected victory BJ as an explicit stimulus, activates
the source Continue button, and checks the new map/context and retained campaign
generation. It does not play Human01 to victory or verify rendered gameplay.

The app library/binary suites (55/638 tests), published-session integration,
selected startup and Continue checks pass. Strict native and WASM Clippy,
formatting and the default desktop build also pass. Failed preliminary compile
and lint receipts remain in the ignored evidence directory beside the final
gates; no blanket warning suppression was used.

The meanings of the native eligibility fields, other controller/ability/effect
states, blocked-route outcomes, ownership-return/post reuse, and exact arrival
extent/polling remain fidelity questions. The implementation reuses the house
point-routing path rather than fitting the observed settled coordinates. Human02
and the full game goal remain under end-to-end acceptance testing.

## Explicit return while hidden (2026-10-26)

The short authored Human02 outro diagnostic reached `ReturnGuardPosts` at host
7.115625008940697 and refused a canonical live `ohun` guard. Diagnostic context
identified AI player zero, unit 2326, home `[-1374.75,1910.25]`, zero current order,
shown false, and no cached, summoned, admitted-order, pause or effect-history
state. Combat was admitted. Only the hidden-state restriction caused the
refusal. This direct-scene run is not earned mission-victory evidence.

A stopped, isolated and muted original-client probe reuses the exact full map,
source grunt/barracks/fortress setup and acknowledged AI return path from the
qualified ordinary-guard probe above. It retains the diagnostic +256 X
displacement. The map hides the displaced idle guard before the first explicit
return request, samples while it remains hidden, shows it without another order,
and later requests a visible return as a control. An independent non-AI peasant
retains its identity, owner, life, visibility, order and position.

All 113 strict records were accepted. The guard was hidden at logical 5.563
with position `[-857,1790]`; the return request at 6.063 was acknowledged. Before
show at 10.063 it was still hidden, had moved to `[-1099.554,1789.905]` near its
source home `[-1113,1790]`, and had settled to zero order. Thus this ordinary
guard's explicit return causes actual motion while hidden, rather than being
ignored or deferred until shown. Those coordinates/times remain diagnostic
observations, not production arrival or scheduling constants. The actual outro
guard is a headhunter; the original probe retains the previously qualified
grunt to isolate the hidden-state variable. No universal class-specific or
paused-return claim follows from it.

The map SHA-256 is
`068901f05d84b68e9e8e6d7a94d01e120901b56f9b7995bf1cb61206e67bc807`.
Raw records, source/build/round-trip receipts, stopped capture receipt and result
remain ignored/local under `.scratch/hidden-guard-return-probe/arm-a/`.

The canonical return handler now allows an otherwise eligible hidden guard to
receive its existing point route. Shared route/motion already supports hidden
stamps: `candidate` preserves the excluded stamp's `active` value, so motion
updates its position without republishing collision cells. No placement or
activity-engine change is needed. A later authored show uses ordinary reveal
placement. There is no new hidden-motion switch, guard store, automatic order
for idle hidden units or visibility/acquisition relaxation. Existing work bounds
and active manual-order exclusions remain. Pause and other unsupported states
still refuse atomically with exact unit/player/home/state context. Native return
arrival extent/polling and unsupported-state semantics retain the earlier
explicit fidelity limits.

Synthetic integration controls cover hidden motion followed by reveal both
mid-route and after arrival, visible collision versus hidden nonoccupancy,
unchanged non-guard public state, and idle hidden units without an explicit
return request. Additional lifecycle controls retain pause refusal/unpause
retry, accepted manual movement after reveal, and post retirement on death,
removal and transfer. Existing placement hidden-stamp tests and return-work
refusal tests remain unchanged. Internal combat observation fields can update
on an unrelated control during projection; the control assertion therefore
compares its public position, owner, shown state, life and order.
