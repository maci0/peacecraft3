# Campaign targeting observations

Two matched diagnostic maps ran on the reviewed Legacy 1.29.2.9231 executable,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
with `-classic`; every marker reported RoC. The programs called
`SetTargetHeroes(true)` or `SetTargetHeroes(false)`, then `SetUnitsFlee(false)`.
They did not run `CampaignAI`, `InitAI` or `CreateCaptains`. These observations
therefore concern that limited setup, not an executing campaign strategy.

Each map created one player-zero `ogru` attacker and two hero/two ordinary
player-three targets. Geometry and creation order matched between maps. The
attacker was paused before targets existed; the attacked-event observer was
registered on the target owner before starting the program. The script paused
all targets, started the program synchronously, then unpaused the attacker.
There were no forced attack orders. The event callback filtered the exact
attacker handle and recorded target user data; periodic samples separately
recorded the nearer targets' life and pause state.

| Measurement | Targeting enabled | Targeting disabled |
| --- | ---: | ---: |
| First observed attack time | 0.188 | 0.188 |
| First target | nearer hero, user data 10 | nearer hero, user data 10 |
| Attack events through 20 seconds | 13 | 13 |
| Sampled nearer-hero life at 20 seconds | 453 | 453 |
| Sampled nearer-ordinary life at 20 seconds | 680 | 680 |

Both runs produced 200 periodic samples and all 219 expected phase, load,
sample and attack files. A player-zero bank write from zero to the diagnostic
watermark 17 preceded the map's after-start marker; the other player's bank
remained zero. The program's load marker reported the requested flag. Its
`I2S(GetAiPlayer())` field was empty in both runs; the collector retains that
text without interpreting it as a number. The independent bank witness remains
separate from this unresolved string-conversion observation.

The measured attacks did not differ between flags. This does not establish
that the native is inert, that heroes are never avoided in campaign combat,
or that any internal unit list was empty. The attached-unit state and the
consumer of the policy were not observed. The production native remains
unsupported until its world behavior has a faithful consumer.

The true/false map hashes are respectively
`5f7984b851e42db9952e670eabcdbce1990111957e496dd269d6ae3c76f2b7b3`
and `cbd0602955d2edaa913497c844decb35422386f725cff7625e7a1b099fe31d9c`.
Script parse/resolve and every packed-member roundtrip passed. The recorder
checks source hashes, actual Preload wrappers, filenames, phases, load bank,
edition, sample/event sequence and output bounds; its two tests exercise both
labels, empty player text, no-attack output and eight independent refusals.
The scripts disable the observer and pause the attacker at completion, with
explicit sample/event caps. Raw files, hashes and all source/build/launch
receipts remain local in ignored `.scratch/campaign-targeting-run/`.
Rendered presentation and native Windows remain unqualified.

## Equal-center-distance comparison

A second pair used the same reviewed client, RoC readback, player-zero attacker,
paused targets and flag-only AI programs. The near hero and near ordinary unit
were each 120 units from the attacker's center: hero east, ordinary north.
Their different collision shapes, facing and creation order were retained and
matched between the true and false runs. No target order was issued. The AI
programs still made no explicit `InitAI` or `CreateCaptains` call.

| Flag | First attacked target | First observed attack | Attacks through 20 seconds | Final near hero / ordinary life |
| --- | --- | --- | --- | --- |
| true | hero, user data 20 | 0.188 | 13, all hero | 453 / 680 |
| false | ordinary, user data 22 | 0.313 | 13, all ordinary | 650 / 449 |

Each run validated 219 marker files, including 200 sequential samples and one
AI load witness. The player-zero bank changed from zero to 17 synchronously;
the other bank stayed zero, both target pause readbacks stayed true, and the
recorded flag matched its program. `GetAiPlayer` text remained empty and is
preserved without interpreting it as an integer. Both owned runs are stopped
and preserved: `targ-equal-r2-true-261023r` and `targ-equal-r2-false-261023r`.

