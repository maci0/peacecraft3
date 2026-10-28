# Hero bar, learned-ability buttons, minimap buttons and idle-worker button

Four differences between a stock Prologue02 capture and the clone, all with
Thrall selected. All addresses are from the selected Legacy 1.29.2.9231
executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.

## Command card: the missing Far Sight button

The map script learns both spells for the hero
(`Maps\Campaign\Prologue02.w3m`, `war3map.j`, the Load Thrall trigger):

```
call SetHeroLevel( udg_Thrall, 2, false )
call SelectHeroSkill( udg_Thrall, 'AOcl' )
call SelectHeroSkill( udg_Thrall, 'AOfs' )
```

Both are level 1 on the unit. `OrcAbilityFunc.txt` gives `AOfs` `Art`,
`Buttonpos=1,2` and `Order=farsight`, next to `AOcl` at `0,2`. The card only
mounted abilities that have a dedicated command (Holy Light, Chain Lightning,
Water Elemental), and Far Sight has no gameplay implementation, so its button
was never built. The ranks come from `SelectHeroSkill`, not from a skill-point
button.

`ObjectHost::learned_ability_buttons` now lists every learned, player-available
ability that authors both a `Buttonpos` (stock profile or map override) and an
`Art` (`AInv` and the harvest-lumber alias have neither; `Adet` and `Adt1` have
a position without art and stay unlisted). The card draws each one the command
list did not already draw, in its authored slot, as `Kind::DisplayOnly`:

- A display-only button consumes the press and issues no order. No order exists
  for these abilities yet; nothing is claimed beyond the icon.
- Implemented commands, the build menu ability and every other functional
  button keep their slot. A display-only button whose slot is taken is skipped,
  and ability order decides between two of them that share a slot (gather and
  return resources use the same `Buttonpos`).
- The tooltip uses the same learned-rank `Tip`/`UberTip` expansion; an expansion
  failure shows the existing tooltip-failure state.

Units other than heroes now also show their learned abilities, such as peon
gather. This follows the stock card; their orders remain unimplemented.

## Hero bar

The selected client builds the bar in `0x61e000`, created from GameUI at
`0x5e8179` with the console parent stored in field `+0x468`, the same field the
command bar receives. Decoded by `pcx_play::hero_bar_layout::Rules`, which
checks every call target, anchor pair and operand and refuses on any change:

- A one-column, seven-row grid (`0x61e07e`, `0x61e080`, loop bound `0x61e12f`)
  whose width and height are set at `0x61e0a4`/`0x61e0b3`. Its inset fields are
  the grid constructor's zero defaults, as for the command grid.
- TOPLEFT to the console parent's TOPLEFT (`0x61e1b1`). The x offset passes
  through `FUN_00ab28d0`, the wide-window mapping described in
  [glue coordinates](glue-coordinate-runtime.md); at the 4:3 logical viewport
  it is the identity, and the clone does not apply the mapping.
- Each cell holds a command-button-sized square (`0x61d490`, `0x61d49f`) at the
  cell's top-left, as in the command grid.
- A health bar (status-bar type 3) hangs from the button's BOTTOM and a mana
  bar (type 4) from the health bar's BOTTOM, each at the offsets read at
  `0x61e2e9`/`0x61e36d`. The shared bar constructor `0x6218d0` supplies the
  texture key table (`0x621ac2`), the per-type tint cases (`0x621af3`), the
  black background rectangle outset by an inset on all four corners
  (`0x6219f4`, `0x621a00`) and the `[0,1]` value range.

Values read from the image on this build: icon 0.038 square at x 0.00375 and
0.02625 below the top, 0.0511786 row pitch, bars 0.038 by 0.003125 with a
0.00125 gap, skin keys `SimpleHpBarConsoleSmall` and
`SimpleManaBarConsoleSmall`, tints `0xff00ff00` and `0xff0000ff`. At 1280x960
the icon lands at x 6, y 42 and the bars at y 105 and 112, which agrees with
the stock capture. The explicit gate
`hero_bar_layout::tests::selected_client_matches_the_decoded_bar_layout`
(`PCX_WAR3_EXE`) decodes the real image; synthetic fixtures use different
geometry and reject changed instructions.

