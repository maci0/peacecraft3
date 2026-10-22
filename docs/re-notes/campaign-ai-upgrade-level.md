# Campaign AI upgrade-level query

The selected AI-common source declares `GetUpgradeLevel(integer) -> integer`. The reviewed client resolves the running AI context's player and queries that player's upgrade state. A controlled RoC run observed a change to the AI player's `Rhlh` level while another player's independently assigned level remained separate; see [campaign AI world queries](campaign-ai-world-queries.md).

The recreation validates the AI declaration before executing its globals or `main`. A query uses the already admitted AI player's identity and the map world's source-qualified `GetPlayerTechCount` upgrade model. It does not allocate another player, retain a second research table, or alter the game clock. A map research change becomes visible to a sleeping AI program when that program wakes.

This only covers upgrade levels in a world with admitted current research and a declared map getter. Unknown upgrade IDs use that getter's source-qualified zero answer. An unavailable or invalidated model refuses contextually. The selected Human02 red program still stops earlier at unsupported `SetTargetHeroes`; this change does not establish campaign AI strategy or chapter completion.
