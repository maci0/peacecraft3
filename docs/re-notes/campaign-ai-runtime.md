# Campaign AI runtime

## Selected-source audit (2026-10-23)

The private campaign continuation still refuses `IssueImmediateOrder` during
initialization, before its later `StartCampaignAI` calls. The source path next
reaches nearby-lumber harvesting. The source audit below is distinct from the
private execution foundation described in this note.

The audit uses the existing `CampaignRecipe` reader with the reviewed
1.29.2.9231 executable
(`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`),
the archive-selected `common.j`, and the RoC edition declaration. Human02
selects `Custom_V0`. The winning AI members all come from the pinned base
`War3x.mpq`; their presence in another archive does not establish precedence.

| Member | SHA-256 |
| --- | --- |
| `Scripts\common.ai` | `29f51929ff3c09988a77748dbefea319b72b32bf4bb1a5f6997760a621454818` |
| `Scripts\h02_red.ai` | `6f2dc10002d0591e7fa3c72a2b6d01b114fe7b897c9cc1eaaafb8f67654f205f` |
| `Scripts\h02_purple.ai` | `04fa7ec5b7509dd15597768f00e99da9692c58651cab5a2211fc8ecb1f7551ff` |

Both AI entrypoints compile through the existing `MapScripts` compiler with
the selected `common.j` and `common.ai` as preludes. This establishes parsing
and name/type resolution, not native support, thread scheduling or gameplay.

The selected red script reaches the library's `CampaignAI`, which initializes
AI state and captains, sleeps, and starts the campaign-basics and construction
threads. Its later defense and assault calls require actual unit production,
group formation and attacks. The purple script installs a hero-level callback
and sleeps. Accepting these entrypoints without those consumers would not
establish working campaign AI.

The runtime must keep each player's AI globals and continuations separate
while using the session's existing world, handles and deterministic clock.
The existing script VM is the execution path to reuse. Shared-world ownership,
AI-native gameplay behavior and cross-program callback invocation remain open.

## Private execution foundation

The private continuation now loads the image-derived common, AI-common and
requested script paths through the retained campaign reader. Exact lookup
precedes the image-derived fallback prefix; only an absent member permits that
fallback. The common source must equal the map's admitted common bytes.

Each player retains a separate VM and sleeping continuations. Execution borrows
the same complete world stack and restores it on return, sleep or refusal.
Initial `main` runs synchronously; later wakes share the map game-clock scheduler
with deterministic player/ticket ordering. Repeated starts retain the established
context in the measured sleeping and completed cases. The AI-owned native
declarations are checked for source, ambiguity, constant flags and scalar
signatures before globals execute or the world is lent.

A synthetic test through the actual `MapStartupHost` and `MapScriptSession`
checks synchronous resource writes, independent programs, a map timer preceding
their wakes, and world restoration after callback refusal. Player-state mutation
from an AI program refuses before writing when map callback registrations are
present; callbacks are not forwarded into the wrong VM. Unsupported AI gameplay
natives report their exact names. The selected red/purple source fixtures reach
`SetTargetHeroes` and `SetHeroLevels` in a synthetic normal-difficulty world;
they are not a complete Human02 run, and the selected map does not start purple.

Combined private receipts under `.scratch/campaign-continuation/ai-combined-*`
record 1427 core tests and 607 app tests passed, strict native and WASM Clippy,
and formatting. The real selected Continue test still refuses Human02's harvest
order after Human01 publishes 152 executed units. Campaign strategy, construction,
combat, cross-program callbacks and full chapter completion remain unqualified.

## Native loader path

In the reviewed image, the `StartCampaignAI` registration at `0x4a0fb7`
selects handler `0x4ac200`. Its dispatch reaches `0xac34d0`, which loads the
common JASS prelude, common AI prelude and requested program through
`0xac02d0`. The prelude operands reference the image's `common.j` and
`common.ai` strings. The file helper first tries the supplied path through
`0x4593b0`; if that load fails, it prepends the image's `Scripts\` token
and retries. The runtime loader should derive those tokens from the admitted
image rather than introduce copied prelude names or fallback content.

This is native-code evidence for the path sequence, not a qualification of
every load failure or archive precedence. The recreation's pinned resolver
must still refuse a present but unreadable winning member. Local disassembly
and operand receipts are retained under `.scratch/campaign-ai/native-start/`.

## Original-client scheduling probe (2026-10-23)

The reviewed client above ran a diagnostic Human02 map with `-classic`;
`VersionGet` reported RoC. The map retained the original configuration and
selected its first two computer-player slots, 0 and 3. Authored gameplay scripts
were replaced by the diagnostic. Each embedded AI program declared its own
same-named counter, incremented it, wrote the counter times a diagnostic amount
to its player's gold, requested `Sleep(0.25)`, then incremented and wrote again.
The amounts 17 and 29 are test stimuli, not game rules.

