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

The production `map_startup_host::assemble` can admit this same session through
`MapStartupBudgets::quests`. `None` leaves quest natives unsupported and does not
require quest declarations on maps that do not use them. `Some(QuestBudget)`
validates the selected declarations before running globals, `config` and `main`.
It adds no second VM, arena or quest store. The constructor behavior retains the
Legacy observation scope below; opting in does not verify another executable.

Delivered-map App tests interleave timers, quests and items, execute assigned
text and flags, then destroy a quest through a published callback. An omitted
budget, exhausted object/item/text/event budget or missing declaration refuses
the candidate while preserving the published quest state, timers and callback.
A successful replacement retires the old callback ticket. Handle tokens remain
session-local; raw numeric tokens are not cross-session capabilities.

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

## Read-only objective state

`ObjectHost::quest_session` exposes the admitted session without a second quest
store. `quest_view::project` returns live quests in creation order and each
quest's live items in their authored creation order. Title, description and
icon text are borrowed from the executed state; absent text stays `None` and
an assigned empty string stays `Some("")`. Flags and item completion are read
from that same session. Destroyed parents and items are absent, and a direct
query of either identity refuses. Projection reserves its output fallibly and
does not copy temporary identity lists or text buffers.

An owned-VM test executes synthetic quest creation, text/flag mutations, dialog
update and destruction through the composed `ObjectHost`, interleaved with
timers in the same arena. The view reflects those executed changes, preserves
the surviving quest and refuses destroyed identities without disturbing the
timers. This is a display-data boundary; grouping, screen layout, rendered
objectives and campaign victory decisions remain open.

## Null quest boolean setters (selected 1.29b image, 2026-10-22)

Unchanged Human01 calls `QuestSetCompletedBJ(udg_QuestOrcs, true)` during its
Warlord victory sequence. The map declares that quest as null and never assigns
it; its created village quest is a different variable. Substituting that quest
or editing the map would change the authored script, so neither is done.

Static reads of the selected `Warcraft III.exe` (SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`)
establish the primary registration and guard chain:

- `QuestSetCompleted`: registration 0x49FB9D, handler 0x4A4940.
- Required/discovered/failed/enabled handlers: 0x4A4A60, 0x4A49A0,
  0x4A4A00 and 0x4A49D0, respectively.
- Each calls quest resolver 0x4828A0. At 0x4828D2 it tests the incoming handle;
  zero branches to 0x4828F3 and returns zero without a lookup.
- Each setter tests that result and returns before any flag write when zero.
  The completed setter's guard is at 0x4A494E; its return is 0x4A4962.
- Item completion handler 0x4A48E0 uses item resolver 0x4829D0 and the same
  zero-result guard. That resolver's null path must be read independently of
  the quest resolver; it is not inferred from the common setter shape.

The shared typed boolean-argument parser now admits null only with an actual
boolean. The five quest flag setters and item completion return without changing
state, allocating handles, or reserving observation events. Non-null identities
retain live/family validation; wrong arity and non-boolean values still refuse.
This does not generalize null semantics to all quest natives or qualify stale
handle behavior. Primary registrations also have stub counterparts in the
separate registrar; this static result does not establish runtime registrar
selection or rendered/native victory parity.

The item resolver was also read directly: its zero-handle test at 0x482A02
branches to 0x482A23, which zeroes the return register. Local raw registration,
setter and resolver reads are retained under `.scratch/campaign-route/quest-*`.

Focused tests cover null calls with both boolean values and exhausted event
capacity, unchanged quest/text/handle state, malformed calls and VM continuation
past an unassigned quest. On unchanged Human01 the ordinary-order controller now
passes the previous null completion refusal at requested tick 91832, then reaches
`UnitRemoveBuffs` at requested tick 92152 (step 1406) in the authored victory
cinematic and continues there.

`UnitRemoveBuffs` and `UnitRemoveBuffsEx` are now implemented as active buff
removal on the wrapper host, with filter control flow matching the selected
client's shared helper `0x6D87A0`. The helper walks the unit's ability chain and
evaluates vtable predicates for negative/positive (`0x1E0`/`0x1E4`), aura
(`0x1E8`), timed life (`0x334`), magic (`0x1D8`), physical (`0x1DC`) and
auto-dispel (`0x338`); the basic wrapper calls it with timed-life/aura enabled
and magic/physical/auto-dispel disabled, while the Ex wrapper reorders its
booleans into the helper's stack order. The active buff state is keyed per unit
and cleaned up on `RemoveUnit`. Buff identity and class traits are not derived
from the shipped `Units\AbilityBuffData.slk` (which carries only `alias`,
`code`, `isEffect`, `race` and editor metadata). An internal, bounded, test-only
buff hook backs the removal filters and lets a future producer attach state: the
producer will supply the rawcode and class traits from its own behavior. The
hook refuses removed units, reports application order without claiming native
ordering, and is capped per unit; it is not a public API and ships only under
test builds until a producer exists. No buff-producing ability is implemented
yet; summons/timed life, sleeps, Defend and item-passive effects are currently
unmodeled as buffs, so the removal path operates on an empty admitted buff set
for the current abilities. This does not claim the full buff runtime is complete.
Focused tests cover the polarity gate, the Ex filters, null/removed/wrong-family/
wrong-arity no-write behavior, and attach-after-remove refusal and per-unit
bounds.

A bounded unchanged Human01 driver using seed 1 progresses past the former
`UnitRemoveBuffs` frontier (step 1406 / tick 92152) and runs to step 3580, where
it stops on the driver's own completion assertion (phase=3 waypoint=6) rather
than on a missing native. Seeds 3,4,5,7,8,9,10,11 reach the same assert; seeds
0,2,6 lose the hero earlier in combat before buff dispatch. The console's final
assertions still follow the two source objective-item assignments rather than a
chapter exit, rendered/native victory or next-map transition.

Private final checks: 1,167 pcx-play library tests pass (33 ignored), strict
all-target Clippy passes with `-D warnings`, and the WASM library check passes.
The three selected-original Human01 opening, authored cinematic-skip and authored
XP-cap gates pass separately. These results do not turn the still-refused long
victory route into a passing acceptance gate.
