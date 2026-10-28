# Campaign upper controls and quest log

`campaign_hud.rs` installs `campaign_upper_buttons.rs` for the receipt-owned
published campaign session. A change in the session witness removes the old UI
and releases its image, font and material assets. Read failures reach
`CampaignHud.last_error`. FDF, skin, globals, font and image receipts retain
the selected edition/version and content hash in `CampaignHud.control_receipts`;
replacing the witness clears them.

The four upper buttons (Quests, Menu, Allies, Log) derive their dimensions, anchor chains, background
texture coordinates and labels from `UI\FrameDef\UI\UpperButtonBar.fdf`,
`UI\war3skins.txt` and `UI\FrameDef\GlobalStrings.fdf`. The small adapter
translates named SIMPLEFRAME String/Texture templates into the existing FDF
parser's frame representation, retaining template inheritance and record values.
Allies is disabled and Log replaces the FDF's Chat labels, both by one rule read
from the client: see [campaign-hud-top-bar.md](campaign-hud-top-bar.md). Quests,
Menu and Log act.
The bar's viewport top-left relation is a recreation choice; the source FDF does
not author a root anchor.

Quests opens a modal using the dimensions and section anchors from
`UI\FrameDef\UI\QuestDialog.fdf` and styles inherited from
`UI\FrameDef\UI\EscMenuTemplates.fdf`. The centered viewport relation is a
recreation choice. Its background tiles and border atlas follow the existing
source-backed backdrop renderer. Image handles retain the authored tiling
choice in their key: tiled backgrounds repeat, while stretched backgrounds and
border atlases clamp, including when one source token serves both roles. Discovered, enabled quests are projected from
the session's `quest_view`, split into required/optional lists, and selectable.
The detail region displays the selected quest's title, item text and
description. A quest list row shows `QUESTCOMPLETED` or `QUESTFAILED` under its
title (N: the client loads only those two strings at 0x00633c1c and 0x00633c3c,
tests completed before failed at 0x00657d6c, and hides the label for neither). A
completed quest item gets `" ( "`, `QUESTCOMPLETED`, `" )"` appended (N: literals
at 0x00657c49 and 0x00657c63). The earlier detail status line, which used
`QUESTNOTCOMPLETED`, is removed: no client code references that string. Inline
text colours and line breaks retain the supplied markup. The authored accept
label and Escape close the panel. Menu publishes
`CampaignHudAction::Menu` for the existing campaign suspension consumer.

The ordered `CampaignHudSet` publishes modal, pointer and Escape consumption
flags before gameplay input. Button activations reuse the shared `Activate`
and completed `PointerClick` queue; a separate `PointerPress` observer retains
UI consumption even when a press and release occur within one command flush. This prevents close clicks selecting terrain and
Escape closing a quest log also skipping the campaign cinematic. Inactive
campaign roots use `Display::None`.

Synthetic checks cover font/template inheritance, anchor offsets, closing a
modal through real press/click/release events in a single command flush,
keyboard activation, and one-frame pointer/Escape consumption. An explicitly ignored test
reads locally extracted original FDFs only when `PCX_HUD_FIXTURE_DIR` names their
directory (the Quest, Log and button frames, `GlobalStrings.fdf`; it also checks the
shipped log cap of 128 and the F12 key). Synthetic checks also cover the
Allies/Log provider rule, one-dialog-at-a-time toggling with Escape, the log
dialog spawn (entry order, scroll to newest, OK action), history recording and
capping, and refresh/despawn in `sync`. Parser and ECS checks do not prove
rendered/native fidelity.

Remaining fidelity questions include native root placement, top-button pushed
and hover art, detail formatting and scroll interaction, and single-player pause
behaviour while viewing quests. Authored quest frames not built: the row icon box
(`QuestListItemIconContainer`, from `QuestProjection.icon_path`), the row button
backdrops and completed/failed/selected highlights (rows are bare buttons), the
`QuestCheckBox` frames, the four `EscMenuScrollBarTemplate` scrollbars, and the
`QuestTitleValue`/`QuestSubtitleValue` texts, which have no authored string and
whose native source is not traced. The client also shows `UNDISCOVERED_QUEST` for
an undiscovered row (0x00657e07); undiscovered quests are filtered out here.
`KEY_QUESTS` now supplies the quest shortcut through the checked shared
`campaign_camera::authored_function_key` mapper. The source highlight is read
from the selected global strings; no particular function key is assumed. The
shortcut appends the same shared button action as pointer/keyboard activation.

