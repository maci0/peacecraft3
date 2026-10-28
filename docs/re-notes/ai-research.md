# Campaign AI research

The campaign AI researches upgrades through three AI-script natives that `common.ai`'s
`StartUpgrade` calls (`GetUpgradeLevel`, `GetUpgradeGoldCost` and `GetUpgradeWoodCost`,
`SetUpgrade`). `h05_green.ai` reaches them through `SetBuildUpgrEx` after its third wave. This
note records what the Legacy executable (SHA-256 `3f2ed012...`) shows, what the recreation
builds from it, and what stays open. Code: `crates/pcx-play/src/unit_training_host.rs`
(`set_ai_upgrade`, `ai_upgrade_level`, `ai_upgrade_cost`, `complete_research`),
`upgrade_research.rs` (`Research`, `Prices`), `campaign_ai.rs` (dispatch).

## Executable evidence

Disassembled with `objdump`; the registered wrappers were found with the native-registration
scan, the addresses are virtual addresses in the Legacy image.

| Fact | Routine |
| --- | --- |
| `GetUpgradeLevel(id)` is the player's researched level plus one when the player's in-progress count for `id` is positive (flag 2 of the tech counter). | wrapper `0xab5570`, counter `0x513620` |
| `GetUpgradeGoldCost(id)` is the gold cost of level `GetUpgradeLevel(id) + 1`. | wrapper `0xab5550` calls `0x6df7e0` |
| `GetUpgradeWoodCost(id)` is the lumber cost of that level. | wrapper `0xab5620` calls `0x6e0a30` |
| The cost of level `n` is `base + mod * (n - 1)`: the routine multiplies the modifier getter by `n - 1` and adds the base getter. Gold reads `goldbase`/`goldmod`, lumber `lumberbase`/`lumbermod`. The column pairs are the ones the upgrade table constructor stores next to those names (`0x6b64a0` to `0x6b6640`, getters `0x6b1dd0`, `0x6b1e70`, `0x6b7380`). | `0x6df7e0`, `0x6df750`, `0x6df810`, `0x6e0a30` |
| `SetUpgrade(id)` calls the AI player's method `0xac3ac0` and then returns false whatever that method did (`xor eax,eax` after the call). `StartUpgrade` ignores the result in `OneBuildLoop`. | wrapper `0xab7680` |
| `0xac3ac0` walks the player's units. For a unit that passes the filter `0xac4030` it takes the unit's `Aque` ability (class id `0x41717565`) or, without one, its `ARes` ability (`0x41526573`), asks the ability whether it can research `id` (vtable slots `0x344` and `0x270`), requires two counters of the `Aque` ability (`+0x3c`, `+0x40`) to be zero, asks a second check with the player (slot `0x328`, zero means accepted), then issues the research command through `0x698910` and `0x690590` and returns 1. With no taker it returns 0. | `0xac3ac0` |
| The table constructor also stores the `timebase` and `timemod` columns (`0x6b6638` onward). A record loader reads two float getters over the slots at offsets `+0x68`/`+0x6c` and `+0x70`/`+0x74`; pairing those slots with the two time columns is by offset only. | `0x6cfc29`, `0x6cfc3a`, getters `0x6b7f60`, `0x6aedc0` |

## Recreation

- `GetUpgradeGoldCost` and `GetUpgradeWoodCost` read `goldbase`, `goldmod`, `lumberbase` and
  `lumbermod` of the selected `UpgradeData` (the set the caller selected), with the map's `w3q`
  edits by field id (last edit wins) and custom upgrades derived from their base row, the same
  reading as the upgrade-effects table. A row that cannot be read fails that upgrade only.
- `GetUpgradeLevel` adds one while a research of the upgrade is queued or running.
- `SetUpgrade` picks the first unit, in arena order, of the AI player that is alive, finished,
  lists the upgrade in `Researches` and has an empty queue. The cost is the next level's gold
  and lumber, charged when the order is accepted. The job joins the shared training queue
  (`Training` with a research level), so refund on cancel, refund on the producer's death, pause
  handling and the queue slot bound are the training code's. The duration is `timebase +
  timemod * (level - 1)` seconds. It finishes through `SetPlayerTechResearched`'s upgrade path
  (`set_researched_with_effects`), after the tick's regeneration so that an effect on a
  moving unit does not rebase a regeneration clock without motion. It raises no training or
  research events (research events are not admitted).
- It orders nothing, and returns, when the player cannot afford the level, a required
  technology is missing, or no unit qualifies. The native returns false in every case.
- The `Requires` profile key lists the technologies for level 1 and `Requires<n>` the ones for
  level `n + 1`. Stock Unholy Strength (`Rume`) has `Requires1=unp1` and `Requires2=unp2`,
  which is the game's tier rule. Each listed technology is checked like a training
  prerequisite (a researched or owned count of at least the research threshold). Amounts
  (`Requiresamount*`) and map edits to requirement fields make that upgrade unreadable.

## Not established

Each of these either refuses loudly or is a choice:

- The vtable checks at `0xac3ac0` (slots `0x344`, `0x270`, `0x328`) and the counters at
  `+0x3c` and `+0x40` are not decoded. "Can research" is the `Researches` list, "idle" is an
  empty queue, "accepted" is requirements plus affordability. If the original also refuses
  while a building trains, or accepts a busy building, the order of who researches differs.
- Arena order stands for the player's unit list order (`0xac4030` filters the list; the filter
  is not decoded).
- Whether the cost is charged at order time (assumed, as for training) and the refund share
  (the training refund rule).
- The `timebase` and `timemod` shape. The loader reads both; the `n - 1` combination is the
  cost routines' shape applied by analogy.
- The key-to-level mapping of `Requires` and `Requires<n>`.
- A second research of an upgrade that is already running, and a level beyond `maxlevel`,
  refuse loudly (the executable's counter would price level `+1` while the check at slot
  `0x328` is undecoded).
- Research events (`EVENT_*_RESEARCH_*`), the hero-ability research path and the player (not
  AI) research command remain unbuilt.

## Tests

Synthetic nonstock: `upgrade_research.rs` (`research_prices_are_base_plus_modifier_per_level_with_map_edits`,
`research_requirements_follow_the_level_keys_and_refuse_unread_forms`), `unit_training_tests.rs`
(`ai_research_*`: cost, duration, level, queue, requirement, funds, idle queue, cancel refund),
`campaign_ai.rs` (`upgrade_natives_reach_the_world_and_set_upgrade_answers_false`).
