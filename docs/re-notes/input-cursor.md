# In-game cursor: art, sequences, tint and pointer states

Slice 9 of the [input behaviour audit](input-behaviour-audit.md) (gaps D1 to D3 and the
scroll arrows of A19). The original draws its own cursor from the local player's race
model and changes its sequence and tint with the pointer state. The recreation did show
the system cursor; it now draws the model.

## Evidence

Tags as in the audit: SB shipped file bytes, EX decompiled executable code, RO runtime
observation, INF inference, UNK not established. Image: Legacy `Warcraft III.exe`,
SHA-256 `3f2ed012...0eed`, 1.29.2.9231. Code facts come from rizin `af` and the `pdg`
decompiler on the addresses named, run for this note. Decompiler output, disassembly and
screenshots stay in the ignored `.scratch/`; no original bytes are committed.

### Cursor model (SB)

| Fact | Evidence |
| --- | --- |
| `UI\war3skins.txt` has `Cursor=UI\Cursor\<Race>Cursor.mdl` in each race section and `Cursor=UI\Cursor\HumanCursor.mdl` in `[Default]`; the files are `.mdx`. | SB |
| All four models carry 14 sequences in one order: Target, Normal, Select, TargetSelect, InvalidTarget, HoldItem, Scroll Left, Scroll Right, Scroll Up, Scroll Down, Scroll Up Left, Scroll Up Right, Scroll Down Left, Scroll Down Right. Every sequence loops. | SB (`mdlx info`) |
| The Human model has four geosets with alpha tracks per sequence, and a second texture record with an empty path and slot 21. | SB |

### Mode setter `0x616660` (EX)

The cursor object keeps its current mode at `[this+0x1b8]` and a mode stack at
`[this+0x1bc]`. The setter returns without effect while `[this+0x1cc]` is nonzero (a lock).
Otherwise a byte table at `0x6168d0` maps the mode (0 to 17) to a case, and each case hands
a list of animation tag indices to the sprite call `0x966c10`. After that `0x6165a0` writes
the tint through `0x96caf0(sprite, 5, argb)`: `0xff` alpha then three bytes from the table
at `0xe94210` (stride 3, by mode). The tag names are registered at `0x5d1d30`: 0 Normal,
1 Select, 2 Target, 3 TargetSelect, 4 InvalidTarget, 5 HoldItem, 6 Scroll, 7 Left,
8 Right, 9 Up, 10 Down.

| Mode | Cause | Tags | Sequence | Tint (R G B) |
| --- | --- | --- | --- | --- |
| 0 | nothing under the pointer | 0 | Normal | ff ff ff |
| 1 | hover, neutral | 1 | Select | ff ff 00 |
| 2 | hover, enemy | 1 | Select | ff 00 00 |
| 3 | hover, own | 1 | Select | 00 ff 00 |
| 4 | targeting, no unit | 2 | Target | ff ff ff |
| 5 | targeting, neutral unit | 3 | TargetSelect | ff ff 00 |
| 6 | targeting, enemy unit | 3 | TargetSelect | ff 00 00 |
| 7 | targeting, own unit | 3 | TargetSelect | 00 ff 00 |
| 8 | signal (ping) mode | 3 | TargetSelect | the local player's colour |
| 9 | holding an item | 5 | HoldItem | ff ff ff |
| 10 to 17 | edge scroll | 6 plus direction tags | Scroll Left, Right, Up, Down, Up Left, Up Right, Down Left, Down Right | ff ff ff |

- A sequence name is the tag names joined by a space (`Scroll Up Left` is tags 6, 9, 7).
  The joining rule is INF; the sprite lookup `0x966c10` was not decoded. Every name any mode
  asks for exists in all four models (SB, checked by the ignored cursor tests).
- Mode 8 is the activation of `CSignalMode` (`0x6284d0`, slot 10 of its vtable); the tint
  table entry for it (`ff 0a 20`) is not used because the setter reads the player colour.
- Mode 9 also replaces model texture slot `0x15` (21) with the carried item's icon
  (`0x967800`, call site `0x616881`). That is the model's empty-path texture record.