This pair establishes an observable automatic target-choice effect in this
configuration without an explicit captain setup call. Together with the
earlier nearer-hero pair, it supports a priority effect rather than blanket
hero exclusion. It does not establish all score terms, distance/range ordering,
attachment classes, transfer behavior or acquisition while already fighting.
Equal center distance is not equal collision extent or equal turning time.

True map SHA-256:
`dfed03d2d43cb159d5bde3971df50afc102dbd3a5c6198b5e7da08e9e61c1933`;
false map SHA-256:
`f4d1cc6888dc4eb552618eb2dab404b006dbd8c6f8f049ee7026089a4c65f063`.
Build receipts pin each script and embedded AI. All 18 members round-tripped,
script parse/resolve and callback declaration checks passed, and four recorder
tests cover actual emitter shape, terminal caps and malformed collections.
The earlier equal-distance draft omitted the bounded emitter changes and was
not launched. Raw records and receipts remain local under
`.scratch/campaign-targeting-equal-distance-r2/`.

## Current difficulty input to the native scorer

In the selected image, the scorer reads an unsigned byte at
`[[0x113b6f4]+0x30]+0x46`. The registered `GetGameDifficulty` handler `0x4927a0`
reads exactly that byte, and the registered `SetGameDifficulty` handler `0x4a8730`
writes it. `GetAIDifficulty(player)` instead reads a separate player field at
`+0x2cc`. The score gate therefore follows current game difficulty, including
setter changes, rather than a second AI-difficulty store.

The scorer can promote the local gate value under additional conditions without
writing the game setting. Those remaining conditions and other score bands still
need qualification. The current `ObjectHost` already admits the selected
`gamedifficulty` symbolic domain and current handle; a consumer should reuse it
and derive ordinal/branch relationships from the checked source. This byte trace
does not establish every startup lobby-flag mapping or the complete target score.
Pinned windows and relationships remain local in
`.scratch/campaign-targeting-root-review/band-enable-setting.md`.

## Creation-order discriminator with targeting disabled

Three seeded runs retained the equal-distance setup and false policy. Both the
map and AI main functions set the diagnostic seed before other calls. The control
created hero, far hero, ordinary, far ordinary; a second map reversed those four
creation calls without moving them; a third kept creation order and exchanged
only the two near positions (east and north).

| Variant | First attacked target | Attack events | Samples |
| --- | --- | --- | --- |
| control | ordinary, user data 22 | 13, all ordinary | 200 |
| reversed creation | hero, user data 20 | 13, all hero | 200 |
| exchanged near positions | ordinary, user data 22 | 13, all ordinary | 200 |

Each run has 219 raw marker files, RoC readback, the independent bank-17 load
witness and paused-target samples. A separate read-only audit matched original
marker files to preserved copies and hashes, parsed their complete Preload
wrappers, rejected duplicate payload keys, recomputed the collections, and
checked staged map/executable identities and all 18 packed members. Script/AI
comparisons allow only run identifiers, diagnostic seeding, the sample cap and
the stated creation/position changes. The executable is the same reviewed
1.29.2.9231 image above. All three containers are stopped. Loader-view evidence
is absent for the reversed and exchanged-position runs; their complete in-game
markers establish execution independently.

The map hashes, in table order, are
`59929dc0e3008afbebf4b18a8523cdedd618045aa7bc3636a36e9c4219ff9428`,
`878c6b8476a0208905af92274aa69619bbcad38e02ac42776051d80253ab089f`,
and `bb5cf20229a09ce8a15ec145a256f4e1bade1b45c34d1be3119dcc20d0c14b24`.
Original observations remain local under
`.scratch/campaign-targeting-enumeration-r1/`; the independent audit is in
`.scratch/campaign-targeting-enumeration-audit/`.

Within this setup, reversing creation order changes the tied choice and exchanging
the near coordinates does not. This supports newest-first enumeration among the
tied near candidates. It contradicts a fixed ordinary-before-hero rule; it does
not establish the native container, every spatial query's ordering, removal or
transfer behavior, later retargeting, or score equality outside this envelope.
The production consumer remains unfinished. Older notes proposing a class-based
false-policy tie rule are superseded by this observation.
