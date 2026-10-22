# Campaign AI runtime

## Selected-source audit (2026-10-23)

The private campaign continuation still refuses `IssueImmediateOrder` during
initialization, before its later `StartCampaignAI` calls. The source path next
reaches nearby-lumber harvesting. AI execution is not implemented by the source
audit below.

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
AI-native contracts, callback invocation and scheduling remain open.

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
scheduling between observations. Repeated starts, callbacks between programs,
full campaign AI and native Windows behavior remain unqualified.

The ignored local receipt is
`.scratch/campaign-ai-scheduling-probe-r2/result.json`, with all 12 raw markers
preserved beside it. The diagnostic map SHA-256 is
`1abd9495206ce14486977cc83aa19d70b831d4d43e540cd408b4ec682f7aa9c4`;
its map-script SHA-256 is
`c518e5993f0338485d29492e648e52d21be67ead9d9db15af7f82b8c73c6903e`.
Four synthetic collector tests passed. An initial collection before all markers
arrived refused; the completed collection qualified all 12. The owned client
was stopped and its run retained.

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
