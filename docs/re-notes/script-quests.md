# Quest state on the gameplay handle arena

`ObjectHost::with_quests(QuestBudget)` admits the selected Program's quest and
quest-item declarations. Twenty-one natives cover creation, text assignments,
the five quest flags, item completion, destruction and dialog-update requests.
Admission checks positional parameter types and return types, declaration source
ambiguity and ancestry reaching `handle`. Formal parameter names are not an ABI.

Quest, item, timer and other object identities come from the same `EnumHost`
arena. Destruction invalidates the quest and its items, releases their retained
text, and preserves unrelated objects. Stale and wrong-family accesses refuse.
Invalidating owned items is the recreation's safety policy; no undefined
original access after destruction was executed to establish native behavior.

Live quest/item counts, retained text and observation-log size have explicit
caller budgets. Layout checks use the actual stored payload and event types.
Growth uses fallible reservation. Refused text replacement keeps the previous
value. A full observation log refuses a mutation before it changes state. These
limits are resource policy, not original game limits. Titles, descriptions and
icon paths come from executed script arguments; they are not copied defaults.

## Original runtime observation

The local executable's VERSIONINFO is `1.29.2.9231`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The reference uses isolated Docker/Wine/Xvfb with software rendering, no network,
and private copies of the supplied installation. It is not native Windows
ground truth.

Both runs enter through the Frozen Throne menu and its Custom Game screen at
1280×960, 96 DPI, using a fresh `quest` profile and the displayed lobby settings.
Other editions' runtime defaults have not been observed by this probe.

A self-authored script in an unsigned copy of the supplied Lost Temple map
creates two quests and one item on each. It stores all initial queries before
any setter, then exercises both boolean values on one live quest and item.
The other quest and item receive no setters. No object is destroyed or queried
after destruction. The archive verification re-extracts fifteen retained members;
only `war3map.j` changes. Optional attributes and the opaque signature trailer are
omitted. This instrumented map is not an unmodified-map acceptance gate.

The staged map SHA-256 is
`a9eebad7c71f036ab8d02582ebc7558764c5b941fce292bf6380cc4774d08d77`.
The script's seed `20261016` is an explicit probe stimulus. Both the visible
in-game marker and a newly created local Preload export identify this probe.
The generated export is read as text, never executed.

Two fresh private installation/profile runs produce byte-identical 970-byte
exports, SHA-256
`2d960c8debc124aa685c0ec9b49a51b8190a16f60533430250d276ded3d16691`.
Both runs also show the marker and queried values in-game. Their containers are
stopped and retained. This repeated observation does not establish all versions
or a completed campaign.

| Query on a new object | Observed value |
| --- | --- |
| `IsQuestRequired` | true |
| `IsQuestCompleted` | false |
| `IsQuestDiscovered` | true |
| `IsQuestFailed` | false |
| `IsQuestEnabled` | true |
| `IsQuestItemCompleted` | false |

Each setter's following query returns the supplied true or false value. These
constructor behaviors are implemented as engine behavior, not as a table of
measured asset records. Other executable profiles remain unverified.

Private preparation and receipts are under
`.scratch/cursor-gameplay-20261016/06-quest-default-probe/` and
`.scratch/cursor-gameplay-20261016/root-quest-integration/`; game copies and
captures stay under `.scratch/original-game-vm/quest-defaults-*/`. No original
map, script, screenshot or generated export is published.

## Acceptance boundary

The observed-default regression fails on the earlier unset-state implementation.
Synthetic tests execute the shared ObjectHost path, both supported declaration
parent shapes, renamed formals, state mutations, budget boundaries, wrong-family
and stale identities, and text cleanup. The same lifecycle tests execute under
WASI. Native and browser-target compilation are separate checks.

Dialog requests are retained events, not a rendered quest window. No campaign
win/defeat decision, UI adapter, hashtable quest save/load, or complete campaign
playthrough is established by this slice.
