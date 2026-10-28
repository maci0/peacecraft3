# Selection rules: subgroups, group panel, Shift, Ctrl, Tab

Slice 7 of the [input behaviour audit](input-behaviour-audit.md). Covers what several selected
units do: subgroups and the active subgroup, the group-button panel, Shift and Ctrl clicks,
the selection cap and Tab. Evidence tags are the audit's (SB, SS, EX, INF, RO) plus the
[live capture](input-live-capture.md) for RO rows. Executable: the Legacy
`Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Addresses are from a static
rizin analysis (`aa; aap; aac; aar`, `pdg`); decompiler output stays in the ignored
`.scratch/`. No live run was made for this note: every EX row is static, and the RO rows are
the earlier capture's. Nothing below was observed with three or more subgroups.

## Rules and where they come from

| Rule | Behaviour | Tag | Source |
| --- | --- | --- | --- |
| Subgroup identity | Units of one type are one subgroup. A hero never shares: two heroes of one type are two subgroups. | EX | add routine `0x4efe80` ("like subgroup" needs equal type, equal flag bits and `0x696810` false for both units; `0x696810` is true for a type whose first letter is uppercase while flag `0x40000000` is clear, and otherwise follows a test on the object at `+0x404`). The flag's meaning is unread, so "hero" here is the type-name rule plus the recreation's hero ability record. |
| Subgroup order | Highest first by a priority key built in `0x6991c0` (hero flag path, per-type record field `+0x1cc`, hero level terms), ties by type id ascending, then by a unit id. | EX (shape), INF (arithmetic) | `0x4efe80` loop, `0x6991c0` |
| Priority data | `UnitData/prio`: Hpal 9, hfoo 6, hkni 3, hpea 1 in the shipped `unitdata.slk`. That the key's type field is this column is inferred. | SB, INF | `Units\unitdata.slk` column `prio` |
| Recreation order | Heroes, then `prio` descending, then rawcode ascending, then selection order. | INF | `pcx_play::selection_subgroups::group` |
| Active subgroup | One subgroup of the selection is active; it is the first by default (hero first, as observed). | EX, RO | selection field `+0x20c`, default setter `0x4f5580` |
| Representative unit | The first unit of the active subgroup is the one named in the log and shown. Portrait, command card, inventory and build menu follow it. | EX (name log), INF (portrait source) | `0x4f4da0` calls `0x4f2670(0)` on the active subgroup |
| Tab, Shift+Tab | Tab moves to the next subgroup and wraps; Shift+Tab moves to the previous one and wraps. Nothing happens with fewer than two subgroups. The selection list does not change. | EX; RO for two subgroups | `0x5f0710` passes `shift == 0` to `0x5f5230`, which requires subgroup count `> 1` (`0x4f24d0`) and calls `0x4f4da0(forward)` |
| Group button, plain click | A unit of the active subgroup is selected alone (the world-click path `0x5fff10`). A unit of another subgroup makes that subgroup active and the selection is unchanged. | EX | `0x5fc020`: `0x4f2330` returns the active subgroup, `0x4f28d0` tests membership, else `0x4f5460` selects the subgroup "by unit" |
| Group button, Shift | The world-click path with Shift: removes a selected unit. | EX | `0x5fc020` jumps to `0x5fff10` when Shift (`0x83fea0`) is held |
| Group button, Ctrl | Selects the units of that unit's subgroup (not the type on screen). | EX | `0x5fc020`: `0x4f2540` finds the subgroup, `0x4f5b20` lists it, `0x5ffae0` selects it |
| Shift+click on a unit | Toggles: adds it, a second Shift+click removes it. | RO | live capture |
| Shift+click on a building | With units selected, replaces the selection with the building. | RO | live capture, run11 |
| Shift+click on a unit with a building selected | Replaces the selection with the unit. | INF | symmetry with the row above |
| Ctrl+click on a unit | Selects all visible own units of that type, up to the cap. | EX, RO | `0x5fff10`, `0x6010e0`, `0x5ffae0`; 4 of 4 footmen |
| Click on empty ground | Keeps the selection. | RO | live capture |
| Selection cap | 12. Adding past it is refused; a Shift+click on a full selection does nothing. | EX | `cmp ..., 0xc` at `0x4ef657`, `0x4efc8b`, read from the image by `pcx_play::selection_box::Parameters` |

The brief for this slice described a group-button click as "select just that unit". The decoded
handler does that only when the unit is in the active subgroup. A click on a unit of another
subgroup switches the subgroup, which is how the `SelectedSubgroupHighlight` buttons behave as
a subgroup picker. The static read is the only evidence; a live check is listed below.

## Group panel layout

Built by the info-panel constructor `0x61e5e0` (group panel created at `0x61e719`), the panel
constructor `0x61ec50` and the button loop `0x638150`. It has no FDF; `SimpleInfoPanel.fdf`
and `ConsoleUI.fdf` define nothing for it. Values were read from the image by
`pcx_play::group_panel_layout::Rules` (call targets, anchor chains, loop bounds and operands
checked; any change refuses). Frame units, the 0.8 by 0.6 logical frame, y up.

| Part | Value | Anchor |
| --- | --- | --- |
| Panel | the unit information frame, `x 0.309375, w 0.188125, h 0.1140625`, all points | `0x61ec97` |
| Grid | 6 columns by 2 rows, `0.1859375 x 0.1015625` | BOTTOM to panel BOTTOM, offset `(-0.0015625, 0)` |
| Cell pitch | grid size divided by the dimensions; cell 0 top left; rows downward | `0x88c970` |
| Button | `0.03125 x 0.046`, at the cell's top left | |
| Subgroup highlight | `0.0375 x 0.05225`, centred on the button, skin key `SelectedSubgroupHighlight`, tint `0xffffe400`, blend field value 3 | CENTER to CENTER |
| Icon | `0.021875` square | CENTER to button TOP, offset `(0, -0.016875)` |
| Health bar | `0.021875 x 0.0025`, skin key `SimpleHpBarConsoleSmall` via the hero bar's bar constructor `0x6218d0` | TOP to icon BOTTOM, offset `(0, -0.0025)` |
| Mana bar | same size, `SimpleManaBarConsoleSmall` | TOP to health bar BOTTOM, offset `(0, -0.0025)`; hidden at construction |

Twelve cells equal the cap of 12, read from separate instructions. Cell 0 lands at
`x 0.30890626, y 0.0555625` (bottom), cell 6 (second row) at `y 0.00478125`. The ignored test
`group_panel_layout::tests::selected_client_matches_the_decoded_panel_layout` prints all 12.

Recreation choices, not native evidence:

- The panel shows only with two or more units selected, and the unit-detail text is not drawn
  then (see below).
- Cell order is subgroup order, then selection order (the update loop that fills the cells was
  not read).
- The mana bar is drawn only for units with a mana pool, because the original hides it at
  construction.
- The blend field value 3 is treated as additive: the highlight texture
  (`human-subgroup-background.blp`) has no alpha channel, so only an additive blend can use it.
  The enum name behind 3 was not decoded. The tint is multiplied into the texture because the
  existing additive material has no tint input.
- Only the icon is clickable. The command-button class `0x622690` may draw more (a hero glow);
  `GroupButtonBorder` and `CommandButtonHeroGlow` are in `war3skins.txt` but their strings do not
  appear in the executable, so neither is drawn.
- No tooltips on group buttons.
- The active subgroup resets to the first one on every selection change, including Shift
  toggles. Whether the original keeps it across additions is unmeasured.

## Unit details and the group panel

The unit-detail frame (`SimpleInfoPanelUnitDetail`) and the group panel are both
`SetAllPoints` children of the same info frame, created hidden in `0x61e5e0`, and the group grid
covers the lower 0.1016 of the 0.1141 frame height where the detail rows sit. The recreation
therefore shows the group panel instead of the detail text for two or more units (INF; the
existing unit panel already showed nothing for them). The earlier capture notes that the "info
panel" follows the active subgroup; it did not separate the detail text from the group
highlight, so this is not reconciled. Measure: select Arthas and four footmen and look for the
name and damage rows.

## Not done

- F1 to F5 hero keys. `0x5fc700` maps virtual keys `0x70` to `0x74` to index 0 to 4 and calls
  `0x5eb390`, which selects the hero at that index of a seven-entry hero array (EX). The
  recreation's hero bar order is creation order, a recreation choice, and the original's array
  order is not decoded, so the key would select an arbitrary hero. Skipped. Camera centring on a
  second press is unread.
- Which units the original keeps past 12 in a drag, Shift mixes with other players' units, and
  Shift+Ctrl: refused or unmeasured as in [box selection](campaign-box-selection.md).
- Order issuing for a multi-unit selection still addresses every owned selected unit. Whether
  the original limits orders to the active subgroup is not decoded.
- Active-subgroup changes are not written to the command log (the original sends
  `CNetCommandUnitSelectSubGroup`).

## Verification

- `pcx-play` unit tests: subgroup grouping and order, hero subgroups, wrapping cycle, the
  Shift toggle rule, and the layout decoder on a synthetic image (geometry unlike the real
  one, changed instructions refuse).
- `peacecraft3` app tests inject mouse and keyboard messages on the synthetic session
  (`campaign_selection_rules::tests`, `campaign_command_card::group_panel::tests`): Shift toggle
  and building replacement, Ctrl+click, empty-ground click, the cap on Ctrl and Shift, Tab and
  Shift+Tab over three subgroups, group-button clicks including the panel system.
- Ignored, headless: `selected_chapter_mounts_the_group_panel_and_follows_the_active_subgroup`
  runs Human01 with `PCX_WAR3_EXE`, `PCX_NATIVE_CHAPTER_INSTALL`,
  `PCX_NATIVE_CHAPTER_MAP=Maps\Campaign\Human01.w3m`, `PCX_NATIVE_CHAPTER_ADDON=roc`. Arthas
  and two footmen mount as three buttons, Arthas highlighted first, and the highlight moves to
  the footmen after one cycle. The existing `selected_chapter_mounts_the_card_under_its_admitted_console_and_retires_it`
  fails at baseline on a missing resource in its own app setup; it was not changed.
- Nothing was rendered to a screen or compared with a capture.

## Open measurements

1. Click a footman in the group panel with the hero subgroup active: expect the subgroup to
   switch with the selection unchanged. Click it again: expect the selection to become that
   unit.
2. Tab and Shift+Tab with three or more subgroups: direction and wrap.
3. Whether the detail text is hidden with several units selected.
4. Whether the active subgroup survives a Shift+click addition.
5. Cell order and mana bar visibility in the panel.