- No case of the setter selects tag 4, so `InvalidTarget` is never chosen by a cursor
  mode. Other callers of `0x966c10` (130 call sites, every sprite) were not traced, so
  whether anything else plays it is UNK. The live capture saw no invalid-target variant.
- The tint colours are consistent with the live capture (RO): green over own, yellow over
  neutral, red over enemy, cyan crosshair over ground, a colour-free tint for the plain
  target and normal modes.

### Hover and target mode choice (EX, relation INF)

| Function | Reads | Result |
| --- | --- | --- |
| `0x5fbdf0` (hover over an object) | object flag `0x100`; helper `0x47f9f0`; helper `0x478da0` | flag set gives 1; `0x47f9f0` true gives 3; `0x478da0` true gives 1; otherwise 2. No object, or an object with flag `0x400`, gives 0. |
| `0x643280` (targeting hover) | `[this+0x14] & 4`; target check `0x4f6020` or `0x4f5f80`; helpers `0x478e00`, `0x5ca370` | no unit or no colouring gives 4; a coloured result is 7, 6 or 5 in the order of the hover modes 3, 2, 1. |

The helper predicates were not decoded. Their relation names (own, neutral, enemy) are INF
from the tint table and the live colours; the assignment of modes 5, 6, 7 to neutral,
enemy, own follows the same parallel with 1, 2, 3. Whether a unit under a targeting cursor
is coloured depends on the command (flag `4`) and on a target check. The live capture
fixes only attack (coloured over own, neutral and enemy units; plain over the selected hero
and a neutral house) and Holy Light (plain everywhere). The recreation rule below is INF
from those cases.

### Edge scroll (EX)

`0x5dfd50` runs for edge events. A begin event maps the direction mask to a cursor mode
through a jump table at `0x5dfe7c` (mask 1 up gives 12, 2 down 13, 4 left 10, 5 up left 14,
6 down left 16, 8 right 11, 9 up right 15, 10 down right 17; masks 3 and 7 keep mode 0),
pushes the old mode, calls the setter and sets the lock `[this+0x1cc]`. The end event
clears the lock and restores the old mode (`0x6130c0`). This matches the live capture: the
arrow replaces the gauntlet in all eight directions, also over the HUD (RO).

### Build placement (EX, UNK)

`CBuildMode` (vtable `0xe9b180`) handlers read for this note call no cursor-mode setter.
The placement texture (`PlacementCursor`, `[Placement]` colours) is a ground overlay, not a
cursor mode. What the pointer shows while a building waits to be placed was not measured.

### Hotspot and size (RO, one run)

One isolated reference run (`tools/original-game-reference/reference.py`: private Docker
container, private Xvfb `:99`, null audio sink; `prepare`, `launch`, `capture`, one `click`,
`capture`, `stop`; no host display, pointer or audio touched). The pointer was moved by a
click at (300, 400) on the main menu background at 1280x960 and the cursor art cropped:
the finger tip sits at about (297.5, 404) and the art spans x 292 to 331, y 404 to 438. The
recreation's offscreen render of the same model, with the model origin at the pointer, puts
the tip at (-2.4, +4.4) from the pointer against (-2.5, +4) in the original and spans about
40 by 35 pixels. So the model origin is the hotspot and the model is drawn at the size the
`.mdx` gives, scaled with the window height like the rest of the 0.8 x 0.6 frame. The main
menu gauntlet looked darker and more teal than the recreation's render; the in-game tone
was not compared (UNK).

## Implementation

| Part | Where |
| --- | --- |
| Rules read from the image: tag names, mode to sequence, tint table, hover, targeting and edge modes, item icon slot. Only the reviewed image is accepted; every site is an address plus an opcode check, and the case bodies are followed by a small instruction reader that refuses any other form. | `crates/pcx-play/src/cursor_rules.rs` (`CursorRules::read`, `look`) |
| Pointer classification, model, camera, sequence and tint, system cursor visibility. | `crates/peacecraft3/src/campaign_cursor.rs` |
| Edge directions of the pointer this frame (`CameraInput::edges`), written by the existing scroll system. | `crates/peacecraft3/src/campaign_camera_input.rs` |
| Kind of the waiting command (`CampaignInput::armed_kind`). | `crates/peacecraft3/src/campaign_input.rs` |
| Interval plans and `set_interval` for a spawned model. | `crates/peacecraft3/src/campaign_score_model.rs` |

