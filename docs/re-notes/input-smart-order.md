# Smart right click and Shift-queued orders

Evidence and decisions for slices 5 and 6 of the [input behaviour audit](input-behaviour-audit.md)
(rows A10 to A13, A15, B1).

## Method and identity

- Executable: the selected Legacy `Warcraft III.exe`, SHA-256
  `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
  (same image as the audit).
- Data: `Units\AbilityData.slk`, `Units\UnitAbilities.slk`, `Units\*AbilityFunc.txt`,
  `Units\HumanUnitFunc.txt` and `Units\CommandStrings.txt`, read through the archive chain
  `War3Local`, `War3xLocal`, `War3x`, `War3`.
- Code: rizin `aa` with the `pdg` decompiler; vtables found by scanning for the class-name
  getter in slot `0x58` (`mov eax, <name>; ret`) and reading slots `0x260` and `0x304`.
  Names are rizin's `fcn.<address>`; roles are assigned here. Decompiler output stayed in the
  ignored `.scratch/smart/`. Nothing from the client is copied into the repository except
  addresses and short facts.
- Static analysis plus recreation runs on Human01 and Human04 (below). The original client was
  not launched for this slice, so no row is a runtime observation of the client.

Evidence tags as in the audit: SB shipped data, EX executable code read here, DOC widely
documented behaviour not verified here, INF inference, UNK not established, REC recreation
choice.

## What the client does (EX)

| # | Fact | Evidence |
| --- | --- | --- |
| S1 | A right click on ground is `fcn.005fd2f0`, on a unit or item `fcn.005fd4c0`. Both validate the order `0xd0003` (smart) against the selection first (`0x4f5d70` for a point, `0x4f5f80` and `0x4f6020` for a target), then send the net command (`0x64c010` point, `0x64c1d0` and `0x64c250` target). | EX |
| S2 | The validators walk the selected units and, per unit, its abilities (`0x6a1790` point, `0x6a19a0` target). An ability accepts when its slot `0x25c` (point) or `0x260` (target) returns 0. The result is the lowest error code over all abilities and units, `0xdd` when nothing handled the order, and 0 as soon as one ability accepts. So a mixed selection is valid when any unit can take the order, and each unit then resolves the order by its own abilities. | EX `0x4f5d70`, `0x4f5f80`, `0x6a1790`, `0x6a19a0` |
| S3 | The base target check `0xcb61c0` calls slot `0x304` (is this order for this target) and, when that is 0, slot `0x268` (ready now). Slot `0x304` is where each ability class reads the target. | EX `0xcb61c0` |
| S4 | A click on a unit, item or tree whose validation ends in an error other than "handled" falls back to the ground click (`0x5fd2f0` at the clicked point) while the global `0x113e350` is 0. The global is cleared before validation and set to 1 by `0x6a19a0` on error `0xdc`, by the move class for a unit target other than self (`0xb645d0`) and by the cargo-load class. So a click on a tree or rock that no ability takes becomes a move to that point, while a click on a unit does not fall back to the ground whenever the selection holds a unit with the move class. | EX `0x5fd4c0`, `0x6a19a0`, `0xb645d0`, `0xb4b980` |
| S5 | When the validation gave a non-zero error and the global of S4 is 1, the error goes to the text routine (`0x5ecc20`) and the order is still sent with `0xd0003`. Whether every such code has a text was not read. | EX `0x5fd4c0` |
| S6 | The order flags are built from the Shift read (`0x83fea0`, either Shift key): bit 0 is Shift. Bits 3, 4 and 2 come from `0x4f2340`, `0x696b50`, `0x65d110` and `0x62f120` (selection size and an option check; formation related, INF). The net command repacks them. Bit 0 is the queue flag: a flagged order is added after the unit's orders, an unflagged one replaces them. | EX `0x5fd2f0`, `0x5fd4c0`, `0x64c010` |
| S7 | Move class (`CAbilityMove`, vtable `0xf24fc8`, slots `0xb64440` and `0xb645d0`) takes `0xd0003`, `0xd0002`, `0xd0012`, `0xd0016`, `0xd0014`, `0xd004b`, `0xd01ec` and `0xd000d`. With no target it answers a smart order with "handled" after the base check. With a target equal to the unit it returns 1 for a smart order. With a non-unit widget (`vtable+0x13c` non-zero) it returns `0xdd`, which is the fallback of S4. With another unit it sets the global of S4 and returns an error code `0xdc` or `0xa6`, so the order is sent and the unit resolves it itself. | EX `0xb64440`, `0xb645d0` |
| S8 | Attack class (`CAbilityAttack`, vtable `0xf1762c`, slots `0xb18000` and `0xb18400`) takes `0xd0002` and `0xd0003` and its own ids `0xd000f` and `0xd0011`. For a target it runs the attackability test `0xb1e9a0` and accepts when it passes. | EX `0xb18000`, `0xb18400` |
| S9 | Harvest classes (`CAbilityHarvestBase` `0xf2b37c`, Lumber, Return, Harvest, Wisp, Acolyte) take `0xd0003`, `0xd0002`, `0xd0032` and `0xd0033`; Lumber, Return and Harvest add `0xd0034`, `0xd0031`, `0xd02b3` and `0xd02b4`. The return branch (`0xb793a0`) is handled only when a field of the ability (`+0x1c8`, the carried-resource state, INF) is non-zero and the target is a unit of the same owner; the resource branch (`0xb79440`) needs a target class bit (`vtable+0x114` flag `0x40`) and, with no unit, a non-unit target. | EX `0xb75d70`, `0xb76a70`, `0xb793a0`, `0xb79440` |
| S10 | Repair class (`CAbilityRepair`, vtable `0xf4ad58`, slot `0xbb55e0`) maps a smart order to its own order (slot `0x350`), validates the target through `0xb28920` and answers `0xdd` when that fails. | EX `0xbb55e0` |
| S11 | Cargo-load class (`CAbilityCargoLoad`, slot `0xb4b980`) takes a smart order or its own order and maps the former to its own order for the check. | EX `0xb4b980` |
| S12 | Rally class (`CAbilityRally`, vtable `0xf16c64`, slot `0xb13520`) takes a target only when `vtable+0x13c` is 0 (a unit) and maps `0xd0003` to `0xd000c`. The recreation's order registry gives `setrally` the id `0xd000c`, `move` `0xd0012`, `attack` `0xd000f` and `harvest` `0xd0032` (read in the Human04 run). The audit's labels for move and attack (A10) were swapped. | EX `0xb13520`; registry run |
| S13 | Inventory class (`CAbilityInventory`, slot `0xb44c30`) overrides the target check. Not decoded; the recreation keeps the existing item pick-up through the `smart` order. | EX class only |

## Data that carries the classes (SB)

| # | Fact |
| --- | --- |
| D1 | The `Order=` profile of the ability names the explicit order: `Ahar`, `Ahrl`, `Aaha`, `Awha` give `harvest`; `Arep` and `Ahrp` give `repair`; `Aloa` and `Sloa` give `load`; `Aren` gives `renew`. |
| D2 | `AbilityData` `targs1` states the target classes: `Ahar` `tree,alive,dead`; `Arep` and `Ahrp` `mechanical,friend,nonancient,ground,air,structure,bridge,alive,dead,invu,vuln`; `Aloa` `ground,friend,invu,vuln`. The resolver does not evaluate them. `mechanical` and `structure` are alternatives in the engine's repair check (`Targetrepair`: "Must target a building or a mechanical unit."), which a flag-by-flag filter would misread. |
| D3 | The Human barracks lists `abilList=Abds` and no rally ability, and has `Trains=hfoo,hrif,hkni` in `HumanUnitFunc.txt`. Three rows (the pocket factories) list `ARal` themselves. The client therefore gives rally to units that train; the recreation reads both. |
| D4 | The peasant lists `Ahar,Amil,Ahrp`; Town Hall `Abdl,Argl`; a gold mine `Agld,Avul`; the hero Arthas `AInv`. |
| D5 | Unit data: `isbldg` and the `type` list (`MECHANICAL`, `PEON`, `TOWNHALL`, ...) in `UnitBalance`, `resource_amount` of mines, the destructable class of trees. |

## Resolution in the recreation

`ObjectHost::resolve_smart_order(unit, target)` in `crates/pcx-play/src/smart_order.rs` gives
the order a unit takes; `campaign_input::issue_smart_orders` applies it to every selected unit
of the local player and queues it with Shift. The ids come from the order registry. The unit's
side comes from its abilities (the `Order=` profile, rally class) and unit data; the target's
side from live state.

| Target | Unit | Result | Status |
| --- | --- | --- | --- |
| Ground | trains (D3) | `setrally` at the point | EX S12 for the id, INF for the ground path |
| Ground | can move | `move` at the point | EX S7 |
| Ground | neither | nothing, silent | EX S7 |
| Item | has an inventory | `smart` on the item | existing path, S13 |
| Item | no inventory | ground rules at the click point | EX S4 |
| Tree | harvest ability with `Ahar` rules | `harvest` on the tree | EX S9, data D1 |
| Tree | harvest ability the runtime does not read (Wisp, Acolyte) | refusal, shown | REC |
| Tree, rock | other | ground rules at the click point | EX S4 |
| Unit, enemy | has a weapon | `attack` | EX S8 |
| Unit, enemy | no weapon | follow (`move` on the target) | DOC |
| Gold mine | worker with `Ahar` rules | `harvest` on the mine | EX S9 |
| Gold mine | harvester the runtime does not read, or a mine it does not read (haunted, entangled) | refusal, shown | REC |
| Own unit | carries cargo, target is a depot taking it | `returnresources` | EX S9 for the class; the runtime returns to the nearest depot, not the clicked one |
| Own or allied structure or mechanical unit | repair ability, under construction | `repair` on the site (resumes construction) | EX S10, DOC |
| Same, damaged | repair ability | refusal, shown: the simulation has no repair | REC |
| Unit that has a load order, or target with one | | refusal, shown: the simulation has no cargo | REC |
| Any other unit (own, allied, neutral) | can move | follow | DOC; EX S7 for the class |
| Self | | nothing, silent | EX S7 |
| Rally structure, target a unit or item | | refusal, shown: rally on a target is not simulated | REC |

A refusal is shown through `CampaignInput::last_error`; silent cases match the client, which
reports nothing for a unit with nothing to do or a click on itself. The recreation issues
the resolved concrete order, so a replay holds `attack`, `harvest` or `setrally` where the
client sends `smart`. The acknowledgement voice treats `harvest` and `repair` as `Yes` (INF).

## Queueing

- Shift held at the click or key press sets the queue flag (S6). The recreation reads it from
  the shared `GameInput` gate into `CampaignInput::queue_orders` once per frame.
- Orders queued: right-click orders, the armed Move, Attack, Patrol and unit-targeted spells,
  Stop and Hold, and immediate abilities. Training, learning and building are not queued.
- `ObjectHost::queue_order` issues at once when the unit has no order and is not holding, and
  otherwise appends to the unit's queue. The queue pops one order per update at which the unit
  reports no order, through the same natives a script uses. An order the unit then refuses is
  dropped and reported. A patrol, a harvest and a hold never finish, so orders behind them
  wait. An order issued by any other route (no Shift, a script, a stop) clears the queue.
- Commands are logged as `PlayerCommand::Queued(<command>)`.
- Length: the client's limit is not established (UNK). The queue is bounded by the host
  (`QUEUED_ORDERS_PER_UNIT`, a resource bound, not a game rule) and a full queue refuses with
  a message.
- Shift keeps targeting armed: after a Shift click on an armed command the cursor stays armed;
  a right click or a click without Shift disarms it. This is documented client behaviour (DOC)
  and was not found in the executable; the target-mode code was not decoded. Treat it as
  unverified until a live capture.

## Verification

- `pcx-play`: `smart_order` unit tests (the rules table on synthetic facts, every branch) and
  `order_queue` tests on the activity fixture (queued move starts on arrival, replacement by
  an unqueued order, hold keeps the queue waiting, bounds, a refused order dropped with a
  reason, not admitted).
- `peacecraft3`: `campaign_smart_order_tests` inject mouse and keyboard messages on the
  synthetic session (Shift as the queue flag, Shift keeping armed standard and unit-targeted
  commands armed, right click with no order is silent).
- Ignored, headless: `selected_chapter_resolves_smart_orders_from_unit_data_and_queues_with_shift`
  runs Human04 with `PCX_WAR3_EXE`, `PCX_NATIVE_CHAPTER_INSTALL`,
  `PCX_NATIVE_CHAPTER_MAP=Maps\Campaign\Human04.w3m`, `PCX_NATIVE_CHAPTER_ADDON=roc`: the
  barracks resolves `setrally` on the ground and refuses a unit target; Arthas resolves a
  move on the ground, an attack on an Undead acolyte, nothing on himself, a follow on a gold
  mine and a move to the point of a tree; a peasant resolves a follow on Arthas and `harvest`
  on the gold mine and on a tree; a Shift right click issues a move, a second queues behind it
  and an unqueued one clears the queue, and both queued commands are logged as queued.
  Human04 starts the player without a town hall, so the harvest runtime refuses to start the
  mine with its own reason ("no gold-accepting depot"); the test asserts that refusal. Harvest
  itself was not run to completion.
- Nothing was rendered to a screen or compared with a capture.

## Not established

- The queue length limit, and the order in which the client reports errors across abilities.
- The point validators (slot `0x25c`) of each class, so what a ground click means for classes
  other than move and rally is inferred.
- The inventory, neutral, haunted and entangled mine, and cargo classes beyond S11.
- Whether the client keeps the cursor armed with Shift, the cursor art and ground markers
  (`0xd0012`, `0xd000f`) of the orders.
- Shift on the minimap right click: the minimap still issues a plain `move`.
- Shift on training buttons (the client trains a batch with Shift; it is not an order queue).
- Live checks: right click on a gold mine, a damaged building and a transport; Shift-queued
  orders behind attack, harvest and construction.