## Message log

Log (and `KEY_LOG`, F12 in the selected `GlobalStrings.fdf`) toggles the message
log from `UI\FrameDef\UI\LogDialog.fdf` (`campaign_message_log.rs`, a child module
of `campaign_upper_buttons.rs`). Geometry, the `EscMenuBackground` and
`EscMenuEditBoxBackground` backdrops, the OK button art and label, the title string
`MESSAGE_LOG` and the area font and line metrics come from the FDFs and
`GlobalStrings.fdf`. OK, Escape and the Log key close it. Opening it closes the
quest log and the reverse (recreation choice). While open it blocks gameplay input
through the same `CampaignHud` flags as the quest log (`quest_log_open` now means
"a bar dialog is open").

Entries come from `campaign_messages::MessageHistory`, which records each
`DisplayTimedTextToPlayer` text shown to the local player once, oldest first,
sampled from the live message list each frame. A message that starts and expires
between two frames is not recorded. `TextAreaMaxLines` (128 in the shipped FDF)
caps the number of entries, not wrapped lines. Recreation choices and open
questions:

- Whether the client logs script text in this log, and which other kinds of text
  (chat, system messages) it holds, is not traced.
- The FDF scrollbar (`LogAreaScrollBar`, `EscMenuScrollBarTemplate`) is not built;
  the wheel scrolls. A new entry rebuilds the dialog and scrolls to the newest.
- The dialog is centered on the viewport; the client's placement is not traced.

## Effective resource bar

`campaign_resource_bar.rs` reads the selected `ResourceBar.fdf` and race skin.
Named texture/string templates, anonymous inherited texture instances, font
sizes, icon dimensions, TextLength and slot anchors remain source-derived.
The viewport top-right attachment is an explicit recreation choice: the FDF
has no root attachment record. Asset receipts retain source identity/version
and hashes in `CampaignHud.resource_receipts`. Unsupported or unknown state
reaches `CampaignHud.resource_error`; unknown counters display no invented text.

The read-only `ObjectHost::player_resource_value` projection supplies effective
local-player gold, lumber, food usage and food cap. Recorded setter requests are
never treated as balances. The food separator is neutral mathematical notation,
an explicit recreation choice until a native source-format binding is retained.
A local compatibility Game.dll printable-string scan found several candidate
ratio formats but no resource-bar call-site binding; those bytes do not establish
native usage or the target release. Upkeep now projects the effective food counter
through selected client threshold/default-table operands and admitted ordered Misc
profiles. Its label keys, custom-tier key/suffix formats and byte capacities come
from the same client image; selected GlobalStrings supplies localized text and
inline colours. Threshold equality stays in the lower tier. Short whole-list
overrides use the native indexed-reader initializer for missing elements.
Unsupported token forms, source shapes, missing strings, over-capacity text or
unknown version stay explicit diagnostics; other counters remain available.
The participant projection does not implement observer/watch-target presentation
or economic gold/lumber tax effects.

Image/final font payloads share an application aggregate cap; frame/image values
come from the assets. Repeated effective values do not rewrite Text components.
Parser fixtures exercise inherited icon/font properties, authored anchor offsets,
unsupported/malformed anchors, and an explicitly ignored original-FDF check.
These checks do not prove the native resource-bar appearance or update behavior.

Native Xvfb/Vulkan software-rendered Human01 replay on 2026-10-20 opens the live
quest list through a fast click, closes it with Escape, then suspends through Menu
and resumes with the selected F10 key. The final receipt/sampler path was also replayed in the v7 native window.
Captures/logs stay local under `.scratch/campaign-integration-20261020-v6/` and
`.scratch/campaign-integration-20261020-v7/`.
No original-game or browser-rendered comparison was made.

The v15 normal native application shows the resource bar after an ordinary
cinematic skip, and the authored F9 key opens the quest log. The displayed food
count changes as the script transfers units; no food parse refusal remains for
authored numeric markers. These local screenshots are under
`.scratch/campaign-objective-20261020/normal-gui-v15/`. They establish visible
widgets and input, not original/native numerical or visual parity. This capture
predates the upkeep consumer. Synthetic boundary/override/localization fixtures
and the explicit selected-image decoder gate do not prove rendered or native
runtime upkeep parity; no synchronized original-client comparison was performed.
