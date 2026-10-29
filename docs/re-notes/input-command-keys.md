# Command card keys and inventory input

Evidence and decisions for slices 11 and 12 of the [input behaviour audit](input-behaviour-audit.md)
(rows B1, B22, A23). Tags as in the audit: SB shipped data bytes, SS shipped string, EX executable
code read here, DOC documented behaviour not verified here, INF inference, REC recreation choice,
UNK not established.

## Method and identity

- Executable: the selected Legacy `Warcraft III.exe`, SHA-256
  `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, 1.29.2.9231 (same image as
  the audit).
- Data: `War3Local`, `War3xLocal`, `War3x`, `War3` (first match wins) and the loose
  `CustomKeyInfo.txt` and `CustomKeysSample.txt`. Files read: `Units\CommandFunc.txt`,
  `Units\CommandStrings.txt`, `Units\MiscGame.txt`, `UI\FrameDef\GlobalStrings.fdf`, the 17
  `*Strings.txt` unit, ability, item and upgrade files (hotkey census) and `UnitAbilities.slk`.
- Code: rizin with `pdg`. Decompiler output and extracted data stay in the ignored
  `.scratch/input-keys/`. Nothing from the client is copied into the repository except addresses
  and short facts.
- Static analysis, plus the recreation run on the real Human04 chapter data (below). The original
  client was not launched for this slice.

## Command card keys (slice 11)

### What the client does

| # | Fact | Evidence |
| --- | --- | --- |
| K1 | The key-down handler `0x5fc700` (`CSelectMode`) sends a key that is not a control-group digit, F1 to F5, F8 or the backtick to the command card, unless Alt (mask bit 4) or Ctrl (mask bit 2) is set. Shift is not a gate. The card is asked first (`0x638ed0`), then the second button list (`0x639140`, the inventory buttons). A match sends the order through `0x5fb630`. | EX `0x5fc700` (gate `0x5fcd0a`) |
| K2 | `0x638ed0` walks every button of the command grid. A button takes part when its fields `+0x98` and `+0x13c` are non-zero (shown and enabled, INF). Its command data holds the stored hotkey at `+0x5ac`, compared with the key's virtual-key code. The one exception: a stored `0x200` (512) also matches key `0x1b` (Escape). A button carries two orders (`+8` and `+0xc`, sentinel `0xd0142` for none), read here as the on and off order of a toggle. | EX `0x638ed0` |
| K3 | `Hotkey`, `UnHotkey` and `Researchhotkey` are read from the ability profile into the object at `+0x7c`, `+0x88` and `+0x94` (`0x5d55d5` to `0x5d55fe`). Each is a comma list with one entry per rank (`0x5c2470`). An entry of digits only is a number (`27`, `512`). Anything else contributes the code of its first character, with no case folding. A letter hotkey is therefore an uppercase letter, which is what `CustomKeyInfo.txt` states. | EX `0x5c2470`; SB `CustomKeyInfo.txt` |
| K4 | `Buttonpos`, `Unbuttonpos` and `Researchbuttonpos` come from the same profile read. A toggle ability shows the `Un` art and position in its on state (`Aneu`: `Buttonpos=0,0`, `Unbuttonpos=3,2`, `Hotkey=H`, `Unhotkey=H`). | EX `0x5d5507` to `0x5d55c8`; SB `NeutralAbilityFunc.txt`, `NeutralAbilityStrings.txt` |
| K5 | The shipped values: of the 985 `Hotkey=` lines in the 17 strings files, all but 21 are one uppercase letter. The rest are 8 empty, 4 `27` (the cancel commands), 5 shop item digits `1` to `5`, lists `B,B,B`, `C,C,C`, `N,N`, and `CmdPurchase` with a lowercase `s`. There are 155 `Researchhotkey=` and 97 `Unhotkey=` lines, all one uppercase letter. | SB (this census) |
| K6 | `CommandStrings.txt` hotkeys: Move `M`, Attack `A`, Attack Ground `G`, Build `B`, Cancel `27` (four commands), Hold `H`, Patrol `P`, Sell Items (`CmdPurchase`) `s`, Rally `Y`, Hero Abilities `O`, Stop `S`. `CustomKeysSample.txt` gives the cancel commands `512`. | SB |
| K7 | `CommandFunc.txt` positions: Attack Ground `3,1`, Rally `3,1`, Hero Abilities `3,1`, Sell Items `0,0`. Rally and Hero Abilities never share a card, and Attack Ground belongs to units with a splash weapon. | SB; INF for the exclusivity |
| K8 | The `customkeys` option row exists in the option table with fallback `0`; the gameplay panel checkbox is `CustomKeysCheckBox`. `CustomKeyInfo.txt` says the file `CustomKeys.txt` is read from the user data folder `Documents\Warcraft III\CustomKeyBindings\`, or from the install folder, and only when the option is on. The executable holds the strings `CustomKeys.txt` (`0xe9045c`) and `CustomKeyBindings` (`0xeb73d0`, entry 14 of the user-folder name table at `0x79a170`) and imports `SHGetFolderPathW` and `SHGetKnownFolderPath`. How the base folder and file name are joined was not decoded. | EX; SB |

### What the recreation does

| Item | Behaviour | Status |
| --- | --- | --- |
| Dispatch | One path serves a click and its key. The card stores each shown button's hotkey; a pressed key's virtual-key code selects the first enabled button in card order that matches (`CommandCard::button_for_keys`), and the button is activated by the same function a click uses (`campaign_input::activate_button`). Only the buttons of the shown page and unit count, as in K2. A button that is disabled (unavailable building, blocked learn) ignores its key. | REC, K2 |
| Key codes | The hotkey is a stored code (`pcx_play::command_hotkey::Hotkey`): the field parser of K3, compared with the Windows virtual-key code of the physical key (`virtual_keys::code`: letters, digits, NumPad digits, F1 to F12, Escape, Space, Enter, Tab, Backspace). | EX K3, K2 |
| Gate | A key pressed with Ctrl or Alt held is ignored by the card (kept). Shift passes. Closed input (menu, quest log, script dialog, disabled control) disarms targeting and ignores keys. | EX K1 |
| Standard commands | All seven profiles in `Kind::ALL` (Move, Stop, Hold Position, Patrol, Attack, Attack Ground, Rally) read `Buttonpos` and the hotkey from the data. | SB K6, K7 |
| Attack Ground (`G`) | Offered to a unit whose weapon has the splash columns and a supported `splashTargs1`, the condition the `attackground` order accepts. Armed, a click on ground issues the point order. Whether the client shows the button on exactly these units is UNK. | REC |
| Rally (`Y`) | Offered to a unit that trains or has `ARal`, the condition the smart order uses. Armed, a ground click issues `setrally`; a click on a unit or item refuses with a shown reason (a rally on a target is not simulated). | REC, [smart order](input-smart-order.md) |
| Hero Abilities (`O`) and learn keys | `CmdSelectSkill` opens the learn page. In it each skill button carries its `Researchhotkey` for the rank being learned, and cancel carries `27` or `512`. Escape is the cancel key. | SB K6; REC |
| Spells and build menus | Their buttons carry the ability `Hotkey` (the spell's rank), the training and building `Hotkey`, so Chain Lightning now has a key like Holy Light and Water Elemental. | SB; REC |
| Cancel | Both stored codes `27` and `512` answer to Escape (`Hotkey::matches`). Escape also raises the end-cinematic event as before. | EX K2 |
| `CustomKeys.txt` | When the `customkeys` option is on (saved checkbox, else the client fallback row, `0`), the file is looked for in `<home>/Documents/Warcraft III/CustomKeyBindings/` then the install folder (K8). Only `Hotkey`, `Unhotkey` and `Researchhotkey` are applied, by section (object code or `Cmd` profile name, case-insensitive) and rank (the last entry serves higher ranks). Lowercase entries, a multi-character value, a repeated field or a header without `]` refuse with the line number, shown as an input error; the stock keys stay. A missing file is not an error. The file is read once per session, native only (a browser has no user folder). `Buttonpos`, `Tip` and the other fields are ignored. | REC; K3, K8 |

Decision for the location: the user data folder is searched through the home folder
(`USERPROFILE`, else `HOME`) because the base folder join is not decoded (K8). A Wine prefix is
not searched, so there the install folder copy is the one that loads.

### Not done, with the reason

- **Shop (`CmdPurchase`, `s`).** The profile exists and parses, but it is not in `Kind::ALL`. The
  recreation has no shop interface (`purchase_stock_item` has no player command), the rule that
  shows the button is UNK, and read as K3 describes the lowercase `s` is the code of F4, not of S.
  If the client folds case elsewhere (not found), Sell Items and Stop would also share a key on a
  unit that has both. Not bound, nothing shown.
- **`Unhotkey`.** It is parsed in `CustomKeys.txt`, but no button in the recreation is in an on
  state, so nothing reads it. `Aneu` (Select Hero) is the shipped case that would.
- **Learning with Ctrl held.** Documented game behaviour, not in the audit. K1 says the card
  ignores a key with Ctrl held, so no Ctrl branch was found. Not implemented.
- **Single digit hotkeys `1` to `5`.** Shop item buttons. As a number below 10 they cannot equal
  a virtual-key code of the digit row, so how the client matches them is UNK. Not bound.
- **Case folding.** Where, if anywhere, the client folds `s` is UNK (K3 finds none).
- Letters other than A to Z, digits, NumPad and F-keys have no virtual-key mapping here; such a
  custom key never fires.

## Inventory input (slice 12)

### What the client does

| # | Fact | Evidence |
| --- | --- | --- |
| I1 | The slot button update stores `table[slot]` in the button's hotkey field `+0x5ac` (`0x649448` to `0x649455`). The table is six dwords at `0x10dcbcc`: `0x67 0x68 0x64 0x65 0x61 0x62`, the virtual keys NumPad 7, 8, 4, 5, 1, 2. The tooltip builder labels the same slots through a jump table (`0x64927c`, targets `0x649283` to `0x6492a6`, each `mov ebx, digit`): 7, 8, 4, 5, 1, 2, so key = `VK_NUMPAD0` + label for every slot. | EX |
| I2 | The second matcher `0x639140` scans the inventory button list in reverse. A button needs `+0x98` and `+0x13c` non-zero, `+0x5ac` equal to the key, an order other than `0xd0142` and flags other than `0x40`. It outputs the button's target, order and flags to the same dispatcher as the card. The key is subject to the Alt and Ctrl gate of K1. | EX `0x639140` |
| I3 | `ITEM_NAME_HOTKEY` is `%s (|cfffed312NumPad %u|r)`, `ITEM_USE_TOOLTIP` is `Left-Click to Use`, `ITEM_PAWN_TOOLTIP` is `Drop item on shop to sell`. The tooltip shows the key only when the tests at `0x649256` and `0x649262` pass; they were not decoded. | SS; EX |
| I4 | `Units\MiscGame.txt`: `DropItemRange=100`, `GiveItemRange=150`, `PickupItemRange=150`, `PawnItemRange=300`. | SB |
| I5 | Drag between slots moves or trades, onto a unit gives, onto the ground drops. The client's drag orders and the exact range semantics were not decoded. | DOC |

### What the recreation does

| Item | Behaviour | Status |
| --- | --- | --- |
| Slot keys | `inventory_layout::Rules` reads the table, the label jump table and the matcher field from the image and refuses unless all three agree on every slot (I1, I2). The six keys are not constants in the code. `inventory_hotkey` turns a physical NumPad key press into the same use request a click makes. Like the card it ignores Ctrl and Alt and closed input. An empty slot or a slot past the unit's capacity has no enabled button. The physical key is used, so NumLock does not matter. | EX I1, I2; REC |
| Click | Only the left button uses an item, on click (press and release on the slot), so a drag does not use it. Any button pressed on a slot stays off the world. Activation on release rather than press is INF. | SS I3; REC |
| Drag between slots | `ObjectHost::player_move_item(.., ItemDrop::Slot)` moves the item to an empty slot or trades with the item there. It fires no event (INF). A release on the item's own slot changes nothing. | DOC I5; REC |
| Drop on ground | A release inside the world view resolves the ground point under the pointer. Within `DropItemRange` of the unit (centre to point) the item drops at once, on the pointed point. Beyond it the holder walks and drops on arrival. | DOC I5, SB I4; REC |
| Give | A release on a unit gives to it. The target must be the holder's player's unit or an ally, alive, able to carry items (`get_items`) and have a free slot; the item must be droppable. Within `GiveItemRange` the give is at once; otherwise the holder walks and follows a moving target. Events and slot choice are those of `UnitAddItem`: a drop for the holder, a pickup for the receiver, first free slot. | DOC I5, SB I4; REC |
| Refusals | A reason is returned and shown through the input error line: the unit is not yours, no item in the slot, the unit cannot act now (dead or paused), the slot does not exist, the item cannot be dropped, the unit already holds the item, only allies can be given items, the target is dead, cannot carry items or its inventory is full, the unit cannot walk there. | REC |
| Another order | Any later order for the holder cancels a walking drop or give, as it cancels a walking pickup. | REC |
| Released off the view | A release over the interface (not on a slot) does nothing. A release while input is closed is discarded. | REC |

### Not done, with the reason

- **Selling by dropping on a shop** (`ITEM_PAWN_TOOLTIP`, `PawnItemRange`). The host has no sell
  command for the player. A drop on a shop unit gives it and is refused by the ally and carry
  rules with the shown reason.
- **The `NumPad N` tooltip label** (`ITEM_NAME_HOTKEY`). Its display condition (I3) is open.
- Inventory commands are not entered in the player command log, as item use already was not.
- A dead or paused holder refusal is covered by code but not by a test; the host fixture cannot
  kill or pause a unit.
- Whether the client drops the item on the pointed point or at the unit, and whether range is
  centre to centre, are UNK (I5).

## Verification

- `pcx-play`, `test-fixtures`: `command_hotkey` (field parser, cancel codes, rank fallback,
  `CustomKeys.txt` parse and refusals), `standard_command` read, `attack_ground` order offer,
  `inventory_layout::slot_keys` on a relocated synthetic image with every refusal, and the host
  inventory tests: slot trade without events, drop in range and after walking, a walk cancelled
  by another order, give with walking and immediate, every give refusal.
- `peacecraft3`, `test-fixtures`, headless (injected key and pointer messages, no window, pointer
  or audio): card key dispatch (hotkey, both cancel codes, Ctrl and Alt on both sides, Shift,
  closed input, other unit, button without a key), `CustomKeys` option and location, slot click
  by button, slot drag events, NumPad slot selection and its gates, release handling.
- Ignored, run against the local install: `selected_chapter_offers_rally_and_moves_items_between_slots_ground_and_heroes`
  (Human04 with `PCX_WAR3_EXE`, `PCX_NATIVE_CHAPTER_INSTALL`,
  `PCX_NATIVE_CHAPTER_MAP=Maps\Campaign\Human04.w3m`, `PCX_NATIVE_CHAPTER_ADDON=roc`): the barracks
  offers `setrally`, a peasant and Arthas do not; Arthas trades two items between slots, drops one
  inside the real drop range, starts walking for a far point, is refused for a peasant (no
  inventory) and gives to a second hero. `the_selected_client_ships_the_option_off` and
  `supplied_client_inventory_layout_and_tooltip_operands_are_admitted` read the real image.
- Not observed: any of this in the original client, a rendered card, a real keyboard or the
  Legacy `CustomKeys.txt` loader.

## Reproduction

`rizin -q -e scr.color=0 -c 'aa; s <addr>; af; pdg' "Warcraft III.exe"` for `0x5fc700`, `0x638ed0`,
`0x639140`, `0x5c2470`, `0x79a170`. The slot key table: `pxw 24 @ 0x10dcbcc`. Strings:
`izz~otkey`. Data: extract with `mpqxtract get` (archives highest priority first).
