# In-game input behaviour audit

Inventory of the original game's in-game mouse and keyboard behaviour, with the
evidence behind each row and the recreation's status. Drag-box selection is being
implemented separately and appears here only as a dependency. Nothing in this
audit changes code.

## Method and identity

- Executable: the selected Legacy `Warcraft III.exe`, SHA-256
  `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
  VERSIONINFO 1.29.2.9231 (same image as [command card](command-card.md)).
- Data: the Legacy install archive chain `War3Local`, `War3xLocal`, `War3x`,
  `War3` (first match wins) plus the loose `CustomKeyInfo.txt` and
  `CustomKeysSample.txt`. Files read: `UI\MiscData.txt`, `UI\MiscUI.txt`,
  `UI\war3skins.txt`, `Units\CommandFunc.txt`, `Units\CommandStrings.txt`,
  `UI\FrameDef\GlobalStrings.fdf`, `UI\FrameDef\UI\EscMenuOptionsPanel.fdf`,
  `UI\Cursor\HumanCursor.mdx`.
- Code: rizin `aa; aap; aac; aar` and the `pdg` decompiler. Function names are
  rizin's `fcn.<address>`; roles are assigned here. Extracted data and decompiler
  output stay in the ignored `.scratch/input-audit/`. No original bytes or tables
  are copied into the repository; only short facts and addresses are quoted.
- Analysis was static. The original client was not launched, so no row below is a
  runtime observation except where a row cites RO or RO-dbg, which come from the
  later [live capture](input-live-capture.md).

Evidence classes used in every table:

| Tag | Meaning |
| --- | --- |
| SB | Shipped data file bytes (read in this audit) |
| SS | Shipped string table entry, or a string inside the executable |
| EX | Executable code, decompiled in this audit, with address |
| DOC | Widely documented original behaviour, not verified here |
| INF | Inference from the above |
| RO | Runtime observation of the original client under Wine (input synthesised by xdotool, screenshots) |
| RO-dbg | Runtime read of client memory through read-only winedbg breakpoints |
| UNK | Not established |

Recreation status: **OK** implemented and faithful to the evidence, **DIFF**
implemented but different, **MISS** absent, **REFUSE** implemented as an explicit
refusal.

## Original input pipeline (EX)

Facts every later row relies on.

| Fact | Evidence |
| --- | --- |
| The window procedure `0x7be4d0` translates Win32 messages into platform events through `0x7be480`. Left/right/middle/X1/X2 button messages (`0x201`-`0x20c`) map to codes 1, 4, 2, 8, 0x10 (`0x7bd8c0`). Capture is taken on the first press and released when all buttons are up. | EX `0x7be4d0`, `0x7bd8c0` |
| Double-click messages (`0x203`, `0x206`, `0x209`) are not translated into events. Any double-click behaviour is built above this layer. | EX `0x7be4d0` |
| `WM_MOUSEWHEEL` (`0x20a`) becomes platform event 0xb carrying the signed high word of `wParam`. | EX `0x7be4d0` |
| Key events carry the Windows virtual-key code and a modifier mask built from `GetKeyState`: 1 Shift, 2 Ctrl, 4 Alt, 8 Win. Left and right Shift are told apart by scan code. Key repeat (lParam bit 30) is passed on as flag `0x40000000`. F4 with Alt held is special-cased. | EX `0x7bddd0` |
| A stack of hash tables maps (modifier mask, virtual key) to an action id and raises a separate "action" event before the raw key event. Table contents are populated through paths with no static caller and were not recovered. | EX `0x7cd160`, `0x7cd2e0` |
| Game code reads modifiers through `0x83fea0` (Shift: VK 0x10/0xA0/0xA1), `0x83fdc0` (Ctrl: 0x11/0xA2/0xA3), `0x83fca0` (Alt: 0x12/0xA4/0xA5). | EX |
| Frame-level events: `0x1a0064` mouse down, `0x1a0065` mouse up, `0x1a0068` world-drag begin (posted for left and middle, not right), `0x40060064`/`0x40060066` key down/up, `0x400500cd` wheel (`+0x2c` is the signed delta). Mouse button field: 1 left, 2 middle, 4 right. | EX `0x610880`, `0x5fba10`, `0x5e12f0` |
| Input mode objects: `CSelectMode` (events at `0x5fba10`), `CDragSelectMode` (left drag), `CDragScrollMode` (middle drag, `0x63bd00`), `CInputMode` base (`0x63c380`). `CCameraWar3` (vtable `0xe92288`) owns camera keys, wheel and edge-scroll events. | EX, RTTI |
| Gameplay input is synchronised as net commands (RTTI `CNetCommand*`): unit orders, selection modify, define/select control group, select/refresh subgroup, arrow key, mouse, ally ping, pause, cheat commands. | EX `0x5a7470`, RTTI |

## A. Mouse

| # | Action | Original behaviour | Evidence | Lives in | Recreation | Handler |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | Left click a unit | Selects on button press, not release. The down handler tests a per-unit selectable flag before selecting. Own, allied, enemy and neutral units are all selectable. | EX `0x5fd820` (down handler calls `0x5fff10`); flag name INF; enemy/neutral DOC | exe | DIFF: selects on press (OK). Picks the nearest unit whose selection circle (UnitUI `scale` times `[SelectionCircle] ScaleFactor` 72, halved) covers the clicked ground point. The original pick method (mesh, circle or bounds) is UNK. | `campaign_input.rs:1031`, `:1133` |
| A2 | Left click on ground | Keeps the selection; an empty drag box (1 to 100 px) keeps it too. The earlier DOC entry (deselects) is refuted by the live capture. The no-unit path of `0x5fd820` calls `0x5ffdd0`, which returns at once for a null unit. | RO ([live capture](input-live-capture.md)); earlier DOC wrong | exe | OK: a click on empty ground keeps the selection | `campaign_input.rs:1244` |
| A3 | Shift+click | Toggles: adds a unit, a second Shift+click removes it. Shift+click on a building with units selected replaces the selection with the building. The shift read is at `0x5fff10`. | EX read; RO ([live capture](input-live-capture.md)) | exe | DIFF: adds only, never removes | `campaign_input.rs:1090`, `:1246` |
| A4 | Ctrl+click | Selects all visible units of the same type. The click path reads Ctrl at `0x83fdc0`, builds a visible same-type list (`0x6010e0`) and selects it (`0x5ffae0`). Confirmed live (4 of 4 footmen). | EX `0x5fff10`; RO | exe | MISS | none |
| A5 | Double-click | Same-type-on-screen selection when the second press follows the first within 500 ms (press to press; yes at 493.5 ms, no at 506.2 ms). No OS double-click event reaches the game (see pipeline) and the exe imports no `GetDoubleClickTime`. | RO ([live capture](input-live-capture.md)) | exe | MISS | none |
| A6 | Selection cap | 12 units. The select-control-group dispatch limits the result to `12 - current count` in add mode, else 12. | EX `0x6cbf40` | exe | MISS: no cap | `crates/pcx-play/src/selection_host.rs:37` |
| A7 | Click on selected unit in a group | Information-panel group buttons: click selects that unit (and makes it primary), Shift removes it, Ctrl selects its type among the selection. | EX `0x5fc020` (Shift via `0x83fea0`, Ctrl via `0x83fdc0`); button meaning INF | exe | MISS: portrait and unit panel have no pointer handling | `campaign_unit_panel.rs`, `campaign_portrait.rs` |
| A8 | Left click on hero/portrait buttons | Event `0x30067` calls `0x5fff10` with the unit stored on the clicked button, so Shift and Ctrl follow A3/A4. That the button is the hero button is INF. | EX `0x5fb580`; role INF | exe | MISS | none |
| A9 | Drag box | Left press on world posts `0x1a0068`, starting `CDragSelectMode`; release calls `0x5fff10` per unit (`0x5f9b40`). Cap and which units qualify not yet read. No drag threshold: a 1 px move already acts as a box, using the same unit region as a click. | EX `0x610880`, `0x5fe070`, `0x5f9b40`; RO ([live capture](input-live-capture.md)) | exe | other agent | n/a |
| A10 | Right click on ground | Issues the **smart** order (id `0xd0003`) at the point. Order flags: bit0 Shift (queue); bits 2-4 come from `0x62f120`, the selection size and an option check (formation related, INF). Validity is checked first (`0x4f5d70`); a failure reports a `[Errors]` string. Click feedback uses attack/move markers (`0xd0012`/`0xd000f`). | EX `0x5fd2f0`; id names DOC (smart 851971, attack 851986, move 851983) | exe; `[Errors]` in `Units\CommandStrings.txt` (SB) | DIFF: always Move for non-enemy ground | `campaign_input.rs:1291`-`1349` |
| A11 | Right click on enemy | Smart order with target (`0x5fd4c0`), resolved to attack. | EX | exe | OK for attack, via `Kind::Attack` | `campaign_input.rs:1291` |
| A12 | Right click on friendly unit, resource, building, item, transport | Smart order resolves to follow, harvest/return, repair, rally, pick up, load. The resolution table was not read. Rally behaviour text is in `CmdRally` Ubertip (SB). | DOC; resolver UNK; rally text SB | exe | MISS except item pick-up (`"smart"` target order to an item) | `campaign_input.rs:1270` |
| A13 | Shift+right click | Queued smart order (flag bit0). | EX `0x5fd2f0` | exe | MISS | none |
| A14 | Right click while a command is armed | Cancels targeting. | DOC; Esc path EX `0x5fc700` is separate | exe | OK | `campaign_input.rs:788` |
| A15 | Left click while a command is armed | Issues the armed order. Shift keeps targeting for queued orders. | DOC | exe | DIFF: clears targeting after one click, no queue | `campaign_input.rs:1159`-`1243` |
| A16 | Middle button drag | `CDragScrollMode`: press (code 2) on world starts a drag scroll (`0x5fd0d0` to `0x5f9ed0`), release ends it. Scroll ratio not read. | EX `0x63bd00`, `0x5fd0d0` | exe | OK (ground-grab drag, INF; see [camera input](input-camera.md)) | `campaign_camera_input.rs` |
| A17 | X1/X2 mouse buttons | Delivered as button codes 8 and 0x10; no game binding located. | EX platform; binding UNK | exe | MISS | none |
| A18 | Mouse wheel | One zoom step per 120 units of delta (`delta / 0x78`, toward zero); positive zooms in (level + 1), negative zooms out. See C1. | EX `0x5e12f0` | exe | OK | `campaign_camera_input.rs` |
| A19 | Edge scroll | Eight direction masks (1 up, 2 down, 4 left, 8 right and sums) run the same scroll helpers as arrow keys with the mouse multiplier. Measured live: the pointer scrolls when within 0.004 of an edge in the 0.8 x 0.6 UI space (strict; 6.4 px horizontal and vertical at 1280x960), with no trigger delay, constant speed equal to arrow-key scroll, and a directional arrow cursor. The producer's address was not located. | EX `0x5dfd50`, `0x5e12f0`; RO ([live capture](input-live-capture.md)) | exe; `mousescroll`/`mousescrolldisable` options | DIFF: margin from the live fit, options from the descriptor fallbacks; no arrow cursor | `campaign_camera_input.rs` |
| A20 | Click on minimap | Left: camera jump with held-drag follow. Right: move/smart order at the point. Both gated like world input. | DOC; recreation per `campaign-minimap.md` | exe | OK (camera, drag, point order); right click is Move not smart | `campaign_minimap.rs:138`, `campaign_input.rs:753` |
| A21 | Alt+click world or minimap | Minimap signal ("ping") to allies, same as the signal button. Text states Alt+left-click. Alt-G arms the signal. | SS `MINIMAPSIGNALTOOLTIP_UBER`, MiscUI `MinimapSignal='G'`; net command `CNetCommandAllyPing` RTTI | data | MISS | none |
| A22 | Click command-card buttons | Press consumes the pointer; the button issues its command. | OK | | OK | `campaign_command_card.rs:373`, `campaign_input.rs:107` |
| A23 | Click inventory slot | Tooltip says "Left-Click to Use". Drag between slots moves, onto units gives, onto ground drops (DOC). | SS `ITEM_USE_TOOLTIP`; drag DOC | data | DIFF: any pointer button uses the item (the observer never reads the button); no drag | `campaign_inventory.rs:157` |
| A24 | Click upper buttons (quests, menu, allies, chat) | Open dialogs; F9/F10/F11/F12 shortcuts. | SS | data | OK for quests and menu; allies, chat and log MISS | `campaign_upper_buttons.rs:162` |
| A25 | Hover tooltips | Command buttons, inventory, upper buttons, resource bar, time of day. Hover over a world unit highlights it (`CPreselectUI`) and sets the hover cursor mode. | SS; cursor mode EX `0x5fbdf0`, `0x5fda30`; `CPreselectUI` RTTI | data | DIFF: HUD tooltips exist (`tooltip.rs`); no world hover highlight or unit-name hover | `tooltip.rs:192` |

## B. Keyboard

| # | Key | Original behaviour | Evidence | Lives in | Recreation | Handler |
| --- | --- | --- | --- | --- | --- | --- |
| B1 | Command-card hotkeys | Per-ability `Hotkey`/`Unhotkey`/`Researchhotkey` letters from the ability data and `CommandStrings.txt` (Move `M`, Attack `A`, Attack Ground `G`, Stop `S`, Hold `H`, Patrol `P`, Rally `Y`, Hero Abilities `O`, Build `B`, Shop `s`). Cancel is `27` in `CommandStrings.txt` and `512` in the CustomKeys sample. Dispatch ignores the key when Ctrl or Alt is held; Shift is read for queueing. | SB; EX `0x5fc700` (Alt/Ctrl gate at `0x5fcd0a`, matching `0x638ed0`/`0x639140`) | data files | DIFF: letters from data for Move/Stop/Hold/Patrol/Attack, training, build menus, Holy Light, Water Elemental. MISS: Attack Ground, Rally, Shop, hero-abilities `O`, learn (`Researchhotkey`) and `Unhotkey` keys; Shift queue. OK: a key pressed with Ctrl or Alt held is ignored. | `campaign_input.rs:834`, `standard_command.rs:6` |
| B2 | Digits 1-9, 0 | `1`..`9` are groups 0..8, `0` is group 9. | EX `0x5fc700` | exe | OK: the binding is decoded from the image | `campaign_control_groups.rs`, [note](input-control-groups.md) |
| B3 | Ctrl+digit | Defines the group from the current selection (replaces; helper `0x4f4fb0` also runs on this path, purpose UNK). Empty or foreign selections report `Emptycontrolgroup`/`Invalidcontrolgroup`/`Nonunitincontrolgroup`. | EX `0x5fc700` (`0x6cc8e0` is the define dispatch); error texts SB | exe; `[Errors]` | OK: refusals use the shipped texts | `campaign_control_groups.rs`, [note](input-control-groups.md) |
| B4 | Shift+digit | Adds the current selection to the group (extends and dedupes). Confirmed live: the group gains the selection and the current selection does not change. Ctrl+Shift+digit behaves as Ctrl+digit (replace). Decompiler output for this branch is noisy. | EX; RO ([live capture](input-live-capture.md)) | exe | OK | `campaign_control_groups.rs::shift_digit` |
| B5 | Digit (no modifier) | Selects the group (`0x6cbf40`). A second press of the same digit within 0.5 s (float compare `last + 0.5 <= now`) centres the camera on one unit (`0x4f2070` result passed to `0x5eae20`; INF that it is the selection's primary unit) and resets the memory. Live: the window ends between 492.9 and 498.9 ms press to press, and the first press never moves the camera. | EX `0x5fc700`; RO ([live capture](input-live-capture.md)) | exe | OK; the window is decoded from the image | `campaign_control_groups.rs`, [note](input-control-groups.md) |
| B6 | Tab, Shift+Tab | Cycles the active subgroup (type) of a mixed selection; the hero subgroup is active first, portrait, info panel and command card follow, and the selection list is unchanged. With two subgroups Tab and Shift+Tab cannot be told apart. `CNetCommandUnitSelectSubGroup` exists in the exe; the key handler was not located. The option `subgrouporder` ("Subgroup order modifier key") feeds it. | RTTI; RO ([live capture](input-live-capture.md)) | exe | MISS | none |
| B7 | F1-F5 | Virtual keys `0x70`-`0x74` call `0x5eb390`, which indexes the hero-bar array (index limit 7) and selects that hero. | EX `0x5fc700`, `0x5eb390` | exe | MISS | none |
| B8 | F8 and `` ` `` | Both call one virtual method on a member of the game UI object (`0x5eb420`). Live: both select an idle worker and centre the camera on it; a second press within about 1.0 s advances to the next idle worker, later restarts at the first. No effect without idle workers. | EX; RO ([live capture](input-live-capture.md)) | exe | DIFF: both keys select and centre on the next idle worker (list and window INF); the FPS overlay moved to F6 | `campaign_camera_input.rs`, `pcx-gfx/src/performance.rs` |
| B9 | F9 Quests, F10 Menu, F11 Allies, F12 Chat/Log | Shown as the shortcuts in the upper-button tooltips. | SS `UPPER_BUTTON_*_TIP`, `KEY_*` | GlobalStrings | OK for F9/F10 (derived from `KEY_QUESTS`/`KEY_MENU`); MISS F11, F12 | `campaign_upper_buttons.rs:640`, `campaign_camera.rs:273` |
| B10 | Esc | Cancel entry in the command card (`Hotkey=27`). Closes menus. Also raises the player end-cinematic event (the recreation's prior note records that the original raises it on every Esc). | SB; EX unread for Esc order of precedence | data | OK in effect: clears targeting, closes skill page, raises event; precedence versus an open modal matches the escape-menu note | `campaign_input.rs:634` |
| B11 | Enter, Ctrl+Enter, Shift+Enter | Opens chat. Ctrl+Enter sends to allies, Shift+Enter to everyone. | SS `UPPER_BUTTON_CHAT_UBER` | data | MISS (no chat) | none |
| B12 | Arrow keys | Scroll the camera (see C1). | EX | exe | OK; applies `keyscroll` | `campaign_camera_input.rs` |
| B13 | Page Up, Page Down | Zoom in and out one level. | EX `0x5e12f0` | exe | OK | `campaign_camera_input.rs` |
| B14 | Insert, Delete | Hold: camera swings to the second or third rotation state; release returns (see C2). Numpad 0 and Numpad `.` key-ups also clear the state. | EX `0x5e12f0` | exe | OK | `campaign_camera_input.rs` |
| B15 | Space | Jumps to the latest alert location; pressing again within 1.0 s steps back through the history. Disabled when the game UI object's `+0x298` is set (meaning UNK). | EX `0x5e1930` | exe | DIFF: history in place, no alert producer yet | `campaign_camera_input.rs` |
| B16 | Backspace | Cycles the camera through a sorted object list; within 1.0 s of the last press it advances, otherwise it restarts at the first entry. The list holds the own Town Hall (live: a melee start with a worker selected jumps to the hall; Human01 with no own building does nothing), not the selection or workers; class tag `0x2b61676c`, flag `0x100` clear. Several halls and the 1.0 s window were not exercised live. | EX `0x5e1120`; RO ([live capture](input-live-capture.md)) | exe | DIFF: own Town Halls in creation order (sort key UNK) | `campaign_camera_input.rs` |
| B17 | Ctrl+C, Alt+C | Centre on the selection. The key letter comes from `MiscUI.txt [Hotkeys] UnitCenter` (`C`); the handler requires Ctrl or Alt in the modifier mask. | SB; EX `0x5e12f0` default branch, `0x5dced0` | data | MISS | none |
| B18 | Alt, `[` , `]` | Alt, `[` (VK `0xDB`) and `]` (VK `0xDD`) change unit health bar display while held (`0x601050`); option `healthbars` inverts. DOC: `[` own, `]` enemy. | EX `0x5fc700`, `0x5fd040`; SS `HEALTH_BARS_INFO` | exe | MISS | none |
| B19 | Alt+F | Toggles formation movement (also an Alt-hold inversion on orders). | SS `FORMATION_*`, MiscUI `FormationToggle='F'`; EX flag in `0x5fd2f0` | data | MISS | none |
| B20 | Alt+S/L/H/O/Q, Ctrl+M, Ctrl+S | Quick save, quick load, help, options, quit; music and sound-effect toggles. Letters in `MiscUI.txt [Hotkeys]` (`S`, `L`, `H`, `O`, `Q`, `M`, `S`). The modifiers appear only in that file's comments. | SB (letters, comments) | data | MISS | none |
| B21 | Alt+T, Alt+A, Alt+R, Alt+G | Minimap terrain, ally colours, creep camps, signal. | SS tooltips, MiscUI | data | MISS | none |
| B22 | Inventory slots | Slots 1-6 use NumPad 7, 8, 4, 5, 1, 2. | EX `0x649283`-`0x6492a6` (label table); SS `ITEM_NAME_HOTKEY` | exe | MISS | none |
| B23 | Pause, +, - | Replay panel shows Pause, `+`, `-`. Single-player campaign pause is the Pause entry in the F10 menu. | SS `REPLAY_TOOLTIP_*` | data | MISS | none |
| B24 | Script dialog buttons | Authored button hotkeys match Enter (13), Esc (27) and character keys case-insensitively. | EX/SB | data | OK | `campaign_script_dialogs.rs:169` |
| B25 | Window keys (Alt+Tab, PrintScreen, Alt+F4) | F4 with Alt is special-cased in the key translator; how is not read. Others UNK. | EX `0x7bddd0`; rest UNK | exe | n/a | none |

## C. Camera

| # | Behaviour | Original | Evidence | Lives in | Recreation |
| --- | --- | --- | --- | --- | --- |
| C1 | Scroll speed | Key scroll vector magnitude = (`keyscroll` / 100 + 0.5) x 3000 (the camera integrates it over time); edge scroll uses `mousescroll` the same way. Lateral motion uses yaw minus pi/2; both axes can run at once and are not renormalised. Values over 100 are ignored (multiplier stays 1.0). | EX `0x5dced0` (the `CCameraWar3` constructor stores the multipliers at `+0x4ec` mouse and `+0x4f0` key, and `mousescrolldisable` at `+0x30`), `0x5e07a0`, `0x5e52f0` | exe literal 3000.0; `[CameraRates] Forward`/`Strafe` = 3000 (SB) agrees; option defaults `mousescroll` 50, `keyscroll` 50, `mousescrolldisable` 0 | OK: `[CameraRates]` times the option multiplier, options from the descriptor fallbacks (no saved preferences yet). `campaign_camera_input.rs` |
| C2 | Zoom levels | Six discrete levels, clamped to 0..5, changed by wheel and Page Up/Down. A change eases over 1.0 s. | EX `0x5e1090`, `0x5de2c0` | exe, `[Camera]` tables | OK |
| C3 | Zoom table | `[Camera]` has six entries per field. The camera routine's code literals match `AOA`, `FOV`, `Rotation`, `Distance` and `FarZ` except at the last level: literal Distance 1200 and AOA 335 versus file 1100 and 339. The live camera settles at the code literals: a debugger read at level 5 gives Distance 1200 and AOA 335 (levels 0 to 5: 1650, 1600, 1500, 1400, 1275, 1200 and 304, 311, 318, 325, 332, 335). | SB, EX `0x5de2c0`; RO-dbg ([live capture](input-live-capture.md)) | both | OK: the literals are read from the image at runtime |
| C4 | Rotation states | `Rotation` has 18 entries: 90 degrees x6 (normal), 20 x6 (Insert held), 160 x6 (Delete held). Index = level + 6 x state. Press and release ease over 0.5 s. | SB, EX `0x5de2c0`, `0x5e12f0` | `[Camera] Rotation` | OK: tables read from the image |
| C5 | Camera lock | The camera handler returns at once when `+0x2c` is set. Edge events also stop when `+0x30` (the `mousescrolldisable` value) is set; wheel, Page Up/Down and Insert/Delete stop when `+0x5c` is set; key, wheel and edge-start events stop when `0x5e0ba0` is true. The meaning of `+0x2c`, `+0x5c` and `0x5e0ba0` is UNK. | EX `0x5e12f0` | exe | DIFF: scripts disabling control, menus, dialogs and the quest log gate it |
| C6 | Bounds | Clamped to the camera bounds; margins `[CameraMargins]` 4/4/2/2 cells. | SB | data | OK (`scroll_to`) |
| C7 | Smoothing | `[Camera] Smoothing=0.00` and the `Smooth*` keys are defined; use not traced. | SB; use UNK | data | UNK |

## D. Cursor and feedback

| # | Behaviour | Original | Evidence | Recreation |
| --- | --- | --- | --- | --- |
| D1 | Cursor art | A per-race MDX model, `Cursor=UI\Cursor\<Race>Cursor.mdl` in `war3skins.txt`. Sequences in the Human cursor: Target, Normal, Select, TargetSelect, InvalidTarget, HoldItem, Scroll Left, Scroll Right, Scroll Up, Scroll Down, Scroll Up Left, Scroll Up Right, Scroll Down Left, Scroll Down Right. | SB | MISS: system cursor |
| D2 | Scroll arrows | Cursor mode values `0xa`..`0x11` map left, right, up, down, up-left, up-right, down-left, down-right (mask 1 up, 2 down, 4 left, 8 right) and align with the MDX scroll sequences when offset by 4. | EX `0x5dfd50`; alignment INF | MISS |
| D3 | Hover states | `0x5fbdf0` and `0x5fda30` set modes 0-3 (`0x5fbdf0`) and 4, 6, 7 (`0x5fda30`) from the unit under the cursor and the armed command. Live: the gauntlet is green over own units and buildings, yellow over neutral, red over enemies, grey-blue elsewhere; with attack armed the crosshair is cyan over ground, green own, yellow neutral, red enemy; with a spell armed it stayed cyan over every target. The mapping of mode values to sequence names is UNK for modes below 10. | EX; RO ([live capture](input-live-capture.md)) | MISS |
| D4 | Placement and area cursors | `PlacementCursor` texture per race in `war3skins.txt`; building placement colours `[Placement]` (SB). | SB | DIFF: `campaign_build_ui` handles building placement; spell area cursor MISS |
| D5 | Selection circle colours | `[SelectionCircle]` friend/neutral/enemy colours and `[TargetingColors]`. | SB | other agent / selection circle note |
| D6 | Error line | Order failures post `[Errors]` strings above the console. | SB, EX `0x5ecc20` | DIFF: `last_error` is a log field, not a world message |
| D7 | Ping and waypoint colours | `[PingColor]` set. | SB | MISS (no ping) |

## E. Options and data that gate input

| Key | Meaning | Exe default (fallback row) | Source |
| --- | --- | --- | --- |
| `mousescroll` | Edge scroll speed slider, 0-100 | 50 | exe option table row, SB slider range in `EscMenuOptionsPanel.fdf` |
| `mousescrolldisable` | Disables edge scroll | 0 | same |
| `keyscroll` | Arrow key speed slider, 0-100 | 50 | same |
| `tooltips` | Tooltip option (INF: "Enhanced Tooltips") | 1 | same |
| `healthbars` | Always show health bars (Alt hides) | 0 | same, SS `HEALTH_BARS_INFO` |
| `formations` / `formationtoggle` | Formation movement and Alt toggle | 1 / 1 | same |
| `subgrouporder` | Subgroup order modifier key | 0 | same |
| `customkeys` | Use `CustomKeys.txt` | 0 | same; file format in `CustomKeyInfo.txt` (SB) |
| `herobar` | Hero bar shown | 1 | same |

The fallback table (80 rows, base `0xe68bd0`, stride 16) is read by
`pcx_play::options_defaults`; see [original options defaults](original-options-defaults.md).
Fallbacks are not the player's saved values.

## F. What the recreation already matches

- Select on press with Shift add (A1, partly A3); right-click attack and move;
  item smart-target (A10-A12 partly).
- Armed-command targeting with right-click cancel (A14), Esc clearing targets.
- Arrow-key scroll direction and rate at default options (C1).
- Minimap left camera jump and drag, right-click point order, input gating.
- Command-card clicks and letter hotkeys for the five standard commands, training,
  build menus, Holy Light, Water Elemental.
- F9 and F10 derived from `GlobalStrings.fdf`, Esc layering over the escape menu
  and quest log, script-dialog hotkeys.

## Gap ranking

Ordered by user-visible impact in a campaign playthrough (first is worst).

| Rank | Gap | Rows | Why it ranks here |
| --- | --- | --- | --- |
| 1 | Smart right-click: harvest, return cargo, repair, follow, rally, load | A10, A12 | Workers cannot gather or build economy by the normal mouse flow |
| 2 | Control groups (Ctrl/Shift/digit, 0.5 s double-tap centre) | B2-B5 | Core multi-unit control and the most used keys |
| 3 | Shift-queued orders and Shift-held targeting | A13, A15, B1 | Build orders and patrol routes depend on it |
| 4 | Zoom: wheel and Page Up/Down, six levels with 1.0 s easing | A18, B13, C2 | Constant use; the camera is fixed today |
| 5 | Selection rules: cap 12, Shift toggle, Ctrl and double-click same-type, group-button clicks | A3-A7 | Large armies are unmanageable and cannot be trimmed |
| 6 | Edge scroll and the scroll-arrow cursor | A19, D2 | Default mouse navigation; blocked by margin capture (UNK) |
| 7 | Cursor art and hover state | D1, D3 | Visible on every frame; the original shows target validity there |
| 8 | Hero keys F1-F5 and hero button clicks | B7, A8 | Hero play in every campaign mission |
| 9 | Tab subgroup cycling | B6 | Needed for mixed selections; handler UNK |
| 10 | Camera jumps: Space, Backspace, Ctrl/Alt+C | B15-B17 | Frequent in combat; needs an alert history |
| 11 | Insert/Delete rotation states | B14, C4 | Visible, less used |
| 12 | Middle-button drag scroll | A16 | Common for mouse users |
| 13 | Command-card completeness: Attack Ground `G`, Rally `Y`, Shop, `O`, learn keys, Ctrl/Alt gate, `27` vs `512` cancel | B1 | Several commands cannot be used by key |
| 14 | Inventory: left-click only, NumPad slots, drag/drop | A23, B22 | Wrong button uses items; no key route |
| 15 | Minimap Alt-ping, minimap buttons | A21, B21 | Multiplayer-oriented; low in campaign |
| 16 | Chat and ally keys F11/F12, Enter family | B9, B11 | Needs the chat dialogs first |
| 17 | Health-bar modifiers Alt, `[` , `]` | B18 | Visible but optional |
| 18 | World hover highlight and unit tooltip | A25 | Polish |
| 19 | Formation toggle | B19 | Only matters with mixed groups |
| 20 | Scroll options applied (`mousescroll`, `keyscroll`, disable) | C1, E | Defaults already match |
| 21 | Error line instead of a log field | D6 | Players cannot see why an order failed |
| 22 | `CustomKeys.txt` | E | Opt-in |
| 23 | Quick keys Alt-S/L/H/O/Q, Ctrl-M/S | B20 | Needs save/load first |
| 24 | Pause/replay keys | B23 | Not campaign-critical |
| 25 | X1/X2 buttons, window keys | A17, B25 | Binding not located |

Drag-box selection (A9) is the other agent's work and not ranked.

## Ordered implementation slices

Each slice is sized for one agent and keeps data in the asset-derived path (no
copied constants; read keys, rates and tables from the data and the supplied exe).

1. **Live capture of the unknowns. Done**, see [input live capture](input-live-capture.md):
   edge-scroll margin, trigger and speed, drag threshold (none), double-click window
   (500 ms), control-group double tap, Tab, hover and armed cursors, Shift+digit,
   Backspace list, F8 and `` ` `` (idle worker), and the zoom table (C3, literals
   win). Still open: Shift+Tab with three or more subgroups, cursor mode to sequence
   names, invalid-target and hold-item cursors, edge-scroll speed in world units and
   its option dependence, multi-hall Backspace, selection cap 12.
2. **Modifier and gate foundation.** One input-state resource for Shift, Ctrl,
   Alt (left and right) and a single "game input open" gate reused by `click`,
   hotkeys and camera. Read the option table (`mousescroll`, `keyscroll`,
   `mousescrolldisable`) from the exe fallbacks and the player's saved values.
3. **Control groups.** Digits, Ctrl define, Shift add, selection cap 12,
   0.5 s double-tap centre, the three `[Errors]` strings.
4. **Zoom and rotation. Done**, see [camera input](input-camera.md). Six levels via wheel and Page Up/Down with 1.0 s easing;
   Insert/Delete rotation states with 0.5 s easing; distance and angle per level from the exe's
   literals (the live camera ignores the `[Camera]` file values at level 5).
5. **Smart right-click.** Resolve target classes (resource, friendly unit,
   own building, item, transport) to harvest, return, follow, rally, load.
   Start from the existing harvest and rally order paths.
6. **Order queueing.** Shift on right-click and on armed targeting; Shift+hotkey.
7. **Selection rules.** Shift toggle, Ctrl and double-click (500 ms) same-type,
   subgroup model, Tab and Shift+Tab, group-button clicks, F1-F5 hero keys. A click
   on empty ground keeps the selection (already the recreation's behaviour).
8. **Edge scroll and drag scroll. Done**, see [camera input](input-camera.md). Margin 0.004 of the 0.8 x 0.6 UI space (strict,
   per axis) from the live capture, `mousescroll` multiplier,
   disable flag, middle-button drag.
9. **Cursor.** Load the per-race cursor MDX from `war3skins.txt`, drive sequences
   from hover state, armed state and scroll direction.
10. **Camera jump keys. Space, Backspace, F8 and backtick done** (Ctrl/Alt+`UnitCenter` open), see [camera input](input-camera.md). Space (alert history, 1.0 s), Backspace (list cycling,
    1.0 s, own Town Hall), F8 and `` ` `` (idle worker, 1.0 s cycle; the recreation's
    F8 debug overlay must move), Ctrl/Alt+`UnitCenter`.
11. **Command card keys.** Generic `Hotkey`/`Unhotkey`/`Researchhotkey` dispatch,
    Attack Ground, Rally, Shop, `O`, Ctrl/Alt gate, cancel `27`/`512`,
    optional `CustomKeys.txt`.
12. **Inventory input.** Left-click only, NumPad slot keys, drag between slots,
    drop and give.
13. **Chat and dialogs.** Enter family, F11 and F12.
14. **Minimap and overlays.** Alt-click ping, minimap buttons, health-bar
    modifiers, world hover highlight, error line.

## Not established

Resolved by the [live capture](input-live-capture.md): edge-scroll margin, trigger
and speed (A19), drag threshold and double-click window (A5, A9), Tab semantics
with two subgroups (B6), F8 and `` ` `` and the Backspace list (B8, B16), the last
zoom level (C3), Shift+digit and Ctrl+Shift+digit (B4).

- The edge-scroll producer's address, and the margin constant's location in the exe
  or data (the value, 0.004 of the 0.8 x 0.6 UI space, is a fit to measurements).
- Edge-scroll speed in world units, and the effect of `mousescroll`.
- The Tab key handler, and Shift+Tab direction with three or more subgroups (B6).
- The target-class resolution table behind the smart order (A12).
- The key-binding hash table contents (action ids behind `0x7cd160`), which may
  hold keys not seen in `CSelectMode` and `CCameraWar3`.
- The hero-bar index semantics of F1-F5 (B7), and several-hall Backspace cycling (B16).
- What `Smoothing` does (C7).
- Mapping from cursor mode values below 10 to sequence names, and the invalid-target
  and hold-item cursors (D3).
- X1/X2, PrintScreen and Alt+Tab handling (A17, B25).
- Observer, replay and multiplayer-only input paths were not read.

## Reproduction

All steps are local and read-only. `mpqxtract get <archives, highest priority
first> '<path>' <out>` extracts a data file. For code, analyse with
`aa; aap; aac; aar`, then `s <address>; pdg`. Event handlers sit in the vtables
of the RTTI classes named above (`CSelectMode` vtable `0xe94044`,
`CCameraWar3` `0xe92288`, `CGameUI` `0xe928a4`).
