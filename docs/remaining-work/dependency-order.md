# Dependency order

Prerequisites come first. Ties use implementation priority, then tracker line, then id. `implementation_order` is that priority by itself and is not a topological order.

| Topo | Id | Priority | Depends on | Gate |
| ---: | --- | ---: | --- | --- |
| 1 | T0788 | 1 | — | Retire the prohibition. Do not treat the interpreter as native-complete. |
| 2 | T0985 | 2 | — | Close only the movement-world server. Gameplay ownership stays on the simulation store. |
| 3 | T0849 | 4 | T0788 | Extend the caller-owned host one missing native at a time. |
| 4 | T0862 | 5 | T0849 | Add measured natives with source diagnostics. Do not fill the table with zero returns. |
| 5 | T0852 | 6 | T0849 | Chapter execution remains open after the Human06 and NightElf06 compile gates. |
| 6 | T0867 | 7 | T0849 | Localization beyond TRIGSTR_ lookup stays open. |
| 7 | T0738 | 8 | — | Keep original game handles and the full gameplay store open. |
| 8 | T0631 | 9 | — | Finish slot and object semantics before treating map info as playable setup. |
| 9 | T0632 | 10 | T0738 | Spawn admitted placements through the existing world. Do not claim the campaign loads. |
| 10 | T1133 | 3 | T0849, T0632 | The named neutral getter is no longer the current stop. Continue from the missing CreateSound file binding ... |
| 11 | T1045 | 11 | T1133 | Original-map execution remains open after metadata admission. |
| 12 | T0633 | 12 | T0631 | Refuse blend weights until source establishes them, then integrate one scene. |
| 13 | T0634 | 13 | T0633 | Keep cliff threshold and water geometry unproven until measured. |
| 14 | T0894 | 14 | T0631 | Integrate the test-only HUD derivation into the in-game viewport. |
| 15 | T0630 | 15 | — | Do not invent circle or polygon regions. Runtime handles wait on a measured shape. |
| 16 | T0638 | 16 | — | Take pan and zoom speeds from RE. Authored W3C records are not those control values. |
| 17 | T0290 | 17 | — | Add attack/build/cast/train only after those orders exist in the sim. |
| 18 | T0742 | 18 | T0738 | Extend orders past select, move and stop without inventing unproved command ids. |
| 19 | T0485 | 19 | T0633 | Show a caller-selected terrain tile in the viewer without using the capture example as the product path. |
| 20 | T0743 | 20 | T0742 | Consume war3map.wpm flags. Local avoidance and formations stay open. |
| 21 | T0712 | 21 | — | Measure allied shared data before parsing it. |
| 22 | T0304 | 33 | T1133 | Add one outcome-bearing scenario after a chapter can execute config and main. |
| 23 | T0639 | 34 | T0638, T0633, T1133 | Load and pan Human mission 1 only after terrain, camera and campaign startup meet. |
| 24 | T0640 | 35 | T0630, T0712 | Parse wpmp and prove one community container. Region and w3i parsers are not that proof. |
| 25 | T0850 | 38 | T0849 | Desugar WTG to JASS with original generated names before Athena can prove the path. |
| 26 | T0864 | 39 | T0862 | Keep Blz and the rest of Blizzard.j open. The few GetBJ player adapters do not close this item. |
| 27 | T0869 | 41 | — | Parse script source selection, safe mode and forced flags from the map ini. |
| 28 | T0870 | 42 | T0862 | Fail missing natives and unresolved rawcodes with the map identifier. |
| 29 | T0088 | 43 | — | Keep the 4K browser checkpoint failure and unsynchronized RNG/clock comparison open. |
| 30 | T0097 | 44 | — | Finish unsupported game-setting actions and original-client appearance. |
| 31 | T0108 | 45 | T0097 | Wire runtime device kind, exclusive mode and the texture-quality renderer consumer. |
| 32 | T0194 | 46 | — | Native panel tint and full-screen opening stay outside this import slice. |
| 33 | T0582 | 47 | — | Render authored hover, pushed and disabled button states. |
| 34 | T0584 | 48 | — | Finish FDF style, text and anchor migration. |
| 35 | T0918 | 49 | T0849 | Track quest and quest-item state from declared natives before this completion gate. |
| 36 | T0586 | 50 | — | Native composited tint and the full-screen opening remain open. |
| 37 | T0919 | 51 | T0918 | Enter endGame only from an authored win condition. |
| 38 | T0886 | 52 | T0849 | Play transmissions from the existing FDF catalog. Declaration ingest is a separate closed item. |
| 39 | T0920 | 53 | T0849, T0631 | Load shipped maps and run their init scripts after the host can start. |
| 40 | T0885 | 54 | T0849, T0738 | Run one selected campaign AI entry and record the next missing interface before any parity claim. |
| 41 | T0917 | 55 | T0886 | Assert ordered intro dialogue per chapter after transmission state exists. |
| 42 | T0751 | 56 | — | Resolve render coupling and gameplay subscribers. TICK_MS is a project convention. |
| 43 | T0865 | 40 | T0862, T0751 | Original timer and event ordering remains the DotA gate, beyond CreateTimer admission. |
| 44 | T0752 | 57 | T0751 | Identify the generator and call-site order. A matching Wine probe is not recreation parity. |
| 45 | T0753 | 58 | T0751, T0862 | Order gameplay events after the campaign host can run past startup. |
| 46 | T0754 | 59 | T0751 | Schedule effects and sounds on the sim clock rather than the startup queue alone. |
| 47 | T0755 | 60 | T0738 | Round-trip the savegame format. war3map.w3s in pcx-map is map sound records, not that format. |
| 48 | T0305 | 61 | T0752 | One trace byte-identical across 100 consecutive runs, and unchanged vs an original-game baseline. Synthetic... |
| 49 | T0756 | 62 | T0751, T0752, T0305 | Require 100 bit-identical original-scripted runs after tick and RNG parity. |
| 50 | T0923 | 63 | T0752 | Combat, terrain and native RNG generation remain open. |
| 51 | T0926 | 64 | T0923 | Keep the native differential corpus open. |
| 52 | T0329 | 65 | — | Keep the four literal mode names open until a caller-facing alias is specified. Do not treat GameMode as th... |
| 53 | T0325 | 66 | T0329 | Select the application mode and measure exact-target archive priority. |
| 54 | T0707 | 67 | — | Derive engine defaults from game.dll. The closed SLK stock tables are a different source. |
| 55 | T0713 | 23 | T0707 | Consume authored unit fields. Do not treat Family::Unit as race or hero rules. |
| 56 | T0739 | 22 | T0738, T0713 | Bind authored HP, mana, attack, armour, resistances and regen with explicit conversions. |
| 57 | T0714 | 24 | T0713 | Keep ability levels closed until a measured object layout and a sim consumer exist. |
| 58 | T0715 | 25 | T0713 | Gameplay item, upgrade, buff and destructable behavior stays open behind the stock cells. |
| 59 | T0716 | 26 | T0714 | Evaluate prerequisites only from authored rules. The field catalog must not count as availability. |
| 60 | T0306 | 27 | T0739, T0752 | Record hp/mana/xp/cooldown only after the sim stores them, and draw RNG inside step. |
| 61 | T0740 | 28 | T0739 | Implement attack-type mitigation only from authored fields. Do not invent crit, dodge or block formulas. |
| 62 | T0741 | 29 | T0714, T0740 | Add one authored spell consumer after ability data and orders can name it. |
| 63 | T0744 | 30 | T0739 | Store cooldown, cast time, mana cost and upkeep on the combat consumer. |
| 64 | T0745 | 31 | T0740 | Add projectile state only with authored speed and impact rules. |
| 65 | T0746 | 32 | T0739 | Model death and experience only after HP exists. |
| 66 | T0721 | 36 | T0739, T0713 | Diff stock stats against the in-game editor after a stats consumer exists. |
| 67 | T0747 | 37 | T0740, T0752 | Compare duel HP curves with the original only after the damage pipeline exists. |
| 68 | T1031 | 68 | — | Complete visual fidelity and original button-state sound remain open. |
| 69 | T0893 | 69 | T0985, T0738 | Connect gameplay to the existing movement lockstep server. Discovery loopback is not this surface. |
| 70 | T0900 | 70 | T0754 | Bind sound-list channels and 3D positioning from authored SLK rows. |
| 71 | T0901 | 71 | — | Extend fixed-work gates as gameplay and rendering land. Report FPS observationally. |
| 72 | T0928 | 72 | T0926 | File a minimal seed into Open unknowns when a native differential exists. |
| 73 | T1015 | 73 | — | Measure live expansion flags. Portrait containment stays an extension. |
| 74 | T1020 | 74 | — | Patched installers, retail discs and full installed-game equivalence stay open. |
| 75 | T0254 | 75 | — | Accept remaining sparse combinations and BZIP2/LZMA only from measured bytes. |
| 76 | T0259 | 76 | — | Replace the assumed DEFAULT_PRIORITY with a measured game.dll open sequence. |
| 77 | T0319 | 77 | — | Keep unsupported BLTE codecs and ESpec explicit. |
| 78 | T0331 | 78 | T0319 | Rendering, shader semantics and native joint selection stay open. |
| 79 | T0364 | 79 | T0329 | Add the fix ledger with rows defaulting to defer. Behaviour::Fixes is not that ledger. |
| 80 | T0484 | 80 | — | Implement asset-cache and async decode inside PcxGfxPlugin. The plugin type exists and its build method is ... |
| 81 | T1061 | 81 | T0484 | Remove synchronous shared-path file I/O from the graphics cache. |
| 82 | T1088 | 82 | — | Verify menu GPU rendering and audio on Windows and macOS. Compile artifacts do not close this. |
| 83 | T1338 | 83 | — | Do not retry the unpaid four-target run. Local checks stay separate from that runner. |
| 84 | T0230 | 84 | — | Attribute the two header words before any reader depends on them. |
| 85 | T0233 | 85 | — | Keep the live-table difference recorded; archives use the fixed table. |
| 86 | T0668 | 86 | — | Leave int width, real versus unreal, and the end word uninterpreted. |
| 87 | T0794 | 87 | — | Treat the historical wave note as partial until a fresh sweep is the acceptance record. |
| 88 | T0805 | 88 | — | Keep the stray-byte community file refused until a measured construct exists. |
| 89 | T0873 | 89 | T0865, T0862 | DotA remains a broad gate after timers, natives and unit creation. |
| 90 | T0876 | 90 | T0850 | Athena stays open until WTG desugar executes. |
| 91 | T0877 | 91 | T0870 | Footman Frenzy stays open as the pre-1.20 tolerance gate. |
| 92 | T0878 | 92 | T1133, T0919 | Human campaign completion stays open. Startup progress does not close it. |
| 93 | T0910 | 93 | T1133 | Add Reign of Chaos and Frozen Throne chapter scenarios after one chapter starts. |
| 94 | T0911 | 94 | T0910 | Add orc, undead and night-elf chapter scenarios after the human path. |
| 95 | T0912 | 95 | T0304 | Capture DemoCampaign expectations from play, not from the object-file census. |
| 96 | T0913 | 96 | T0910, T0752 | Require 100 identical runs only after the scenario files exist. |
| 97 | T0914 | 97 | T0910 | Pin each original baseline to an evidence id. Wine menu captures are not chapter baselines. |
| 98 | T0929 | 98 | T0928, T0914 | This corpus gate stays open until native differentials exist. |
| 99 | T0991 | 99 | T0893 | A real 1.29b client joining remains open. Ephemeral discovery loopback is not that join. |
| 100 | T0992 | 100 | T0991 | Emulator-realm login stays open. |
| 101 | T1018 | 101 | T0893 | Public multiplayer and gameplay lobby stay open. |