`campaign_command_card` mounts one row per living, shown hero of the local
player under the same console parent, with the icon from the unit's `UnitUI`
`Art` (`training_command::icon`). Fill widths follow `life/max_life` and
`mana/max_mana` each frame; rows are rebuilt only when the hero set or an icon
changes. A press selects that hero.

Recreation choices, not native evidence:

- Hero order is unit creation order, and heroes past the seventh are not shown.
- Dead or hidden heroes are omitted; native dead-hero presentation is unread.
- The fill is stretched to the fraction rather than texture-cropped.
- The level-up point model (`HeroBarPointModel`, `0x61e3dd`), the hero hotkeys,
  the tooltip and double-click camera centering are not implemented.
- A hero with no mana pool shows an empty mana bar.

## Minimap side buttons

Five buttons are built by the GameUI constructor (`0x5ed170`), not by an FDF;
`ConsoleUI.fdf` supplies only the empty slots. Each is 0.023125 wide by
0.020625 high (`0x873b40`/`0x873830` operands), TOPLEFT to the console parent's
BOTTOMLEFT (point 0 to point 6, parent field `+0x468`), stored at GameUI
`+0x448`..`+0x458`. Top to bottom, with the offset operands read from the image:

| Field | Control | x | top y | State source |
| --- | --- | --- | --- | --- |
| `+0x448` | signal | 0.1537 | 0.1437 | none; one art set (`0x5ed283..0x5ed2bd`) |
| `+0x44c` | terrain | 0.1537 | 0.1219 | minimap flag `+0x818` (`0x5f4620`) |
| `+0x450` | ally colours | 0.1537 | 0.1 | option `0x38` `allyFilter` (`0x5f2c40`) |
| `+0x454` | creeps | 0.1537 | 0.0775 | minimap flag `+0x81c` (`0x5f35f0`) |
| `+0x458` | formation | 0.155 | 0.05 | option `0x2d` `formations` (`0x5f36a0`) |

`pcx_play::minimap_buttons_layout::Rules` decodes these sites and refuses on any
changed call target, anchor pair, field store or option mapping. The explicit gate
`minimap_buttons_layout::tests::selected_client_matches_the_decoded_buttons`
(`PCX_WAR3_EXE`) decodes the real image.

Art and state:

- Each state has an Enabled, Pushed and Disabled skin key (`MiniMap<Control>Button...`
  in `war3skins.txt`, race section then `[Default]`). Terrain, creep and formation
  take the first (active / on) set when the value is nonzero and the second otherwise.
  Ally colours takes the active set for value 2, the inactive set for 1 and the off
  set for anything else (`0x5f2c63`: two `sub eax,1` / `je` pairs).
- The option table is 16-byte records at `0xe68bd0`: name pointer first, default string
  pointer at `+0xc` (reader at `0x42b5c0`). Defaults: `allyFilter` (0x38) 1,
  `creepFilter` (0x39) 1, `terrainFilter` (0x3a) 1, `formations` (0x2d) 1. The CMinimap
  constructor reads 0x38 into `+0x824` (nonzero test), 0x39 into `+0x81c` and 0x3a into
  `+0x818` (`0x6208be..0x620946`). So the client starts with terrain active, ally colours
  inactive (value 1), creeps active and formation on. The stock Prologue02 capture does
  not match all of these: its terrain button uses the file named `...terrain-inactive`
  (white eye over gold mountains), its ally and creep buttons look grayed (likely the
  Disabled art, set by the script native `EnableMinimapFilterButtons(ally, creep)`), and
  its formation button matches the on art. Persisted options or that native may explain
  it; neither is established, and the clone starts from the option-table defaults.
- Tooltip keys are chosen by the hover handler at `0x5efd5a`: `MINIMAPSIGNALTOOLTIP`,
  `MINIMAPTERRAINTOOLTIP`, `MINIMAPALLYCOLORTOOLTIP`, `MINIMAPCREEPCOLORTOOLTIP`,
  `FORMATION_TOOLTIP`, each with an `_UBER` description from `GlobalStrings.fdf`.
