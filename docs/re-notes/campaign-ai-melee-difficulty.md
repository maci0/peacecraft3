# Caller-owned melee difficulty

The baseline unchanged Undead06 and Undead08 campaign continuations refused
`MeleeDifficulty` in the supplied `common.ai`; receipts remain under
`/tmp/pc3-horizon-sweep/roc-Undead06.w3m/test.log` and the corresponding
Undead08 path. This is a separate query from story-game difficulty.

## Selected native input chain

Selected executable: Legacy 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The primary registration at `0xab69e4` binds `MeleeDifficulty`, `()I` and
handler `0xab5a90`. The handler resolves the current AI object through the same
context/global/reference pair as `GetAiPlayer`, then tests that AI object's
flags. It does not read the `common.ai` globals `ai_player` or `difficulty`.

The following are bounded native-code observations, not original-client runtime
measurements. `campaign_ai::melee_difficulty::Rules` checks these instruction
relationships and decodes the actual operands from the selected image.

| Native role | Relationship |
| --- | --- |
| `0xab5a90` | Query current AI flags at `+0x2d0`; the low flag returns its immediate result, otherwise the combined predicate is added to the decoded base. |
| `0xabd3ed` | AI creation maps its incoming AI-difficulty ordinal into those same low/high flag operands. |
| `0x4c0ed0` | Player wrapper passes its own number and the supplied difficulty to the AI creator. |
| `0x47e546` | Ordinary startup passes the player's `+0x2cc` value to that wrapper. |
| `0x490ab2` | `GetAIDifficulty(player)` reads the same player field. |
| `0x51dfd8` | Startup configuration copies a slot byte into that player field. |
| `0x51e693`, `0x4bdf8c` | Fresh slot initialization and player reset agree on that input's default. |

The query observes the AI object created from this player input. Its flag field
is not the similarly numbered field of a player or the current game-difficulty
byte. The constructor's initialized flags and its `AI-1` class relationship are
checked against the requested allocation class. The selected image maps the
three admitted AI inputs to the melee query results; production code embeds
neither a Normal return constant nor the source flag masks/ordinal mapping.
The [targeting scorer](campaign-ai-targeting.md#current-difficulty-input-to-the-native-scorer)
uses the separate current game-difficulty input and is unchanged here.

A complete lobby parser is not added. A caller with external lobby input must
supply explicit per-player AI-difficulty ordinals through the admitted snapshot;
unrecognized inputs refuse rather than becoming Normal. Fresh single-player
preparation, which has no external lobby override, obtains its default from the
checked fresh-slot/player reset relationship and retains authored player numbers.
Generic preparation does not install this fresh-game input automatically.

## Recreation and ownership

Each AI host captures its own source-qualified result when its runtime context
is created, before globals and `main` execute. Script variables cannot choose
another player's input. Updating the runtime's startup input affects future
contexts, while existing contexts retain their captured value through sleep and
repeated `StartCampaignAI` calls. This is the recreation's owned-context boundary;
it does not reproduce the native allocator or require matching native internals.

The source snapshot is bounded by the existing active-AI budget. Duplicate or
negative player rows, unqualified input levels, source budget failures, malformed
instruction joins and missing player rows are explicit errors. A missing input
refuses the query only; unrelated AI programs can still start. An absent input
is captured as unqualified and is not retroactively guessed after an update.
The new query neither orders units nor consumes RNG or changes world state.

`SourceNames` carries the optional creation input to campaign assembly. Fresh
preparation derives it, `MapStartupHost` installs it in the existing AI runtime,
and `AiHost` returns its captured integer under the checked `common.ai` ABI.
`AiWorld`, player records, ObjectHost and WTS/real-number forwarding are unchanged.
The only constructor literal for `SourceNames` initializes this input absent;
existing source callers compile in the affected native and WASM targets.

## Validation

Six named tests pass in the isolated worktree target, including both local
original-input gates:

- Nonstock constructor levels, relocated flag bits, nonstock result values and
  fresh defaults drive the snapshot; invalid and missing inputs refuse.
- Broken registration/context/class/player/lobby joins, exact work budgets and
  every truncated synthetic source window refuse without indexing past input.
- Two AI programs use different caller-owned inputs despite spoofed script globals;
  sleep, input replacement, repeat start and a newly created context retain the
  correct capture behavior.
- Missing admission/rows and malformed native arguments or declarations refuse;
  an AI that does not query this input can still start.
- The explicitly selected executable reader qualifies the native creation/lobby joins.
- The supplied original `common.ai` `SetBuildUpgr` helper executes through the
  captured query and its real `SetBuildAll` script path, after changing its
  `difficulty` and `ai_player` globals. Neither helper is replaced with a stub.

The local helper gate used these supplied script identities:

- `common.j`: SHA-256
  `3c21e998bf80d1e04718bd3f2ee7ef2bf1b83359e5b303a56cf59cd61a2afdcd`.
- `common.ai`: SHA-256
  `29f51929ff3c09988a77748dbefea319b72b32bf4bb1a5f6997760a621454818`.

The AI regression suite passes 54 tests with two explicit original-input ignores;
the two new original-input gates above were run separately and passed. Strict
native and WASM Clippy with all targets and warnings denied, both library builds,
formatting and diff checks pass. The isolated target is
`/tmp/pc3-fidelity-sol61-07-target`; exact commands, worktree path and exit codes
are local under `/tmp/pc3-lane07-melee-validation/3/`. Tests used a verified private
offscreen display and null audio. Test-only debug symbols were disabled for this
crate, with assertions enabled. Bounded disassembly and source inputs remain
local under `/tmp/pc3-lane07-melee/`; no original assets are committed.

The actual unchanged Undead06/Undead08 continuations still require a post-merge
replay. No original-client or browser runtime comparison, complete campaign, or
tick-identical gameplay claim is made by these parser/script/build checks.
