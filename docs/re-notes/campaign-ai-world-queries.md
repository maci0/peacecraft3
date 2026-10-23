# Campaign AI world queries

This note records source and original-runtime evidence for the next campaign
gameplay consumers. The private runtime now implements
[the upgrade-level query](campaign-ai-upgrade-level.md) through the shared map
world. Gold and wood costs also read the admitted catalog and canonical player's
hero-token state. The remaining query envelopes and production systems are
unfinished; the tables below record bounded source and runtime evidence.

The later [campaign-mode integration](campaign-ai-mode.md) forwards `GetPlayers`
through the existing map-configuration host. It reads the map's `SetPlayers`
value, with no copied slot count or separate AI state. The published two-player
fixture checks that configured count before and after an AI sleep.

## Selected native paths

The reviewed executable is 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Local registration receipts and bounded disassembly are retained under
`.scratch/campaign-economy-native/` and `.scratch/campaign-tactics-native/`.

| Native | Handler | Observed source path |
| --- | --- | --- |
| `GetUnitGoldCost` | `0xab5430` | Ordinary IDs reach data query `0x6939c0`; uppercase-leading IDs first check player state and can return zero. |
| `GetUnitWoodCost` | `0xab54c0` | Corresponding player-dependent branch before data query `0x6971a0`. |
| `GetUnitBuildTime` | `0xab5260` | Ordinary IDs reach `0x67c3d0`; uppercase-leading IDs instead reach context query `0xabe900`. |
| `GetUnitCount` | `0xab52e0` | Adds the context query `0xac0ad0` to the player count with mask `0x36`. |
| `GetUnitCountDone` | `0xab53a0` | Player count with mask `0x30`. |
| `GetPlayerUnitTypeCount` | `0xab5190` | Resolves the supplied player and uses mask `0x36`. |
| `GetUpgradeLevel` | `0xab5570` | Resolves the running context's player, then queries its upgrade state. |

The count wrappers reach `0x4bd4a0` and its `0x513620` accumulator. The
meaning of every count-mask bit, queue classification and equivalence rule is
not established here. A count of currently executed units alone does not
establish complete construction or training behavior.

`SetTargetHeroes` registration `0xab68b4` selects handler `0xab75e0`, which
calls `0xac36e0`. It changes bit `0x200` at context offset `0x2d0` and visits
attached unit-AI handles through `0x6933c0`, setting or clearing bit `0x20` at
their offset `0x28`. `SetUnitsFlee` handler `0xab7630` changes bit `0x10` at
the context's `0x2d0` field. These writes do not establish target preference,
avoidance or retreat behavior by themselves; the recorded bit-test candidates
remain incomplete evidence for the gameplay policy. The
[matched targeting experiment](campaign-ai-targeting.md) ran successfully but
observed no difference in its limited nearer-hero setup; it does not qualify a
targeting implementation.

`CreateCaptains` handler `0xab4750` reaches `0xabd160`, which creates two typed
object references and supplies scalar pairs. Their gameplay group identity is
unqualified; see the later [captain target evidence](campaign-ai-captains.md).
`SetHeroLevels` handler `0xab70e0` retains a code callback
at context offset `0x40`; invoking that callback requires its owning program,
rather than forwarding its function ID into the map VM. The other registration
table's `0xcbda30` stub remains unqualified and is not edition evidence.

## Controlled economy and lifecycle experiment

A Human02-derived diagnostic ran with `-classic`, with explicit RoC readback.
It retained the original configuration, unit edits and terrain, omitted authored
unit/trigger initialization, and used an empty placed-destructable member.
The first computer slot came from the original configuration. Its independent
AI program wrote each query triple to that player's gold, lumber and gathered
lumber, then wrote a phase ID last. The map sampled the completed phase before
changing the next stimulus. Three created footmen were independently checked
for non-null handles and their type; later samples also recorded the killed
unit's life, removed unit's type, transferred unit's owner and hero-token value.

