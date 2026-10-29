# Input: error line, chat and dialog keys, minimap and overlays

Evidence and decisions for slices 13 and 14 of the
[input behaviour audit](input-behaviour-audit.md). Evidence tags are the audit's: SB
shipped data, SS string, EX executable code read here, DOC documented but not verified,
INF inference, UNK not established. Nothing here is a runtime observation: the original
client was not run for this note.

## Method and identity

- Executable: Legacy `Warcraft III.exe`, SHA-256
  `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, 1.29.2.9231 (the
  audit's image).
- Data: `Units\CommandStrings.txt`, `UI\MiscUI.txt`, `UI\MiscData.txt`,
  `UI\war3skins.txt`, `UI\FrameDef\GlobalStrings.fdf`, `UI\FrameDef\UI\ChatDialog.fdf`,
  `UI\FrameDef\UI\UpperButtonBar.fdf`, `UI\Minimap\Minimap-Ping.mdx`, read through the
  archive chain `War3Local`, `War3xLocal`, `War3x`, `War3`.
- Code: rizin `aa; aap; aac` and `pdg`. Function names are rizin's `fcn.<address>`; roles
  are assigned here. Decompiler output stays in the ignored `.scratch/`.

## 1. The error line

A refusal is posted to a single line above the console. The client's own comment in
`MiscUI.txt [FontHeights]` calls it "single line of error text that appears above the
console" (`WorldFrameMessage=0.015`, SB).

| # | Fact | Evidence |
| --- | --- | --- |
| E1 | `0x5ecc20` takes an error code, maps it to an `[Errors]` key through the name switch `0x5f0840` (the control-group codes 4 to 6 are in [control groups](input-control-groups.md)), looks the text up with `0x5c1ab0(key, variant)` and posts it through `0x5ed6c0`. A key with no text posts nothing and plays no text. | EX `0x5ecc20`, `0x5ed6c0` |
| E2 | `0x5ed6c0` first clears the frame at `UI+0x424` (`0x88b7b0`), then calls its virtual method at vtable offset `0x70` (`0x88b510`) with the text, the colour word `0xFFFED312` and the hold time `10.0` (`0x41200000`). If the text lookup fails it posts the fixed string `Unknown Error!` the same way. So one error is on screen at a time and a new one replaces it. | EX `0x5ed6f3`, `0x5ed713`, `0x5ed72b`, `0x5ed73c`, `0x5ed74b` |
| E3 | The colour word is the gold of the HUD (`|Cfffed312` in `GlobalStrings.fdf`): R `0xFE`, G `0xD3`, B `0x12`, alpha `0xFF`. | EX, SB |
| E4 | The frame is a `CSimpleMessageFrame` (vtable `0xec92d8`, constructor `0x88b2b0`) of `0.38 x 0.05` UI units, its bottom centre anchored to the console's bottom centre at offset `(0, 0.18)`. [Message layout](script-text-messages.md) already decodes this rectangle (`WorldFrameMessage`); the script text frame stacks above it. In the 0.8 x 0.6 UI that is left `0.21`, bottom `0.18`. | EX `0x5e83e4`..`0x5e8427`, `0x5e7ca5` |
| E5 | Font: `FontHeights/WorldFrameMessage` (0.015) from `MiscUI.txt`, the skin key `MessageFont`, flags `1` (the same bold-size flag as the script text, see the glyph-size section of [script text messages](script-text-messages.md)). Shadow: colour `0xFF000000`, offset `(0.001, -0.001)` frame units. | EX `0x5e8395`, `0x5e83ba`, `0x5e83c4`, `0x5e8432`..`0x5e8456`; SB |
| E6 | The line is held for its hold time, then its alpha falls: each frame the remaining fade time is lowered by the frame time and the alpha byte becomes remaining / fade x 255; at zero the line is removed. The fade time is the frame class's default `3.0` (`0x40400000` stored at `this+0x150` by the constructor; the error frame never changes it). So total life is 10 s opaque plus 3 s linear fade. | EX `0x88bb80`, `0x88b3ce` |
| E7 | Text is left justified (the constructor's default `this+0x140 = 1`; the upkeep frame sets `2` explicitly and is the centred line) and stacked from the bottom (`this+0x174 = 1`). The value `1` meaning left is INF from the token order `JUSTIFYLEFT`, `JUSTIFYCENTER`, `JUSTIFYRIGHT` and the 1, 2, 4 flag setter `0x887560`. | EX `0x88b3c4`, `0x88b3d8`, `0x5e84e8`; INF |
| E8 | Sound: codes with a named sound play it (`NoGoldSound`, `NoLumberSound`, `NoFoodSound`, `NoManaSound`, `InventoryFullSound`, `CantPlaceSound`, `OffBlightUndead`, `CantRootNightElf`, race-selected variants for food and two more); every other code plays the label `InterfaceError` (`0xe90b14`). | EX `0x5ecc20`, `0x5d8a20`, `0x5d9240`, `0x5d9290` |
| E9 | Race variants: the text lookup takes a variant index (`race - 1` for races 2 to 5, else 0) only for codes 3, `0x5a` and `0x5b`; `Nofood` has four comma separated entries in `CommandStrings.txt` `[Errors]`, one per race. | EX `0x5ecc20`, `0x5ed6c0`; SB |

### Implementation

| Part | Where |
| --- | --- |
| Decoder of E2 to E7 against the reviewed image, with the opacity curve | `crates/pcx-play/src/error_line_rules.rs` |
| Frame rectangle accessor on the message layout | `crates/pcx-play/src/message_layout.rs` `error_frame` |
| Keyed font height and font path reader | `crates/pcx-objects/src/editor/messages.rs` `read_keyed` |
| Line state (`post`, `current`, `revision`) | `crates/peacecraft3/src/campaign_error_line.rs` |
| Drawing (relative layout, colour, shadow, fade) | `crates/peacecraft3/src/campaign_error_view.rs` |
| First poster: the control group refusals | `crates/peacecraft3/src/campaign_control_groups.rs` |

The decoder refuses another image, a changed instruction, a vtable slot that is not
`0x88b510`, a hold or fade that disagrees or is not positive, and a justification or stack
direction other than the reviewed ones. The timer is `Time<Real>`; the client's frame time
source for this frame is not read (INF).

The control-group notice no longer clears on the next accepted key. The client has no such
rule: only a newer post or the timers remove the line (E2, E6).

### Not wired, and why

- Order refusals in `campaign_input` and the smart order (`SmartRefusal`) are developer
  diagnostics ("is not simulated yet", "smart order data"), not the client's `[Errors]`
  strings. Which `[Errors]` key the client raises for each unsupported class (load,
  repair, rally on a target, haunted or entangled mines) is not read, so nothing is posted;
  the text stays in `CampaignInput::last_error`.
- Drag-box refusals (`selection_box::Refusal`) are recreation rules for cases whose
  client behaviour is unestablished; the client shows no text for them that was found.
- Gold, lumber, food, mana and placement refusals: the command card and build paths do not
  yet report why an order failed, only that it did. The error codes are listed in E8 for
  when they do.
- The sounds in E8 are not played: the recreation has no in-game UI sound-label player.
- Line capacity: the frame holds `height / line pitch` line slots (set at layout by
  `0x88b950`). The slot count and the wrapping of one string across slots are not modelled;
  a long text wraps inside the clipped frame.

## 2. Dialog keys and chat

All of these are in the client's key-down handler `0x5eeee0` (reached from the event
dispatch `0x5ee6a0` for event `0x40060064`), not in `CSelectMode`.

| # | Key | Behaviour | Evidence |
| --- | --- | --- | --- |
| C1 | F9 | When the interface is live, the Quests button of the upper bar (`UI+0x410`, field `+0x13c`) is enabled and this UI is the active one: toggles the dialog at `UI+0x280`. | EX `0x5eeee0` case `0x78` |
| C2 | F10 | Same with the Menu button (`+0x134`): toggles the dialog at `UI+0x268`. | EX case `0x79` |
| C3 | F11 | With the state of `0x5ad790` clear: the Allies button (`+0x138`) enabled: toggles the dialog at `UI+0x278`. With that state set it instead steps to the next player slot whose state is 1 (`0x5f4260`). The Allies button is disabled in a local session ([upper buttons](campaign-hud-panels.md)), so F11 does nothing there (INF). | EX case `0x7a`; INF |
| C4 | F12 | Chat/Log button (`+0x164`) enabled: toggles the dialog at `UI+0x27c`. The dialog is the Log in a local session and the Chat dialog otherwise (button text `KEY_LOG` against `KEY_CHAT`, `UPPER_BUTTON_LOG_TIP`/`UPPER_BUTTON_CHAT_TIP`). | EX case `0x7b`; SS |
| C5 | Enter | When the interface is live, the chat bar object (`UI+0x45c`, flags and `UI+0x18c != 1`) allows it and `0x5ad790` is clear: calls the chat edit bar's method at vtable offset `0x114` (`UI+0x43c`) with `1`. The chat bar's constructor ends with the same call and `0`, so `1` shows the bar. | EX case `0xd`, `0x61c520` |
| C6 | Shift+Enter, Ctrl+Enter | Not in `0x5eeee0`; the tooltip text states them (Ctrl allies, Shift everyone). The edit bar's own key handler was not read. | SS `UPPER_BUTTON_CHAT_UBER`; UNK |
| C7 | Alt held | With the interface live and Alt held (either key, `0x83fca0`; other modifiers are not read), the key is compared with ten UI fields loaded from `MiscUI.txt [Hotkeys]` by the constructor: `+0x46c` QuickSave, `+0x470` QuickLoad, `+0x474` QuickHelp, `+0x478` QuickOptions, `+0x47c` QuickQuit, `+0x480` MinimapSignal, `+0x484` MinimapTerrain, `+0x488` MinimapColors, `+0x48c` MinimapCreeps, `+0x490` FormationToggle. The last five act only while their button (`UI+0x448`..`0x458`, field `+0x13c`) is enabled: terrain flips the flag at `(UI+0x400)+0x818`, creeps the flag at `+0x81c`, then the button is refreshed. The first five open the menu or run the quick action (panel numbers 3 and 6 for options and quit were not decoded). | EX `0x5eefc4`..`0x5ef220`, `0x5e77fb`..`0x5e78e9`; SB (letters `S L H O Q G T A R F`) |

The recreation already has F9 and F12 on the upper buttons (`AuthoredShortcut`), F10 through
the campaign camera's menu key, Allies disabled without a shortcut, and tests for each
(`campaign_upper_buttons`, `campaign_camera`). No code changed for C1 to C4; the audit row
B9 was out of date for F12.

### Chat edit bar: recorded, not built

The edit bar is created in code (`0x61c520`: font height `ChatEditBar` 0.013, no FDF), its
submit path was not read, and there is no live capture of single-player chat other than the
map-reveal cheat line, which the client takes from the same bar. What a submitted line does
in a local session (cheat codes only, or a message in the log) is therefore UNK. Building a
typing box whose submit target is guessed would invent behaviour, so Enter, Shift+Enter and
Ctrl+Enter stay unbound. To finish: decode the edit bar's submit handler (vtable
`0xe9587c`), the cheat dispatch and the `COLON_MESSAGE_*` labels.

## 3. Minimap and overlays

| # | Item | Finding | Evidence |
| --- | --- | --- | --- |
| M1 | Alt + left click on the world | The select mode's button handler `0x5fd170` (event `0x10064`): left button with Alt held and `0x5ad790` clear posts an event with id `0xa0068` and the click position (`0x4d71a0`), arms a 0.5 s timer (`0x840980`), and returns handled, so the press is not a selection. That the event is the minimap signal is INF from the tooltip. | EX `0x5fd170`; SS `MINIMAPSIGNALTOOLTIP_UBER` |
| M2 | Alt + left click on the minimap | The tooltip says Alt and left click on the minimap or the game world signals; the minimap's own handler was not read, so that the click does not also move the camera is INF from "perform the same action". | SS |
| M3 | Ping visual | `UI\Minimap\Minimap-Ping.mdx` with sequences `Birth`, `Stand`, `Death`, `AllyPing Birth`, `AllyPing Stand`, `AllyPing Death`; colours `[PingColor]` in `MiscData.txt` (`PingColor` 255,0,255,0; attack 255,255,0,0; advisor 255,255,255,0; waypoint 255,255,255,255). The tooltip says the signal shows on the allies' minimaps. Whether the local player's own minimap shows it, and what the world shows, is UNK. | SB, SS |
| M4 | Health bars | `Alt`, `[` (VK `0xDB`) and `]` (VK `0xDD`) are cases of `CSelectMode`'s key handler (`0x5fcc88`, `0x5fd040`); option `healthbars`. The recreation draws no in-world health bars at all, so the modifiers have nothing to change. | EX; audit B18 |
| M5 | World hover highlight | `CPreselectUI` exists in the RTTI; its drawing and rule were not read. | RTTI |
| M6 | Formation toggle | `Alt+F` and the orders' formation flag (`0x5fd2f0`). The simulation has no formation movement. | SB, EX |

### What changed

- Alt + left click is a press the select mode consumes (M1): no selection starts and no
  box is dragged (`campaign_box_select::gesture_input`, `campaign_input::click` without box
  rules). With a command armed the press still issues it: that belongs to the target mode,
  which this does not change. The minimap does not move the camera or start a drag on an
  Alt + left press either (`campaign_minimap::consume_pointer`). No ping is drawn.

### Not built

- Alt plus the `[Hotkeys]` letter of the five minimap filter buttons (C7) is decoded but not
  wired. To finish: read the five letters from `MiscUI.txt`, and on Alt plus the letter push
  the button entity into `ButtonActions` from `campaign_minimap_buttons`. The pairing of key
  name to button is INF from the names and tooltips.
- Ping marker (M3) and the signal event's receivers: the MDX model on the minimap, the 0.5 s
  timer's purpose and the ally recipients are unread. The event id `0xa0068`, the timer and
  the `[PingColor]` values are recorded above for that work.
- Health-bar modifiers (M4): no world health bars exist to modify.
- Hover highlight (M5) and formation toggle (M6): the rules are unread or have no consumer.
  The Alt+F key presses the formation button, whose option value is stored but changes nothing.
- Quick save, load, help, options and quit (`Alt-S`, `Alt-L`, `Alt-H`, `Alt-O`, `Alt-Q`): the
  panel numbers behind options and quit and the save and load paths are unread.

## Reproduction

```
mpqxtract get War3Local.mpq War3xLocal.mpq War3x.mpq War3.mpq 'Units\CommandStrings.txt' out
rizin -q -e scr.color=0 -c 'aa; aap; aac; s 0x5ed6c0; af; pdg' 'Warcraft III.exe'
cargo test -p pcx-play --lib error_line_rules
PCX_WC3_INSTALL=<install> cargo test -p pcx-play --lib error_line_rules -- --ignored
cargo test -p peacecraft3 --features test-fixtures --bin peacecraft3 campaign_error_view
cargo test -p peacecraft3 --features test-fixtures --test map_loading control_group
```
