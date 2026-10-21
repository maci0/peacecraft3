# Water Elemental: implementation in progress

The implementation binds the learned `AHwe` row, derives its order from the
selected ability profile, and reads each rank's unit id, count, mana cost,
cooldown, duration and Area from AbilityData and scalar map edits. The caster's
UnitWeapons `castpt` delays the effect. These are asset-derived inputs;
the scheduling and placement described below are recreation choices.

Accepted casts stop movement, select the authored spell sequence when present,
and defer creation until the cast point. An accepted replacement order cancels
the pending cast without spending mana. `GetUnitCurrentOrder` reports the bound
summoning order while it is pending. At the effect, the ordinary unit factory
loads the summoned unit's definition and model; mana and cooldown belong to the
caster. Expiry uses the shared death path. Summons are marked dynamically for
`UNIT_TYPE_SUMMONED` and bypass ordinary computer guard assignment. Cache storage
refuses them because that format cannot preserve their remaining lifetime.

Each created unit queues the source-declared unit/player summon events with a
separate context for `GetSummoningUnit` and `GetSummonedUnit`. The context clears
after delivery. The recreation associates these registrations with the caster
and queues spell effect before summon events; native event subject and ordering
are not yet measured. Failures during the shared factory or event queue stop the
session; this is not a transaction that rolls back earlier created units.

The player's primary selected owned unit can cast through the ability profile's
Hotkey. It uses the same immediate-order native and records accepted input in the
session command log. This does not yet draw an ability button in the command panel.
Spell and Birth sequences, when authored, return to Stand after their source
interval, provided no other animation has replaced that selection and epoch.
Native animation blending, cast backswing and pause behavior remain unqualified.

Placement requests the caster's position plus the rank's Area along its facing,
then uses the factory's existing nearest-valid adjustment. The Area relationship
has static native-code evidence below. Native collision adjustment and exact
floating-point/animation behavior remain unqualified. Cast-point scheduling, animation transitions, lifetime pause rules
and event ordering also need original-runtime comparison. Parser or host tests
do not establish rendered or native parity.

The row's BuffID is not installed as a queryable buff, and there is no timed-life
status bar or dispel integration. Generic `UnitApplyTimedLife` remains unsupported.
Those gaps must stay separate from the implemented summon lifetime/death path.

## Initial unchanged Human03 observation (v1)

With selected Legacy client 1.29.2.9231 (SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`),
Human03 SHA-256
`a62e36a1825a2edb0e2259d36db0c44fb50cb877a457014ed568168045559b82`
issues `waterelemental` in `Trig_JainaKillOgre_Actions`. The local diagnostic
creates `hwat`, owner 3, at tick 999 at `[5104, -3408]`. It subsequently stops at
tick 1324 on movement work exhaustion for another unit (handle 830). This
establishes the recreation's source-map cast path, not campaign acceptance or
native summoning fidelity. The ignored receipt is
`.scratch/validation/summon-human03-first.log` in the working checkout.

The Area follow-up's bounded source-map gate still creates the summon at tick
999, now at accepted position `[5313.75, -3458]`. A longer diagnostic with no
injected player movement reaches tick 2570 (game time 40.15625), then refuses
movement work for unit 780. It sent no cinematic-skip input before that failure.
The old diagnostic injected one player movement order, so the later refusal
cannot be attributed solely to the Area change. These are recreation runs on
unchanged map bytes, not original-client observations or chapter completion.
Receipts are `summon-area-original.log` and `summon-area-human03-long.log` in the
ignored validation directory. The long diagnostic exits 101; it is not a green
acceptance gate.

## Native code evidence and remaining questions

On the selected executable, the constructor's instruction at `0xBA5CD3` installs
the method table `0xF45F04`. Its identifier getter at `0xF45F20` points to
`0xBA68D0`, which returns AHwe. Other entries include `0xBA6990` at `0xF46214`
and the shared summon body `0xBA69A0` at `0xF462CC`; the last body also appears
in eight other tables.

The placement scalar is Area, not a copied distance: the name table at
`0x10DCE00` points to the `Area` string at `0xE9EE74`. The loader references that
name at `0x6AD126`, formats its rank suffix, and stores the numeric result at
level-record `+0x18` at `0x6AD152`. Getter `0x46E900` reads that same slot. The
summon body calls it at `0xBA6A01`, then passes its result through `0xCAF240` to
`0xCAF350`. This helper forms the vector `[Area, 0, 0]`, transforms it with the
caster's facing rotation and adds its horizontal components to the caster's
position before factory `0x687CA0` adjusts placement.

The recreation uses the coordinate convention also authored in the selected
Blizzard.j: `PolarProjectionBJ` projects with positive cosine/sine, and
`SetUnitFacingToFaceLocTimed` uses `AngleBetweenPoints`'s degree angle. Synthetic
cardinal/rank tests check that convention and the source distance. They do not
prove native trigonometric rounding or collision adjustment. No measured Area
value or native layout offset is copied into production data.

There is static support for the summon-event pairing: after creation,
`0xBA6A9F` calls `0x68AB70` with the caster object and created unit. That helper
checks the caster's unit-event registration and fires through it, retaining
separate references for caster and created unit. Primary registrations at
`0x49C8AF` and `0x49C8C3` bind the two response natives to `0x496630` and
`0x4965A0`; both handlers check the summon event kinds before reading their
context. The stub registration family also contains both names. This does not
establish runtime registration mode, GetTriggerUnit's full behavior or callback
ordering. The context/model tests remain recreation tests.
Ignored receipts are `summon-native-refs.json` and `summon-native-*.asm` under
`.scratch/validation/` in the working checkout. The supplied IDA C exports are
launcher code, so they do not establish this selected engine's summoning body.
