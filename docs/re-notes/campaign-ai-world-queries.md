# Campaign AI world queries

This note records source and original-runtime evidence for the next campaign
gameplay consumers. The private runtime now implements
[the upgrade-level query](campaign-ai-upgrade-level.md) through the shared map
world. The other listed queries and the targeting and production systems that
use them remain unfinished.

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

`CreateCaptains` handler `0xab4750` reaches `0xabd160`, which performs actual
group and home setup. `SetHeroLevels` handler `0xab70e0` retains a code callback
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