Rules for the pointer state, highest first (`campaign_cursor::pointer`):

| State | Pointer | Basis |
| --- | --- | --- |
| Pointer in an edge band | `Scroll(mask)` | EX lock, RO |
| Building placement waiting | Normal, with `Unestablished(BuildPlacement)` | UNK, see above |
| Pointer over the interface, outside the world view, or game input closed | Normal | RO (gauntlet over HUD panels) |
| Attack waiting, unit under the pointer that is not selected | `Target(Some(relation))` | INF from RO |
| Attack waiting otherwise; move, patrol or a spell waiting | `Target(None)` | RO for attack and Holy Light; move and patrol INF |
| No command waiting, unit under the pointer | `Hover(relation)` | EX, RO |
| Otherwise | Normal | EX |

- The relation is `ObjectHost::local_relation` (own, neutral passive, enemy), the same one
  that colours selection circles.
- The unit under the pointer uses the click picker's rule (nearest visible live unit whose
  selection circle covers the ground point). The rule is a copy of the inline code in
  `campaign_input::click`; unify them once the smart right-click change there has landed.
- "Over the interface" means Bevy's hover map holds an entity with an image, text, button
  or background colour, or the pointer lies outside the scene camera's viewport.
- The model is drawn by its own orthographic camera (order 2000, render layer 11) that is
  `FRAME_H` (0.6) tall at any window size, so the cursor scales with the window height. The
  system cursor is hidden only while the model is drawn and returns outside the window, in
  menus and when the campaign view is inactive.
- The item-icon slot has no source in the recreation (no item is carried), so it is a 1 by
  1 transparent texture and `HoldItem` is never selected. The slot number comes from the
  image, and any other empty texture path refuses the model.
- Missing sequences are reported, not guessed: `CursorDiagnostic::MissingSequence` (the
  normal cursor stands in) and `CursorDiagnostic::Unestablished(Situation)`, kept on
  `CampaignCursor::diagnostics` and logged once.
- A failed load (unsupported image, missing model) keeps the system cursor and records
  `CampaignCursor::last_error`.

## Verification

- `cargo test -p pcx-play --lib cursor_rules --features test-fixtures`: synthetic image
  (changed instruction form, missing tag, unknown image refuse; sequences, tints, scroll
  masks decode).
- `cargo test -p peacecraft3 --features test-fixtures --bin peacecraft3 campaign_cursor`:
  chosen sequence and tint for synthetic hover, armed and edge states with the fixture
  rules; window position, edge band and focus reach the cursor state; the system cursor
  hides and returns.
- Ignored tests with local assets (`PCX_WC3_INSTALL` with the Legacy install,
  `PCX_CURSOR_ASSETS` with `UI\war3skins.txt` and `UI\Cursor\*` extracted from it):
  the decoder against the reviewed client (`reviewed_client_yields_the_documented_cursor_rules`
  in `pcx-play`); all four race models carry every sequence the client asks for; posing
  each synthetic state selects that sequence's interval and tint; a missing sequence gives
  the typed diagnostic.
- `draw_every_pointer_state_offscreen` (ignored; also `PCX_CURSOR_CAPTURE`, a Vulkan
  adapter, no window) renders nine states and checks that each draws and that the hover
  and target tints come out grey-blue, green, yellow, red, cyan, green and red.

## Not established

- The helper predicates behind own, neutral and enemy in `0x5fbdf0` and `0x643280`, the
  meaning of object flag `0x100`, and the flag and target check that decide whether a
  targeting cursor is coloured. The neutral house observation (plain under attack) is not
  modelled: the recreation colours it yellow.
- The cursor while a building waits to be placed, over items, destructables and trees, and
  while dragging a selection box.
- `InvalidTarget` and `HoldItem` producers; the recreation has no carried-item state.
- The signal (ping) cursor: no signal action exists in the recreation.
- In-game tone of the gauntlet against the recreation; scroll arrows and tints were not
  compared frame by frame.
- Hover, interface and viewport gating were exercised by unit tests only, not in a running
  campaign window.
