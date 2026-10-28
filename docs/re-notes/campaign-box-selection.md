# Campaign drag-box selection

Left press and drag on the world selects with a green rectangle, as in the
original. This note records where each rule comes from. Evidence classes:

- **exe**: static reads of the selected 1.29.2.9231 `Warcraft III.exe`, SHA-256
  `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
- **shipped bytes**: the installation's archives.
- **observation**: the unmodified client run under the private Docker/Wine/Xvfb
  reference harness at 1024x768, 96 DPI (a Wine runtime reference, not native
  Windows). Game: Custom Game, Booty Bay, random race (Orc), five peons and a
  town hall beside a gold mine. Captures stay in the ignored `.scratch/`.
- **inference**: stated, not verified.

## Rectangle

- exe: the selection manager (vtable `0xe94074`, constructor `0x5f7ea0`) reads
  `[SelectionCircle] ColorFriend` through the profile lookup at `0x5f80a8..0x5f80c3`
  (section string `0xe90570`, key string `0xe90580`) into manager field `+0xfc`.
  The draw routine `0x5ff0e0` passes that field as a constant colour for the
  rectangle's line vertices (stride 0, primitive type 2). The routine at `0x5ff240`
  draws the same lines in `0xf0ffffff` and is chosen by manager field `+0x20`;
  what sets that field is not established. The `[SelectionCircle]` section in
  `UI\MiscData.txt` has `ColorFriend=255,0,255,0` (shipped bytes). The
  recreation reads the colour from the same profile, through
  `CampaignInput::circles`.
- observation: while the button is held the rectangle is a pure `#00FF00`
  outline with no fill, sampled at `(429..430, 300)` and `(500, 249)` of
  `drag-held.png`. It is drawn from the first pixel of movement: a 2 px sweep already
  shows a 2x1 outline. Edge width was 2 px on vertical edges and 1 px on the
  horizontal edge at this resolution. The recreation draws one logical pixel
  (`OUTLINE_PX`, a recreation choice). Width at other resolutions is not measured.
- observation: units inside the rectangle show their selection circle while the
  button is still held, and the command card stays empty until release. The
  recreation does not draw these live preselect circles (`CPreselectUI` exists in the
  exe). Selection commits on release.
- No FDF, texture or skin in the archives defines the rectangle.

## Gesture

- exe: left button down in `CSelectMode` (message handler `0x5fba10`, case
  `0x1a0068` to `0x5fe070`, sub-code 1) switches the UI to the 12-byte
  `CDragSelectMode` object (vtable `0xe927f0`, stored at UI `+0x258`, created at
  `0x5e896f`) and starts the manager (`0x5f9fe0`). The drag mode handles key
  down in `0x63e270`: Escape (`0x1b`) calls the manager end-drag with commit 0
  (`0x5f9b40`) and restores the default mode; button release (`0x642010`)
  calls it with commit 1. Escape therefore cancels without selecting.
- exe: the pick routine `0x601410` builds the rectangle from the press position
  and the cursor; width or height of zero takes a point-pick path (flag
  `0x5` instead of `0x1`). There is no pixel threshold. The recreation matches this:
  a release with zero width or height is a plain click at the press position,
  which selects exactly as before this feature.
- observation: clicking empty ground or sweeping a rectangle over empty ground keeps the
  current selection.
- An armed targeting command or build placement claims the press (the target mode
  replaces `CSelectMode` in the exe, inference from the mode split). The recreation
  starts no gesture then and draws no rectangle; the press targets as before. Arming a
  command during a drag cancels it.

## Which units a rectangle takes

- exe: the native pick (`0x601410`, about 1,000 decompiled lines, not read in
  full) sets up its own projection, tests candidate objects against the
  rectangle, sorts them into several lists and filters on per-object flag bits
  and virtual calls (for example `object[8] & 2`, `object[0x123] & 0x400`,
  vtable `+0x13c` and `+0x100`). The recreation does not reproduce that; it projects each live, shown, visible unit's
  selection circle (UnitUI `scale` times `ScaleFactor`, 16 samples at terrain height)
  through the real scene camera and takes units whose projected circle touches
  the rectangle (inference).
- exe: `CSelectionWar3` refuses to add past 12: `cmp dword [edi+0x10], 0xc` at
  `0x4ef657` and `cmp dword [ebx+0x1f8], 0xc` at `0x4efc8b`. The recreation reads
  both operands from the image at run time and requires them to agree
  (`pcx_play::selection_box::Parameters`).
- observation: a rectangle covering own peons and the own town hall selects the peons
  only; the hall alone selects the hall; the neutral gold mine alone selects
  the mine (neutral circle, yellow); peons and the mine select the peons only.
- inference (same tiers): own units, then own structures, then other players'.
  Several own structures are taken together. Other players' units alone are taken
  only when exactly one is in the rectangle.
- observation: Shift plus a rectangle adds to the selection and does not toggle units
  already selected.
- Not established, so refused or flagged instead of guessed:
  - several units of other players in one rectangle: refused with a message;
  - Shift with another player's unit in the current selection or the pick: refused;
  - which units survive when more than 12 qualify: the recreation keeps the first 12
    in unit creation order and records that the original's choice is not established;
  - hero priority and `prio` ordering, items and destructables, the unit
    selectability flag (the host does not model `BlzIsUnitSelectable` or the `Aloc`
    effect on selection), and the script natives `EnableSelect` and
    `EnableDragSelect` (strings exist in the exe, the host does not implement them).

## Double click

- exe: the manager constructor stores a 500 ms window (`mov [esi+0x10], 0x1f4`
  at `0x5f7edd`). `0x5f9fe0` marks a second press a double click when it arrives
  inside that window and within `0.01` of the first on each axis (float at `0xe7e968`
  loaded at `0x5fa087`). A swept press does not chain. The recreation reads the window and
  tolerance from the image.
- exe: the tolerance is compared in the UI frame space (0.8 by 0.6; `0x601410`
  normalises the cursor by the world frame rectangle), so the recreation converts it to
  pixels with the 0.6 frame height (inference).
- observation: double click on a peon selected all five peons, the only five on screen.
  The recreation selects every own unit of the clicked type inside the scene viewport
  (same 12 cap). Another player's unit stays a single selection. The native
  routine `0x5fff10` does this; its on-screen test was not decompiled.

## Interactions left alone

- Ctrl groups are untouched (`CNetCommandUnitDefineControlGroup` paths are not used).
- The portrait, command card, inventory and build menu follow the active subgroup of a
  multi-unit selection, and the group panel shows the selected units; see
  [selection rules](input-selection-rules.md). Shift on a click now toggles the unit; Shift
  with a rectangle still only adds.

## Verification

- `pcx-play` library tests cover the gesture state machine, pick tiers, cap,
  Shift, double click and the image reader on a synthetic PE. The ignored
  `original_rules_come_from_the_selected_image` test needs
  `PCX_SELECTION_CLIENT_IMAGE` and read `12`, `500` and `0.01` from the
  selected image.
- App tests in `campaign_input` inject mouse events into the real systems on a
  synthetic session without a host window, pointer or audio.
- Disassembly and decompilation used the existing local analysis snapshot; scratch
  output stays in `.scratch/`.