- Press handlers: the terrain and creep presses store the negated flag (`0x652e60`,
  `0x652980`) and refresh the art; the formation press negates option 0x2d
  (`0x5eec6c`); the ally press adds one and wraps to zero after 2 (`0x5efc83`), a
  three-mode cycle described by the tooltip text.
- The constructor hides the formation and signal buttons when a game-global flag
  (`0x5ad790`, field `+0x898` equal to 1) is set; the flag is not decoded, and the
  clone always shows them.

`campaign_minimap_buttons.rs` mounts the five buttons under the console parent with
their pushed art while pressed, global-string tooltips and a per-session option value
per button (the client persists options; the clone does not). Presses:

- Terrain: the minimap shows black terrain. The client's updater (`0x658ac0`, flag
  test at `0x658b3b`) fills the minimap image with `0xff000000` instead of copying the
  map image, then writes the fog alpha; `campaign_minimap::publish_minimap_alpha` zeroes
  the colour planes the same way. Markers still draw over it.
- Signal, ally colours, creeps, formation: only the stored value and the art change.
  The clone has no signal ping targeting, creep-camp indicators, ally/enemy minimap
  colour mode (the role colours from `FogColorPlayer/Ally/Enemy` are always used) or
  formation movement. The Disabled art keys are decoded but unused: the native
  `EnableMinimapFilterButtons` is not implemented.
- Hotkeys named in the tooltips (Alt-T, Alt-A, Alt-R, Alt-F, Alt-G) are not wired.

## Idle-worker button

`CIdleButton` (`0x620c80`, created at `0x5e81ba`, stored at GameUI `+0x420`) is
decoded by `minimap_buttons_layout::Rules::idle`: BOTTOMLEFT to the console parent's
BOTTOMLEFT at (0.00375, 0.17) through the wide-window mapping (identity at 4:3),
0.038 by 0.038, an inner command button (`0x61d3b0`), tooltip `IDLE_PEON`
("Idle Workers (F8)") and `IDLE_PEON_DESC`, and icon skin key `IdlePeon` (race
section: `BTNPeon` for Orc). The race section is chosen by the local player's race
(`0x62fc10`, called with the player's race field from `0x5f2ae0`). The count badge in
the stock capture matches the command-button rank badge; the clone reuses that badge
geometry, art and font from `command_layout::Rules`.

Which units are idle workers, when the button hides and what a press selects are not
decoded. Recreation choices, labelled as such in the code:

- `ObjectHost::idle_workers(player)` lists living, shown `UNIT_TYPE_PEON` units of the
  player whose `GetUnitCurrentOrder` is 0 and that the harvest controller does not run
  (gold miners inside a mine are hidden and so excluded).
- The button is hidden at a count of zero and shows the count otherwise.
- A press selects the next idle worker in creation order, wrapping, and does not move
  the camera. The F8 hotkey is not wired.

## Not verified

No pixel-parity claim, and no renderer capture of the new rows or the Far Sight
button was taken. Headless checks: the layout reader (synthetic fixtures and the
explicit `PCX_WAR3_EXE` gate), the learned-button listing (synthetic hero host),
the card's slot filter, and the explicit Human01 mount test, which now also
checks that the hero bar mounts one row for the player's hero under the console
parent with fill widths equal to the live fractions. The Far Sight button itself
was not exercised against Prologue02; the listing was only tested with synthetic
abilities and the Human01 hero.

## Verification

Headless, with `PCX_WAR3_EXE`, `PCX_NATIVE_CHAPTER_INSTALL`, `PCX_NATIVE_CHAPTER_MAP=Maps\Campaign\Prologue02.w3m`,
`PCX_NATIVE_CHAPTER_ADDON=roc`: `campaign_minimap_buttons::tests::selected_chapter_mounts_the_buttons_and_applies_presses`
mounts the five buttons and the idle button under the console parent with global-string
tooltips and initial art. At chapter start the only Orc peon (`opeo`) is hidden, so the count is
0 and the button is hidden; after the test shows it and enables user control the count is 1, a
press selects it, and two terrain presses hide then restore the minimap terrain flag. No renderer
capture was taken; the stock capture's count of 1 agrees with the shown peon, not proven for the
same game time.