| Map observation | Game clock | Player 0 gold | Player 3 gold |
| --- | --- | --- | --- |
| Before either start | 1.000 | 0 | 0 |
| Immediately after the first `StartCampaignAI` returns | 1.000 | 17 | 0 |
| Immediately after the second returns | 1.000 | 17 | 29 |
| Later sample | 1.125 | 17 | 29 |
| Later sample | 1.563 | 34 | 58 |
| Final sample | 9.063 | 34 | 58 |

These runtime observations support separate AI globals operating on the same
player resource state. Initial execution reaches the first write before
`StartCampaignAI` returns; deferring all initial execution until a later timer
boundary would contradict this case. The samples bound the second writes to
after 1.125 and by 1.563; they do not establish exact wake times or instruction
scheduling between observations. Callbacks between programs, full campaign AI
and native Windows behavior remain unqualified.

The ignored local receipt is
`.scratch/campaign-ai-scheduling-probe-r2/result.json`, with all 12 raw markers
preserved beside it. The diagnostic map SHA-256 is
`1abd9495206ce14486977cc83aa19d70b831d4d43e540cd408b4ec682f7aa9c4`;
its map-script SHA-256 is
`c518e5993f0338485d29492e648e52d21be67ead9d9db15af7f82b8c73c6903e`.
Four synthetic collector tests passed. An initial collection before all markers
arrived refused; the completed collection qualified all 12. The owned client
was stopped and its run retained.

### A repeated start while the first program sleeps

A second diagnostic used the same player for both start calls. Program A
wrote 17, requested a 0.25-second sleep and later wrote 34. Program B would
write 29, request a 1-second sleep and later write 58. The map observed 17
immediately after each start call; A later reached 34 and retained that value
through the final sample. It then started the identical B program on a fresh
computer player, which reached 29 and then 58. This fresh-player control
establishes that B could load and execute in the same run.

All 12 markers reported RoC. This supports retaining the initial program and
its continuation when a different program is requested for the same player
while it sleeps. It does not qualify repeated starts after `main` returns,
after loading fails, or through other AI start natives.

The controlled receipt and raw markers remain under the ignored
`.scratch/campaign-ai-repeat-probe-r2/`. Its map SHA-256 is
`35b2d3df67340750b19ef26e8556bd8ad481c905039402e12ae694267d767d2b`;
its script SHA-256 is
`f1e9039d7998f2198084e750faa632746a702c5e045232f82b0a35a02f19e0ba`.
Three synthetic collector/stimulus tests passed. The earlier repeat run without
the fresh-player control remains preserved separately; the controlled run is
the evidence for the conclusion above. Both owned client runs were stopped.

A further variant requested B again at map time 2.063 after A's final write
had already been observed at 1.563. A has no further source operations after
that write except returning from `main`. The repeated call left the bank at
34 immediately and through the final 9.063 sample. The fresh-player B control
again completed at 58. This supports retaining the established context in this
completed-entrypoint path as well; failed initial loads, explicitly stopped AI
and other start natives remain outside the measurement.

All 12 markers qualified on RoC. The ignored
`.scratch/campaign-ai-completed-repeat-probe/` retains the receipt, raw markers
and four passing collector/stimulus checks. Map SHA-256:
`9a17259e01e349949149ea3f42d0dc7c9faf5e51320c94264f602b0df4516746`;
script SHA-256:
`bcbe2f9edd8c17aef73c8b2771bad6c94bbea89fe574a5c2b7b961f7052d4eaa`.
The owned client is stopped. The runtime's initially proposed completed-context
refusal needs correction to match this measured case.

## Relationship to the earlier Human02 probes

The selected `War3Local.mpq::Maps\Campaign\Human02.w3m` has SHA-256
`80bdb1044403d628d3a0b645f318f0da5755a8f16749761b557b2de7ec81aff2`.
The earlier native-probe source map has SHA-256
`d606be8dd85bfd04fb7701dffbb4ea31cc748466889ad2a94d621f6223dfca04`.
These container identities remain distinct.

Bounded checked extraction found byte-identical `war3map.j`, `war3map.w3i`,
`war3map.w3u`, `war3map.doo`, `war3mapUnits.doo`, `war3map.w3e`,
`war3map.wpm` and `war3map.wts`; `war3map.w3a` and `war3map.w3b` are absent
in both. In particular, the shared script has SHA-256
`938e9f2bebea5470fcc9b345b337ea5a7dff2fb0eb2022f9571200147212ccbe`.
This supports comparisons that depend on those members. It does not establish
equality of unexamined members or runtime behavior.

Local-only receipts and source copies remain under `.scratch/campaign-ai/`
in the private campaign worktree: `source-receipt.json`, `compile.log`,
`map-comparison.json` and `map-comparison.log`. The helper commands exited zero;
no native client run or production runtime change was made for this audit.
