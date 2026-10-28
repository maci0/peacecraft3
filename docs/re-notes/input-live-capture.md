# Input behaviour, live capture from the original client

Measurements of the open facts in the [input behaviour audit](input-behaviour-audit.md)
(slice 1). Every value below is a **runtime observation (RO)** of the original client
under Wine in the isolated reference container. It is not native Windows ground truth.
Decompiler-based facts keep the audit's tags (EX, SB); where a debugger read decides a
question it is marked RO-dbg.

## Method

- Client: Legacy 1.29.2.9231, `Warcraft III.exe` SHA-256
  `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed` (from each run's
  `source.json`), launched with `-classic -loadfile` through
  `tools/original-game-reference/reference.py` (`prepare`, `launch`, `stop`). Image
  `sha256:ed3ad1b0a5be`, container flags from the tool: `--network none`, `--cap-drop ALL`,
  read-only root, private Xvfb `:99`, PulseAudio null sink. Before any input each run's
  `display.txt` named the private `:99` display at the launch size and `audio-sinks.txt`
  listed only the `reference` null sink.
- Maps: `Maps\Campaign\Human01.w3m` from `War3Local.mpq` (extracted with `mpqxtract`,
  staged under `Maps\PCX-Reference\`) for units, an enemy and neutral units; the installed
  `Maps\(2)Harrow.w3m` melee map for a Town Hall, workers and a gold mine. The map-reveal
  cheat `iseedeadpeople` (chat line) is used in the Human01 runs so terrain is visible for
  scroll registration. It does not touch input handling.
- Input: the tool's `key` and `click` commands are single-shot with fixed 0.3 s timing, so
  timed sequences ran through a local step runner (`xdotool` mouse and key verbs plus
  `import -window root` crops, each step logged with `date +%s%N`) executed by
  `docker exec` in the same container. Same X server, same device path as the tool.
- Frames: the game draws its own cursor, so screenshots show cursor art. One rendered game
  frame is about 30 ms here (software Mesa); a crop takes 15 to 25 ms. Timing results
  therefore carry about 30 ms of quantisation. Intervals quoted as "press to press" are
  logged runner timestamps.
- Debugger: read-only winedbg command files (`break`, `cont`, `info reg`, `x/`) under the
  `debug.bat` harness pattern from `.scratch/ai-placement`. Attaching winedbg to a running
  game fails with error 5 (access denied), so debugger runs launch the game under winedbg.
  An earlier attach attempt on a plain run was followed by that game exiting with code 0;
  the run was discarded and the cause was not established.
- Analysis scripts (phase correlation for camera shift, frame differences, log parsing) and
  the runner stay in the ignored `.scratch/input-live/`. No original bytes, frames or
  decompiler output are committed.

| Run | Size | Use |
| --- | --- | --- |
| run3 | 1280x960 | Human01 plain: edge scroll, selection, drag, double click, control groups, Tab, hover cursors |
| run9 | 1024x768 | Human01 plain: edge-scroll margin |
| run10 | 1920x1080 | Human01 plain: edge-scroll margin |
| run8 | 1280x960 | Human01 under winedbg: zoom routine reads (final) |
| run6, run7 | 1280x960 | Earlier debugger runs. run6 read the setter's stop arguments (zoom level argument, wheel); run7 read the ease-duration argument. Their local-variable reads used wrong frame offsets and are not used |
| run11, run12 | 1280x960 | `(2)Harrow` plain: F8, backtick, worker hover (run11); Backspace with a worker selected (run12) |
| run1, run5 | 1280x960 | Exploration and a debugger command file with bad `x/` syntax; no number below comes from them |

## Results

### Edge scroll (A19, C1, D2)

| Fact | Value | Evidence |
| --- | --- | --- |
| Scrolling pointer positions at 1280x960 | left x 0 to 6 (7 does not), right x 1274 to 1279 (1273 does not), top y 0 to 6 (7 does not), bottom y 954 to 959 (953 does not) | RO, run3 |
| At 1024x768 | left 0 to 5, right 1019 to 1023, top 0 to 5, bottom 763 to 767 | RO, run9 |
| At 1920x1080 | left 0 to 9, right 1911 to 1919, top 0 to 7; bottom not established (the 1079 trial did not move the camera vertically and 1073 to 1078 were not sampled) | RO, run10 |
| Rule that fits all 11 measured edge boundaries (4 at 1280, 4 at 1024, 3 at 1920) | pointer within m of an edge, strict: `x < m_h` or `x > W - m_h`, `y < m_v` or `y > H - m_v`, with `m_h = 0.004 * W / 0.8` and `m_v = 0.004 * H / 0.6`, so 0.004 in the 0.8 x 0.6 UI space on both axes, 6.4 px at 1280 wide, 5.12 at 1024, 9.6 at 1920; vertical 6.4, 5.12, 7.2 px | RO fit; the constant is not located in the exe |
| Diagonals | Corners scroll on both axes at once; (6,6) scrolls, (7,7) does not at 1280x960 | RO, run3 |
| Trigger delay | None beyond the render pipeline. The scroll arrow cursor and the first camera displacement appear in the same rendered frame (about 40 ms after the pointer event); a non-scrolling pointer move shows its new cursor at the same latency | RO, run3 |
| Speed profile | Constant: 27 px in the partial first frame, then 64 to 80 px per frame; a time-lapse fit gives 2.28 px/ms (left), 2.18 (right), 2.15 (up) at default zoom, 1280x960. No acceleration ramp | RO, run3 |
| Versus arrow keys | Held Left 2.24 px/ms, held Up 1.91 px/ms in the same setup; edge scroll equals key scroll within about 12 percent. Option values were not varied; the prefix is fresh, so the exe defaults apply (the audit table lists 50) | RO, run3 |
| Cursor art | A directional arrow replaces the gauntlet in all eight directions, including over the HUD at the bottom edge | RO, run3 |

World units per second were not read; the figures above are screen pixels at zoom level 0.

### Selection (A2 to A5, A9)

| Fact | Value | Evidence |
| --- | --- | --- |
| Click on empty ground | Keeps the selection. This contradicts the audit's DOC entry for A2 | RO, run3 |
| Drag box on empty ground (1 to 100 px) | Keeps the selection | RO, run3 |
| Drag threshold | None. A 1 px diagonal press-move-release already acts as a box: a box whose edge touches a unit's pick region selects it while a plain click at the same point does not. The green rectangle is visible from 2 px diagonal. A drag below some threshold counting as a click is therefore not present; a release with no movement is a click | RO, run3 |
| Pick region | Click and box use the same unit region: for a footman at 1280x960 the upper boundary is y 350 for both a click and a box edge, and x within 20 px of the centre selects while 40 px away does not | RO, run3 |
| Double-click | Second left press within 500 ms of the first press selects all visible units of the clicked type (4 of 4 footmen). Yes at 493.5 ms, no at 506.2 ms; with a 200 ms hold, yes at 487.6, no at 510.3, so the window runs press to press. The exe imports no `GetDoubleClickTime` (`imports.txt`), so the value is not the OS setting | RO, run3 |
| Shift+click | Toggles: adds a unit, a second Shift+click removes it | RO, run3 |
| Ctrl+click | Selects all visible units of that type | RO, run3 |
| Units and buildings | Shift+click a Town Hall with a worker selected replaces the selection with the hall | RO, run11 |

### Control groups (B3 to B5)

| Fact | Value | Evidence |
| --- | --- | --- |
| Ctrl+digit | Replaces the group | RO, run3 |
| Shift+digit | Adds the current selection to the group; the current selection does not change | RO, run3 |
| Ctrl+Shift+digit | Same as Ctrl+digit (replaced a non-empty group); no separate meaning | RO, run3 |
| Digit on an empty group | Selection unchanged | RO, run3 |
| Double-tap centre window | The second press centres the camera when the interval is 482.2 or 492.9 ms and does not at 492.9 (repeat), 498.9, 501.0 or 507.3 ms. The first press never moves the camera. Consistent with the audit's 0.5 s constant | RO, run3 |

### Tab (B6)

With Arthas (hero) and four footmen selected, the active subgroup after the box select is
the hero; each Tab switches to the other type (portrait, info panel and command card follow)
while the group icons keep all five units. Shift+Tab alternates the same way. With two
subgroups the two directions cannot be told apart. Not measured with three or more types.
RO, run3.

### Camera zoom table (C2, C3)

RO-dbg, run8: breakpoints at `0x5de437` and `0x5de5fb` inside the zoom routine `0x5de2c0`
read the camera's current distance and angle before each zoom change, two and a half
seconds after the previous change (the ease had finished).

| Level | Settled distance | Settled angle of attack |
| --- | --- | --- |
| 0 (game start) | 1650 | 304 |
| 1 | 1600 | 311 |
| 2 | 1500 | 318 |
| 3 | 1400 | 325 |
| 4 | 1275 | 332 |
| 5 | 1200 | 335 |

The last level settles at the executable literal 1200 and 335, not the `[Camera]` file
values 1100 and 339 from the audit. Levels 0 to 4 equal the file values. The ease duration
argument of the routine is 1.0 (run7). Page Up and wheel up request level + 1 (zoom in),
Page Down and wheel down level - 1; Page Up at level 5 reaches the setter with 6 and
changes nothing, Page Down at level 0 does not reach the setter. Wheel events from
`xdotool click 4/5` reach the same setter. A screenshot-scale fit between levels was tried
and discarded (correlation 0.4 to 0.66).

### Camera jump keys (B15, B16, B8)

| Fact | Value | Evidence |
| --- | --- | --- |
| Backspace | In the melee start, with a worker selected and the camera moved away by a minimap click, centres the camera on the own Town Hall (not on the selected worker), twice in a row. In Human01 (own units, no own building, Arthas selected) it does nothing over 3 presses. So the list holds the Town Hall and is not the selection. Cycling over several halls and the 1.0 s restart rule were not exercised | RO, run12, run3 |
| F8 and `` ` `` | Both select an idle worker and centre the camera on it. A second press up to 984.7 ms after the first advances to another worker (frame difference 24 to 26 against noise below 1.5); at 1014.3 ms and 1168.4 ms it restarts at the first worker. Window about 1.0 s. In Human01, with no workers, neither key does anything. Consistent with the audit's B8 note that both call one method | RO, run11, run3 |

### Hover and armed cursors (D1, D3)

Cursor art observed at the pointer, 1280x960. The mode-to-sequence mapping is decoded in [input cursor](input-cursor.md).

| State | Cursor |
| --- | --- |
| Ground, trees, HUD panels | Grey-blue gauntlet |
| Own unit or building (selected or not) | Green gauntlet |
| Neutral passive unit or building (Human01 villagers, tents, house) | Yellow gauntlet |
| Enemy unit (Blackrock grunt) | Red gauntlet |
| Melee gold mine, with or without a worker selected | Green gauntlet (owner not established) |
| Attack armed (`a`): ground, HUD, the selected hero, the house | Cyan crosshair |
| Attack armed over an own footman | Green crosshair |
| Attack armed over a neutral villager | Yellow crosshair |
| Attack armed over an enemy | Red crosshair |
| Holy Light armed, over ground, enemy, neutral, own footman, house | Cyan crosshair everywhere; no invalid-target variant seen; "Select Target" text shown |
| Esc while armed | Gauntlet returns |

## Not measured

- Shift+Tab direction with three or more subgroups; the invalid-target and hold-item cursors (the setter never selects them,
  see [input cursor](input-cursor.md)); why the melee gold mine is green.
- Edge-scroll speed in world units and its dependence on `mousescroll`; the bottom margin at
  1920x1080; whether the margin constant lives in the exe or the data.
- Backspace with several halls and its 1.0 s restart window; the exact definition of an idle
  worker for F8; the selection cap of 12 (only five units were available); Shift+digit
  dedupe of a unit already in the group.
- Double-click on a unit outside the type's visible set, and any use of the second click
  position. Smart-order target classes (A12) were out of scope.
- Native Windows behaviour. All numbers come from Wine with xdotool-synthesised events.