All 14 bounded markers qualified. Five synthetic collector tests passed,
including incomplete phases, failed creation, failed lifecycle stimulus and
oversized payload refusal. The map hash is
`6d502e56eddf564a5b563a7a777863dab6b0edb810e6fa3bd962cb9bf44e5461`;
the script hash is
`8d637b94ea1248daa066a053a24dece07f9040f5120720b1babca022dd619bd2`.
The source/script/AI/map hashes, extracted member checks, typed records and
raw markers remain in `.scratch/campaign-economy-probe-r2/`. The owned run
`/home/maci/.cache/pc3-probe/aecon-261023b` is stopped and preserved. An earlier
offline draft without independent lifecycle witnesses remains separate and
was not launched.

| Query stimulus | Gold cost | Wood cost | Build time |
| --- | ---: | ---: | ---: |
| Ordinary `hfoo` | 160 | 0 | 20 |
| `Huth`, hero tokens 0 | 500 | 100 | 0 |
| `Huth`, hero tokens 1 | 0 | 0 | 0 |
| `Huth`, hero tokens reset to 0 | 500 | 100 | 0 |
| Unknown ID 0 | 0 | 0 | 0 |

The retained `Custom_V0` UnitBalance witness has the same ordinary costs and
time; its `Huth` build-time cell is 55. The observed hero query is therefore
not an unconditional read of that cell. No altar, training queue or owned hero
was present, so this run does not establish the result for a trainable or
reviving hero. The token experiment establishes observable player-dependent
cost behavior, not every internal condition in that branch.

| Lifecycle stimulus | `GetUnitCount` | `GetUnitCountDone` | `GetPlayerUnitTypeCount` |
| --- | ---: | ---: | ---: |
| Three completed `hfoo` units | 3 | 3 | 3 |
| Kill one | 2 | 2 | 2 |
| Remove another | 1 | 1 | 1 |
| Transfer the last to the other player | 0 | 0 | 0 |
| Unknown ID 0 | 0 | 0 | 0 |

For `Rhlh`, `GetUpgradeLevel` and the explicit own-player
`GetPlayerTechCount(..., true)` both returned zero initially and one after
setting that player's level to one. The other player's explicit query returned
two after its independent level assignment. This distinguishes the running
program's player from a global or last-written level.

These results cover completed ordinary units and the named hero/token fixture.
Training and construction queues, custom-ID equivalence, upgrades' gameplay
effects, hero revival/production, signed or overflowing query results and
native Windows behavior remain open. Production values must still come from
the selected catalog, metadata and player state; the observations above are
evidence, not constants to copy into the implementation.

## Canonical world count implementation

`GetUnitCount`, `GetUnitCountDone` and `GetPlayerUnitTypeCount` now query the
existing canonical unit arena. They match the current exact type and owner,
exclude dead/removed units, and include hidden live units. Unknown type IDs and
a null explicit player return zero. Invalid player identities, unavailable
liveness and nonfinite life refuse with the query name. Queries do not mutate
units, handles, clocks or random state. Owned queries use the running AI's
canonical player; an explicit player's query does not require that player to
have an AI program. Owned AI-common declaration and argument checks happen
before world execution.

All currently accepted factories create completed units, so total and completed
counts coincide in this supported world. The dispatch retains the completion
mode separately; production/training/construction natives still refuse, and
their future pending work must feed these queries rather than a second count
cache. This is a recreation using shared world state, not a copy of native
counter tables.

The selected-image REA review retained under `.scratch/ai-unit-counts/` confirms
the distinct handler paths listed above. In `0x513620`, masks `0x30` and `0x36`
select different backing counter contributions; `0xac0ad0` also walks attached
unit records matching the requested type. The exact lifecycle meaning of every
native backing counter remains unqualified. This static review supplements the
earlier completed-unit runtime experiment; no new original-client run was made.

Synthetic lifecycle tests cover death, removal, transfer, hiding, unknown type,
unknown liveness and query immutability. Dispatch fixtures deliberately return
different total/completed values to check forwarding. The published two-AI
session checks owner separation and all three queries before and after sleep.
Validation retained in `.scratch/ai-unit-counts/gates/`: 1,691 core tests,
55 app-library and 638 app-binary tests, the published fixture, selected Human01
startup and victory Continue, strict native/WASM Clippy and default desktop
build pass. The existing campaign-mode image gate also passes; it is not a new
count-policy decoder. The unchanged Human02 opening-minute test remains red,
now at `IgnoredUnits` at 7.312500 seconds rather than `GetUnitCount`. Full
campaign AI and pending production behavior remain unfinished.
