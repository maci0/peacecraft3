# Campaign upper controls and quest log

`campaign_hud.rs` installs `campaign_upper_buttons.rs` for the receipt-owned
published campaign session. A change in the session witness removes the old UI
and releases its image, font and material assets. Read failures reach
`CampaignHud.last_error`. FDF, skin, globals, font and image receipts retain
the selected edition/version and content hash in `CampaignHud.control_receipts`;
replacing the witness clears them.

The Quests and Menu buttons derive their dimensions, anchor chains, background
texture coordinates and labels from `UI\FrameDef\UI\UpperButtonBar.fdf`,
`UI\war3skins.txt` and `UI\FrameDef\GlobalStrings.fdf`. The small adapter
translates named SIMPLEFRAME String/Texture templates into the existing FDF
parser's frame representation, retaining template inheritance and record values.
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
The detail region displays the selected quest's title, outcome status, item
text and description. Inline text colours and line breaks retain the supplied
markup. The authored accept label and Escape close the panel. Menu publishes
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
directory. Parser and ECS checks do not prove rendered/native fidelity.

Remaining fidelity questions include native root placement, top-button pushed
and hover art, quest-row icons and completion checkboxes, detail formatting and
scroll interaction, and single-player pause behaviour while viewing quests.
`KEY_QUESTS` now supplies the quest shortcut through the checked shared
`campaign_camera::authored_function_key` mapper. The source highlight is read
from the selected global strings; no particular function key is assumed. The
shortcut appends the same shared button action as pointer/keyboard activation.

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
native usage or the target release. Upkeep remains blank with a diagnostic until
an effective source-qualified tier is available.

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
